//! Solo 自身のハーネスを動かし、Codex モデルの応答を UI イベントに変換する。
mod workspace_diff;
use crate::{
    approval::{self, ApprovalReply, ApprovalRequest},
    codex::{Authentication, DEFAULT_MODEL, DeviceLogin},
    event::{Emitter, Envelope, Event, Sequencer, SessionId, Usage},
    harness::{
        self, Cancellation, Limits, Message, StopReason, ToolCall, Update,
        workspace::{WorkspaceTools, canonical_workspace, system_prompt},
    },
    text::preview,
};
use async_channel::{Receiver, Sender};
use serde_json::json;
use std::{
    cell::RefCell,
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    thread,
};
use workspace_diff::DiffTracking;

pub enum Delivery {
    Event(Envelope),
    Login(DeviceLogin),
    Authenticated,
    LoginCancelled,
    History(Vec<Message>),
    Approval {
        request: Box<ApprovalRequest>,
        reply: Sender<ApprovalReply>,
    },
    Error(String),
}

impl From<Envelope> for Delivery {
    fn from(envelope: Envelope) -> Self {
        Self::Event(envelope)
    }
}

pub struct Config {
    pub session_id: SessionId,
    pub title: String,
    pub workspace: PathBuf,
    pub prompt: String,
    pub history: Vec<Message>,
    pub start_sequence: u64,
    pub model: String,
}

pub struct Controller {
    cancellation: Cancellation,
}

impl Controller {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn disconnect(&self) {
        self.cancel();
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub fn start(config: Config) -> io::Result<(Controller, Receiver<Delivery>)> {
    let workspace = canonical_workspace(&config.workspace)?;
    let cancellation = Cancellation::default();
    let controller = Controller {
        cancellation: cancellation.clone(),
    };
    let (sender, receiver) = async_channel::bounded(256);
    thread::Builder::new()
        .name(format!("solo-model-{}", config.session_id))
        .spawn(move || {
            let mut emitter = Emitter::new(
                Sequencer::new(
                    config.session_id.clone(),
                    config.start_sequence,
                    format!("solo-turn-{}", config.start_sequence + 1),
                ),
                sender.clone(),
            );
            if config.start_sequence == 0 {
                emitter.emit(Event::SessionCreated {
                    title: config.title.clone(),
                    workspace_id: workspace.display().to_string(),
                    settings: json!({"backend":"codex","provider":"openai-codex"}),
                });
            }
            emitter.emit(Event::TurnStarted {
                prompt: config.prompt.clone(),
            });
            let result = catch_unwind(AssertUnwindSafe(|| {
                run(config, workspace, &sender, &mut emitter, &cancellation)
            }));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(_)) if cancellation.is_cancelled() => {
                    emitter.emit(Event::TurnCancelled {
                        reason: "実行を中止しました".into(),
                    });
                }
                Ok(Err(error)) => emitter.emit(Event::TurnFailed {
                    reason: error.to_string(),
                }),
                Err(payload) => emitter.emit(Event::TurnFailed {
                    reason: format!(
                        "実行ワーカーが異常終了しました: {}",
                        panic_message(&payload)
                    ),
                }),
            }
        })?;
    Ok((controller, receiver))
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("原因を取得できませんでした")
}

pub fn start_login() -> io::Result<(Controller, Receiver<Delivery>)> {
    let cancellation = Cancellation::default();
    let controller = Controller {
        cancellation: cancellation.clone(),
    };
    let (sender, receiver) = async_channel::bounded(4);
    thread::Builder::new()
        .name("solo-chatgpt-login".into())
        .spawn(move || {
            let network_cancel = cancellation.child_token();
            let result = (|| -> io::Result<()> {
                let auth = Authentication::new()?;
                auth.login_device(
                    |login| {
                        let _ = sender.send_blocking(Delivery::Login(login));
                    },
                    &network_cancel,
                )?;
                Ok(())
            })();
            let delivery = match result {
                Ok(()) if !cancellation.is_cancelled() => Delivery::Authenticated,
                _ if cancellation.is_cancelled() => Delivery::LoginCancelled,
                Err(error) => Delivery::Error(error.to_string()),
                Ok(()) => Delivery::LoginCancelled,
            };
            let _ = sender.send_blocking(delivery);
        })?;
    Ok((controller, receiver))
}

fn run(
    config: Config,
    workspace: PathBuf,
    sender: &Sender<Delivery>,
    emitter: &mut Emitter<Delivery>,
    cancellation: &Cancellation,
) -> io::Result<()> {
    let network_cancel = cancellation.child_token();
    let auth = Authentication::new()?;
    if !auth.is_logged_in()? {
        auth.login_device(
            |login| {
                let _ = sender.send_blocking(Delivery::Login(login));
            },
            &network_cancel,
        )?;
    }
    let _ = sender.send_blocking(Delivery::Authenticated);
    if cancellation.is_cancelled() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "中止"));
    }
    let model_name = if config.model.is_empty() {
        DEFAULT_MODEL.to_owned()
    } else {
        config.model
    };
    let mut model = auth
        .model(model_name.clone(), network_cancel)?
        .with_system_prompt(system_prompt(&workspace));
    // DiffTracking が実行直前にベースラインを取り、完了後に pending へ差分を載せる。
    let mut tools = DiffTracking::new(
        WorkspaceTools::new(&workspace)?.with_cancellation(cancellation.clone()),
        workspace.clone(),
    );
    let pending_diffs = tools.pending();
    let mut messages = config.history;
    messages.push(Message::User {
        text: config.prompt,
    });
    let approval_sender = sender.clone();
    let approval_workspace = workspace.clone();
    let approval_cancellation = cancellation.clone();
    // policy(on_update より先に呼ばれる)と on_update が emitter を交互に使うため
    // RefCell で包む。emit は逐次呼ばれ、借用が入れ子になることはない。
    let emitter = RefCell::new(emitter);
    let mut policy = |call: &ToolCall| {
        let request_id = format!("approval-{}", emitter.borrow().sequence() + 1);
        if let Some((decision, source)) = approval::precheck(call, &approval_workspace) {
            emitter.borrow_mut().emit(Event::ApprovalDecided {
                request_id,
                accepted: decision,
                source: source.into(),
            });
            return decision;
        }
        let request = ApprovalRequest::tool(call, &approval_workspace);
        emitter.borrow_mut().emit(Event::ApprovalRequested {
            request_id: request_id.clone(),
            title: request.title.clone(),
            executor: request.executor.clone(),
            command: request.display_command.clone(),
            details: request.details.clone(),
        });
        let reply = approval::ask(
            &approval_sender,
            request,
            |request, reply| Delivery::Approval { request, reply },
            &approval_cancellation,
        );
        let (accepted, source) = match reply {
            Some(reply) => (reply.accepted, reply.source.label()),
            None if approval_cancellation.is_cancelled() => (false, "cancelled"),
            None => (false, "closed"),
        };
        emitter.borrow_mut().emit(Event::ApprovalDecided {
            request_id,
            accepted,
            source: source.into(),
        });
        accepted
    };
    let mut model_requests = 0;
    let mut log_offset = 0;
    let limits = Limits::default();
    let run = harness::run(
        &mut model,
        &mut tools,
        &mut policy,
        messages,
        &limits,
        cancellation,
        |update| match update {
            Update::ModelRequested => {
                model_requests += 1;
                emitter.borrow_mut().emit(Event::ModelRequestStarted {
                    provider: "OpenAI Codex".into(),
                    model: model_name.clone(),
                    request_id: format!("request-{model_requests}"),
                });
            }
            Update::Assistant(text) | Update::AssistantDelta(text) => {
                emitter.borrow_mut().emit(Event::MessageDelta {
                    message_id: format!("message-{model_requests}"),
                    text,
                })
            }
            Update::ToolProposed(call) => {
                emitter.borrow_mut().emit(Event::ToolStarted {
                    agent_id: None,
                    invocation_id: call.id,
                    command: format!("{} {}", call.name, call.arguments),
                    cwd: workspace.display().to_string(),
                });
            }
            Update::ToolFinished { call_id, result } => {
                if let Some(diffs) = pending_diffs.borrow_mut().remove(&call_id) {
                    for (relative, diff) in diffs {
                        emitter.borrow_mut().emit(Event::DiffUpdated {
                            path: relative,
                            unified_diff: diff,
                        });
                    }
                }
                let content = preview(&result.content, 512);
                emitter.borrow_mut().emit(Event::Log {
                    level: if result.is_error { "error" } else { "tool" }.into(),
                    preview: content,
                    offset: log_offset,
                    bytes: result.content.len() as u64,
                });
                log_offset += result.content.len() as u64;
                emitter.borrow_mut().emit(Event::ToolFinished {
                    invocation_id: call_id,
                    exit_code: if result.is_error { 1 } else { 0 },
                });
            }
            Update::Stopped(_) => {}
        },
    );
    finish_turn(run, &limits, sender, emitter.into_inner());
    Ok(())
}

/// すべての停止理由で会話履歴を先に返し、停止理由に応じた終了イベントを送る。
/// 失敗・上限・中止の turn も履歴に残し、「続けて」の再開に使えるようにする。
fn finish_turn(
    run: harness::Run,
    limits: &Limits,
    sender: &Sender<Delivery>,
    emitter: &mut Emitter<Delivery>,
) {
    let _ = sender.send_blocking(Delivery::History(run.messages));
    match run.stop {
        StopReason::Completed => emitter.emit(Event::TurnCompleted {
            reason: "実行完了".into(),
            usage: run
                .usage
                .map(|usage| Usage {
                    input_tokens: Some(usage.input_tokens),
                    output_tokens: Some(usage.output_tokens),
                    cost_usd: None,
                })
                .unwrap_or_default(),
        }),
        StopReason::Cancelled => emitter.emit(Event::TurnCancelled {
            reason: "実行を中止しました".into(),
        }),
        other => emitter.emit(Event::TurnFailed {
            reason: other.message(limits),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::wait_for_reply;

    #[test]
    fn approval_answers_are_preserved_and_closed_or_cancelled_waits_decline() {
        for allowed in [false, true] {
            let (reply, answer) = async_channel::bounded(1);
            reply.send_blocking(ApprovalReply::user(allowed)).unwrap();
            assert_eq!(
                wait_for_reply(answer, &Cancellation::default()),
                Some(ApprovalReply::user(allowed))
            );
        }
        let (reply, answer) = async_channel::bounded(1);
        drop(reply);
        assert_eq!(wait_for_reply(answer, &Cancellation::default()), None);

        let cancelled = Cancellation::default();
        cancelled.cancel();
        let (reply, answer) = async_channel::bounded(1);
        reply.send_blocking(ApprovalReply::user(true)).unwrap();
        assert_eq!(wait_for_reply(answer, &cancelled), None);
    }

    #[test]
    fn finish_turn_sends_history_before_the_terminal_event() {
        let (sender, receiver) = async_channel::unbounded();
        let mut emitter = Emitter::new(
            Sequencer::new(SessionId::parse("s1").unwrap(), 0, "turn-1".to_owned()),
            sender.clone(),
        );
        let limits = Limits::default();
        let run = harness::Run {
            messages: vec![Message::User { text: "u".into() }],
            model_requests: limits.max_model_requests,
            tool_calls: 0,
            stop: StopReason::ModelLimit,
            usage: None,
        };
        finish_turn(run, &limits, &sender, &mut emitter);
        assert!(matches!(receiver.try_recv().unwrap(), Delivery::History(_)));
        let Delivery::Event(envelope) = receiver.try_recv().unwrap() else {
            panic!("the terminal delivery must be an event")
        };
        assert_eq!(envelope.payload["type"], "turn_failed");
        assert!(envelope.payload["reason"].as_str().unwrap().contains("64"));
    }

    #[test]
    fn finish_turn_reports_usage_on_completion() {
        let (sender, receiver) = async_channel::unbounded();
        let mut emitter = Emitter::new(
            Sequencer::new(SessionId::parse("s1").unwrap(), 0, "turn-1".to_owned()),
            sender.clone(),
        );
        let run = harness::Run {
            messages: vec![],
            model_requests: 1,
            tool_calls: 0,
            stop: StopReason::Completed,
            usage: Some(harness::TokenUsage {
                input_tokens: 12,
                output_tokens: 34,
            }),
        };
        finish_turn(run, &Limits::default(), &sender, &mut emitter);
        assert!(matches!(receiver.try_recv().unwrap(), Delivery::History(_)));
        let Delivery::Event(envelope) = receiver.try_recv().unwrap() else {
            panic!("the terminal delivery must be an event")
        };
        assert_eq!(envelope.payload["type"], "turn_completed");
        assert_eq!(envelope.payload["usage"]["input_tokens"], 12);
        assert_eq!(envelope.payload["usage"]["output_tokens"], 34);
    }

    #[test]
    fn cancellation_interrupts_approval_wait_even_if_the_reply_is_still_owned_by_the_ui() {
        let cancellation = Cancellation::default();
        let controller = Controller {
            cancellation: cancellation.clone(),
        };
        let (reply, answer) = async_channel::bounded(1);
        let (done, result) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = done.send(wait_for_reply(answer, &cancellation).map(|reply| reply.accepted));
        });
        controller.cancel();
        let completed = result.recv_timeout(std::time::Duration::from_secs(1));
        drop(reply);
        worker.join().unwrap();
        assert_eq!(
            completed.expect("cancellation must not wait for a UI reply"),
            None
        );
    }

    #[test]
    fn controller_cancellation_reaches_io_without_reverse_propagation() {
        for explicit in [false, true] {
            let cancellation = Cancellation::default();
            let network = cancellation.child_token();
            let independent_request = cancellation.child_token();
            independent_request.cancel();
            assert!(!cancellation.is_cancelled());
            assert!(!network.is_cancelled());
            let controller = Controller {
                cancellation: cancellation.clone(),
            };
            if explicit {
                controller.cancel();
            } else {
                drop(controller);
            }
            assert!(cancellation.is_cancelled());
            assert!(network.is_cancelled());
            assert!(
                cancellation.child_token().is_cancelled(),
                "a late worker must inherit an earlier stop request"
            );
        }
    }
}

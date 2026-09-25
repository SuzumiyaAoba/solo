//! Solo 自身のハーネスを動かし、Codex モデルの応答を UI イベントに変換する。
use crate::{
    approval::ApprovalRequest,
    codex_subscription::{Authentication, DEFAULT_MODEL, DeviceLogin},
    event::{Envelope, Event, SCHEMA_VERSION, Usage, preview},
    harness::{
        self, Cancellation, Limits, Message, StopReason, ToolCall, Update,
        workspace::WorkspaceTools,
    },
};
use async_channel::{Receiver, Sender};
use serde_json::json;
use std::{
    collections::HashMap,
    fs, io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

pub enum Delivery {
    Event(Envelope),
    Login(DeviceLogin),
    Authenticated,
    LoginCancelled,
    History(Vec<Message>),
    Approval {
        request: Box<ApprovalRequest>,
        reply: Sender<bool>,
    },
    Error(String),
}

pub struct Config {
    pub session_id: String,
    pub title: String,
    pub workspace: PathBuf,
    pub prompt: String,
    pub history: Vec<Message>,
    pub start_sequence: u64,
    pub model: String,
}

pub struct Controller {
    cancellation: Cancellation,
    network_cancel: CancellationToken,
}

impl Controller {
    pub fn cancel(&self) {
        self.cancellation.cancel();
        self.network_cancel.cancel();
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
    let workspace = config.workspace.canonicalize()?;
    if !workspace.is_dir() {
        return Err(io::Error::other("workspace がディレクトリではありません"));
    }
    let cancellation = Cancellation::default();
    let network_cancel = CancellationToken::new();
    let controller = Controller {
        cancellation: cancellation.clone(),
        network_cancel: network_cancel.clone(),
    };
    let (sender, receiver) = async_channel::bounded(256);
    thread::Builder::new()
        .name(format!("solo-model-{}", config.session_id))
        .spawn(move || {
            let mut emitter = Emitter::new(
                config.session_id.clone(),
                config.start_sequence,
                sender.clone(),
            );
            if config.start_sequence == 0 {
                emitter.emit(Event::SessionCreated {
                    title: config.title.clone(),
                    workspace_id: workspace.display().to_string(),
                    settings: json!({"backend":"solo_harness","provider":"openai-codex"}),
                });
            }
            emitter.emit(Event::TurnStarted {
                prompt: config.prompt.clone(),
            });
            let result = catch_unwind(AssertUnwindSafe(|| {
                run(
                    config,
                    workspace,
                    &sender,
                    &mut emitter,
                    &cancellation,
                    &network_cancel,
                )
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
    let network_cancel = CancellationToken::new();
    let controller = Controller {
        cancellation: cancellation.clone(),
        network_cancel: network_cancel.clone(),
    };
    let (sender, receiver) = async_channel::bounded(4);
    thread::Builder::new()
        .name("solo-chatgpt-login".into())
        .spawn(move || {
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
    emitter: &mut Emitter,
    cancellation: &Cancellation,
    network_cancel: &CancellationToken,
) -> io::Result<()> {
    let auth = Authentication::new()?;
    if !auth.is_logged_in()? {
        auth.login_device(
            |login| {
                let _ = sender.send_blocking(Delivery::Login(login));
            },
            network_cancel,
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
    let mut model = auth.model(model_name.clone(), network_cancel.clone())?;
    let mut tools = WorkspaceTools::new(&workspace)?.with_cancellation(cancellation.clone());
    let mut messages = config.history;
    messages.push(Message::User {
        text: config.prompt,
    });
    let approval_sender = sender.clone();
    let approval_workspace = workspace.clone();
    let mut policy = move |call: &ToolCall| {
        if matches!(call.name.as_str(), "read" | "search") {
            return true;
        }
        let (reply, answer) = async_channel::bounded(1);
        if approval_sender
            .send_blocking(Delivery::Approval {
                request: Box::new(ApprovalRequest::tool(call, &approval_workspace)),
                reply,
            })
            .is_err()
        {
            return false;
        }
        answer.recv_blocking().unwrap_or(false)
    };
    let mut model_requests = 0;
    let mut log_offset = 0;
    let mut edit_snapshots: HashMap<String, (String, String)> = HashMap::new();
    let run = harness::run(
        &mut model,
        &mut tools,
        &mut policy,
        messages,
        &Limits::default(),
        cancellation,
        |update| match update {
            Update::ModelRequested => {
                model_requests += 1;
                emitter.emit(Event::ModelRequestStarted {
                    provider: "OpenAI Codex".into(),
                    model: model_name.clone(),
                    request_id: format!("request-{model_requests}"),
                });
            }
            Update::Assistant(text) | Update::AssistantDelta(text) => {
                emitter.emit(Event::MessageDelta {
                    message_id: format!("message-{model_requests}"),
                    text,
                })
            }
            Update::ToolProposed(call) => {
                if call.name == "edit"
                    && let Some(relative) = call.arguments["path"].as_str()
                    && let Some(before) = read_workspace_file(&workspace, relative)
                {
                    edit_snapshots.insert(call.id.clone(), (relative.into(), before));
                }
                emitter.emit(Event::ToolStarted {
                    invocation_id: call.id,
                    command: format!("{} {}", call.name, call.arguments),
                    cwd: workspace.display().to_string(),
                });
            }
            Update::ToolFinished { call_id, result } => {
                if !result.is_error
                    && let Some((relative, before)) = edit_snapshots.remove(&call_id)
                    && let Some(after) = read_workspace_file(&workspace, &relative)
                    && let Some(diff) = unified_diff(&relative, &before, &after)
                {
                    emitter.emit(Event::DiffUpdated {
                        path: relative,
                        unified_diff: diff,
                    });
                }
                let content = preview(&result.content, 512);
                emitter.emit(Event::Log {
                    level: if result.is_error { "error" } else { "tool" }.into(),
                    preview: content,
                    offset: log_offset,
                    bytes: result.content.len() as u64,
                });
                log_offset += result.content.len() as u64;
                emitter.emit(Event::ToolFinished {
                    invocation_id: call_id,
                    exit_code: if result.is_error { 1 } else { 0 },
                });
            }
            Update::Stopped(_) => {}
        },
    );
    match run.stop {
        StopReason::Completed => {
            let _ = sender.send_blocking(Delivery::History(run.messages));
            emitter.emit(Event::TurnCompleted {
                reason: "実行完了".into(),
                usage: Usage::default(),
            });
        }
        StopReason::Cancelled => emitter.emit(Event::TurnCancelled {
            reason: "実行を中止しました".into(),
        }),
        other => emitter.emit(Event::TurnFailed {
            reason: format!("{other:?}"),
        }),
    }
    Ok(())
}

fn read_workspace_file(workspace: &std::path::Path, relative: &str) -> Option<String> {
    let path = workspace.join(relative).canonicalize().ok()?;
    if !path.starts_with(workspace) || fs::metadata(&path).ok()?.len() > 1024 * 1024 {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn unified_diff(relative: &str, before: &str, after: &str) -> Option<String> {
    if before == after {
        return None;
    }
    let dir = tempfile::tempdir().ok()?;
    let old = dir.path().join("before");
    let new = dir.path().join("after");
    fs::write(&old, before).ok()?;
    fs::write(&new, after).ok()?;
    let output = Command::new("git")
        .args(["diff", "--no-index", "--no-color", "--unified=3", "--"])
        .arg(&old)
        .arg(&new)
        .output()
        .ok()?;
    if output.status.code() != Some(1) {
        return None;
    }
    let diff = String::from_utf8(output.stdout).ok()?;
    let hunk = diff.find("@@ ")?;
    let diff = format!("--- a/{relative}\n+++ b/{relative}\n{}", &diff[hunk..]);
    (diff.len() <= 512 * 1024).then_some(diff)
}

struct Emitter {
    session_id: String,
    sequence: AtomicU64,
    turn_id: String,
    sender: Sender<Delivery>,
}

impl Emitter {
    fn new(session_id: String, start_sequence: u64, sender: Sender<Delivery>) -> Self {
        Self {
            session_id,
            sequence: AtomicU64::new(start_sequence),
            turn_id: format!("solo-turn-{}", start_sequence + 1),
            sender,
        }
    }

    fn emit(&self, event: Event) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let envelope = Envelope {
            schema_version: SCHEMA_VERSION,
            event_id: format!("{}-{sequence}", self.session_id),
            session_id: self.session_id.clone(),
            sequence,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            turn_id: Some(self.turn_id.clone()),
            payload: serde_json::to_value(event).expect("serializable event"),
        };
        let _ = self.sender.send_blocking(Delivery::Event(envelope));
    }
}

#[cfg(test)]
mod tests {
    use super::unified_diff;

    #[test]
    fn edit_diff_uses_workspace_path_and_correct_hunk() {
        let diff = unified_diff("src/a.rs", "one\ntwo\n", "one\nthree\n").unwrap();
        assert!(diff.starts_with("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@"));
        assert!(diff.contains("-two\n+three\n"));
    }
}

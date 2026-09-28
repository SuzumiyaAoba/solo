//! ACP v1 Agent のプロセスを UI セッション単位で保持する。
mod connection;
mod reader;
mod stdio;
mod writer;
use crate::{
    acp::{self, AgentProfile, Client},
    approval::{self, ApprovalReply, ApprovalRequest},
    diffgen::unified_diff,
    event::{Emitter, Envelope, Event, Sequencer, SessionId, Usage},
    harness::Cancellation,
    text::preview,
};
use async_channel::{Receiver, Sender};
pub use connection::Controller;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, BufReader},
    path::{Path, PathBuf},
    thread,
};
use writer::Writer;

pub enum Delivery {
    Event(Envelope),
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
    pub local_session_id: SessionId,
    pub title: String,
    pub workspace: PathBuf,
    pub profile: AgentProfile,
    pub start_sequence: u64,
}

pub fn start(config: Config) -> io::Result<(Controller, Receiver<Delivery>)> {
    let connection::Transport {
        controller,
        receiver,
        mut client,
        input,
        cleanup,
    } = connection::Transport::start(&config)?;
    thread::Builder::new()
        .name(format!("solo-acp-{}", config.local_session_id))
        .spawn(move || {
            let guard = cleanup;
            let connection = &guard.connection;
            if let Err(error) = run(config, &mut client, input, connection) {
                let _ = connection
                    .output
                    .send_blocking(Delivery::Error(error.to_string()));
            }
        })?;
    Ok((controller, receiver))
}

fn run(
    config: Config,
    client: &mut Client<BufReader<reader::Reader>, Writer>,
    input: Receiver<String>,
    connection: &connection::Connection,
) -> io::Result<()> {
    let sender = &connection.output;
    let mut emitter = Emitter::new(
        Sequencer::new(
            config.local_session_id,
            config.start_sequence,
            String::new(),
        ),
        sender.clone(),
    );
    if config.start_sequence == 0 {
        emitter.emit(Event::SessionCreated {
            title: config.title,
            workspace_id: config.workspace.display().to_string(),
            settings: json!({"backend":"acp","agent":config.profile.id}),
        });
    }
    let initialized = client.initialize()?;
    let methods = initialized["authMethods"].as_array();
    let mut session = client.new_session(&config.workspace, &mut |_| None);
    if session.as_ref().err().is_some_and(acp::is_auth_required) {
        let method = methods
            .and_then(|methods| {
                methods
                    .iter()
                    .filter(|m| m["type"].is_null() || m["type"] == "agent")
                    .find_map(|m| m["id"].as_str())
            })
            .ok_or_else(|| {
                io::Error::other(
                    "ACP agent のログインが必要です。agent 自身の CLI でログインしてください",
                )
            })?;
        client.authenticate(method, &mut |_| None)?;
        session = client.new_session(&config.workspace, &mut |_| None);
    }
    let session = session?;
    connection.initialized(session.clone())?;
    while let Ok(prompt) = input.recv_blocking() {
        if connection.is_disconnected() {
            break;
        }
        let cancellation = Cancellation::default();
        emitter.begin_turn(format!("acp-turn-{}", emitter.sequence() + 1));
        emitter.emit(Event::TurnStarted {
            prompt: prompt.clone(),
        });
        emitter.emit(Event::ModelRequestStarted {
            provider: format!("ACP / {}", config.profile.name),
            model: "agent の設定".into(),
            request_id: format!("acp-{}", emitter.sequence()),
        });
        let mut bridge = Bridge::default();
        let result = client.prompt_with_start(
            &session,
            &prompt,
            &mut |message| {
                if message["method"] == "session/update"
                    && message["params"]["sessionId"] == session
                {
                    bridge.handle(&message["params"]["update"], &mut emitter);
                    return None;
                }
                if message["method"] == "session/request_permission"
                    && message["params"]["sessionId"] == session
                {
                    return Some(bridge.request_permission(
                        &message["params"],
                        sender,
                        &mut emitter,
                        &config.profile,
                        &config.workspace,
                        &cancellation,
                    ));
                }
                None
            },
            || connection.start_turn(cancellation.clone()),
        );
        connection.finish_turn();
        match result {
            Ok(reason) if reason == "end_turn" => emitter.emit(Event::TurnCompleted {
                reason: format!("{} が完了しました", config.profile.name),
                usage: Usage::default(),
            }),
            Ok(reason) if reason == "cancelled" => emitter.emit(Event::TurnCancelled {
                reason: "ACP agent の中止を確認しました".into(),
            }),
            Ok(reason) => emitter.emit(Event::TurnFailed {
                reason: format!("ACP agent の停止理由: {reason}"),
            }),
            Err(error) if acp::is_request_cancelled(&error) => emitter.emit(Event::TurnCancelled {
                reason: error.to_string(),
            }),
            Err(error) => {
                emitter.emit(Event::Disconnected {
                    reason: format!("ACP agent の結果を確認できません: {error}"),
                });
                return Ok(());
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Bridge {
    tools: HashMap<String, ToolState>,
    log_offset: u64,
    /// messageId の無い chunk に割り当てる区切り番号。tool 呼出しを挟むと進める。
    segment: u64,
    segment_has_text: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ToolPhase {
    #[default]
    Proposed,
    Running,
    Finished,
}

#[derive(Default)]
struct ToolState {
    phase: ToolPhase,
    metadata: serde_json::Map<String, Value>,
}

impl Bridge {
    fn remember_tool(&mut self, update: &Value) {
        let Some(id) = update["toolCallId"].as_str() else {
            return;
        };
        let tool = self.tools.entry(id.into()).or_default();
        tool.metadata.insert("toolCallId".into(), json!(id));
        for key in ["name", "title", "kind", "rawInput"] {
            if let Some(value) = update.get(key).filter(|value| !value.is_null()) {
                tool.metadata.insert(key.into(), value.clone());
            }
        }
    }
    fn permission_params(&mut self, params: &Value) -> Value {
        let mut params = params.clone();
        self.remember_tool(&params["toolCall"]);
        if let Some(state) = params["toolCall"]["toolCallId"]
            .as_str()
            .and_then(|id| self.tools.get(id))
            && let Some(tool) = params["toolCall"].as_object_mut()
        {
            for (key, value) in &state.metadata {
                tool.insert(key.clone(), value.clone());
            }
        }
        params
    }
    /// session/request_permission: UI 承認を問い、options から対応する optionId を返す。
    /// 送信不能・中止・不一致は cancelled として返す。
    /// 問い合わせと決定は ApprovalRequested / ApprovalDecided として記録する。
    fn request_permission(
        &mut self,
        params: &Value,
        sender: &Sender<Delivery>,
        out: &mut Emitter<Delivery>,
        profile: &AgentProfile,
        workspace: &Path,
        cancellation: &Cancellation,
    ) -> Value {
        let params = self.permission_params(params);
        // permission_params は toolCall のメタデータだけを補うので options は同一。
        // ask() が params を move するため、optionId の探索だけ先に済ませる。
        let option_id = |kind: &str| {
            params["options"]
                .as_array()
                .and_then(|options| options.iter().find(|option| option["kind"] == kind))
                .and_then(|option| option["optionId"].as_str())
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        };
        let (allow, reject) = (option_id("allow_once"), option_id("reject_once"));
        let request_id = format!("approval-{}", out.sequence() + 1);
        let request = ApprovalRequest::acp(params, profile, workspace);
        out.emit(Event::ApprovalRequested {
            request_id: request_id.clone(),
            title: request.title.clone(),
            executor: request.executor.clone(),
            command: request.display_command.clone(),
            details: request.details.clone(),
        });
        let reply = approval::ask(
            sender,
            request,
            |request, reply| Delivery::Approval { request, reply },
            cancellation,
        );
        let (accepted, source) = match reply {
            Some(reply) => (reply.accepted, reply.source.label()),
            None if cancellation.is_cancelled() => (false, "cancelled"),
            None => (false, "closed"),
        };
        out.emit(Event::ApprovalDecided {
            request_id,
            accepted,
            source: source.into(),
        });
        if cancellation.is_cancelled() {
            return json!({"outcome":{"outcome":"cancelled"}});
        }
        match if accepted { allow } else { reject } {
            Some(option_id) => {
                json!({"outcome":{"outcome":"selected","optionId":option_id}})
            }
            None => json!({"outcome":{"outcome":"cancelled"}}),
        }
    }
    fn handle(&mut self, update: &Value, out: &mut Emitter<Delivery>) {
        match update["sessionUpdate"].as_str() {
            Some("agent_message_chunk") => {
                if let Some(text) = update["content"]["text"].as_str() {
                    // ACP v1 の chunk は messageId を持たないことが多い。tool 呼出しの
                    // 前後で別ブロックになるよう、区切り番号で message_id を分ける。
                    let id = update["messageId"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("acp-message-{}", self.segment));
                    self.segment_has_text = true;
                    out.emit(Event::MessageDelta {
                        message_id: id,
                        text: text.into(),
                    });
                }
            }
            Some("tool_call" | "tool_call_update") => {
                self.remember_tool(update);
                if let Some(id) = update["toolCallId"].as_str()
                    && let Some(tool) = self.tools.get_mut(id)
                {
                    if tool.phase == ToolPhase::Proposed {
                        tool.phase = ToolPhase::Running;
                        // tool の後に続く本文は別のメッセージブロックに分ける。
                        if self.segment_has_text {
                            self.segment += 1;
                            self.segment_has_text = false;
                        }
                        out.emit(Event::ToolStarted {
                            agent_id: None,
                            invocation_id: id.into(),
                            command: update["title"].as_str().unwrap_or("ACP tool").into(),
                            cwd: String::new(),
                        });
                    }
                    if let Some(content) = update["content"].as_array() {
                        for item in content {
                            if item["type"] == "content"
                                && let Some(text) = item["content"]["text"].as_str()
                            {
                                out.emit(Event::Log {
                                    level: "tool".into(),
                                    preview: preview(text, 512),
                                    offset: self.log_offset,
                                    bytes: text.len() as u64,
                                });
                                self.log_offset += text.len() as u64;
                            } else if item["type"] == "diff"
                                && let (Some(path), Some(new)) =
                                    (item["path"].as_str(), item["newText"].as_str())
                            {
                                let old = item["oldText"].as_str().unwrap_or("");
                                if let Some(diff) = unified_diff(path, old, new) {
                                    out.emit(Event::DiffUpdated {
                                        path: path.into(),
                                        unified_diff: diff,
                                    });
                                }
                            }
                        }
                    }
                    if matches!(update["status"].as_str(), Some("completed" | "failed")) {
                        if tool.phase != ToolPhase::Finished {
                            tool.phase = ToolPhase::Finished;
                            out.emit(Event::ToolFinished {
                                invocation_id: id.into(),
                                exit_code: if update["status"] == "completed" {
                                    0
                                } else {
                                    -1
                                },
                            });
                        }
                        // 完了済みという記録は残し、承認用の入力は次の要求へ引き継がない。
                        tool.metadata.clear();
                    }
                }
            }
            Some("plan") => {
                if let Some(entries) = update["entries"].as_array() {
                    let text = entries
                        .iter()
                        .filter_map(|entry| entry["content"].as_str())
                        .collect::<Vec<_>>()
                        .join(" / ");
                    out.emit(Event::Log {
                        level: "plan".into(),
                        preview: preview(&text, 512),
                        offset: self.log_offset,
                        bytes: text.len() as u64,
                    });
                    self.log_offset += text.len() as u64;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;

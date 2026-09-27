//! ACP v1 Agent のプロセスを UI セッション単位で保持する。
mod connection;
mod reader;
mod stdio;
mod writer;
use crate::{
    acp::{self, AgentProfile, Client},
    approval::{ApprovalRequest, wait_for_reply},
    event::{Envelope, Event, Sequencer, Usage, preview},
    harness::Cancellation,
};
use async_channel::{Receiver, Sender};
pub use connection::Controller;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, BufReader},
    path::PathBuf,
    thread,
};
use writer::Writer;

pub enum Delivery {
    Event(Envelope),
    Approval {
        request: Box<ApprovalRequest>,
        reply: Sender<bool>,
    },
    SessionId(String),
    Error(String),
}

pub struct Config {
    pub local_session_id: String,
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
        config.local_session_id,
        config.start_sequence,
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
    sender
        .send_blocking(Delivery::SessionId(session.clone()))
        .map_err(|_| io::Error::other("UI が閉じました"))?;
    while let Ok(prompt) = input.recv_blocking() {
        if connection.is_disconnected() {
            break;
        }
        let cancellation = Cancellation::default();
        emitter.begin_turn(&prompt);
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
                    let params = bridge.permission_params(&message["params"]);
                    let (reply, answer) = async_channel::bounded(1);
                    if sender
                        .send_blocking(Delivery::Approval {
                            request: Box::new(ApprovalRequest::acp(
                                params,
                                &config.profile,
                                &config.workspace,
                            )),
                            reply,
                        })
                        .is_err()
                    {
                        return Some(json!({"outcome":{"outcome":"cancelled"}}));
                    }
                    let accepted = wait_for_reply(answer, &cancellation);
                    if cancellation.is_cancelled() {
                        return Some(json!({"outcome":{"outcome":"cancelled"}}));
                    }
                    let kind = if accepted {
                        "allow_once"
                    } else {
                        "reject_once"
                    };
                    let option = message["params"]["options"]
                        .as_array()
                        .and_then(|options| options.iter().find(|option| option["kind"] == kind))
                        .and_then(|option| option["optionId"].as_str())
                        .filter(|id| !id.is_empty());
                    return Some(match option {
                        Some(option_id) => {
                            json!({"outcome":{"outcome":"selected","optionId":option_id}})
                        }
                        None => json!({"outcome":{"outcome":"cancelled"}}),
                    });
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

struct Emitter {
    sequencer: Sequencer,
    sender: Sender<Delivery>,
}
impl Emitter {
    fn new(session_id: String, sequence: u64, sender: Sender<Delivery>) -> Self {
        Self {
            sequencer: Sequencer::new(session_id, sequence, String::new()),
            sender,
        }
    }
    fn sequence(&self) -> u64 {
        self.sequencer.sequence()
    }
    fn begin_turn(&mut self, prompt: &str) {
        self.sequencer
            .begin_turn(format!("acp-turn-{}", self.sequencer.sequence() + 1));
        self.emit(Event::TurnStarted {
            prompt: prompt.into(),
        });
    }
    fn emit(&mut self, event: Event) {
        let _ = self
            .sender
            .send_blocking(Delivery::Event(self.sequencer.next(event)));
    }
}

#[derive(Default)]
struct Bridge {
    tools: HashMap<String, ToolState>,
    log_offset: u64,
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
    fn handle(&mut self, update: &Value, out: &mut Emitter) {
        match update["sessionUpdate"].as_str() {
            Some("agent_message_chunk") => {
                if let Some(text) = update["content"]["text"].as_str() {
                    let id = update["messageId"].as_str().unwrap_or("acp-message");
                    out.emit(Event::MessageDelta {
                        message_id: id.into(),
                        text: text.into(),
                    });
                }
            }
            Some("tool_call" | "tool_call_update") => {
                self.remember_tool(update);
                if let Some(id) = update["toolCallId"].as_str() {
                    let tool = self.tools.get_mut(id).expect("tool metadata was recorded");
                    if tool.phase == ToolPhase::Proposed {
                        tool.phase = ToolPhase::Running;
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
                                out.emit(Event::DiffUpdated {
                                    path: path.into(),
                                    unified_diff: simple_diff(path, old, new),
                                });
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

fn simple_diff(path: &str, old: &str, new: &str) -> String {
    let old_lines: Vec<_> = old.lines().collect();
    let new_lines: Vec<_> = new.lines().collect();
    let mut diff = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -1,{} +1,{} @@\n",
        old_lines.len(),
        new_lines.len()
    );
    for line in old_lines {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in new_lines {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

#[cfg(test)]
mod tests;

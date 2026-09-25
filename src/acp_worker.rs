//! ACP v1 Agent のプロセスを UI セッション単位で保持する。
use crate::{
    acp::{AgentProfile, Client},
    event::{Envelope, Event, SCHEMA_VERSION, Usage, preview},
};
use async_channel::{Receiver, Sender};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashSet,
    io::{self, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

pub enum Delivery {
    Event(Envelope),
    Approval {
        method: String,
        params: Value,
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

#[derive(Clone)]
struct Writer(Arc<Mutex<ChildStdin>>);

impl Writer {
    fn send(&self, value: Value) -> io::Result<()> {
        let mut line = serde_json::to_vec(&value)?;
        line.push(b'\n');
        self.0
            .lock()
            .map_err(|_| io::Error::other("ACP stdin lock"))?
            .write_all(&line)
    }
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("ACP stdin lock"))?
            .write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Controller {
    child: Arc<Mutex<Child>>,
    writer: Writer,
    prompts: Sender<String>,
    acp_session: Arc<Mutex<Option<String>>>,
    active: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

impl Controller {
    pub fn prompt(&self, text: String) -> io::Result<()> {
        self.prompts
            .try_send(text)
            .map_err(|_| io::Error::other("ACP agent に入力を送れませんでした"))
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let id = self.acp_session.lock().ok().and_then(|id| id.clone());
        if self.active.load(Ordering::Acquire)
            && let Some(id) = id
            && self
                .writer
                .send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":id}}))
                .is_ok()
        {
            return;
        }
        self.disconnect();
    }
    pub fn disconnect(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.disconnect();
    }
}

pub fn start(config: Config) -> io::Result<(Controller, Receiver<Delivery>)> {
    let mut child = Command::new(&config.profile.command)
        .args(&config.profile.args)
        .current_dir(&config.workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let reader = BufReader::new(child.stdout.take().expect("piped stdout"));
    let writer = Writer(Arc::new(Mutex::new(
        child.stdin.take().expect("piped stdin"),
    )));
    let child = Arc::new(Mutex::new(child));
    let acp_session = Arc::new(Mutex::new(None));
    let active = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let (prompts, input) = async_channel::bounded(1);
    let (sender, receiver) = async_channel::bounded(256);
    let controller = Controller {
        child: child.clone(),
        writer: writer.clone(),
        prompts,
        acp_session: acp_session.clone(),
        active: active.clone(),
        cancelled: cancelled.clone(),
    };
    thread::Builder::new()
        .name(format!("solo-acp-{}", config.local_session_id))
        .spawn(move || {
            let mut client = Client::new(reader, writer);
            let result = run(
                config,
                &mut client,
                sender.clone(),
                input,
                &acp_session,
                &active,
                &cancelled,
            );
            if let Err(error) = result {
                let _ = sender.send_blocking(Delivery::Error(error.to_string()));
            }
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
        })?;
    Ok((controller, receiver))
}

fn run(
    config: Config,
    client: &mut Client<BufReader<std::process::ChildStdout>, Writer>,
    sender: Sender<Delivery>,
    input: Receiver<String>,
    session_slot: &Mutex<Option<String>>,
    active: &AtomicBool,
    cancelled: &AtomicBool,
) -> io::Result<()> {
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
    if session
        .as_ref()
        .err()
        .is_some_and(|error| error.to_string().to_ascii_lowercase().contains("auth"))
    {
        let method = methods
            .and_then(|methods| {
                methods
                    .iter()
                    .find(|m| m["type"].is_null() || m["type"] == "agent")
            })
            .and_then(|m| m["id"].as_str())
            .ok_or_else(|| {
                io::Error::other(
                    "ACP agent のログインが必要です。agent 自身の CLI でログインしてください",
                )
            })?;
        client.authenticate(method, &mut |_| None)?;
        session = client.new_session(&config.workspace, &mut |_| None);
    }
    let session = session?;
    if let Ok(mut slot) = session_slot.lock() {
        *slot = Some(session.clone());
    }
    sender
        .send_blocking(Delivery::SessionId(session.clone()))
        .map_err(|_| io::Error::other("UI が閉じました"))?;
    while let Ok(prompt) = input.recv_blocking() {
        cancelled.store(false, Ordering::Release);
        emitter.begin_turn(&prompt);
        emitter.emit(Event::ModelRequestStarted {
            provider: format!("ACP / {}", config.profile.name),
            model: "agent の設定".into(),
            request_id: format!("acp-{}", emitter.sequence),
        });
        active.store(true, Ordering::Release);
        let bridge = RefCell::new(Bridge::default());
        let result = client.prompt(&session, &prompt, &mut |message| {
            if message["method"] == "session/update" && message["params"]["sessionId"] == session {
                bridge
                    .borrow_mut()
                    .handle(&message["params"]["update"], &mut emitter);
                return None;
            }
            if message["method"] == "session/request_permission"
                && message["params"]["sessionId"] == session
            {
                let (reply, answer) = async_channel::bounded(1);
                if sender
                    .send_blocking(Delivery::Approval {
                        method: "session/request_permission".into(),
                        params: message["params"].clone(),
                        reply,
                    })
                    .is_err()
                {
                    return Some(json!({"outcome":{"outcome":"cancelled"}}));
                }
                let accepted = answer.recv_blocking().unwrap_or(false);
                if cancelled.load(Ordering::Acquire) {
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
                    .and_then(|option| option["optionId"].as_str());
                return Some(match option {
                    Some(option_id) => {
                        json!({"outcome":{"outcome":"selected","optionId":option_id}})
                    }
                    None => json!({"outcome":{"outcome":"cancelled"}}),
                });
            }
            None
        });
        active.store(false, Ordering::Release);
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
    session_id: String,
    sequence: u64,
    turn_id: String,
    sender: Sender<Delivery>,
}
impl Emitter {
    fn new(session_id: String, sequence: u64, sender: Sender<Delivery>) -> Self {
        Self {
            session_id,
            sequence,
            turn_id: String::new(),
            sender,
        }
    }
    fn begin_turn(&mut self, prompt: &str) {
        self.turn_id = format!("acp-turn-{}", self.sequence + 1);
        self.emit(Event::TurnStarted {
            prompt: prompt.into(),
        });
    }
    fn emit(&mut self, event: Event) {
        self.sequence += 1;
        let envelope = Envelope {
            schema_version: SCHEMA_VERSION,
            event_id: format!("{}-{}", self.session_id, self.sequence),
            session_id: self.session_id.clone(),
            sequence: self.sequence,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            turn_id: Some(self.turn_id.clone()),
            payload: serde_json::to_value(event).expect("event serialization"),
        };
        let _ = self.sender.send_blocking(Delivery::Event(envelope));
    }
}

#[derive(Default)]
struct Bridge {
    tools: HashSet<String>,
    log_offset: u64,
}
impl Bridge {
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
                if let Some(id) = update["toolCallId"].as_str() {
                    if self.tools.insert(id.into()) {
                        out.emit(Event::ToolStarted {
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
                    if matches!(update["status"].as_str(), Some("completed" | "failed"))
                        && self.tools.remove(id)
                    {
                        out.emit(Event::ToolFinished {
                            invocation_id: id.into(),
                            exit_code: if update["status"] == "completed" {
                                0
                            } else {
                                -1
                            },
                        });
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
mod tests {
    #[cfg(unix)]
    use super::*;
    #[cfg(unix)]
    use crate::projection::{Apply, Session, Status};
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };

    #[cfg(unix)]
    #[test]
    fn two_turns_share_one_agent_process_and_permission_is_forwarded() {
        let dir = tempfile::tempdir().unwrap();
        let agent = dir.path().join("fake-acp");
        fs::write(&agent, r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"agentCapabilities":{},"authMethods":[]}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"acp-1"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"tool_call","toolCallId":"tool-1","title":"read","status":"pending"}}}'
echo '{"jsonrpc":"2.0","id":77,"method":"session/request_permission","params":{"sessionId":"acp-1","toolCall":{"toolCallId":"tool-1"},"options":[{"optionId":"a","kind":"allow_once","name":"Allow"},{"optionId":"r","kind":"reject_once","name":"Reject"}]}}'
IFS= read -r line
case "$line" in *'"optionId":"a"'*) ;; *) exit 3;; esac
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"tool-1","status":"completed","content":[{"type":"diff","path":"/tmp/a","oldText":"old","newText":"new"}]}}}'
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"m1","content":{"type":"text","text":"first"}}}}'
echo '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"m2","content":{"type":"text","text":"second"}}}}'
echo '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
"##).unwrap();
        let mut permissions = fs::metadata(&agent).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&agent, permissions).unwrap();
        let (controller, receiver) = start(Config {
            local_session_id: "local".into(),
            title: "test".into(),
            workspace: dir.path().to_path_buf(),
            profile: AgentProfile {
                id: "a".into(),
                name: "Agent A".into(),
                command: agent.display().to_string(),
                args: vec![],
            },
            start_sequence: 0,
        })
        .unwrap();
        let mut session = Session::new("local".into(), "test".into());
        for (turn, prompt) in ["first prompt", "second prompt"].into_iter().enumerate() {
            controller.prompt(prompt.into()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                match receiver.try_recv() {
                    Ok(Delivery::Event(event)) => {
                        assert_eq!(session.apply(event), Apply::Applied);
                        if session.status == Status::Completed
                            && session.accepted > if turn == 0 { 2 } else { 8 }
                        {
                            break;
                        }
                    }
                    Ok(Delivery::Approval { reply, .. }) => reply.send_blocking(true).unwrap(),
                    Ok(Delivery::SessionId(id)) => assert_eq!(id, "acp-1"),
                    Ok(Delivery::Error(error)) => panic!("{error}"),
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
            assert_eq!(session.status, Status::Completed, "{}", session.reason);
        }
        assert!(session.chat.iter().any(|block| block.text == "first"));
        assert!(session.chat.iter().any(|block| block.text == "second"));
        assert_eq!(session.diffs.len(), 1);
        drop(controller);
    }
}

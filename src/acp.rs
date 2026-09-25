//! ACP v1 の最小 client。Agent は stdio の JSON-RPC 2.0 peer として扱う。
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::{self, BufRead, Read, Write},
    path::Path,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    agents: Vec<AgentProfile>,
}

/// 未設定は空リスト。設定の不正値は起動時に知らせ、任意の command を自動起動しない。
pub fn load_agents(workspace: &Path) -> io::Result<Vec<AgentProfile>> {
    let path = workspace.join(".solo/agents.json");
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let registry: Registry = serde_json::from_str(&source).map_err(invalid)?;
    let mut ids = HashSet::new();
    if registry.agents.len() > 16 {
        return Err(invalid("ACP agent は最大16件です"));
    }
    for agent in &registry.agents {
        if agent.id.trim().is_empty()
            || agent.name.trim().is_empty()
            || agent.command.trim().is_empty()
            || !ids.insert(agent.id.as_str())
        {
            return Err(invalid(
                "ACP agent の id/name/command は必須で、id は重複できません",
            ));
        }
    }
    Ok(registry.agents)
}

pub struct Client<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    next_id: u64,
}

const MAX_JSON_LINE_BYTES: usize = 16 * 1024 * 1024;

impl<R: BufRead, W: Write> Client<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_id: 0,
        }
    }

    pub fn initialize(&mut self) -> io::Result<Value> {
        let result = self.request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {},
                "clientInfo": {"name":"solo","title":"Solo","version":env!("CARGO_PKG_VERSION")}
            }),
            &mut |_| None,
        )?;
        if result["protocolVersion"] != 1 {
            return Err(invalid("ACP protocol v1 に対応していません"));
        }
        Ok(result)
    }

    pub fn authenticate(
        &mut self,
        method_id: &str,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
    ) -> io::Result<()> {
        self.request("authenticate", json!({"methodId":method_id}), on_message)?;
        Ok(())
    }

    pub fn new_session(
        &mut self,
        cwd: &Path,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
    ) -> io::Result<String> {
        let result = self.request(
            "session/new",
            json!({"cwd":cwd,"mcpServers":[]}),
            on_message,
        )?;
        result["sessionId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid("ACP sessionId がありません"))
    }

    pub fn prompt(
        &mut self,
        session_id: &str,
        text: &str,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
    ) -> io::Result<String> {
        let result = self.request(
            "session/prompt",
            json!({"sessionId":session_id,"prompt":[{"type":"text","text":text}]}),
            on_message,
        )?;
        result["stopReason"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid("ACP stopReason がありません"))
    }

    pub fn send_notification(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}))
    }

    fn request(
        &mut self,
        method: &str,
        params: Value,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
    ) -> io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let message = self.read()?;
            if message["id"] == id && message.get("method").is_none() {
                if let Some(error) = message.get("error") {
                    return Err(invalid(format!(
                        "{method}: {}",
                        error["message"].as_str().unwrap_or("ACP error")
                    )));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| invalid("ACP result がありません"));
            }
            if let Some(request_id) = message.get("id")
                && message.get("method").is_some()
            {
                match on_message(&message) {
                    Some(result) => self.send(json!({"jsonrpc":"2.0","id":request_id,"result":result}))?,
                    None => self.send(json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32601,"message":"Solo はこの client method に対応していません"}}))?,
                }
            } else if message.get("method").is_some() {
                on_message(&message);
            } else {
                // 他の request の応答は、現在の応答と混同しない。
                return Err(invalid("予期しない ACP 応答 ID"));
            }
        }
    }

    fn send(&mut self, value: Value) -> io::Result<()> {
        let mut line = serde_json::to_vec(&value)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()
    }

    fn read(&mut self) -> io::Result<Value> {
        let mut line = Vec::new();
        let bytes = (&mut self.reader)
            .take((MAX_JSON_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "ACP agent が切断されました",
            ));
        }
        if bytes > MAX_JSON_LINE_BYTES || line.last() != Some(&b'\n') {
            return Err(invalid(
                "ACP agent の応答が大きすぎるか、途中で切断されました",
            ));
        }
        let value: Value = serde_json::from_slice(&line).map_err(invalid)?;
        if value["jsonrpc"] != "2.0" {
            return Err(invalid("ACP JSON-RPC version が不正です"));
        }
        Ok(value)
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn protocol_v1_wire_and_updates() {
        let messages = [
            json!({"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"agentCapabilities":{}}}),
            json!({"jsonrpc":"2.0","id":1,"result":{"sessionId":"s1"}}),
            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi"}}}}),
            json!({"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}),
        ];
        let mut input = Vec::new();
        for message in messages {
            serde_json::to_writer(&mut input, &message).unwrap();
            input.push(b'\n');
        }
        let mut client = Client::new(BufReader::new(Cursor::new(input)), Vec::new());
        client.initialize().unwrap();
        let session = client
            .new_session(Path::new("/tmp"), &mut |_| None)
            .unwrap();
        let mut text = String::new();
        let reason = client
            .prompt(&session, "hello", &mut |message| {
                if message["method"] == "session/update" {
                    text.push_str(
                        message["params"]["update"]["content"]["text"]
                            .as_str()
                            .unwrap(),
                    );
                }
                None
            })
            .unwrap();
        assert_eq!(reason, "end_turn");
        assert_eq!(text, "hi");
    }

    #[test]
    fn registry_rejects_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".solo")).unwrap();
        fs::write(dir.path().join(".solo/agents.json"), r#"{"agents":[{"id":"a","name":"A","command":"a"},{"id":"a","name":"B","command":"b"}]}"#).unwrap();
        assert!(load_agents(dir.path()).is_err());
    }
}

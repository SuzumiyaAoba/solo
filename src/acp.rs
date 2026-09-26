//! ACP v1 の最小 client。Agent は stdio の JSON-RPC 2.0 peer として扱う。
use crate::storage::read_optional;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
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

const MAX_REGISTRY_BYTES: u64 = 1024 * 1024;

/// 未設定は空リスト。設定の不正値は起動時に知らせ、任意の command を自動起動しない。
pub fn load_agents(workspace: &Path) -> io::Result<Vec<AgentProfile>> {
    let path = workspace.join(".solo/agents.json");
    let Some(source) = read_optional(
        &path,
        MAX_REGISTRY_BYTES,
        "ACP agent の設定が 1 MiB を超えています",
    )?
    else {
        return Ok(Vec::new());
    };
    let registry: Registry = serde_json::from_slice(&source).map_err(invalid)?;
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
const AUTH_REQUIRED: i32 = -32000;
const REQUEST_CANCELLED: i32 = -32800;

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for RpcError {}

pub(crate) fn is_auth_required(error: &io::Error) -> bool {
    rpc_error_code(error) == Some(AUTH_REQUIRED)
}

pub(crate) fn is_request_cancelled(error: &io::Error) -> bool {
    rpc_error_code(error) == Some(REQUEST_CANCELLED)
}

fn rpc_error_code(error: &io::Error) -> Option<i32> {
    error
        .get_ref()?
        .downcast_ref::<RpcError>()
        .map(|error| error.code)
}

fn rpc_error(method: &str, value: Value) -> io::Error {
    let mut error: RpcError = match serde_json::from_value(value) {
        Ok(error) => error,
        Err(error) => return invalid(format!("ACP error の形式が不正です: {error}")),
    };
    error.message = format!("{method}: {}", error.message);
    let kind = match error.code {
        AUTH_REQUIRED => io::ErrorKind::PermissionDenied,
        REQUEST_CANCELLED => io::ErrorKind::Interrupted,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, error)
}

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
        self.prompt_with_start(session_id, text, on_message, || Ok(()))
    }

    /// prompt の送信完了後、応答を読む前に実行中の状態を公開する。
    pub(crate) fn prompt_with_start(
        &mut self,
        session_id: &str,
        text: &str,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
        started: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<String> {
        let result = self.request_with_start(
            "session/prompt",
            json!({"sessionId":session_id,"prompt":[{"type":"text","text":text}]}),
            on_message,
            started,
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
        self.request_with_start(method, params, on_message, || Ok(()))
    }

    fn request_with_start(
        &mut self,
        method: &str,
        params: Value,
        on_message: &mut impl FnMut(&Value) -> Option<Value>,
        started: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        started()?;
        loop {
            let mut message = self.read()?;
            if message["id"] == id && message.get("method").is_none() {
                if message.get("result").is_some() == message.get("error").is_some() {
                    return Err(invalid(
                        "ACP 応答には result と error のどちらか一方が必要です",
                    ));
                }
                if let Some(error) = message.get_mut("error") {
                    return Err(rpc_error(method, error.take()));
                }
                return message
                    .get_mut("result")
                    .map(Value::take)
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
        write_message(&mut self.writer, value)
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

/// 制御通知も通常の要求も、一行全体をまとめて writer へ渡す。
pub(crate) fn write_message(writer: &mut impl Write, value: Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(&value)?;
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{BufReader, Cursor},
    };

    #[test]
    fn rpc_error_codes_are_preserved_independently_of_the_message() {
        for (code, message, expected) in [
            (
                AUTH_REQUIRED,
                "認証が必要です",
                io::ErrorKind::PermissionDenied,
            ),
            (
                REQUEST_CANCELLED,
                "停止しました",
                io::ErrorKind::Interrupted,
            ),
            (-32603, "authentication cache failure", io::ErrorKind::Other),
        ] {
            let response = json!({"jsonrpc":"2.0","id":0,"error":{"code":code,"message":message,"data":{"detail":"kept off the display"}}});
            let mut input = serde_json::to_vec(&response).unwrap();
            input.push(b'\n');
            let mut client = Client::new(Cursor::new(input), Vec::new());
            let error = client
                .new_session(Path::new("/tmp"), &mut |_| None)
                .unwrap_err();
            assert_eq!(error.kind(), expected);
            assert_eq!(error.to_string(), format!("session/new: {message}"));
            assert_eq!(is_auth_required(&error), code == AUTH_REQUIRED);
            assert_eq!(is_request_cancelled(&error), code == REQUEST_CANCELLED);
        }
    }

    #[test]
    fn malformed_or_ambiguous_responses_cannot_trigger_authentication() {
        for body in [
            json!({"error":{"code":AUTH_REQUIRED}}),
            json!({"error":{"message":"authentication required"}}),
            json!({"error":{"code":AUTH_REQUIRED,"message":null}}),
            json!({"error":{"code":"-32000","message":"authentication required"}}),
            json!({"error":null}),
            json!({"error":{"code":AUTH_REQUIRED,"message":"authentication required"},"result":{}}),
            json!({}),
        ] {
            let mut response = body;
            response["jsonrpc"] = json!("2.0");
            response["id"] = json!(0);
            let mut input = serde_json::to_vec(&response).unwrap();
            input.push(b'\n');
            let mut client = Client::new(Cursor::new(input), Vec::new());
            let error = client
                .new_session(Path::new("/tmp"), &mut |_| None)
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(!is_auth_required(&error));
            assert!(!is_request_cancelled(&error));
        }
    }

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

    #[test]
    fn registry_distinguishes_missing_invalid_and_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_agents(dir.path()).unwrap().is_empty());
        fs::create_dir(dir.path().join(".solo")).unwrap();
        let path = dir.path().join(".solo/agents.json");
        for invalid_source in [b"".as_slice(), b"{", b"\xff"] {
            fs::write(&path, invalid_source).unwrap();
            assert_eq!(
                load_agents(dir.path()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut source =
            r#"{"agents":[{"id":"a","name":"日本語","command":"agent","args":["--stdio"]}]}"#
                .as_bytes()
                .to_vec();
        source.resize(MAX_REGISTRY_BYTES as usize, b' ');
        fs::write(&path, &source).unwrap();
        let agents = load_agents(dir.path()).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "日本語");
        assert_eq!(agents[0].args, ["--stdio"]);
        source.push(b' ');
        fs::write(&path, &source).unwrap();
        let error = load_agents(dir.path()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("1 MiB"));
    }
}

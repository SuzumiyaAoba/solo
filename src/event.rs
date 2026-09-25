use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;

/// 時刻ではなく session 内の sequence で順序を判断する。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub schema_version: u32,
    pub event_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub turn_id: Option<String>,
    pub payload: Value,
}

impl Envelope {
    pub fn decode(&self) -> Result<Decoded, serde_json::Error> {
        let kind = self.payload.get("type").and_then(Value::as_str);
        if self.schema_version != SCHEMA_VERSION
            || kind.is_some_and(|kind| !Event::KNOWN_TYPES.contains(&kind))
        {
            return Ok(Decoded::Unknown {
                kind: kind.unwrap_or("(type なし)").to_owned(),
                payload: self.payload.clone(),
            });
        }
        serde_json::from_value(self.payload.clone()).map(Decoded::Known)
    }
}

#[derive(Debug)]
pub enum Decoded {
    Known(Event),
    Unknown { kind: String, payload: Value },
}

/// 未取得の計測値は None。0 とは区別する。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionCreated {
        title: String,
        workspace_id: String,
        settings: Value,
    },
    TurnStarted {
        prompt: String,
    },
    ModelRequestStarted {
        provider: String,
        model: String,
        request_id: String,
    },
    MessageDelta {
        message_id: String,
        text: String,
    },
    ToolStarted {
        invocation_id: String,
        command: String,
        cwd: String,
    },
    ToolFinished {
        invocation_id: String,
        exit_code: i32,
    },
    Log {
        level: String,
        preview: String,
        offset: u64,
        bytes: u64,
    },
    DiffUpdated {
        path: String,
        unified_diff: String,
    },
    TurnCompleted {
        reason: String,
        usage: Usage,
    },
    TurnFailed {
        reason: String,
    },
    TurnCancelled {
        reason: String,
    },
    Disconnected {
        reason: String,
    },
}

impl Event {
    const KNOWN_TYPES: &[&str] = &[
        "session_created",
        "turn_started",
        "model_request_started",
        "message_delta",
        "tool_started",
        "tool_finished",
        "log",
        "diff_updated",
        "turn_completed",
        "turn_failed",
        "turn_cancelled",
        "disconnected",
    ];
}

/// 巨大な一行も UTF-8 を壊さず、表示側へ渡す量を制限する。
pub fn preview(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

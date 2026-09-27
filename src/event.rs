use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// sequence と turn の管理は producer に任せ、共通のメタデータを付与する。
    pub(crate) fn new(session_id: &str, sequence: u64, turn_id: String, event: Event) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            event_id: format!("{session_id}-{sequence}"),
            session_id: session_id.to_owned(),
            sequence,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            turn_id: Some(turn_id),
            payload: serde_json::to_value(event).expect("serializable event"),
        }
    }

    pub fn decode(&self) -> Result<Decoded, serde_json::Error> {
        Decoded::from_payload(self.schema_version, self.payload.clone())
    }
}
/// producer 側の sequence / turn_id 採番。時刻ではなく sequence で順序を確定する。
/// `next` は送信前に採番し、`peek` + `advance` は送信成功時だけ採番を確定する。
pub struct Sequencer {
    session_id: String,
    sequence: u64,
    turn_id: String,
}

impl Sequencer {
    /// `turn_id` は SessionCreated など turn 開始前のイベントに付く先行値。
    pub fn new(session_id: impl Into<String>, start_sequence: u64, turn_id: String) -> Self {
        Self {
            session_id: session_id.into(),
            sequence: start_sequence,
            turn_id,
        }
    }

    /// 現在の sequence（最後に確定したイベント番号。開始前は start_sequence）。
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// 次のイベントから使う turn_id。TurnStarted の emit 前に呼ぶ。
    pub fn begin_turn(&mut self, turn_id: String) {
        self.turn_id = turn_id;
    }

    /// 採番せずに次の envelope だけ作る。送信に成功したら `advance` で確定する。
    pub fn peek(&self, event: Event) -> Envelope {
        Envelope::new(
            &self.session_id,
            self.sequence + 1,
            self.turn_id.clone(),
            event,
        )
    }

    /// 採番して envelope を返す。チャネル送信をそのまま続ける producer 向け。
    pub fn next(&mut self, event: Event) -> Envelope {
        let envelope = self.peek(event);
        self.advance();
        envelope
    }

    /// `peek` で作った envelope の送信が成功したときに採番を確定する。
    pub fn advance(&mut self) {
        self.sequence += 1;
    }
}

#[derive(Debug)]
pub enum Decoded {
    Known(Event),
    Unknown { kind: String, payload: Value },
}

impl Decoded {
    /// 所有済みの payload はコピーせずに event へ変換する。
    pub(crate) fn from_payload(
        schema_version: u32,
        payload: Value,
    ) -> Result<Self, serde_json::Error> {
        let kind = payload.get("type").and_then(Value::as_str);
        if schema_version != SCHEMA_VERSION
            || kind.is_some_and(|kind| !Event::KNOWN_TYPES.contains(&kind))
        {
            return Ok(Self::Unknown {
                kind: kind.unwrap_or("(type なし)").to_owned(),
                payload,
            });
        }
        serde_json::from_value(payload).map(Self::Known)
    }
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
    ToolFinished {
        invocation_id: String,
        exit_code: i32,
    },
    AgentStarted {
        agent_id: String,
        name: String,
        task: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_agent_id: Option<String>,
    },
    AgentFinished {
        agent_id: String,
        success: bool,
        summary: String,
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
        "agent_started",
        "agent_finished",
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
    let end = text.floor_char_boundary(max_bytes);
    format!("{}…", &text[..end])
}

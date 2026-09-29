use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    io::{self, ErrorKind},
    ops::Deref,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 1;

/// envelope の timestamp_ms と、イベントを伴わない状態遷移の記録時刻に使う現在時刻。
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// ローカル session の識別子。session_store がディレクトリ名に使うため文字種を検査する。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// 利用可能な文字は [A-Za-z0-9_-] のみ（ディレクトリ名として安全にするため）。
    pub fn parse(raw: impl AsRef<str>) -> io::Result<Self> {
        let raw = raw.as_ref();
        if raw.is_empty()
            || raw.len() > 40
            || !raw
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "session_id の形式が不正です",
            ));
        }
        Ok(Self(raw.to_owned()))
    }
}

impl Deref for SessionId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// worker が採番するターン識別子。ローカル生成なので内容は検査しない。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnId(String);

impl From<String> for TurnId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl Deref for TurnId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 時刻ではなく session 内の sequence で順序を判断する。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub schema_version: u32,
    pub event_id: String,
    pub session_id: SessionId,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub turn_id: Option<TurnId>,
    pub payload: Value,
}

impl Envelope {
    /// sequence と turn の管理は producer に任せ、共通のメタデータを付与する。
    pub(crate) fn new(
        session_id: &SessionId,
        sequence: u64,
        turn_id: TurnId,
        event: Event,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            event_id: format!("{session_id}-{sequence}"),
            session_id: session_id.clone(),
            sequence,
            timestamp_ms: now_ms(),
            turn_id: Some(turn_id),
            payload: serde_json::to_value(event).expect("serializable event"),
        }
    }

    /// payload を複製せず decode する。unknown kind のときだけ payload を複製する。
    pub fn decode(&self) -> Result<Decoded, serde_json::Error> {
        Decoded::from_payload_ref(self.schema_version, &self.payload)
    }
}
/// producer 側の sequence / turn_id 採番。時刻ではなく sequence で順序を確定する。
/// `next` は送信前に採番し、`peek` + `advance` は送信成功時だけ採番を確定する。
pub struct Sequencer {
    session_id: SessionId,
    sequence: u64,
    turn_id: TurnId,
}

impl Sequencer {
    /// `turn_id` は SessionCreated など turn 開始前のイベントに付く先行値。
    pub fn new(session_id: SessionId, start_sequence: u64, turn_id: impl Into<TurnId>) -> Self {
        Self {
            session_id,
            sequence: start_sequence,
            turn_id: turn_id.into(),
        }
    }

    /// 現在の sequence（最後に確定したイベント番号。開始前は start_sequence）。
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// 次のイベントから使う turn_id。TurnStarted の emit 前に呼ぶ。
    pub fn begin_turn(&mut self, turn_id: impl Into<TurnId>) {
        self.turn_id = turn_id.into();
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

/// Sequencer に Delivery チャネルを束ねた送信器。各ワーカーの Event→Envelope
/// 送信経路をここに一本化する。
pub struct Emitter<D> {
    sequencer: Sequencer,
    sender: async_channel::Sender<D>,
}

impl<D: Send> Emitter<D>
where
    D: From<Envelope>,
{
    pub fn new(sequencer: Sequencer, sender: async_channel::Sender<D>) -> Self {
        Self { sequencer, sender }
    }

    /// 確定済みの sequence（最後に送信したイベント番号）。
    pub fn sequence(&self) -> u64 {
        self.sequencer.sequence()
    }

    /// 次のイベントから使う turn_id。TurnStarted の emit 前に呼ぶ。
    pub fn begin_turn(&mut self, turn_id: impl Into<TurnId>) {
        self.sequencer.begin_turn(turn_id);
    }

    /// 採番して送信する。送信失敗でも sequence は進める（切断中の欠番は復元側で扱う）。
    pub fn emit(&mut self, event: Event) {
        let _ = self
            .sender
            .send_blocking(D::from(self.sequencer.next(event)));
    }

    /// 採番せずに次の envelope だけ作る。送信は呼出し側が `commit` で確定する。
    pub fn peek(&self, event: Event) -> Envelope {
        self.sequencer.peek(event)
    }

    /// `peek` で作った envelope の送信が成功したときに採番を確定する。
    pub fn commit(&mut self) {
        self.sequencer.advance();
    }

    /// 送信を呼出し側の関数で行い、成功したときだけ採番する。
    /// 途中中断を許す producer（mock）向け。
    pub fn send_with(
        &mut self,
        event: Event,
        send: impl FnOnce(&async_channel::Sender<D>, D) -> bool,
    ) -> bool {
        let envelope = self.sequencer.peek(event);
        if send(&self.sender, D::from(envelope)) {
            self.sequencer.advance();
            true
        } else {
            false
        }
    }

    /// イベント以外の Delivery を送る際に使うチャネル。
    pub fn sender(&self) -> &async_channel::Sender<D> {
        &self.sender
    }
}

#[derive(Debug)]
pub enum Decoded {
    Known(Event),
    Unknown { kind: String, payload: Value },
}

/// 未知 schema か未知の type なら、kind 名(なければ "(type なし)")を返す。
/// type が無い・文字列でない payload は未知ではなく「既知 type の破損」として
/// 呼出し側の serde エラーに回す。
fn unknown_kind(schema_version: u32, kind: Option<&str>) -> Option<&str> {
    (schema_version != SCHEMA_VERSION
        || kind.is_some_and(|kind| !Event::KNOWN_TYPES.contains(&kind)))
    .then(|| kind.unwrap_or("(type なし)"))
}

impl Decoded {
    /// 所有済みの payload はコピーせずに event へ変換する。
    pub(crate) fn from_payload(
        schema_version: u32,
        payload: Value,
    ) -> Result<Self, serde_json::Error> {
        let kind = payload.get("type").and_then(Value::as_str);
        if let Some(kind) = unknown_kind(schema_version, kind) {
            return Ok(Self::Unknown {
                kind: kind.to_owned(),
                payload,
            });
        }
        serde_json::from_value(payload).map(Self::Known)
    }

    /// 借用 payload の decode。unknown 判定に payload を複製しない。
    fn from_payload_ref(schema_version: u32, payload: &Value) -> Result<Self, serde_json::Error> {
        let kind = payload.get("type").and_then(Value::as_str);
        if let Some(kind) = unknown_kind(schema_version, kind) {
            return Ok(Self::Unknown {
                kind: kind.to_owned(),
                payload: payload.clone(),
            });
        }
        Event::deserialize(payload).map(Self::Known)
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
        /// "read"/"exec" などのツール名。古いイベントや ACP の報告では None。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
    },
    ToolFinished {
        invocation_id: String,
        exit_code: i32,
        /// 結果の一行要約。無い場合は投影側で終了コードから組み立てる。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
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
    /// 承認 UI への問い合わせと採番は worker が emit する。決定は ApprovalDecided。
    ApprovalRequested {
        request_id: String,
        title: String,
        executor: String,
        command: Option<String>,
        details: Value,
        /// 承認対象のツール呼び出し。特定できない場合は None。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invocation_id: Option<String>,
    },
    /// user・auto・denied いずれもここに残る。accepted=false は拒否・中止を含む。
    ApprovalDecided {
        request_id: String,
        accepted: bool,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invocation_id: Option<String>,
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
        /// 変更を起こしたツール呼び出し。特定できない場合は None。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invocation_id: Option<String>,
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
        "approval_requested",
        "approval_decided",
        "turn_failed",
        "turn_cancelled",
        "disconnected",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Event の全 variant を 1 つずつ。variant を増やしたら KNOWN_TYPES と
    /// この一覧の両方を更新する(片方だけだと unknown 扱いにずれる)。
    fn all_variants() -> Vec<Event> {
        vec![
            Event::SessionCreated {
                title: "t".into(),
                workspace_id: "w".into(),
                settings: Value::Null,
            },
            Event::TurnStarted { prompt: "p".into() },
            Event::ModelRequestStarted {
                provider: "p".into(),
                model: "m".into(),
                request_id: "r".into(),
            },
            Event::MessageDelta {
                message_id: "m".into(),
                text: "t".into(),
            },
            Event::ToolStarted {
                invocation_id: "i".into(),
                command: "c".into(),
                cwd: "d".into(),
                agent_id: None,
                tool: None,
            },
            Event::ToolFinished {
                invocation_id: "i".into(),
                exit_code: 0,
                summary: None,
            },
            Event::AgentStarted {
                agent_id: "a".into(),
                name: "n".into(),
                task: "t".into(),
                parent_agent_id: None,
            },
            Event::AgentFinished {
                agent_id: "a".into(),
                success: true,
                summary: "s".into(),
            },
            Event::ApprovalRequested {
                request_id: "r".into(),
                title: "t".into(),
                executor: "e".into(),
                command: None,
                details: Value::Null,
                invocation_id: None,
            },
            Event::ApprovalDecided {
                request_id: "r".into(),
                accepted: true,
                source: "s".into(),
                invocation_id: None,
            },
            Event::Log {
                level: "tool".into(),
                preview: "p".into(),
                offset: 0,
                bytes: 0,
            },
            Event::DiffUpdated {
                path: "p".into(),
                unified_diff: "d".into(),
                invocation_id: None,
            },
            Event::TurnCompleted {
                reason: "r".into(),
                usage: Usage::default(),
            },
            Event::TurnFailed { reason: "r".into() },
            Event::TurnCancelled { reason: "r".into() },
            Event::Disconnected { reason: "r".into() },
        ]
    }

    fn envelope(schema_version: u32, payload: Value) -> Envelope {
        Envelope {
            schema_version,
            event_id: "e".into(),
            session_id: SessionId::parse("s").unwrap(),
            sequence: 1,
            timestamp_ms: 0,
            turn_id: None,
            payload,
        }
    }

    #[test]
    fn every_event_variant_is_registered_and_round_trips() {
        let events = all_variants();
        assert_eq!(
            events.len(),
            Event::KNOWN_TYPES.len(),
            "variant と KNOWN_TYPES の数が一致しません"
        );
        for event in events {
            let payload = serde_json::to_value(&event).unwrap();
            let kind = payload["type"].as_str().unwrap().to_owned();
            assert!(
                Event::KNOWN_TYPES.contains(&kind.as_str()),
                "{kind} が KNOWN_TYPES にありません"
            );
            let decoded = envelope(SCHEMA_VERSION, payload).decode().unwrap();
            assert!(
                matches!(decoded, Decoded::Known(_)),
                "{kind} を既知イベントとして decode できませんでした"
            );
        }
    }

    #[test]
    fn decode_distinguishes_unknown_kinds_from_broken_payloads() {
        // 未知の type・未知の schema は欠落を避けて Unknown として保持する。
        for envelope in [
            envelope(SCHEMA_VERSION, json!({"type":"future_event","data":1})),
            envelope(99, json!({"type":"turn_started","prompt":"p"})),
        ] {
            assert!(matches!(envelope.decode(), Ok(Decoded::Unknown { .. })));
        }
        // 既知 type の破損・type 欠落は未知イベントではなく decode エラー。
        for envelope in [
            envelope(SCHEMA_VERSION, json!({"type":"turn_started"})),
            envelope(SCHEMA_VERSION, json!({"note":"no type"})),
        ] {
            assert!(envelope.decode().is_err());
        }
    }
}

use crate::event::{Decoded, Envelope, Event, Usage, preview};
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAX_LOG_ROWS: usize = 1_000;
pub const MAX_CHAT_BLOCKS: usize = 16_384;
pub const CHAT_BLOCK_BYTES: usize = 1_024;
pub const MAX_DIFF_LINES: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Connecting,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
    Disconnected,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "待機中",
            Self::Connecting => "接続中",
            Self::Running => "受信中",
            Self::Cancelling => "停止要求中",
            Self::Completed => "完了",
            Self::Cancelled => "中止",
            Self::Failed => "失敗",
            Self::Disconnected => "切断・結果未確認",
        }
    }
    pub fn is_active(self) -> bool {
        matches!(self, Self::Connecting | Self::Running | Self::Cancelling)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Speaker {
    User,
    Assistant,
    Notice,
}

#[derive(Clone, Debug)]
pub struct ChatBlock {
    pub speaker: Speaker,
    pub message_id: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct LogRow {
    pub sequence: u64,
    pub level: String,
    pub text: String,
    pub offset: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    pub kind: DiffKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Header,
    Context,
    Added,
    Removed,
}

#[derive(Debug)]
pub struct Diff {
    pub path: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
}

impl Diff {
    pub fn parse(path: String, text: &str) -> Self {
        let mut old = 0;
        let mut new = 0;
        let mut lines = Vec::new();
        let mut truncated = false;
        for line in text.lines() {
            if lines.len() == MAX_DIFF_LINES {
                truncated = true;
                break;
            }
            let kind = if line.starts_with("@@") {
                let mut parts = line.split_whitespace().skip(1);
                old = parts
                    .next()
                    .and_then(|p| p.trim_start_matches('-').split(',').next()?.parse().ok())
                    .unwrap_or(0);
                new = parts
                    .next()
                    .and_then(|p| p.trim_start_matches('+').split(',').next()?.parse().ok())
                    .unwrap_or(0);
                DiffKind::Header
            } else if line.starts_with("+++")
                || line.starts_with("---")
                || line.starts_with("diff ")
                || line.starts_with("index ")
                || line.starts_with('\\')
            {
                DiffKind::Header
            } else if line.starts_with('+') {
                DiffKind::Added
            } else if line.starts_with('-') {
                DiffKind::Removed
            } else {
                DiffKind::Context
            };
            let old_number = matches!(kind, DiffKind::Context | DiffKind::Removed).then_some(old);
            let new_number = matches!(kind, DiffKind::Context | DiffKind::Added).then_some(new);
            old += u32::from(old_number.is_some());
            new += u32::from(new_number.is_some());
            lines.push(DiffLine {
                old: old_number,
                new: new_number,
                text: preview(line, 2_048),
                kind,
            });
        }
        Self {
            path,
            lines,
            truncated,
        }
    }
}

#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub status: Status,
    pub reason: String,
    pub turn_id: Option<String>,
    pub usage: Usage,
    pub provider: String,
    pub chat: VecDeque<ChatBlock>,
    pub chat_discarded: usize,
    pub logs: VecDeque<LogRow>,
    pub logs_discarded: usize,
    pub diffs: Vec<Diff>,
    pub tools: HashMap<String, Option<i32>>,
    pub last_sequence: u64,
    pub accepted: u64,
    pub duplicates: u64,
    pub unknown: u64,
    pub rejected: u64,
    pub log_bytes: u64,
    pub incomplete: bool,
    turn_open: bool,
    seen: HashSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Apply {
    Applied,
    Duplicate,
    Rejected,
}

impl Session {
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            workspace: String::new(),
            status: Status::Idle,
            reason: String::new(),
            turn_id: None,
            usage: Usage::default(),
            provider: "疑似プロバイダー".into(),
            chat: VecDeque::new(),
            chat_discarded: 0,
            logs: VecDeque::new(),
            logs_discarded: 0,
            diffs: Vec::new(),
            tools: HashMap::new(),
            last_sequence: 0,
            accepted: 0,
            duplicates: 0,
            unknown: 0,
            rejected: 0,
            log_bytes: 0,
            incomplete: false,
            turn_open: false,
            seen: HashSet::new(),
        }
    }

    pub fn apply(&mut self, envelope: Envelope) -> Apply {
        if envelope.session_id != self.id {
            self.reject("別セッションのイベントを拒否しました");
            return Apply::Rejected;
        }
        if self.seen.contains(&envelope.event_id) {
            self.duplicates += 1;
            return Apply::Duplicate;
        }
        if envelope.sequence <= self.last_sequence || envelope.event_id.is_empty() {
            self.reject("順序が逆転した、または ID がないイベントを拒否しました");
            return Apply::Rejected;
        }
        if envelope.sequence != self.last_sequence + 1 {
            self.incomplete = true;
            self.notice(format!(
                "イベント欠落: {} → {}",
                self.last_sequence, envelope.sequence
            ));
        }
        self.last_sequence = envelope.sequence;
        self.seen.insert(envelope.event_id.clone());
        let decoded = match envelope.decode() {
            Ok(decoded) => decoded,
            Err(error) => {
                self.incomplete = true;
                self.reject(&format!("不正なイベント: {error}"));
                return Apply::Rejected;
            }
        };
        let event = match decoded {
            Decoded::Unknown { kind, payload } => {
                self.unknown += 1;
                self.notice(format!(
                    "未対応イベント: {kind} / schema {} / {}",
                    envelope.schema_version,
                    preview(&payload.to_string(), 384)
                ));
                self.accepted += 1;
                return Apply::Applied;
            }
            Decoded::Known(event) => event,
        };
        // 遅れて届いた旧 turn の delta/完了を、新しい turn へ混入させない。
        if !matches!(
            event,
            Event::SessionCreated { .. } | Event::TurnStarted { .. }
        ) && (envelope.turn_id != self.turn_id || self.turn_id.is_none())
        {
            self.reject("turn_id が現在の実行と一致しません");
            return Apply::Rejected;
        }
        if matches!(event, Event::TurnStarted { .. }) && envelope.turn_id.is_none() {
            self.reject("TurnStarted に turn_id がありません");
            return Apply::Rejected;
        }
        if matches!(event, Event::TurnStarted { .. }) && self.turn_open {
            self.incomplete = true;
            self.reject("実行中の turn を別の TurnStarted で上書きできません");
            return Apply::Rejected;
        }
        if matches!(event, Event::SessionCreated { .. }) && self.accepted > 0 {
            self.reject("作成済みセッションへの SessionCreated を拒否しました");
            return Apply::Rejected;
        }
        if !matches!(
            event,
            Event::SessionCreated { .. } | Event::TurnStarted { .. }
        ) && !self.turn_open
        {
            self.reject("終了済み turn へのイベントを拒否しました");
            return Apply::Rejected;
        }
        let invalid_tool = match &event {
            Event::ToolStarted { invocation_id, .. } => self.tools.contains_key(invocation_id),
            Event::ToolFinished { invocation_id, .. } => {
                !matches!(self.tools.get(invocation_id), Some(None))
            }
            _ => false,
        };
        if invalid_tool {
            self.incomplete = true;
            self.reject("tool の開始・終了記録が一致しません");
            return Apply::Rejected;
        }
        self.accepted += 1;
        match event {
            Event::SessionCreated {
                title,
                workspace_id,
                ..
            } => {
                self.title = title;
                self.workspace = workspace_id;
            }
            Event::TurnStarted { prompt } => {
                if self.status != Status::Cancelling {
                    self.status = Status::Running;
                }
                self.turn_open = true;
                self.reason.clear();
                self.usage = Usage::default();
                self.incomplete = false;
                self.tools.clear();
                self.turn_id = envelope.turn_id;
                self.append(
                    Speaker::User,
                    &format!("prompt-{}", envelope.sequence),
                    &prompt,
                );
            }
            Event::ModelRequestStarted {
                provider, model, ..
            } => self.provider = format!("{provider} / {model}"),
            Event::MessageDelta { message_id, text } => {
                self.append(Speaker::Assistant, &message_id, &text);
            }
            Event::Log {
                level,
                preview: text,
                offset,
                bytes,
            } => {
                self.log_bytes += bytes;
                self.log(LogRow {
                    sequence: envelope.sequence,
                    level,
                    text,
                    offset: Some(offset),
                });
            }
            Event::ToolStarted {
                invocation_id,
                command,
                cwd,
            } => {
                self.tools.insert(invocation_id, None);
                self.notice(format!("疑似実行開始: {command}  ({cwd})"));
            }
            Event::ToolFinished {
                invocation_id,
                exit_code,
            } => {
                self.tools.insert(invocation_id.clone(), Some(exit_code));
                self.notice(format!("疑似実行終了: {invocation_id} / exit {exit_code}"));
            }
            Event::DiffUpdated { path, unified_diff } => {
                let diff = Diff::parse(path.clone(), &unified_diff);
                if let Some(existing) = self.diffs.iter_mut().find(|diff| diff.path == path) {
                    *existing = diff;
                } else {
                    self.diffs.push(diff);
                }
            }
            Event::TurnCompleted { reason, usage } => {
                self.usage = usage;
                if self.tools.values().any(Option::is_none) || self.incomplete {
                    self.finish(
                        Status::Disconnected,
                        "完了通知を受信しましたが、未完了の tool またはイベントの欠落があります"
                            .into(),
                    );
                } else {
                    self.finish(Status::Completed, reason);
                }
            }
            Event::TurnCancelled { reason } => self.finish(Status::Cancelled, reason),
            Event::TurnFailed { reason } => self.finish(Status::Failed, reason),
            Event::Disconnected { reason } => self.finish(Status::Disconnected, reason),
        }
        Apply::Applied
    }

    pub fn transport_closed(&mut self) {
        if self.status.is_active() {
            self.finish(
                Status::Disconnected,
                "完了イベントを受信する前に接続が閉じました".into(),
            );
        }
    }

    fn finish(&mut self, status: Status, reason: String) {
        self.turn_open = false;
        self.status = status;
        self.reason = reason.clone();
        self.append(
            Speaker::Notice,
            &format!("end-{}", self.last_sequence),
            &reason,
        );
        self.notice(reason);
    }

    fn reject(&mut self, message: &str) {
        self.rejected += 1;
        self.notice(message.into());
    }

    fn notice(&mut self, text: String) {
        self.log(LogRow {
            sequence: self.last_sequence,
            level: "event".into(),
            text: preview(&text, 512),
            offset: None,
        });
    }

    fn log(&mut self, row: LogRow) {
        if self.logs.len() == MAX_LOG_ROWS {
            self.logs.pop_front();
            self.logs_discarded += 1;
        }
        self.logs.push_back(row);
    }

    fn append(&mut self, speaker: Speaker, message_id: &str, mut text: &str) {
        while !text.is_empty() {
            let reuse = self.chat.back().is_some_and(|block| {
                block.message_id == message_id
                    && block.speaker == speaker
                    && block.text.len() < CHAT_BLOCK_BYTES - 4
            });
            if !reuse {
                if self.chat.len() == MAX_CHAT_BLOCKS {
                    self.chat.pop_front();
                    self.chat_discarded += 1;
                }
                self.chat.push_back(ChatBlock {
                    speaker: speaker.clone(),
                    message_id: message_id.into(),
                    text: String::new(),
                });
            }
            let block = self.chat.back_mut().expect("a block was just added");
            let mut end = text.len().min(CHAT_BLOCK_BYTES - block.text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            block.text.push_str(&text[..end]);
            text = &text[end..];
        }
    }
}

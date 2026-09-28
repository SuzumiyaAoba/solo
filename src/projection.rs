mod diff;
mod thread;
pub use diff::{Diff, DiffKind, DiffLine, MAX_DIFF_LINES};
pub use thread::{
    ActivityKind, ActivityState, ExecutionActivity, ExecutionThread, MAX_EXECUTION_THREADS,
    MAX_THREAD_ACTIVITIES,
};

use crate::event::{Decoded, Envelope, Event, Usage, preview};
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAX_LOG_ROWS: usize = 1_000;
pub const MAX_CHAT_BLOCKS: usize = 16_384;
pub const CHAT_BLOCK_BYTES: usize = 1_024;
pub const MAX_TOOL_ACTIVITIES: usize = 256;

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
    pub thread_id: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct LogRow {
    pub sequence: u64,
    pub level: String,
    pub text: String,
    pub offset: Option<u64>,
}

#[derive(Debug)]
pub struct ToolActivity {
    pub invocation_id: String,
    pub command: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
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
    pub tool_activity: VecDeque<ToolActivity>,
    pub threads: VecDeque<ExecutionThread>,
    pub threads_discarded: usize,
    pub thread_revision: u64,
    pub last_sequence: u64,
    pub accepted: u64,
    pub duplicates: u64,
    pub unknown: u64,
    pub rejected: u64,
    pub log_bytes: u64,
    pub incomplete: bool,
    turn_open: bool,
    seen: HashSet<String>,
    agents: HashMap<String, bool>,
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
            tool_activity: VecDeque::new(),
            threads: VecDeque::new(),
            threads_discarded: 0,
            thread_revision: 0,
            last_sequence: 0,
            accepted: 0,
            duplicates: 0,
            unknown: 0,
            rejected: 0,
            log_bytes: 0,
            incomplete: false,
            turn_open: false,
            seen: HashSet::new(),
            agents: HashMap::new(),
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
        let gap = envelope.sequence != self.last_sequence + 1;
        if gap {
            self.incomplete = true;
            self.notice(format!(
                "イベント欠落: {} → {}",
                self.last_sequence, envelope.sequence
            ));
        }
        self.last_sequence = envelope.sequence;
        self.seen.insert(envelope.event_id);
        let decoded = match Decoded::from_payload(envelope.schema_version, envelope.payload) {
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
            Event::ToolStarted {
                invocation_id,
                agent_id,
                ..
            } => {
                self.tools.contains_key(invocation_id)
                    || agent_id
                        .as_ref()
                        .is_some_and(|id| self.agents.get(id) != Some(&false))
            }
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
        let invalid_agent = match &event {
            Event::AgentStarted {
                agent_id,
                parent_agent_id,
                ..
            } => {
                agent_id.is_empty()
                    || self.agents.contains_key(agent_id)
                    || parent_agent_id
                        .as_ref()
                        .is_some_and(|id| self.agents.get(id) != Some(&false))
            }
            Event::AgentFinished { agent_id, .. } => self.agents.get(agent_id) != Some(&false),
            _ => false,
        };
        if invalid_agent {
            self.incomplete = true;
            self.reject("サブエージェントの開始・終了記録が一致しません");
            return Apply::Rejected;
        }
        if let Some(thread) = self.threads.back_mut()
            && thread.record(&event)
        {
            self.thread_revision += 1;
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
                self.incomplete = gap;
                self.tools.clear();
                self.agents.clear();
                self.tool_activity.clear();
                self.turn_id = envelope.turn_id;
                if self.threads.len() == MAX_EXECUTION_THREADS {
                    self.threads.pop_front();
                    self.threads_discarded += 1;
                }
                self.threads.push_back(ExecutionThread::new(
                    envelope.sequence,
                    self.turn_id.clone().expect("validated turn id"),
                    &prompt,
                ));
                self.thread_revision += 1;
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
                ..
            } => {
                self.tools.insert(invocation_id.clone(), None);
                if self.tool_activity.len() == MAX_TOOL_ACTIVITIES {
                    self.tool_activity.pop_front();
                }
                self.tool_activity.push_back(ToolActivity {
                    invocation_id,
                    command: preview(&command, 512),
                    cwd: preview(&cwd, 512),
                    exit_code: None,
                });
                self.notice(format!("実行開始: {command}  ({cwd})"));
            }
            Event::ToolFinished {
                invocation_id,
                exit_code,
            } => {
                self.tools.insert(invocation_id.clone(), Some(exit_code));
                if let Some(activity) = self
                    .tool_activity
                    .iter_mut()
                    .find(|activity| activity.invocation_id == invocation_id)
                {
                    activity.exit_code = Some(exit_code);
                }
                self.notice(format!("実行終了: {invocation_id} / exit {exit_code}"));
            }
            Event::AgentStarted { agent_id, name, .. } => {
                self.agents.insert(agent_id, false);
                self.notice(format!("サブエージェント開始: {name}"));
            }
            Event::AgentFinished {
                agent_id,
                success,
                summary,
            } => {
                self.agents.insert(agent_id.clone(), true);
                self.notice(format!(
                    "サブエージェント終了: {agent_id} / {} / {summary}",
                    if success { "完了" } else { "失敗" }
                ));
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
                if self.tools.values().any(Option::is_none)
                    || self.agents.values().any(|done| !done)
                    || self.incomplete
                {
                    self.finish(
                        Status::Disconnected,
                        "完了通知を受信しましたが、未完了の実行またはイベントの欠落があります"
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

    /// 永続化ストアから復元した未完了の実行を「結果未確認」として閉じる。
    pub fn mark_recovered(&mut self, reason: impl Into<String>) {
        if self.status.is_active() || self.turn_open {
            self.incomplete = true;
            self.finish(Status::Disconnected, reason.into());
        }
    }

    /// 実行結果と独立した不整合(保存打ち切り・途中行の破損など)を記録する。
    pub fn flag_incomplete(&mut self, reason: impl Into<String>) {
        self.incomplete = true;
        self.notice(reason.into());
    }

    pub fn transport_failed(&mut self, reason: String) {
        let status = if self.status == Status::Cancelling {
            Status::Cancelled
        } else {
            Status::Failed
        };
        self.finish(status, reason);
    }

    pub fn unreviewed_count(&self) -> usize {
        self.diffs.iter().filter(|diff| !diff.reviewed).count()
    }

    /// 更新途中や省略された差分を「確認済み」と扱わない。
    pub fn toggle_reviewed(&mut self, index: usize) -> bool {
        if self.status.is_active() {
            return false;
        }
        let Some(diff) = self.diffs.get_mut(index).filter(|diff| !diff.truncated) else {
            return false;
        };
        diff.reviewed = !diff.reviewed;
        true
    }

    fn finish(&mut self, status: Status, reason: String) {
        self.turn_open = false;
        self.status = status;
        self.reason = reason.clone();
        if let Some(thread) = self
            .threads
            .back_mut()
            .filter(|thread| thread.status.is_active())
        {
            thread.finish(status, &reason);
            self.thread_revision += 1;
        }
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
                    thread_id: self.threads.back().map(|thread| thread.id),
                });
            }
            let block = self.chat.back_mut().expect("a block was just added");
            let end = text.floor_char_boundary(CHAT_BLOCK_BYTES - block.text.len());
            block.text.push_str(&text[..end]);
            text = &text[end..];
        }
    }
}

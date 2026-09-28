//! 依頼ごとに保持する、件数を制限した実行スレッド。描画や worker に依存しない。
use super::Status;
use crate::event::{Event, TurnId};
use crate::text::preview;
use std::collections::VecDeque;

pub const MAX_EXECUTION_THREADS: usize = 128;
pub const MAX_THREAD_ACTIVITIES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl ActivityState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "実行中",
            Self::Succeeded => "完了",
            Self::Failed => "失敗",
            Self::Cancelled => "中止",
            Self::Interrupted => "結果未確認",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Tool,
    Agent,
}

/// ツール呼び出しに紐づく承認の状態。Allowed/Denied は決定の出どころ(source)を保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityApproval {
    Pending,
    Allowed(String),
    Denied(String),
}

#[derive(Debug)]
pub struct ExecutionActivity {
    pub id: String,
    pub kind: ActivityKind,
    /// 呼び出したツール名("read"・"exec" など)。古いイベントや取得不能な報告では None。
    pub tool: Option<String>,
    pub title: String,
    pub detail: String,
    pub parent_agent_id: Option<String>,
    pub state: ActivityState,
    pub result: String,
    /// 開始・終了イベントの envelope 時刻。片方だけなら所要時間は出さない。
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
    /// この呼び出しが起こした差分のファイル。DiffUpdated の invocation_id で紐づける。
    pub changed_paths: Vec<String>,
    /// 呼び出しに対応する承認要求の状態。
    pub approval: Option<ActivityApproval>,
}

impl ExecutionActivity {
    /// 開始・終了の両方が記録されている場合だけ所要時間を返す。
    pub fn duration_ms(&self) -> Option<u64> {
        self.finished_ms?.checked_sub(self.started_ms?)
    }
}

#[derive(Debug)]
pub struct ExecutionThread {
    /// TurnStarted の sequence。同じ turn_id を再利用する旧 producer でも衝突しない。
    pub id: u64,
    pub turn_id: TurnId,
    pub prompt: String,
    pub status: Status,
    pub reason: String,
    /// TurnStarted と turn 終了イベントの envelope 時刻。
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
    pub activities: VecDeque<ExecutionActivity>,
    pub discarded: usize,
}

impl ExecutionThread {
    pub(super) fn new(id: u64, turn_id: TurnId, prompt: &str, started_ms: u64) -> Self {
        Self {
            id,
            turn_id,
            prompt: preview(prompt, 4096),
            status: Status::Running,
            reason: String::new(),
            started_ms: Some(started_ms),
            finished_ms: None,
            activities: VecDeque::new(),
            discarded: 0,
        }
    }

    pub fn activity_count(&self) -> usize {
        self.activities.len() + self.discarded
    }

    /// スレッド全体の所要時間。turn の終了イベントの時刻から算出する。
    pub fn duration_ms(&self) -> Option<u64> {
        self.finished_ms?.checked_sub(self.started_ms?)
    }

    /// スレッド内の全実行が変更したファイルの和集合。パスは初出順で重複しない。
    pub fn changed_files(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = Vec::new();
        for activity in &self.activities {
            for path in &activity.changed_paths {
                if !paths.contains(&path.as_str()) {
                    paths.push(path.as_str());
                }
            }
        }
        paths
    }

    /// tool/agent の実行開始・終了・承認・変更ファイルを記録する。
    /// 関心外のイベントは false を返し、UI の再描画契機を増やさない。
    pub(super) fn record(&mut self, event: &Event, timestamp_ms: u64) -> bool {
        let entry = match event {
            Event::ToolStarted {
                invocation_id,
                command,
                cwd,
                agent_id,
                tool,
            } => ExecutionActivity {
                id: invocation_id.clone(),
                kind: ActivityKind::Tool,
                tool: tool.clone(),
                title: preview(command, 1024),
                detail: preview(cwd, 1024),
                parent_agent_id: agent_id.clone(),
                state: ActivityState::Running,
                result: String::new(),
                started_ms: Some(timestamp_ms),
                finished_ms: None,
                changed_paths: Vec::new(),
                approval: None,
            },
            Event::AgentStarted {
                agent_id,
                name,
                task,
                parent_agent_id,
            } => ExecutionActivity {
                id: agent_id.clone(),
                kind: ActivityKind::Agent,
                tool: None,
                title: preview(name, 256),
                detail: preview(task, 1024),
                parent_agent_id: parent_agent_id.clone(),
                state: ActivityState::Running,
                result: String::new(),
                started_ms: Some(timestamp_ms),
                finished_ms: None,
                changed_paths: Vec::new(),
                approval: None,
            },
            Event::ToolFinished {
                invocation_id,
                exit_code,
                summary,
            } => {
                self.complete(
                    invocation_id,
                    ActivityKind::Tool,
                    *exit_code == 0,
                    summary
                        .as_deref()
                        .map(|summary| preview(summary, 1024))
                        .unwrap_or_else(|| format!("終了コード {exit_code}")),
                    timestamp_ms,
                );
                return true;
            }
            Event::AgentFinished {
                agent_id,
                success,
                summary,
            } => {
                self.complete(
                    agent_id,
                    ActivityKind::Agent,
                    *success,
                    preview(summary, 1024),
                    timestamp_ms,
                );
                return true;
            }
            Event::DiffUpdated {
                path,
                invocation_id,
                ..
            } => {
                return self.record_changed_path(invocation_id.as_deref(), path);
            }
            Event::ApprovalRequested { invocation_id, .. } => {
                return self.record_approval(invocation_id.as_deref(), ActivityApproval::Pending);
            }
            Event::ApprovalDecided {
                invocation_id,
                accepted,
                source,
                ..
            } => {
                return self.record_approval(
                    invocation_id.as_deref(),
                    if *accepted {
                        ActivityApproval::Allowed(source.clone())
                    } else {
                        ActivityApproval::Denied(source.clone())
                    },
                );
            }
            _ => return false,
        };
        if self.activities.len() == MAX_THREAD_ACTIVITIES {
            self.activities.pop_front();
            self.discarded += 1;
        }
        self.activities.push_back(entry);
        true
    }

    fn complete(
        &mut self,
        id: &str,
        kind: ActivityKind,
        success: bool,
        result: String,
        finished_ms: u64,
    ) {
        if let Some(activity) = self
            .activities
            .iter_mut()
            .find(|entry| entry.id == id && entry.kind == kind)
        {
            activity.state = if success {
                ActivityState::Succeeded
            } else {
                ActivityState::Failed
            };
            activity.result = result;
            activity.finished_ms = Some(finished_ms);
        }
    }

    /// invocation_id で特定できる tool 実行だけを変更・承認の対象にする。
    /// 上限で捨てられた実行や、ID が無い・不明なイベントは素通りさせる。
    fn tool_activity_mut(&mut self, invocation_id: Option<&str>) -> Option<&mut ExecutionActivity> {
        let id = invocation_id?;
        self.activities
            .iter_mut()
            .find(|entry| entry.kind == ActivityKind::Tool && entry.id == id)
    }

    fn record_changed_path(&mut self, invocation_id: Option<&str>, path: &str) -> bool {
        let Some(activity) = self.tool_activity_mut(invocation_id) else {
            return false;
        };
        if !activity.changed_paths.iter().any(|known| known == path) {
            activity.changed_paths.push(path.to_owned());
        }
        true
    }

    fn record_approval(&mut self, invocation_id: Option<&str>, approval: ActivityApproval) -> bool {
        let Some(activity) = self.tool_activity_mut(invocation_id) else {
            return false;
        };
        activity.approval = Some(approval);
        true
    }

    pub(super) fn finish(&mut self, status: Status, reason: &str, timestamp_ms: u64) {
        self.status = status;
        self.finished_ms = Some(timestamp_ms);
        self.reason = preview(reason, 1024);
        for activity in &mut self.activities {
            if activity.state == ActivityState::Running {
                activity.state = if status == Status::Cancelled {
                    ActivityState::Cancelled
                } else {
                    ActivityState::Interrupted
                };
            }
        }
    }
}

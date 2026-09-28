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

#[derive(Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Tool,
    Agent,
}

#[derive(Debug)]
pub struct ExecutionActivity {
    pub id: String,
    pub kind: ActivityKind,
    pub title: String,
    pub detail: String,
    pub parent_agent_id: Option<String>,
    pub state: ActivityState,
    pub result: String,
}

#[derive(Debug)]
pub struct ExecutionThread {
    /// TurnStarted の sequence。同じ turn_id を再利用する旧 producer でも衝突しない。
    pub id: u64,
    pub turn_id: TurnId,
    pub prompt: String,
    pub status: Status,
    pub reason: String,
    pub activities: VecDeque<ExecutionActivity>,
    pub discarded: usize,
}

impl ExecutionThread {
    pub(super) fn new(id: u64, turn_id: TurnId, prompt: &str) -> Self {
        Self {
            id,
            turn_id,
            prompt: preview(prompt, 4096),
            status: Status::Running,
            reason: String::new(),
            activities: VecDeque::new(),
            discarded: 0,
        }
    }

    pub fn activity_count(&self) -> usize {
        self.activities.len() + self.discarded
    }

    pub(super) fn record(&mut self, event: &Event) -> bool {
        let entry = match event {
            Event::ToolStarted {
                invocation_id,
                command,
                cwd,
                agent_id,
            } => ExecutionActivity {
                id: invocation_id.clone(),
                kind: ActivityKind::Tool,
                title: preview(command, 1024),
                detail: preview(cwd, 1024),
                parent_agent_id: agent_id.clone(),
                state: ActivityState::Running,
                result: String::new(),
            },
            Event::AgentStarted {
                agent_id,
                name,
                task,
                parent_agent_id,
            } => ExecutionActivity {
                id: agent_id.clone(),
                kind: ActivityKind::Agent,
                title: preview(name, 256),
                detail: preview(task, 1024),
                parent_agent_id: parent_agent_id.clone(),
                state: ActivityState::Running,
                result: String::new(),
            },
            Event::ToolFinished {
                invocation_id,
                exit_code,
            } => {
                self.complete(
                    invocation_id,
                    ActivityKind::Tool,
                    *exit_code == 0,
                    format!("終了コード {exit_code}"),
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
                );
                return true;
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

    fn complete(&mut self, id: &str, kind: ActivityKind, success: bool, result: String) {
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
        }
    }

    pub(super) fn finish(&mut self, status: Status, reason: &str) {
        self.status = status;
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

//! 同じ workspace の実行を直列化する、UI に依存しない順番待ち。
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedRun {
    pub session_id: String,
    pub backend: usize,
    pub prompt: String,
}

#[derive(Debug, Default)]
pub struct RunQueue {
    pending: VecDeque<QueuedRun>,
    pub paused: bool,
}

impl RunQueue {
    /// 1 セッションにつき 1 件。再送で順序や依頼を上書きしない。
    pub fn push(&mut self, run: QueuedRun) -> bool {
        if run.prompt.trim().is_empty() || self.position(&run.session_id).is_some() {
            return false;
        }
        self.pending.push_back(run);
        true
    }

    pub fn position(&self, session_id: &str) -> Option<usize> {
        self.pending
            .iter()
            .position(|run| run.session_id == session_id)
            .map(|n| n + 1)
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn cancel(&mut self, session_id: &str) -> Option<QueuedRun> {
        let index = self
            .pending
            .iter()
            .position(|run| run.session_id == session_id)?;
        self.pending.remove(index)
    }

    pub fn next(&mut self, workspace_busy: bool) -> Option<QueuedRun> {
        if workspace_busy || self.paused {
            return None;
        }
        self.pending.pop_front()
    }
}

/// 最初の依頼をセッション名にする。日本語や絵文字の途中で切らない。
pub fn task_title(prompt: &str) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("新しいタスク");
    let mut graphemes = line.graphemes(true);
    let mut title = graphemes.by_ref().take(36).collect::<String>();
    if graphemes.next().is_some() {
        title.push('…');
    }
    title
}

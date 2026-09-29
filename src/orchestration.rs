//! 同じ workspace の実行を直列化する、UI に依存しない順番待ち。
use crate::backend::BackendKind;
use crate::event::SessionId;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedRun {
    pub session_id: SessionId,
    /// 保存形式(QueueEntry)と同じ識別。表示側の選択 index は持たない。
    pub backend: BackendKind,
    /// UI から届いた prompt。末尾改行を残すとキュー一覧のタイトルが崩れるので strip する。
    pub prompt: String,
}

#[derive(Debug, Default)]
pub struct RunQueue {
    pending: VecDeque<QueuedRun>,
    paused: bool,
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

    pub fn position(&self, session_id: &SessionId) -> Option<usize> {
        self.pending
            .iter()
            .position(|run| run.session_id == *session_id)
            .map(|n| n + 1)
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 失敗・切断時に呼出し側が止める。再開は set_paused(false)。
    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn cancel(&mut self, session_id: &SessionId) -> Option<QueuedRun> {
        let index = self
            .pending
            .iter()
            .position(|run| run.session_id == *session_id)?;
        self.pending.remove(index)
    }

    pub fn next(&mut self, workspace_busy: bool) -> Option<QueuedRun> {
        if workspace_busy || self.paused {
            return None;
        }
        self.pending.pop_front()
    }

    /// 保存用に先頭からの参照列を返す。消費しない。
    pub fn entries(&self) -> impl Iterator<Item = &QueuedRun> {
        self.pending.iter()
    }

    /// 復元した待ち行列で置き換える。paused は呼出側の判断で与える。
    /// push と同じ条件(空 prompt・重複 session)を保存データにも適用し、
    /// 落とした件数を返す。壊れた永続化データがキューの不変条件を壊さないため。
    pub fn restore(&mut self, entries: impl IntoIterator<Item = QueuedRun>, paused: bool) -> usize {
        let mut pending = VecDeque::new();
        let mut dropped = 0;
        for run in entries {
            if run.prompt.trim().is_empty()
                || pending
                    .iter()
                    .any(|queued: &QueuedRun| queued.session_id == run.session_id)
            {
                dropped += 1;
            } else {
                pending.push_back(run);
            }
        }
        self.pending = pending;
        self.paused = paused;
        dropped
    }
}

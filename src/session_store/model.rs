pub use crate::backend::BackendKind;
use crate::{
    event::{SCHEMA_VERSION, SessionId},
    harness::Message,
    projection::SessionProjection,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct RestoredSession {
    pub session: SessionProjection,
    pub meta: Option<SessionMeta>,
    pub history: Vec<Message>,
    /// `<dir>/logs/*.log`。復元の対象外だが全文参照用に列挙する(ファイル名ソート)。
    pub log_paths: Vec<PathBuf>,
    pub interrupted: bool,
    pub corrupted_lines: usize,
    pub truncated_tail: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionMeta {
    pub version: u32,
    pub session_id: SessionId,
    pub title: String,
    pub serial: u64,
    pub backend: Option<BackendKind>,
    pub provider: String,
    pub draft: String,
    pub unread_result: bool,
    /// reviewed==true の diff.path。
    pub reviewed: Vec<String>,
    pub store_capped: bool,
    pub closed: bool,
    pub updated_ms: u64,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            session_id: SessionId::default(),
            title: String::new(),
            serial: 0,
            backend: None,
            provider: String::new(),
            draft: String::new(),
            unread_result: false,
            reviewed: Vec::new(),
            store_capped: false,
            closed: false,
            updated_ms: 0,
        }
    }
}

impl SessionMeta {
    pub fn touch(&mut self) {
        self.updated_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceState {
    pub version: u32,
    /// 選択中の session_id。
    pub selected: Option<SessionId>,
    pub queue: StoredQueue,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            selected: None,
            queue: StoredQueue::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StoredQueue {
    pub paused: bool,
    pub entries: Vec<QueueEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueEntry {
    pub session_id: SessionId,
    pub backend: BackendKind,
    pub prompt: String,
}

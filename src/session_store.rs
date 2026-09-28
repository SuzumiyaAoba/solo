//! セッションの追記型イベントストアと、クラッシュ後の復元・掃除・エクスポート。
//! レイアウト: `~/.solo/sessions/<workspace-dir>/<session-id>/{state.json,events.jsonl,history.json,logs/}`。
mod model;
mod store;
#[cfg(test)]
mod tests;
mod transcript;

pub use model::{
    BackendKind, QueueEntry, RestoredSession, SessionMeta, StoredQueue, WorkspaceState,
};
pub use store::{
    MAX_EVENT_STORE_BYTES, MAX_HISTORY_BYTES, SessionFile, WorkspaceStore, exports_root,
    user_sessions_root, workspace_dir_name,
};
pub(crate) use transcript::transcript_markdown;

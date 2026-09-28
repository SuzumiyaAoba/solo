//! Phase 0 のコアと共通 UI。イベント契約・表示投影・疑似ストリームは GPUI に依存しない。
pub mod acp;
pub mod acp_worker;
pub mod approval;
pub mod auto_approval;
pub mod backend;
pub mod codex;
pub mod codex_worker;
pub mod command_rules;
pub mod config;
pub mod design_tokens;
pub mod diffgen;
pub mod event;
pub mod harness;
pub mod mock;
pub mod orchestration;
pub mod projection;
pub mod projects;
pub mod session_store;
mod storage;
pub mod text;

#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod design;

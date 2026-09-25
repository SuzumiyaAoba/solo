//! Phase 0 のコアと共通 UI。イベント契約・表示投影・疑似ストリームは GPUI に依存しない。
pub mod acp;
pub mod acp_worker;
pub mod codex_subscription;
pub mod design_tokens;
pub mod event;
pub mod harness;
pub mod mock;
pub mod projection;
pub mod subscription_worker;
pub mod text;

#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod design;

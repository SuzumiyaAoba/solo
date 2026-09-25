//! GPUI に依存しない Phase 0 のイベント契約・表示投影・疑似ストリーム。
pub mod event;
pub mod mock;
pub mod projection;
pub mod text;
pub mod design_tokens;

#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod design;

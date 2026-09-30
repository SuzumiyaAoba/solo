//! codex_worker / acp_worker / mock の3系統ワーカーに共通する起動骨格。
//! `Delivery` の variant と `Controller` の中身は backend 固有(cancel/切断の意味が
//! 異なる)ため共有せず、スレッド起動とセッション先頭イベントだけをここに置く。
use crate::event::{Emitter, Envelope, Event, Sequencer, SessionId, TurnId};
use async_channel::Sender;
use serde_json::Value;
use std::{io, thread::JoinHandle};

/// UI へ流す delivery チャンネルの容量。3系統で揃える。
pub const CHANNEL_CAPACITY: usize = 256;

/// `solo-{label}-{session_id}` 名のワーカースレッドを起動する。
/// JoinHandle は終了を監視するものだけが保持し、他は捨てる。
pub fn spawn<F>(label: &str, session_id: SessionId, body: F) -> io::Result<JoinHandle<()>>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .name(format!("solo-{label}-{session_id}"))
        .spawn(body)
}

/// Sequencer と送信チャンネルから Emitter を組み立てる。
pub fn emitter<D>(
    session_id: SessionId,
    start_sequence: u64,
    turn_id: impl Into<TurnId>,
    sender: Sender<D>,
) -> Emitter<D>
where
    D: Send + From<Envelope>,
{
    Emitter::new(Sequencer::new(session_id, start_sequence, turn_id), sender)
}

/// start_sequence == 0 の起動だけ SessionCreated を emit する。
/// settings には `{"backend": ...}` 形式の識別 JSON を渡す。
/// 送信の成否で中断を判断する producer (mock) は `Emitter::send_with` を直接使う。
pub fn emit_session_created<D>(
    emitter: &mut Emitter<D>,
    start_sequence: u64,
    title: &str,
    workspace_id: String,
    settings: Value,
) where
    D: Send + From<Envelope>,
{
    if start_sequence == 0 {
        emitter.emit(Event::SessionCreated {
            title: title.to_owned(),
            workspace_id,
            settings,
        });
    }
}

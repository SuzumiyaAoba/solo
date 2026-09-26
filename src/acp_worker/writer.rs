//! 通知は待たずに enqueue し、通常の要求は送信結果を待つ。非同期ポンプが書込み順序を保持する。
use crate::acp::write_message;
use async_channel::{Receiver, Sender};
use serde_json::Value;
use std::io::{self, Write};

struct Frame {
    bytes: Vec<u8>,
    done: Option<Sender<io::Result<()>>>,
}

#[derive(Clone)]
pub(super) struct Writer(Sender<Frame>);

impl Writer {
    pub(super) fn channel() -> (Self, Writes) {
        let (sender, receiver) = async_channel::bounded(16);
        (Self(sender), Writes(receiver))
    }

    pub(super) fn notify(&self, message: Value) -> io::Result<()> {
        let mut bytes = Vec::new();
        write_message(&mut bytes, message)?;
        self.0
            .try_send(Frame { bytes, done: None })
            .map_err(io::Error::other)
    }

    pub(super) fn close(&self) {
        self.0.close();
    }
}

impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let (done, result) = async_channel::bounded(1);
        self.0
            .send_blocking(Frame {
                bytes: bytes.into(),
                done: Some(done),
            })
            .map_err(|_| closed())?;
        result.recv_blocking().map_err(|_| closed())??;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) struct Writes(Receiver<Frame>);

impl Writes {
    pub(super) async fn pump(
        &self,
        mut output: impl tokio::io::AsyncWrite + Unpin,
    ) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        while let Ok(frame) = self.0.recv().await {
            let result = if self.0.is_closed() {
                Err(closed())
            } else {
                match output.write_all(&frame.bytes).await {
                    Ok(()) => output.flush().await,
                    Err(error) => Err(error),
                }
            };
            let failed = result.is_err();
            if let Some(done) = frame.done {
                let _ = done.send(result).await;
            }
            if failed {
                return Err(closed());
            }
        }
        Ok(())
    }

    pub(super) fn close(&self) {
        self.0.close();
        while let Ok(frame) = self.0.try_recv() {
            if let Some(done) = frame.done {
                let _ = done.try_send(Err(closed()));
            }
        }
    }
}

impl Drop for Writes {
    fn drop(&mut self) {
        self.close();
    }
}

fn closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        "ACP agent への送信が終了しました",
    )
}

#[cfg(test)]
mod tests;

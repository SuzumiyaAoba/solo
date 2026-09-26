//! 一つの I/O スレッドで送受信し、切断時は待機中の read / write も破棄する。
use super::{
    reader,
    reader::Reader,
    writer::{Writer, Writes},
};
use async_channel::Sender;
use std::{
    io,
    process::{ChildStdin, ChildStdout},
    thread,
};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

pub(super) struct Worker {
    incoming: Sender<io::Result<Vec<u8>>>,
    outgoing: Writes,
    on_close: Option<Box<dyn FnOnce() + Send>>,
}

pub(super) fn channel() -> (Reader, Writer, Worker) {
    let (reader, incoming) = reader::channel();
    let (writer, outgoing) = Writer::channel();
    (
        reader,
        writer,
        Worker {
            incoming,
            outgoing,
            on_close: None,
        },
    )
}

impl Worker {
    pub(super) fn start(
        mut self,
        stdin: ChildStdin,
        stdout: ChildStdout,
        shutdown: CancellationToken,
        on_close: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        self.on_close = Some(Box::new(on_close));
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let (stdin, stdout) = {
            let _entered = runtime.enter();
            (
                tokio::process::ChildStdin::from_std(stdin)?,
                tokio::process::ChildStdout::from_std(stdout)?,
            )
        };
        thread::Builder::new()
            .name("solo-acp-io".into())
            .spawn(move || {
                let result = runtime.block_on(async {
                    tokio::select! {
                        _ = shutdown.cancelled() => Ok(()),
                        result = reader::pump(stdout, &self.incoming) => result,
                        result = self.outgoing.pump(stdin) => result,
                    }
                });
                self.outgoing.close();
                if let Err(error) = result {
                    let _ = self.incoming.try_send(Err(error));
                }
            })?;
        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.outgoing.close();
        if let Some(on_close) = self.on_close.take() {
            on_close();
        }
        // 受信済みデータを残し、最後の応答を処理してから EOF にする。
        self.incoming.close();
    }
}

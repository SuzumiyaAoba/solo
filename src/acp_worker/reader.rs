//! 非同期の標準出力を同期 JSON-RPC client へ渡す、サイズを制限したキュー。
use async_channel::{Receiver, Sender};
use std::io::{self, Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt};

pub(super) struct Reader {
    input: Receiver<io::Result<Vec<u8>>>,
    current: Cursor<Vec<u8>>,
}

pub(super) fn channel() -> (Reader, Sender<io::Result<Vec<u8>>>) {
    let (output, input) = async_channel::bounded(16);
    (
        Reader {
            input,
            current: Cursor::new(Vec::new()),
        },
        output,
    )
}

impl Read for Reader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            let count = Read::read(&mut self.current, bytes)?;
            if count > 0 {
                return Ok(count);
            }
            match self.input.recv_blocking() {
                Ok(chunk) => self.current = Cursor::new(chunk?),
                Err(_) => return Ok(0),
            }
        }
    }
}

pub(super) async fn pump(
    mut input: impl AsyncRead + Unpin,
    output: &Sender<io::Result<Vec<u8>>>,
) -> io::Result<()> {
    let mut buffer = vec![0; 16 * 1024];
    loop {
        let count = input.read(&mut buffer).await?;
        if count == 0 {
            return Ok(());
        }
        if output.send(Ok(buffer[..count].to_vec())).await.is_err() {
            return Ok(());
        }
    }
}

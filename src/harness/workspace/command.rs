//! コマンド実行の出力制限、中止、終了コードとプロセスの回収。
use super::{Cancellation, ToolResult, WorkspaceTools, invalid};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

impl WorkspaceTools {
    pub(super) fn exec(
        &self,
        command: &str,
        timeout_seconds: Option<u64>,
    ) -> io::Result<ToolResult> {
        if command.is_empty() {
            return Err(invalid("command は空にできません".into()));
        }
        let timeout = match timeout_seconds {
            Some(seconds) if seconds == 0 || Duration::from_secs(seconds) > self.max_timeout => {
                return Err(invalid(format!(
                    "timeout_seconds は 1〜{} の整数で指定してください",
                    self.max_timeout.as_secs()
                )));
            }
            Some(seconds) => Duration::from_secs(seconds),
            None => self.timeout,
        };
        let cancelled = || {
            self.cancellation
                .as_ref()
                .is_some_and(Cancellation::is_cancelled)
        };
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "command を中止しました",
            ));
        }
        let stdout = tempfile::tempfile()?;
        let stderr = tempfile::tempfile()?;
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?;
        let child = ChildCleanup(&mut child);
        let start = Instant::now();
        let status = loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "command を中止しました",
                ));
            }
            match child.0.try_wait()? {
                Some(status) => break status,
                None if start.elapsed() >= timeout => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("command が制限時間（{} 秒）を超えました", timeout.as_secs()),
                    ));
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        // 出力は予算を stdout/stderr で分け、小さい側が食い潰されないようにする。
        let budget = self.max_output_bytes;
        let half = budget / 2;
        let out_len = stdout.metadata()?.len();
        let err_len = stderr.metadata()?.len();
        let out_share = if out_len + err_len <= budget as u64 {
            out_len as usize
        } else if err_len < half as u64 {
            budget - err_len as usize
        } else if out_len < half as u64 {
            out_len as usize
        } else {
            half
        };
        let err_share = budget - out_share;
        let output = format!(
            "{}{}",
            excerpt(stdout, out_len, out_share)?,
            excerpt(stderr, err_len, err_share)?,
        );
        let code = status
            .code()
            .map_or_else(|| "シグナル".to_owned(), |code| code.to_string());
        Ok(ToolResult {
            content: format!("exit: {status}\n{output}"),
            is_error: !status.success(),
            summary: Some(format!(
                "終了コード {code} · stdout {out_len} バイト / stderr {err_len} バイト"
            )),
        })
    }
}

/// 中止・timeout・I/O エラーを含むすべての戻り経路で、直接起動したプロセスを回収する。
struct ChildCleanup<'a>(&'a mut Child);

impl Drop for ChildCleanup<'_> {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// ファイルを share バイト以内の文字列にする。長い場合は先頭と末尾を残して
/// 省略し、切断由来の U+FFFD が出ないよう UTF-8 の境界を避ける。
fn excerpt(mut file: File, len: u64, share: usize) -> io::Result<String> {
    if len <= share as u64 {
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        // 置換文字でサイズが増えても、表示文字列を上限内に収める。
        text.truncate(text.floor_char_boundary(share));
        return Ok(text);
    }
    let omitted = len - share as u64;
    let marker = format!("\n[… {omitted} バイト省略 …]\n");
    if share < marker.len() + 16 {
        // 省略表記すら収まらない小さな share では先頭だけを返す。
        let bytes = read_head(&mut file, share)?;
        return Ok(decode_head(&bytes, share));
    }
    let head_len = (share - marker.len()) / 2;
    let tail_len = share - marker.len() - head_len;
    let mut head = decode_head(&read_head(&mut file, head_len)?, head_len);
    let mut tail = decode_tail(&mut file, len, tail_len)?;
    // 末尾側で置換文字が膨らんだ分は、tail の前側を削って tail_len 内に収める。
    if tail.len() > tail_len {
        tail.drain(..tail.ceil_char_boundary(tail.len() - tail_len));
    }
    // 残り容量を超える head も詰めて、戻り値全体を share 内に収める。
    let head_room = share.saturating_sub(marker.len() + tail.len());
    if head.len() > head_room {
        head.truncate(head.floor_char_boundary(head_room));
    }
    Ok(format!("{head}{marker}{tail}"))
}

fn read_head(file: &mut File, bytes: usize) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut head = Vec::new();
    file.take(bytes as u64).read_to_end(&mut head)?;
    Ok(head)
}

/// 先頭側の抜粋。末尾の不完全な UTF-8 は落とし、途中の不正バイトは
/// 置換文字にしたうえで byte 数を max 以内に収める。
fn decode_head(bytes: &[u8], max: usize) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(error) if error.error_len().is_none() => {
            // 末尾で切れた場合: valid_up_to までは必ず UTF-8。
            std::str::from_utf8(&bytes[..error.valid_up_to()])
                .unwrap_or("")
                .to_owned()
        }
        Err(_) => {
            let mut text = String::from_utf8_lossy(bytes).into_owned();
            text.truncate(text.floor_char_boundary(max));
            text
        }
    }
}

/// 末尾側の抜粋。先頭の継続バイトを飛ばして文字の途中から始めない。
fn decode_tail(file: &mut File, len: u64, bytes: usize) -> io::Result<String> {
    file.seek(SeekFrom::End(-(bytes.min(len as usize) as i64)))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    let mut skip = 0;
    while skip < tail.len() && skip < 3 && (0x80..=0xbf).contains(&tail[skip]) {
        skip += 1;
    }
    Ok(String::from_utf8_lossy(&tail[skip..]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_stops_and_reaps_a_running_process() {
        let mut child = Command::new("sh")
            .args(["-c", "exec sleep 5"])
            .spawn()
            .unwrap();
        drop(ChildCleanup(&mut child));
        let status = child.try_wait().unwrap();
        if status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(status.is_some());
    }
}

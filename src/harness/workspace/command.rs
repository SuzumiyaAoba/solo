//! コマンド実行の出力制限、中止、終了コードとプロセスの回収。
use super::{Cancellation, ToolResult, WorkspaceTools};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

impl WorkspaceTools {
    pub(super) fn exec(&self, command: &str) -> io::Result<ToolResult> {
        if command.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "command は空にできません",
            ));
        }
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
                None if start.elapsed() >= self.timeout => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "command が制限時間を超えました",
                    ));
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        let mut output = read_prefix(stdout, self.max_output_bytes)?;
        output.push_str(&read_prefix(
            stderr,
            self.max_output_bytes.saturating_sub(output.len()),
        )?);
        Ok(ToolResult {
            content: format!("exit: {status}\n{output}"),
            is_error: !status.success(),
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

fn read_prefix(mut file: File, limit: usize) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(limit as u64).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    // 不正な UTF-8 の置換文字でサイズが増えても、表示文字列を上限内に収める。
    text.truncate(text.floor_char_boundary(limit));
    Ok(text)
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

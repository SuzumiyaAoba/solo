//! ローカル workspace 用の基本 tool。exec の実行権限は呼出し側 Policy が決める。
use super::{Cancellation, ToolCall, ToolExecutor, ToolResult, ToolSpec};
use serde_json::json;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub struct WorkspaceTools {
    root: PathBuf,
    specs: Vec<ToolSpec>,
    cancellation: Option<Cancellation>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl WorkspaceTools {
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(std::io::Error::other(
                "workspace がディレクトリではありません",
            ));
        }
        Ok(Self {
            root,
            cancellation: None,
            timeout: Duration::from_secs(30),
            max_output_bytes: 64 * 1024,
            specs: vec![
                spec(
                    "read",
                    "workspace 内の UTF-8 ファイルを読む",
                    json!({"path":"string"}),
                ),
                spec(
                    "search",
                    "workspace 内の UTF-8 ファイルから文字列を探す",
                    json!({"query":"string"}),
                ),
                spec(
                    "edit",
                    "既存ファイルの一致する箇所を一度だけ置換する",
                    json!({"path":"string","old":"string","new":"string"}),
                ),
                spec(
                    "exec",
                    "workspace で shell command を実行する",
                    json!({"command":"string"}),
                ),
            ],
        })
    }

    pub fn with_cancellation(mut self, cancellation: Cancellation) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    fn path(&self, raw: &str) -> Result<PathBuf, String> {
        let candidate = self.root.join(raw);
        let path = candidate.canonicalize().map_err(|e| e.to_string())?;
        if !path.starts_with(&self.root) || !path.is_file() {
            return Err("workspace 外、または通常ファイルではありません".into());
        }
        Ok(path)
    }

    fn read(&self, raw: &str) -> Result<String, String> {
        let path = self.path(raw)?;
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        if metadata.len() > self.max_output_bytes as u64 {
            return Err("ファイルが読み取り上限を超えています".into());
        }
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn edit(&self, raw: &str, old: &str, new: &str) -> Result<String, String> {
        if old.is_empty() {
            return Err("old は空にできません".into());
        }
        let path = self.path(raw)?;
        let original = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if original.matches(old).count() != 1 {
            return Err("old はファイル中で一度だけ一致する必要があります".into());
        }
        let updated = original.replacen(old, new, 1);
        let parent = path.parent().ok_or("親ディレクトリがありません")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        use std::io::Write;
        temp.write_all(updated.as_bytes())
            .map_err(|e| e.to_string())?;
        let permissions = fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions();
        temp.as_file()
            .set_permissions(permissions)
            .map_err(|e| e.to_string())?;
        // 読込み後の外部編集と symlink 差替えを検知する。
        if self.path(raw)? != path
            || fs::read(&path).map_err(|e| e.to_string())? != original.as_bytes()
        {
            return Err("編集中にファイルが変更されました".into());
        }
        temp.persist(&path).map_err(|e| e.to_string())?;
        Ok(format!("{} を更新しました", path.display()))
    }

    fn search(&self, query: &str) -> Result<String, String> {
        if query.is_empty() {
            return Err("query は空にできません".into());
        }
        let mut dirs = vec![self.root.clone()];
        let mut visited = 0usize;
        let mut output = String::new();
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                visited += 1;
                if visited > 20_000 {
                    return Err("検索対象が上限を超えました".into());
                }
                let ty = entry.file_type().map_err(|e| e.to_string())?;
                if ty.is_dir() {
                    if entry.file_name() != ".git" && entry.file_name() != "target" {
                        dirs.push(entry.path());
                    }
                } else if ty.is_file() {
                    let metadata = entry.metadata().map_err(|e| e.to_string())?;
                    if metadata.len() > 1024 * 1024 {
                        continue;
                    }
                    if let Ok(text) = fs::read_to_string(entry.path()) {
                        for (line_no, line) in text.lines().enumerate() {
                            if line.contains(query) {
                                let relative = entry
                                    .path()
                                    .strip_prefix(&self.root)
                                    .map_err(|e| e.to_string())?
                                    .display()
                                    .to_string();
                                let row = format!("{}:{}:{}\n", relative, line_no + 1, line);
                                if output.len() + row.len() > self.max_output_bytes {
                                    output.push_str("[検索結果を省略]\n");
                                    return Ok(output);
                                }
                                output.push_str(&row);
                            }
                        }
                    }
                }
            }
        }
        Ok(output)
    }

    fn exec(&self, command: &str) -> Result<String, String> {
        if command.is_empty() {
            return Err("command は空にできません".into());
        }
        let stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
        let stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(stderr.try_clone().map_err(|e| e.to_string())?))
            .spawn()
            .map_err(|e| e.to_string())?;
        let start = Instant::now();
        let status = loop {
            if self
                .cancellation
                .as_ref()
                .is_some_and(Cancellation::is_cancelled)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command を中止しました".into());
            }
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if start.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("command が制限時間を超えました".into());
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        let mut out = read_prefix(stdout, self.max_output_bytes)?;
        let err = read_prefix(stderr, self.max_output_bytes.saturating_sub(out.len()))?;
        out.push_str(&err);
        Ok(format!("exit: {status}\n{out}"))
    }
}

impl ToolExecutor for WorkspaceTools {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn execute(&mut self, call: &ToolCall) -> ToolResult {
        let string = |key: &str| {
            call.arguments
                .get(key)
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{key} が必要です"))
        };
        let result = match call.name.as_str() {
            "read" => string("path").and_then(|path| self.read(path)),
            "search" => string("query").and_then(|query| self.search(query)),
            "edit" => string("path").and_then(|path| {
                string("old")
                    .and_then(|old| string("new").and_then(|new| self.edit(path, old, new)))
            }),
            "exec" => string("command").and_then(|command| self.exec(command)),
            _ => Err("未知の tool".into()),
        };
        match result {
            Ok(value) => ToolResult::ok(value),
            Err(error) => ToolResult::error(error),
        }
    }
}

fn spec(name: &str, description: &str, properties: serde_json::Value) -> ToolSpec {
    let required: Vec<_> = properties
        .as_object()
        .expect("tool parameters are objects")
        .keys()
        .cloned()
        .collect();
    let properties = properties
        .as_object()
        .expect("tool parameters are objects")
        .iter()
        .map(|(key, _)| (key.clone(), json!({"type":"string"})))
        .collect::<serde_json::Map<_, _>>();
    ToolSpec {
        name: name.into(),
        description: description.into(),
        parameters: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    }
}

fn read_prefix(mut file: File, limit: usize) -> Result<String, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

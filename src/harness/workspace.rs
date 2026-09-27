//! ローカル workspace 用の基本 tool。exec の実行権限は呼出し側 Policy が決める。
mod command;

use super::{Cancellation, ToolCall, ToolExecutor, ToolResult, ToolSpec};
use crate::storage::read_text;
use serde_json::json;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};
/// 副作用のない組込み tool。承認 policy は read/search を確認なしで許可してよい。
pub fn is_read_only_tool(name: &str) -> bool {
    matches!(name, "read" | "search")
}

/// 基本ツールの表示順。`specs` の配列と一致させる。
pub const TOOL_NAMES: &[&str] = &["read", "search", "edit", "exec"];

/// 基本ツールの1行説明。ツールタブにそのまま表示する。
pub fn tool_description(name: &str) -> &'static str {
    match name {
        "read" => "workspace 内の UTF-8 ファイルを読む",
        "search" => "workspace 内の UTF-8 ファイルから文字列を探す",
        "edit" => "既存ファイルの一致する箇所を一度だけ置換する",
        "exec" => "workspace で shell command を実行する",
        _ => "外部ツール",
    }
}

pub struct WorkspaceTools {
    root: PathBuf,
    specs: Vec<ToolSpec>,
    cancellation: Option<Cancellation>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl WorkspaceTools {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::other("workspace がディレクトリではありません"));
        }
        Ok(Self {
            root,
            cancellation: None,
            timeout: Duration::from_secs(30),
            max_output_bytes: 64 * 1024,
            specs: TOOL_NAMES
                .iter()
                .map(|name| {
                    let fields: &[&str] = match *name {
                        "read" => &["path"],
                        "search" => &["query"],
                        "edit" => &["path", "old", "new"],
                        "exec" => &["command"],
                        _ => &[],
                    };
                    spec(name, tool_description(name), fields)
                })
                .collect(),
        })
    }

    pub fn with_cancellation(mut self, cancellation: Cancellation) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    fn path(&self, raw: &str) -> io::Result<PathBuf> {
        let candidate = self.root.join(raw);
        let path = candidate.canonicalize()?;
        if !path.starts_with(&self.root) || !path.is_file() {
            return Err(io::Error::other(
                "workspace 外、または通常ファイルではありません",
            ));
        }
        Ok(path)
    }

    fn read(&self, raw: &str) -> io::Result<String> {
        let path = self.path(raw)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > self.max_output_bytes as u64 {
            return Err(io::Error::other("ファイルが読み取り上限を超えています"));
        }
        read_text(
            &path,
            self.max_output_bytes as u64,
            "ファイルが読み取り上限を超えています",
        )
    }

    fn edit(&self, raw: &str, old: &str, new: &str) -> io::Result<String> {
        if old.is_empty() {
            return Err(io::Error::other("old は空にできません"));
        }
        let path = self.path(raw)?;
        let original = fs::read_to_string(&path)?;
        if original.matches(old).count() != 1 {
            return Err(io::Error::other(
                "old はファイル中で一度だけ一致する必要があります",
            ));
        }
        let updated = original.replacen(old, new, 1);
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("親ディレクトリがありません"))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(updated.as_bytes())?;
        temp.as_file()
            .set_permissions(fs::metadata(&path)?.permissions())?;
        // 読込み後の外部編集と symlink 差替えを検知する。
        if self.path(raw)? != path || fs::read(&path)? != original.as_bytes() {
            return Err(io::Error::other("編集中にファイルが変更されました"));
        }
        temp.persist(&path).map_err(|error| error.error)?;
        Ok(format!("{} を更新しました", path.display()))
    }

    fn search(&self, query: &str) -> io::Result<String> {
        if query.is_empty() {
            return Err(io::Error::other("query は空にできません"));
        }
        let mut dirs = vec![self.root.clone()];
        let mut visited = 0usize;
        let mut output = String::new();
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                visited += 1;
                if visited > 20_000 {
                    return Err(io::Error::other("検索対象が上限を超えました"));
                }
                let ty = entry.file_type()?;
                if ty.is_dir() {
                    if entry.file_name() != ".git" && entry.file_name() != "target" {
                        dirs.push(entry.path());
                    }
                } else if ty.is_file() {
                    let metadata = entry.metadata()?;
                    if metadata.len() > 1024 * 1024 {
                        continue;
                    }
                    if let Ok(text) =
                        read_text(&entry.path(), 1024 * 1024, "検索対象の読み取り上限です")
                    {
                        for (line_no, line) in text.lines().enumerate() {
                            if line.contains(query) {
                                let relative = entry
                                    .path()
                                    .strip_prefix(&self.root)
                                    .map_err(io::Error::other)?
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

    fn execute_call(&self, call: &ToolCall) -> io::Result<ToolResult> {
        let string = |key: &str| {
            call.arguments
                .get(key)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("{key} が必要です"))
                })
        };
        match call.name.as_str() {
            "read" => self.read(string("path")?).map(ToolResult::ok),
            "search" => self.search(string("query")?).map(ToolResult::ok),
            "edit" => self
                .edit(string("path")?, string("old")?, string("new")?)
                .map(ToolResult::ok),
            "exec" => self.exec(string("command")?),
            _ => Err(io::Error::other("未知の tool")),
        }
    }
}

impl ToolExecutor for WorkspaceTools {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn execute(&mut self, call: &ToolCall) -> ToolResult {
        self.execute_call(call)
            .unwrap_or_else(|error| ToolResult::error(error.to_string()))
    }
}

fn spec(name: &str, description: &str, fields: &[&str]) -> ToolSpec {
    let properties = fields
        .iter()
        .map(|key| ((*key).to_owned(), json!({"type":"string"})))
        .collect::<serde_json::Map<_, _>>();
    let required: Vec<_> = properties.keys().collect();
    ToolSpec {
        name: name.into(),
        description: description.into(),
        parameters: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    }
}

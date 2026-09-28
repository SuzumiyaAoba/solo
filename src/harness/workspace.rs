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
pub const TOOL_READ: &str = "read";
pub const TOOL_SEARCH: &str = "search";
pub const TOOL_EDIT: &str = "edit";
pub const TOOL_EXEC: &str = "exec";

pub fn is_read_only_tool(name: &str) -> bool {
    matches!(name, TOOL_READ | TOOL_SEARCH)
}

/// 探索・差分追跡から外すディレクトリ。ベンダーやビルド成果物は対象外にする。
pub(crate) fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | ".next" | "dist" | "build" | ".venv" | "__pycache__"
    )
}

/// 基本ツールの表示順。`specs` の配列と一致させる。
pub const TOOL_NAMES: &[&str] = &[TOOL_READ, TOOL_SEARCH, TOOL_EDIT, TOOL_EXEC];

/// 基本ツールの1行説明。ツールタブにそのまま表示する。
pub fn tool_description(name: &str) -> &'static str {
    match name {
        TOOL_READ => "workspace 内の UTF-8 ファイルを読む",
        TOOL_SEARCH => "workspace 内の UTF-8 ファイルから文字列を探す",
        TOOL_EDIT => "既存ファイルの一致する箇所を一度だけ置換する",
        TOOL_EXEC => "workspace で shell command を実行する",
        _ => "外部ツール",
    }
}

pub struct WorkspaceTools {
    root: PathBuf,
    specs: Vec<ToolSpec>,
    cancellation: Option<Cancellation>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    /// `search` が走査するエントリ数の上限。超過時は部分結果と注記を返す。
    pub max_search_entries: usize,
}

impl WorkspaceTools {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = canonical_workspace(root.as_ref())?;
        Ok(Self {
            root,
            cancellation: None,
            timeout: Duration::from_secs(30),
            max_output_bytes: 64 * 1024,
            max_search_entries: 20_000,
            specs: TOOL_NAMES
                .iter()
                .map(|name| {
                    let fields: &[&str] = match *name {
                        TOOL_READ => &["path"],
                        TOOL_SEARCH => &["query"],
                        TOOL_EDIT => &["path", "old", "new"],
                        TOOL_EXEC => &["command"],
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
        resolve_file(&self.root, raw)
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
        let mut capped = false;
        // workspace 直下の読み取り失敗だけはエラーとして返し、途中の失敗はスキップする。
        'dirs: while let Some(dir) = dirs.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(error) if dir == self.root => return Err(error),
                Err(_) => continue,
            };
            for entry in entries {
                let Ok(entry) = entry else { continue };
                visited += 1;
                if visited > self.max_search_entries {
                    capped = true;
                    break 'dirs;
                }
                let Ok(ty) = entry.file_type() else { continue };
                if ty.is_dir() {
                    if !is_ignored_dir(&entry.file_name().to_string_lossy()) {
                        dirs.push(entry.path());
                    }
                } else if ty.is_file() {
                    let Ok(metadata) = entry.metadata() else {
                        continue;
                    };
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
        if capped {
            output.push_str("[検索対象の上限に達したため、一部のファイルのみ検索しました]\n");
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
            TOOL_READ => self.read(string("path")?).map(ToolResult::ok),
            TOOL_SEARCH => self.search(string("query")?).map(ToolResult::ok),
            TOOL_EDIT => self
                .edit(string("path")?, string("old")?, string("new")?)
                .map(ToolResult::ok),
            TOOL_EXEC => self.exec(string("command")?),
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

/// workspace root を canonicalize し、ディレクトリであることを確認する。
pub(crate) fn canonical_workspace(root: &Path) -> io::Result<PathBuf> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::other("workspace がディレクトリではありません"));
    }
    Ok(root)
}

/// workspace 内の相対パスを canonicalize し、root 配下の通常ファイルに限定する。
pub(crate) fn resolve_file(root: &Path, raw: &str) -> io::Result<PathBuf> {
    let candidate = root.join(raw);
    let path = candidate.canonicalize()?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(io::Error::other(
            "workspace 外、または通常ファイルではありません",
        ));
    }
    Ok(path)
}

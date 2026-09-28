//! search/list の走査。除外ディレクトリと上限は両ツールで共有する。
use super::{WorkspaceTools, invalid, is_ignored_dir};
use crate::{harness::ToolResult, storage::read_text, text::preview};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// 一覧・glob 結果の表示上限。
const MAX_LIST_ENTRIES: usize = 1000;
/// search がファイルを読む上限。
const MAX_SEARCH_FILE_BYTES: u64 = 1024 * 1024;

impl WorkspaceTools {
    /// pattern 無しは直下の一覧、有りは glob に一致するファイルを再帰的に返す。
    pub(super) fn list(&self, raw: Option<&str>, pattern: Option<&str>) -> io::Result<ToolResult> {
        let dir = resolve_dir(&self.root, raw.unwrap_or(""))?;
        match pattern {
            None => {
                // 名前順に並べてからディレクトリの / を付ける。
                let all = sorted_entries(&dir)?;
                let overflow = all.len() > MAX_LIST_ENTRIES;
                let entries: Vec<String> = all
                    .iter()
                    .take(MAX_LIST_ENTRIES)
                    .map(|entry| {
                        let mut name = entry.file_name().to_string_lossy().into_owned();
                        if entry.file_type().is_ok_and(|ty| ty.is_dir()) {
                            name.push('/');
                        }
                        name
                    })
                    .collect();
                if entries.is_empty() {
                    return Ok(ToolResult::ok("(空のディレクトリです)").with_summary("0 件"));
                }
                let mut output = entries.join("\n") + "\n";
                if overflow {
                    output.push_str("[一覧の上限に達したため一部のみ表示しました]\n");
                }
                let summary = if overflow {
                    format!("{} 件（{} 件中・上限で省略）", entries.len(), all.len())
                } else {
                    format!("{} 件", entries.len())
                };
                Ok(ToolResult::ok(output).with_summary(summary))
            }
            Some(pattern) => {
                let matcher = glob(pattern)?;
                let mut visited = 0usize;
                let mut found = Vec::new();
                collect_matching(
                    &self.root,
                    &dir,
                    &dir,
                    &matcher,
                    &mut visited,
                    self.max_search_entries,
                    &mut found,
                );
                found.sort();
                if found.is_empty() {
                    return Ok(
                        ToolResult::ok("(一致するファイルはありません)").with_summary("0 件")
                    );
                }
                let shown = found.len().min(MAX_LIST_ENTRIES);
                let mut output = String::new();
                for path in found.iter().take(MAX_LIST_ENTRIES) {
                    output.push_str(path);
                    output.push('\n');
                }
                let summary = if found.len() > MAX_LIST_ENTRIES {
                    output.push_str("[一覧の上限に達したため一部のみ表示しました]\n");
                    format!("{shown} 件（{} 件中・上限で省略）", found.len())
                } else {
                    format!("{shown} 件")
                };
                Ok(ToolResult::ok(output).with_summary(summary))
            }
        }
    }

    /// query を含む行を `パス:行番号:行` で返す。regex/glob/path で絞り込める。
    pub(super) fn search(
        &self,
        query: &str,
        raw: Option<&str>,
        glob_pattern: Option<&str>,
        regex: Option<bool>,
    ) -> io::Result<ToolResult> {
        if query.is_empty() {
            return Err(invalid("query は空にできません".into()));
        }
        let pattern = if regex.unwrap_or(false) {
            Matcher::Regex(
                regex::Regex::new(query)
                    .map_err(|error| invalid(format!("正規表現が不正です: {error}")))?,
            )
        } else {
            Matcher::Literal(query.to_owned())
        };
        let glob = glob_pattern.map(glob).transpose()?;
        // path はディレクトリなら配下全体、ファイルならその 1 件だけを対象にする。
        let base = match raw {
            Some(raw) if !raw.is_empty() => {
                let path = self.root.join(raw).canonicalize()?;
                if !path.starts_with(&self.root) {
                    return Err(io::Error::other("workspace 外のパスは検索できません"));
                }
                if path.is_file() {
                    let mut output = String::new();
                    let mut truncated = false;
                    let mut matches = 0usize;
                    self.search_file(&path, &pattern, &mut output, &mut truncated, &mut matches);
                    return Ok(finish_search(output, false, matches));
                }
                if !path.is_dir() {
                    return Err(io::Error::other(
                        "workspace 外、またはディレクトリ・通常ファイルではありません",
                    ));
                }
                path
            }
            _ => self.root.clone(),
        };
        let glob_path_mode = glob_pattern.is_some_and(|p| p.contains('/'));
        let mut dirs = vec![base.clone()];
        let mut visited = 0usize;
        let mut output = String::new();
        let mut truncated = false;
        let mut capped = false;
        let mut matches = 0usize;
        // 検索の起点の読み取り失敗だけはエラーとして返し、途中の失敗はスキップする。
        'dirs: while let Some(dir) = dirs.pop() {
            let entries = match sorted_entries(&dir) {
                Ok(entries) => entries,
                Err(error) if dir == base => return Err(error),
                Err(_) => continue,
            };
            for entry in entries {
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
                    if let Some(glob) = &glob {
                        let target = if glob_path_mode {
                            entry.path().strip_prefix(&base).ok().map(Path::to_path_buf)
                        } else {
                            Some(PathBuf::from(entry.file_name()))
                        };
                        if !target.is_some_and(|target| glob.is_match(target)) {
                            continue;
                        }
                    }
                    let Ok(metadata) = entry.metadata() else {
                        continue;
                    };
                    if metadata.len() > MAX_SEARCH_FILE_BYTES {
                        continue;
                    }
                    self.search_file(
                        &entry.path(),
                        &pattern,
                        &mut output,
                        &mut truncated,
                        &mut matches,
                    );
                    if truncated {
                        break 'dirs;
                    }
                }
            }
        }
        Ok(finish_search(output, capped, matches))
    }

    /// 1 ファイルを検索して一致行を追記する。出力上限に達したら省略注記を付けて打ち切る。
    fn search_file(
        &self,
        path: &Path,
        pattern: &Matcher,
        output: &mut String,
        truncated: &mut bool,
        matches: &mut usize,
    ) {
        let Ok(text) = read_text(path, MAX_SEARCH_FILE_BYTES, "検索対象の読み取り上限です")
        else {
            return;
        };
        for (line_no, line) in text.lines().enumerate() {
            if pattern.is_match(line) {
                let Ok(relative) = path.strip_prefix(&self.root) else {
                    continue;
                };
                let row = format!(
                    "{}:{}:{}\n",
                    relative.display(),
                    line_no + 1,
                    preview(line, 500)
                );
                if output.len() + row.len() > self.max_output_bytes {
                    output.push_str("[検索結果を省略]\n");
                    *truncated = true;
                    return;
                }
                output.push_str(&row);
                *matches += 1;
            }
        }
    }
}

enum Matcher {
    Literal(String),
    Regex(regex::Regex),
}

impl Matcher {
    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Literal(query) => line.contains(query.as_str()),
            Self::Regex(regex) => regex.is_match(line),
        }
    }
}

/// 結果の終端処理。一致が無ければ文言を返し、上限注記だけは残す。
fn finish_search(mut output: String, capped: bool, matches: usize) -> ToolResult {
    if capped {
        output.push_str("[検索対象の上限に達したため、一部のファイルのみ検索しました]\n");
    }
    if output.is_empty() {
        return ToolResult::ok("(一致する行はありません)")
            .with_summary(format!("{matches} 件の一致"));
    }
    ToolResult::ok(output).with_summary(format!("{matches} 件の一致"))
}

/// ディレクトリの項目を名前順に並べる。探索順を決定的にするため。
fn sorted_entries(dir: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

/// globset のマッチャー。`*` は `/` を跨がず、`**` は跨ぐ。
fn glob(pattern: &str) -> io::Result<globset::GlobMatcher> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|error| invalid(format!("glob が不正です: {error}")))
}

/// `dir` 以下を再帰的に走査し、`base` からの相対パスが glob に一致する
/// ファイルを workspace 相対で集める。symlink のディレクトリには入らない。
fn collect_matching(
    root: &Path,
    base: &Path,
    dir: &Path,
    matcher: &globset::GlobMatcher,
    visited: &mut usize,
    max_entries: usize,
    found: &mut Vec<String>,
) {
    let Ok(entries) = sorted_entries(dir) else {
        return;
    };
    for entry in entries {
        *visited += 1;
        if *visited > max_entries {
            return;
        }
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_dir() {
            if !is_ignored_dir(&entry.file_name().to_string_lossy()) {
                collect_matching(
                    root,
                    base,
                    &entry.path(),
                    matcher,
                    visited,
                    max_entries,
                    found,
                );
            }
        } else if ty.is_file()
            && entry
                .path()
                .strip_prefix(base)
                .is_ok_and(|relative| matcher.is_match(relative))
            && let Ok(from_root) = entry.path().strip_prefix(root)
        {
            found.push(from_root.display().to_string());
        }
    }
}

/// workspace 内のディレクトリを canonicalize する。path 省略はルート。
pub(crate) fn resolve_dir(root: &Path, raw: &str) -> io::Result<PathBuf> {
    let path = root.join(raw).canonicalize()?;
    if !path.starts_with(root) || !path.is_dir() {
        return Err(io::Error::other(
            "workspace 外、またはディレクトリではありません",
        ));
    }
    Ok(path)
}

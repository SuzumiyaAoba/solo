//! read/edit/write の実装と、書き込み先のパス解決。
use super::{MAX_READ_FILE_BYTES, WorkspaceTools, invalid, resolve_file};
use crate::{harness::ToolResult, storage::read_text};
use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

/// `write` の content の上限。
const MAX_WRITE_BYTES: usize = 1024 * 1024;

impl WorkspaceTools {
    /// offset/limit を両方省略した場合は全文を返す（既存挙動）。
    /// どちらかを指定した場合は行範囲を返し、行数のフッタを末尾に付ける。
    pub(super) fn read(
        &mut self,
        raw: &str,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> io::Result<ToolResult> {
        let path = resolve_file(&self.root, raw)?;
        if offset.is_none() && limit.is_none() {
            let size = fs::metadata(&path)?.len();
            let too_large = || {
                format!(
                    "ファイルが読み取り上限（{} KiB）を超えています（{} バイト）。offset と limit で行範囲を指定してください",
                    self.max_output_bytes / 1024,
                    size
                )
            };
            if size > self.max_output_bytes as u64 {
                return Err(io::Error::other(too_large()));
            }
            let text =
                read_text(&path, self.max_output_bytes as u64, &too_large()).map_err(|error| {
                    // UTF-8 でないファイルだけ専用の文言に変える。
                    if error.kind() == io::ErrorKind::InvalidData
                        && error
                            .get_ref()
                            .is_some_and(|source| source.is::<std::string::FromUtf8Error>())
                    {
                        io::Error::other("UTF-8 のテキストファイルではありません")
                    } else {
                        error
                    }
                })?;
            self.observe(&path, text.as_bytes());
            let total = text.lines().count();
            return Ok(ToolResult::ok(text).with_summary(format!("{total} 行")));
        }
        let offset = offset.unwrap_or(1);
        let limit = limit.unwrap_or(2000);
        if offset < 1 || limit < 1 {
            return Err(invalid(
                "offset と limit は 1 以上の整数で指定してください".into(),
            ));
        }
        let size = fs::metadata(&path)?.len();
        if size > MAX_READ_FILE_BYTES {
            return Err(io::Error::other(format!(
                "ファイルが大きすぎます（{size} バイト）。16 MiB までのファイルを読めます"
            )));
        }
        let bytes = crate::storage::read_bytes(
            &path,
            MAX_READ_FILE_BYTES,
            &format!("ファイルが大きすぎます（{size} バイト）。16 MiB までのファイルを読めます"),
        )?;
        let text = String::from_utf8(bytes)
            .map_err(|_| io::Error::other("UTF-8 のテキストファイルではありません"))?;
        self.observe(&path, text.as_bytes());
        // \n だけで区切る。CRLF の \r は edit の old との一致のため残す。
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.last() == Some(&"") {
            lines.pop();
        }
        let total = lines.len() as u64;
        if offset > total {
            return Err(io::Error::other(format!(
                "offset {offset} がファイルの行数 {total} を超えています"
            )));
        }
        let mut output = String::new();
        let mut end = offset - 1;
        let mut truncated = false;
        for (index, line) in lines
            .iter()
            .skip((offset - 1) as usize)
            .take(limit as usize)
            .enumerate()
        {
            let needed = line.len() + usize::from(!output.is_empty());
            if output.len() + needed > self.max_output_bytes {
                truncated = true;
                if index == 0 {
                    // 先頭行だけで上限を超える場合は、その行を切ってでも返す。
                    let cut = line.floor_char_boundary(self.max_output_bytes);
                    output.push_str(&line[..cut]);
                    end = offset;
                }
                break;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(line);
            end = offset + index as u64;
        }
        if truncated {
            output.push_str(&format!(
                "\n[行 {offset}-{end} / 全 {total} 行 · 出力上限のため {end} 行目までを表示]"
            ));
        } else {
            output.push_str(&format!("\n[行 {offset}-{end} / 全 {total} 行]"));
        }
        Ok(ToolResult::ok(output).with_summary(format!("行 {offset}-{end} / 全 {total} 行")))
    }

    pub(super) fn edit(
        &mut self,
        raw: &str,
        old: &str,
        new: &str,
        replace_all: Option<bool>,
    ) -> io::Result<ToolResult> {
        if old.is_empty() {
            return Err(invalid("old は空にできません".into()));
        }
        let path = resolve_file(&self.root, raw)?;
        let original = fs::read_to_string(&path)?;
        let count = original.matches(old).count();
        if count == 0 {
            return Err(io::Error::other(
                "old に一致する箇所がありません。read で現在の内容を確認してください",
            ));
        }
        let replace_all = replace_all.unwrap_or(false);
        if count > 1 && !replace_all {
            return Err(io::Error::other(format!(
                "old が {count} 箇所に一致します。前後の行を含めて一意にするか、replace_all を true にしてください"
            )));
        }
        let updated = if replace_all {
            original.replace(old, new)
        } else {
            original.replacen(old, new, 1)
        };
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("親ディレクトリがありません"))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(updated.as_bytes())?;
        temp.as_file()
            .set_permissions(fs::metadata(&path)?.permissions())?;
        // 読込み後の外部編集と symlink 差替えを検知する。
        if resolve_file(&self.root, raw)? != path || fs::read(&path)? != original.as_bytes() {
            return Err(io::Error::other("編集中にファイルが変更されました"));
        }
        temp.persist(&path).map_err(|error| error.error)?;
        self.observe(&path, updated.as_bytes());
        let relative = relative(&self.root, &path);
        Ok(
            ToolResult::ok(format!("{relative} を更新しました（{count} 箇所を置換）"))
                .with_summary(format!("{relative} の {count} 箇所を置き換え")),
        )
    }

    pub(super) fn write(&mut self, raw: &str, content: &str) -> io::Result<ToolResult> {
        if content.len() > MAX_WRITE_BYTES {
            return Err(invalid("content は 1 MiB 以下にしてください".into()));
        }
        let path = resolve_writable(&self.root, raw)?;
        let existing = path.exists();
        if existing {
            match self.observation(&path) {
                None => {
                    return Err(io::Error::other(
                        "既存のファイルを上書きするには、先に read で現在の内容を確認してください。一部だけ変更する場合は edit を使ってください",
                    ));
                }
                Some(false) => {
                    return Err(io::Error::other(
                        "read の後にファイルが変更されています。もう一度 read で確認してください",
                    ));
                }
                Some(true) => {}
            }
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("親ディレクトリがありません"))?;
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(content.as_bytes())?;
        if existing {
            temp.as_file()
                .set_permissions(fs::metadata(&path)?.permissions())?;
            // persist 直前にもう一度、観測した内容のままか確認する。
            if self.observation(&path) != Some(true) {
                return Err(io::Error::other(
                    "read の後にファイルが変更されています。もう一度 read で確認してください",
                ));
            }
            temp.persist(&path).map_err(|error| error.error)?;
        } else {
            temp.persist_noclobber(&path).map_err(|error| {
                if error.error.kind() == io::ErrorKind::AlreadyExists {
                    io::Error::other("書き込み中に同名のファイルが作成されました")
                } else {
                    error.error
                }
            })?;
        }
        self.observe(&path, content.as_bytes());
        let lines = content.lines().count();
        let bytes = content.len();
        let relative = relative(&self.root, &path);
        let (result, summary) = if existing {
            (
                format!("{relative} を上書きしました（{lines} 行）"),
                format!("{relative} を上書き（{bytes} バイト）"),
            )
        } else {
            (
                format!("{relative} を作成しました（{lines} 行）"),
                format!("{relative} を作成（{bytes} バイト）"),
            )
        };
        Ok(ToolResult::ok(result).with_summary(summary))
    }
}

/// workspace 相対の表示用パス。失敗し得ない（呼出し側で root 配下を確認済み）。
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 新規作成も含む書き込み先の解決。存在する最も深い祖先を canonicalize して
/// root 配下を確認し、その先は通常名の要素だけを許可する。
fn resolve_writable(root: &Path, raw: &str) -> io::Result<PathBuf> {
    if raw.is_empty() {
        return Err(invalid("path が必要です".into()));
    }
    let candidate = root.join(raw);
    let components: Vec<Component> = candidate.components().collect();
    let mut existing_len = components.len();
    let ancestor = loop {
        let prefix: PathBuf = components[..existing_len].iter().collect();
        if prefix.exists() {
            break prefix.canonicalize()?;
        }
        if existing_len == 0 {
            return Err(io::Error::other("workspace 外には書き込めません"));
        }
        existing_len -= 1;
    };
    if !ancestor.starts_with(root) {
        return Err(io::Error::other("workspace 外には書き込めません"));
    }
    let rest = &components[existing_len..];
    if rest
        .iter()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::other("workspace 外には書き込めません"));
    }
    let target: PathBuf = rest.iter().fold(ancestor, |mut path, component| {
        path.push(component);
        path
    });
    if target.exists() {
        let canonical = target.canonicalize()?;
        if !canonical.starts_with(root) {
            return Err(io::Error::other("workspace 外には書き込めません"));
        }
        if canonical.is_dir() {
            return Err(io::Error::other("ディレクトリには書き込めません"));
        }
        if !canonical.is_file() {
            return Err(io::Error::other(
                "workspace 外、または通常ファイルではありません",
            ));
        }
        return Ok(canonical);
    }
    Ok(target)
}

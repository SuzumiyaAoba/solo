//! exec/edit 前後の workspace 差分を検出し、unified diff に変換する。
use crate::{
    diffgen::unified_diff,
    harness::{
        ToolCall, ToolExecutor, ToolResult, ToolSpec,
        workspace::{TOOL_EDIT, TOOL_EXEC, WorkspaceTools, is_ignored_dir, resolve_file},
    },
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

/// `exec` 前後の workspace 差分検出用スナップショット。
/// 深いディレクトリと大きいファイルは除外し、UI 差分に乗せるファイルだけを保持する。
/// `complete=false` は走査上限で打ち切られたことを表す(新規・削除の判定は信用しない)。
pub(super) struct WorkspaceSnapshot {
    files: BTreeMap<String, FileState>,
    complete: bool,
}

struct FileState {
    content: Option<String>,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

pub(super) fn workspace_snapshot(root: &PathBuf) -> WorkspaceSnapshot {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.clone()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if visited >= 20_000 {
                return WorkspaceSnapshot {
                    files,
                    complete: false,
                };
            }
            visited += 1;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if !is_ignored_dir(&name) {
                    stack.push(path);
                }
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            let content = (meta.len() <= 256 * 1024)
                .then(|| read_workspace_file(root, &relative))
                .flatten();
            files.insert(
                relative,
                FileState {
                    content,
                    len: meta.len(),
                    modified: meta.modified().ok(),
                },
            );
        }
    }
    WorkspaceSnapshot {
        files,
        complete: true,
    }
}

/// スナップショットとの差分(新規・変更・削除)を最大 `limit` 件返す。
/// 走査が打ち切られた側があると新規・削除は誤検出になるため、両方 complete の時だけ報告する。
pub(super) fn snapshot_changes(
    root: &PathBuf,
    before: &WorkspaceSnapshot,
    limit: usize,
) -> Vec<(String, String, String)> {
    let after = workspace_snapshot(root);
    let membership = before.complete && after.complete;
    let mut changes = Vec::new();
    for (path, state) in &after.files {
        match before.files.get(path) {
            Some(prev) => {
                let unchanged_meta = prev.len == state.len && prev.modified == state.modified;
                let unchanged_content = prev.content == state.content;
                if unchanged_meta || unchanged_content {
                    continue;
                }
                // 内容を取得できないファイル(サイズ上限など)は差分に出せない。
                let (Some(old), Some(new)) = (prev.content.clone(), state.content.clone()) else {
                    continue;
                };
                changes.push((path.clone(), old, new));
            }
            None if membership => {
                if let Some(content) = &state.content {
                    changes.push((path.clone(), String::new(), content.clone()));
                }
            }
            None => {}
        }
        if changes.len() >= limit {
            return changes;
        }
    }
    if membership {
        for (path, prev) in &before.files {
            if after.files.contains_key(path) {
                continue;
            }
            if let Some(old) = prev.content.clone() {
                changes.push((path.clone(), old, String::new()));
                if changes.len() >= limit {
                    return changes;
                }
            }
        }
    }
    changes
}

/// workspace 内の通常ファイルを読む。パス解決は `WorkspaceTools::path` と同じ `resolve_file`。
pub(super) fn read_workspace_file(workspace: &Path, relative: &str) -> Option<String> {
    let path = resolve_file(workspace, relative).ok()?;
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return None;
    }
    crate::storage::read_text(&path, 1024 * 1024, "差分の読み取り上限です").ok()
}

/// call_id → (path, unified_diff)。Update::ToolFinished で DiffUpdated へ流す。
type PendingDiffs = Rc<RefCell<HashMap<String, Vec<(String, String)>>>>;

/// `execute` 直前にベースラインを取り、結果から差分を pending に載せる executor。
/// 承認待ち(ToolProposed→policy)の間に入った外部編集を agent の差分に混ぜないため、
/// スナップショットは実行直前に取る。
pub(super) struct DiffTracking {
    inner: WorkspaceTools,
    root: PathBuf,
    pending: PendingDiffs,
}

impl DiffTracking {
    pub(super) fn new(inner: WorkspaceTools, root: PathBuf) -> Self {
        Self {
            inner,
            root,
            pending: Rc::default(),
        }
    }

    /// 完了した呼出しの差分を取り出す共有マップ。
    pub(super) fn pending(&self) -> PendingDiffs {
        self.pending.clone()
    }
}

/// `execute` 直前のベースライン。edit は対象ファイルの本文、exec は workspace 全体。
enum Baseline {
    Edit {
        path: String,
        before: Option<String>,
    },
    Exec(WorkspaceSnapshot),
}

impl ToolExecutor for DiffTracking {
    fn specs(&self) -> &[ToolSpec] {
        self.inner.specs()
    }

    fn execute(&mut self, call: &ToolCall) -> ToolResult {
        let baseline = match call.name.as_str() {
            TOOL_EDIT => call.arguments["path"]
                .as_str()
                .map(|relative| Baseline::Edit {
                    path: relative.to_owned(),
                    before: read_workspace_file(&self.root, relative),
                }),
            TOOL_EXEC => Some(Baseline::Exec(workspace_snapshot(&self.root))),
            _ => None,
        };
        let result = self.inner.execute(call);
        // exec は非ゼロ終了でもファイルを書いていることがあるので、成否に関係なく比較する。
        let diffs: Vec<(String, String)> = match baseline {
            Some(Baseline::Edit { path, before }) if !result.is_error => before
                .and_then(|before| read_workspace_file(&self.root, &path).map(|a| (before, a)))
                .and_then(|(before, after)| unified_diff(&path, &before, &after))
                .map(|diff| vec![(path, diff)])
                .unwrap_or_default(),
            Some(Baseline::Exec(before)) => snapshot_changes(&self.root, &before, 8)
                .into_iter()
                .filter_map(|(path, old, new)| {
                    unified_diff(&path, &old, &new).map(|diff| (path, diff))
                })
                .collect(),
            _ => Vec::new(),
        };
        if !diffs.is_empty() {
            self.pending.borrow_mut().insert(call.id.clone(), diffs);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffgen::unified_diff;

    #[test]
    fn diff_snapshots_only_read_bounded_workspace_files() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("small"), "日本語").unwrap();
        fs::write(root.join("large"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
        fs::write(outside.path().join("file"), "outside").unwrap();
        assert_eq!(
            read_workspace_file(&root, "small").as_deref(),
            Some("日本語")
        );
        assert!(read_workspace_file(&root, "large").is_none());
        assert!(read_workspace_file(&root, ".").is_none());
        assert!(
            read_workspace_file(&root, outside.path().join("file").to_str().unwrap()).is_none()
        );
    }

    #[test]
    fn edit_diff_uses_workspace_path_and_correct_hunk() {
        let diff = unified_diff("src/a.rs", "one\ntwo\n", "one\nthree\n").unwrap();
        assert!(diff.starts_with("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@"));
        assert!(diff.contains("-two\n+three\n"));
    }

    #[test]
    fn exec_snapshot_captures_create_modify_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("keep.txt"), "same").unwrap();
        fs::write(root.join("modify.txt"), "before").unwrap();
        fs::write(root.join("delete.txt"), "gone").unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git").join("internal"), "x").unwrap();

        let before = workspace_snapshot(&root);
        fs::write(root.join("modify.txt"), "after").unwrap();
        fs::write(root.join("new.txt"), "added").unwrap();
        fs::remove_file(root.join("delete.txt")).unwrap();
        fs::write(root.join(".git").join("internal"), "y").unwrap();

        let mut paths: Vec<_> = snapshot_changes(&root, &before, 8).into_iter().collect();
        paths.sort();
        let names: Vec<_> = paths.iter().map(|(p, _, _)| p.as_str()).collect();
        assert_eq!(names, ["delete.txt", "modify.txt", "new.txt"]);
        let modify = paths.iter().find(|(p, _, _)| p == "modify.txt").unwrap();
        assert_eq!((modify.1.as_str(), modify.2.as_str()), ("before", "after"));
        let new_file = paths.iter().find(|(p, _, _)| p == "new.txt").unwrap();
        assert_eq!((new_file.1.as_str(), new_file.2.as_str()), ("", "added"));
        let deleted = paths.iter().find(|(p, _, _)| p == "delete.txt").unwrap();
        assert_eq!((deleted.1.as_str(), deleted.2.as_str()), ("gone", ""));
    }

    #[test]
    fn snapshot_changes_limits_created_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let before = workspace_snapshot(&root);
        for i in 0..20 {
            fs::write(root.join(format!("new-{i:02}.txt")), "created").unwrap();
        }
        assert_eq!(snapshot_changes(&root, &before, 8).len(), 8);
    }

    #[test]
    fn incomplete_snapshots_report_only_modifications() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("stay.txt"), "v1").unwrap();
        fs::write(root.join("gone.txt"), "v1").unwrap();
        let mut before = workspace_snapshot(&root);
        before.complete = false;
        fs::write(root.join("stay.txt"), "v2-longer").unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        fs::write(root.join("new.txt"), "v1").unwrap();
        let changes = snapshot_changes(&root, &before, 8);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "stay.txt");
    }

    #[test]
    fn exec_diffs_excludes_user_edits_before_the_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("user.txt"), "user").unwrap();
        let mut tools = DiffTracking::new(WorkspaceTools::new(&root).unwrap(), root.clone());
        let pending = tools.pending();
        // 承認待ちの間に入った編集:ベースラインは実行直前に取るので差分に出ない。
        fs::write(root.join("user.txt"), "edited by user").unwrap();
        let result = tools.execute(&ToolCall {
            id: "exec-1".into(),
            name: TOOL_EXEC.into(),
            arguments: serde_json::json!({"command":"printf new > agent.txt; exit 3"}),
        });
        // 非ゼロ終了でも書いたファイルは差分に出る。
        assert!(result.is_error);
        let diffs = pending.borrow_mut().remove("exec-1").unwrap();
        let names: Vec<_> = diffs.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(names, ["agent.txt"]);
    }

    #[test]
    fn edit_diffs_follow_the_result_and_failed_edits_leave_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("file.txt"), "before").unwrap();
        let mut tools = DiffTracking::new(WorkspaceTools::new(&root).unwrap(), root);
        let pending = tools.pending();
        let result = tools.execute(&ToolCall {
            id: "edit-1".into(),
            name: TOOL_EDIT.into(),
            arguments: serde_json::json!({"path":"file.txt","old":"before","new":"after"}),
        });
        assert!(!result.is_error);
        let diffs = pending.borrow_mut().remove("edit-1").unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].0, "file.txt");
        assert!(diffs[0].1.contains("-before"));
        assert!(diffs[0].1.contains("+after"));

        let result = tools.execute(&ToolCall {
            id: "edit-2".into(),
            name: TOOL_EDIT.into(),
            arguments: serde_json::json!({"path":"file.txt","old":"missing","new":"x"}),
        });
        assert!(result.is_error);
        assert!(pending.borrow_mut().remove("edit-2").is_none());
    }
}

//! 前後テキストから unified diff を生成する共通部品。worker(backend)ごとの
//! 品質差をなくすため、DiffUpdated に載る差分はすべてここを通す。
use std::{fs, process::Command};

/// before/after のテキスト差分を unified diff 形式で返す。差がない・生成に
/// 失敗した・上限(512KiB)を超えた場合は None。
pub fn unified_diff(relative: &str, before: &str, after: &str) -> Option<String> {
    if before == after {
        return None;
    }
    let dir = tempfile::tempdir().ok()?;
    let old = dir.path().join("before");
    let new = dir.path().join("after");
    fs::write(&old, before).ok()?;
    fs::write(&new, after).ok()?;
    let output = Command::new("git")
        .args(["diff", "--no-index", "--no-color", "--unified=3", "--"])
        .arg(&old)
        .arg(&new)
        .output()
        .ok()?;
    if output.status.code() != Some(1) {
        return None;
    }
    let diff = String::from_utf8(output.stdout).ok()?;
    let hunk = diff.find("@@ ")?;
    let diff = format!("--- a/{relative}\n+++ b/{relative}\n{}", &diff[hunk..]);
    (diff.len() <= 512 * 1024).then_some(diff)
}

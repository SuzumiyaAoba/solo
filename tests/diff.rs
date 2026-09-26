use solo::projection::{Diff, DiffKind};

#[test]
fn hunk_content_that_looks_like_file_headers_keeps_its_line_numbers() {
    let diff = Diff::parse(
        "counter.rs".into(),
        "--- a/counter.rs\n+++ b/counter.rs\n@@ -8,2 +12,2 @@\n---old\n+++new\n context\n",
    );
    assert_eq!(diff.line_counts(), (1, 1));
    assert_eq!(diff.lines[3].kind, DiffKind::Removed);
    assert_eq!((diff.lines[3].old, diff.lines[3].new), (Some(8), None));
    assert_eq!(diff.lines[4].kind, DiffKind::Added);
    assert_eq!((diff.lines[4].old, diff.lines[4].new), (None, Some(12)));
    assert_eq!((diff.lines[5].old, diff.lines[5].new), (Some(9), Some(13)));
}

#[test]
fn file_headers_after_a_finished_hunk_do_not_count_as_changes() {
    let diff = Diff::parse(
        "changes".into(),
        "--- a/one\n+++ b/one\n@@ -0,0 +1 @@\n+++ one\n\
         --- a/two\n+++ b/two\n@@ -2 +0,0 @@\n--- two\n",
    );
    assert_eq!(diff.line_counts(), (1, 1));
    assert_eq!(diff.lines[3].kind, DiffKind::Added);
    assert_eq!(diff.lines[4].kind, DiffKind::Header);
    assert_eq!(diff.lines[5].kind, DiffKind::Header);
    assert_eq!(diff.lines[7].kind, DiffKind::Removed);
    assert_eq!(diff.lines[7].old, Some(2));
}

#[test]
fn line_number_overflow_is_visible_instead_of_panicking_or_wrapping() {
    let diff = Diff::parse(
        "large".into(),
        "@@ -4294967295,2 +4294967295,2 @@\n first\n second\n",
    );
    assert_eq!(diff.lines[1].old, Some(u32::MAX));
    assert_eq!(diff.lines[1].new, Some(u32::MAX));
    assert_eq!(diff.lines[2].old, None);
    assert_eq!(diff.lines[2].new, None);
    assert!(
        diff.truncated,
        "unrepresentable line numbers must prevent review confirmation"
    );
}

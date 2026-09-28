mod common;

use solo::{
    event::{Event, SessionId, Usage},
    projection::{Apply, Diff, DiffKind, SessionProjection},
};

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

#[test]
fn context_lines_between_removed_and_added_do_not_pair() {
    let diff = Diff::parse("f".into(), "@@ -1,3 +1,3 @@\n-foo\n ctx\n+bar\n");
    assert!(
        diff.lines.iter().all(|line| line.changed.is_none()),
        "context-separated lines must not be highlighted as a modification"
    );
}

#[test]
fn multi_line_replacements_pair_in_order() {
    let diff = Diff::parse(
        "f".into(),
        "@@ -1,2 +1,2 @@\n-alpha one\n-beta two\n+alpha 1\n+beta 2\n",
    );
    assert_eq!(diff.lines[1].changed, Some(6..9));
    assert_eq!(diff.lines[2].changed, Some(5..8));
    assert_eq!(diff.lines[3].changed, Some(6..7));
    assert_eq!(diff.lines[4].changed, Some(5..6));
}

#[test]
fn no_newline_markers_do_not_break_pairing() {
    let diff = Diff::parse(
        "f".into(),
        "@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
    );
    assert_eq!(diff.lines[1].changed, Some(0..3));
    assert_eq!(diff.lines[3].changed, Some(0..3));
}

#[test]
fn diff_origin_tracks_the_reporting_turn_and_invocation() {
    let mut session = SessionProjection::new(SessionId::parse("s1").unwrap(), "test".into());
    assert!(Diff::parse("a".into(), "+x").origin.is_none());
    assert_eq!(
        session.apply(common::envelope(
            "s1",
            "first",
            1,
            Event::TurnStarted { prompt: "p".into() }
        )),
        Apply::Applied
    );
    session.apply(common::envelope(
        "s1",
        "first",
        2,
        Event::DiffUpdated {
            path: "src/a.rs".into(),
            unified_diff: "@@ -0,0 +1 @@\n+x".into(),
            invocation_id: Some("write-1".into()),
        },
    ));
    let origin = session.diffs()[0].origin.as_ref().unwrap().clone();
    assert_eq!(&*origin.turn_id, "first");
    assert_eq!(origin.invocation_id.as_deref(), Some("write-1"));
    // 次の turn で同じファイルが上書きされたら、origin も新しい実行に付け替わる。
    session.apply(common::envelope(
        "s1",
        "first",
        3,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    ));
    session.apply(common::envelope(
        "s1",
        "second",
        4,
        Event::TurnStarted {
            prompt: "again".into(),
        },
    ));
    session.apply(common::envelope(
        "s1",
        "second",
        5,
        Event::DiffUpdated {
            path: "src/a.rs".into(),
            unified_diff: "@@ -0,0 +1 @@\n+y".into(),
            invocation_id: None,
        },
    ));
    assert_eq!(session.diffs().len(), 1);
    let origin = session.diffs()[0].origin.as_ref().unwrap();
    assert_eq!(&*origin.turn_id, "second");
    assert_eq!(origin.invocation_id, None);
}

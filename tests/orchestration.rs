mod common;

use solo::{
    backend::BackendKind,
    event::{Envelope, Event, SessionId, Usage},
    orchestration::{QueuedRun, RunQueue},
    projection::{Apply, MAX_TOOL_ACTIVITIES, SessionProjection},
    text::task_title,
};

fn sid(s: &str) -> SessionId {
    SessionId::parse(s).unwrap()
}

fn run(id: &str, backend: BackendKind) -> QueuedRun {
    QueuedRun {
        session_id: sid(id),
        backend,
        prompt: format!("{id} の依頼🙂"),
    }
}

#[test]
fn queue_serializes_work_and_keeps_the_original_backend_and_prompt() {
    let mut queue = RunQueue::default();
    let first = run(
        "first",
        BackendKind::Mock {
            key: "threads".into(),
        },
    );
    let second = run("second", BackendKind::Subscription);
    assert!(queue.push(first.clone()));
    assert!(queue.push(second.clone()));
    assert!(
        !queue.push(run(
            "first",
            BackendKind::Mock {
                key: "events-100k".into(),
            }
        )),
        "a duplicate must not replace the original request"
    );
    assert_eq!(queue.position(&sid("second")), Some(2));
    assert_eq!(
        queue.next(true),
        None,
        "another agent still owns the workspace"
    );
    queue.set_paused(true);
    assert_eq!(queue.next(false), None, "errors require an explicit resume");
    queue.set_paused(false);
    assert_eq!(queue.next(false), Some(first));
    assert_eq!(queue.position(&sid("second")), Some(1));
    assert_eq!(queue.next(false), Some(second));
    assert!(queue.is_empty());
}

#[test]
fn cancellation_restores_exact_draft_without_reordering_remaining_work() {
    let mut queue = RunQueue::default();
    for id in ["a", "b", "c"] {
        queue.push(run(id, BackendKind::Acp { id: "agent".into() }));
    }
    assert_eq!(
        queue.cancel(&sid("b")),
        Some(run("b", BackendKind::Acp { id: "agent".into() }))
    );
    assert_eq!(queue.position(&sid("c")), Some(2));
    assert_eq!(queue.cancel(&sid("missing")), None);
    assert_eq!(
        queue.next(false),
        Some(run("a", BackendKind::Acp { id: "agent".into() }))
    );
    assert_eq!(
        queue.next(false),
        Some(run("c", BackendKind::Acp { id: "agent".into() }))
    );
    assert!(!queue.push(QueuedRun {
        prompt: " \n ".into(),
        ..run("empty", BackendKind::Acp { id: "agent".into() })
    }));
}

#[test]
fn restore_applies_the_same_invariants_as_push() {
    let mut queue = RunQueue::default();
    let dropped = queue.restore(
        vec![
            run("a", BackendKind::Subscription),
            run("a", BackendKind::Subscription),
            QueuedRun {
                prompt: "  \n ".into(),
                ..run("b", BackendKind::Subscription)
            },
            run("c", BackendKind::Subscription),
        ],
        true,
    );
    assert_eq!(dropped, 2, "重複・空 prompt の保存データは落とす");
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.position(&sid("a")), Some(1));
    assert_eq!(queue.position(&sid("b")), None);
    assert_eq!(queue.position(&sid("c")), Some(2));
    assert_eq!(
        queue.next(false),
        None,
        "paused な復元は明示再開まで動かない"
    );
    queue.set_paused(false);
    assert_eq!(queue.next(false).unwrap().session_id, sid("a"));
}

#[test]
fn titles_use_the_first_nonempty_line_and_preserve_graphemes() {
    assert_eq!(task_title("\n  日本語の改善  \n完了条件"), "日本語の改善");
    let family = "👩‍👩‍👧‍👦";
    assert_eq!(
        task_title(&family.repeat(37)),
        format!("{}…", family.repeat(36))
    );
    assert_eq!(task_title("\n\t"), "新しいタスク");
}

fn event(n: u64, payload: Event) -> Envelope {
    common::envelope("s", "turn", n, payload)
}

fn started() -> SessionProjection {
    let mut session = SessionProjection::new(sid("s"), "task".into());
    session.apply(event(
        1,
        Event::TurnStarted {
            prompt: "inspect".into(),
        },
    ));
    session
}

#[test]
fn review_cannot_confirm_inflight_or_truncated_changes_and_new_diff_invalidates_it() {
    let mut session = started();
    let change = event(
        2,
        Event::DiffUpdated {
            path: "src/app.rs".into(),
            unified_diff: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new".into(),
            invocation_id: None,
        },
    );
    session.apply(change.clone());
    assert_eq!(session.diffs()[0].line_counts(), (1, 1));
    assert!(!session.toggle_reviewed(0));
    session.apply(event(
        3,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    ));
    assert!(session.toggle_reviewed(0));
    assert_eq!(session.unreviewed_count(), 0);
    assert_eq!(session.apply(change), Apply::Duplicate);
    assert_eq!(
        session.unreviewed_count(),
        0,
        "a duplicate event must not undo a review"
    );
    session.apply(event(
        4,
        Event::TurnStarted {
            prompt: "continue".into(),
        },
    ));
    session.apply(event(
        5,
        Event::DiffUpdated {
            path: "src/app.rs".into(),
            unified_diff: "+updated".into(),
            invocation_id: None,
        },
    ));
    assert_eq!(session.unreviewed_count(), 1);
    session.apply(event(
        6,
        Event::DiffUpdated {
            path: "long".into(),
            unified_diff: format!("+{}", "x".repeat(3000)),
            invocation_id: None,
        },
    ));
    session.apply(event(
        7,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    ));
    assert!(session.diffs()[1].truncated);
    assert!(!session.toggle_reviewed(1));
    assert!(!session.toggle_reviewed(99));
}

#[test]
fn activity_tracks_tool_outcomes_and_resets_for_the_next_turn() {
    let mut session = started();
    session.apply(event(
        2,
        Event::ToolStarted {
            agent_id: None,
            tool: None,
            invocation_id: "build".into(),
            command: "cargo test".into(),
            cwd: "/workspace".into(),
        },
    ));
    session.apply(event(
        3,
        Event::ToolFinished {
            invocation_id: "build".into(),
            exit_code: 1,
            summary: None,
        },
    ));
    assert_eq!(session.tool_activity()[0].command, "cargo test");
    assert_eq!(session.tool_activity()[0].cwd, "/workspace");
    assert_eq!(session.tool_activity()[0].exit_code, Some(1));
    session.apply(event(
        4,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    ));
    session.apply(event(
        5,
        Event::TurnStarted {
            prompt: "next".into(),
        },
    ));
    assert!(session.tool_activity().is_empty());
    assert!(session.tools().is_empty());
}

#[test]
fn activity_is_bounded_without_losing_tool_completion_validation() {
    let mut session = started();
    for n in 0..=MAX_TOOL_ACTIVITIES {
        session.apply(event(
            2 + n as u64,
            Event::ToolStarted {
                agent_id: None,
                tool: None,
                invocation_id: n.to_string(),
                command: "read".into(),
                cwd: ".".into(),
            },
        ));
    }
    assert_eq!(session.tool_activity().len(), MAX_TOOL_ACTIVITIES);
    assert_eq!(session.tool_activity()[0].invocation_id, "1");
    assert_eq!(
        session.apply(event(
            3 + MAX_TOOL_ACTIVITIES as u64,
            Event::ToolFinished {
                invocation_id: "0".into(),
                exit_code: 0,
                summary: None,
            }
        )),
        Apply::Applied
    );
    assert_eq!(session.tools()["0"], Some(0));
}

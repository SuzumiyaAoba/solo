mod common;

use solo::{
    event::{Event, SessionId, Usage},
    projection::{
        ActivityKind, ActivityState, Apply, MAX_EXECUTION_THREADS, MAX_THREAD_ACTIVITIES,
        SessionProjection, Status,
    },
};

fn sid(s: &str) -> SessionId {
    SessionId::parse(s).unwrap()
}

fn apply(session: &mut SessionProjection, turn: &str, event: Event) -> Apply {
    let sequence = session.last_sequence() + 1;
    session.apply(common::envelope(&session.id, turn, sequence, event))
}
fn start(session: &mut SessionProjection, turn: &str) {
    assert_eq!(
        apply(
            session,
            turn,
            Event::TurnStarted {
                prompt: format!("依頼 {turn} 🙂")
            }
        ),
        Apply::Applied
    );
}
fn complete(session: &mut SessionProjection, turn: &str) {
    apply(
        session,
        turn,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    );
}
fn agent(id: &str, parent: Option<&str>) -> Event {
    Event::AgentStarted {
        agent_id: id.into(),
        name: format!("担当 {id}"),
        task: "調査".into(),
        parent_agent_id: parent.map(Into::into),
    }
}
fn tool(id: &str, owner: Option<&str>) -> Event {
    Event::ToolStarted {
        invocation_id: id.into(),
        command: "read src/ui/views.rs".into(),
        cwd: "/workspace".into(),
        agent_id: owner.map(Into::into),
    }
}

#[test]
fn previous_request_keeps_its_activities_when_identifiers_are_reused() {
    let mut session = SessionProjection::new(sid("s1"), "channel".into());
    for turn in ["first", "second"] {
        start(&mut session, turn);
        assert_eq!(
            apply(&mut session, turn, agent("research", None)),
            Apply::Applied
        );
        assert_eq!(
            apply(&mut session, turn, tool("read", Some("research"))),
            Apply::Applied
        );
        apply(
            &mut session,
            turn,
            Event::ToolFinished {
                invocation_id: "read".into(),
                exit_code: 0,
            },
        );
        apply(
            &mut session,
            turn,
            Event::AgentFinished {
                agent_id: "research".into(),
                success: true,
                summary: format!("{turn} の結果"),
            },
        );
        complete(&mut session, turn);
    }
    assert_eq!(session.threads().len(), 2);
    assert_ne!(session.threads()[0].id, session.threads()[1].id);
    for (i, turn) in ["first", "second"].iter().enumerate() {
        let thread = &session.threads()[i];
        assert_eq!(&*thread.turn_id, *turn);
        assert_eq!(thread.status, Status::Completed);
        assert_eq!(thread.activities[0].result, format!("{turn} の結果"));
        assert_eq!(
            thread.activities[1].parent_agent_id.as_deref(),
            Some("research")
        );
        assert!(
            thread
                .activities
                .iter()
                .all(|a| a.state == ActivityState::Succeeded)
        );
        assert!(
            session
                .chat()
                .iter()
                .any(|block| block.thread_id == Some(thread.id) && block.text.contains(turn))
        );
    }
}

#[test]
fn old_tool_payloads_remain_compatible() {
    let old: Event = serde_json::from_value(serde_json::json!({ "type":"tool_started", "invocation_id":"read", "command":"read", "cwd":"." })).unwrap();
    assert!(matches!(old, Event::ToolStarted { agent_id: None, .. }));
}

#[test]
fn agent_and_tool_lifecycles_are_validated_before_recording() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    start(&mut session, "turn");
    assert_eq!(
        apply(&mut session, "turn", tool("orphan", Some("unknown"))),
        Apply::Rejected
    );
    assert_eq!(
        apply(&mut session, "turn", agent("orphan", Some("unknown"))),
        Apply::Rejected
    );
    assert_eq!(
        apply(&mut session, "turn", agent("root", None)),
        Apply::Applied
    );
    assert_eq!(
        apply(&mut session, "turn", agent("root", None)),
        Apply::Rejected
    );
    assert_eq!(
        apply(&mut session, "turn", agent("child", Some("root"))),
        Apply::Applied
    );
    assert_eq!(
        apply(
            &mut session,
            "old-turn",
            Event::AgentFinished {
                agent_id: "child".into(),
                success: true,
                summary: "late".into()
            }
        ),
        Apply::Rejected
    );
    assert_eq!(session.threads()[0].activities.len(), 2);
    assert_eq!(
        session.threads()[0].activities[1].state,
        ActivityState::Running
    );
    assert_eq!(
        apply(
            &mut session,
            "turn",
            Event::AgentFinished {
                agent_id: "child".into(),
                success: false,
                summary: "failed".into()
            }
        ),
        Apply::Applied
    );
    assert_eq!(
        apply(
            &mut session,
            "turn",
            Event::AgentFinished {
                agent_id: "child".into(),
                success: true,
                summary: "duplicate".into()
            }
        ),
        Apply::Rejected
    );
    assert_eq!(
        session.threads()[0].activities[1].state,
        ActivityState::Failed
    );
}

#[test]
fn unfinished_agents_prevent_success_and_terminal_states_clear_running_rows() {
    for outcome in [
        Status::Completed,
        Status::Cancelled,
        Status::Failed,
        Status::Disconnected,
    ] {
        let mut session = SessionProjection::new(sid("s"), "".into());
        start(&mut session, "turn");
        apply(&mut session, "turn", agent("running", None));
        apply(&mut session, "turn", tool("done", None));
        apply(
            &mut session,
            "turn",
            Event::ToolFinished {
                invocation_id: "done".into(),
                exit_code: 1,
            },
        );
        match outcome {
            Status::Completed => complete(&mut session, "turn"),
            Status::Cancelled => {
                apply(
                    &mut session,
                    "turn",
                    Event::TurnCancelled {
                        reason: "cancelled".into(),
                    },
                );
            }
            Status::Failed => session.transport_failed("worker failed".into()),
            _ => session.transport_closed(),
        }
        let thread = &session.threads()[0];
        assert_eq!(
            thread.status,
            if outcome == Status::Completed {
                Status::Disconnected
            } else {
                outcome
            }
        );
        assert_eq!(
            thread.activities[0].state,
            if outcome == Status::Cancelled {
                ActivityState::Cancelled
            } else {
                ActivityState::Interrupted
            }
        );
        assert_eq!(thread.activities[1].state, ActivityState::Failed);
        assert_eq!(thread.activities[1].result, "終了コード 1");
    }
}

#[test]
fn connecting_failure_does_not_rewrite_a_completed_thread() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    start(&mut session, "turn");
    complete(&mut session, "turn");
    session.transport_failed("next connection failed".into());
    assert_eq!(session.threads()[0].status, Status::Completed);
    assert_eq!(session.threads()[0].reason, "done");
}

#[test]
fn bounded_history_keeps_identity_and_validates_evicted_activities() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    for n in 0..=MAX_EXECUTION_THREADS {
        start(&mut session, &format!("turn-{n}"));
        complete(&mut session, &format!("turn-{n}"));
    }
    assert_eq!(session.threads().len(), MAX_EXECUTION_THREADS);
    assert_eq!(session.threads_discarded(), 1);
    assert_eq!(&*session.threads()[0].turn_id, "turn-1");
    start(&mut session, "tools");
    for n in 0..=MAX_THREAD_ACTIVITIES {
        apply(&mut session, "tools", tool(&n.to_string(), None));
    }
    for n in 0..=MAX_THREAD_ACTIVITIES {
        assert_eq!(
            apply(
                &mut session,
                "tools",
                Event::ToolFinished {
                    invocation_id: n.to_string(),
                    exit_code: 0
                }
            ),
            Apply::Applied
        );
    }
    complete(&mut session, "tools");
    let thread = session.threads().back().unwrap();
    assert_eq!(thread.activities.len(), MAX_THREAD_ACTIVITIES);
    assert_eq!(thread.discarded, 1);
    assert_eq!(thread.activity_count(), MAX_THREAD_ACTIVITIES + 1);
    assert_eq!(thread.activities[0].id, "1");
    assert!(
        thread
            .activities
            .iter()
            .all(|a| a.kind == ActivityKind::Tool && a.state == ActivityState::Succeeded)
    );
    assert_eq!(thread.status, Status::Completed);
}

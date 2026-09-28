mod common;

use solo::{
    event::{Event, SessionId, Usage},
    projection::{
        ActivityApproval, ActivityKind, ActivityState, Apply, MAX_EXECUTION_THREADS,
        MAX_THREAD_ACTIVITIES, SessionProjection, Status,
    },
};

fn sid(s: &str) -> SessionId {
    SessionId::parse(s).unwrap()
}

fn apply(session: &mut SessionProjection, turn: &str, event: Event) -> Apply {
    let sequence = session.last_sequence() + 1;
    session.apply(common::envelope(&session.id, turn, sequence, event))
}

/// envelope の timestamp_ms を指定して適用する。所要時間の検証用。
fn apply_at(session: &mut SessionProjection, turn: &str, timestamp_ms: u64, event: Event) -> Apply {
    let sequence = session.last_sequence() + 1;
    let mut envelope = common::envelope(&session.id, turn, sequence, event);
    envelope.timestamp_ms = timestamp_ms;
    session.apply(envelope)
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
        tool: Some("read".into()),
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
                summary: Some("210 行".into()),
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

/// 新しい任意フィールドは schema_version を上げずに追加したので、
/// 永続化された旧形式の JSON は None として読める必要がある。
#[test]
fn old_event_payloads_decode_with_empty_optional_fields() {
    let started: Event = serde_json::from_value(serde_json::json!({
        "type": "tool_started", "invocation_id": "t", "command": "c", "cwd": "."
    }))
    .unwrap();
    assert!(matches!(
        started,
        Event::ToolStarted {
            tool: None,
            agent_id: None,
            ..
        }
    ));
    let finished: Event = serde_json::from_value(serde_json::json!({
        "type": "tool_finished", "invocation_id": "t", "exit_code": 0
    }))
    .unwrap();
    assert!(matches!(
        finished,
        Event::ToolFinished { summary: None, .. }
    ));
    let diff: Event = serde_json::from_value(serde_json::json!({
        "type": "diff_updated", "path": "a.rs", "unified_diff": "@@"
    }))
    .unwrap();
    assert!(matches!(
        diff,
        Event::DiffUpdated {
            invocation_id: None,
            ..
        }
    ));
    let requested: Event = serde_json::from_value(serde_json::json!({
        "type": "approval_requested", "request_id": "r", "title": "t",
        "executor": "shell", "command": null, "details": {}
    }))
    .unwrap();
    assert!(matches!(
        requested,
        Event::ApprovalRequested {
            invocation_id: None,
            ..
        }
    ));
    let decided: Event = serde_json::from_value(serde_json::json!({
        "type": "approval_decided", "request_id": "r", "accepted": true, "source": "user"
    }))
    .unwrap();
    assert!(matches!(
        decided,
        Event::ApprovalDecided {
            invocation_id: None,
            ..
        }
    ));
}

/// 要約・所要時間・承認・変更ファイルを実行単位とスレッド全体の両方で辿れる。
#[test]
fn activities_carry_tool_summary_timing_approval_and_changed_files() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    apply_at(
        &mut session,
        "turn",
        1_000,
        Event::TurnStarted {
            prompt: "変更".into(),
        },
    );
    apply_at(
        &mut session,
        "turn",
        1_100,
        Event::ToolStarted {
            invocation_id: "write-1".into(),
            command: "write src/a.rs".into(),
            cwd: "/w".into(),
            agent_id: None,
            tool: Some("write".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        1_200,
        Event::ApprovalRequested {
            request_id: "r-1".into(),
            title: "write src/a.rs".into(),
            executor: "shell".into(),
            command: None,
            details: serde_json::json!({}),
            invocation_id: Some("write-1".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        1_500,
        Event::ApprovalDecided {
            request_id: "r-1".into(),
            accepted: true,
            source: "user".into(),
            invocation_id: Some("write-1".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        1_800,
        Event::DiffUpdated {
            path: "src/a.rs".into(),
            unified_diff: "@@ -0,0 +1 @@\n+fn a() {}".into(),
            invocation_id: Some("write-1".into()),
        },
    );
    // 同一パスの再報告は重複追加せず、別ファイルは別パスとして追記する。
    apply_at(
        &mut session,
        "turn",
        1_900,
        Event::DiffUpdated {
            path: "src/a.rs".into(),
            unified_diff: "@@ -0,0 +2 @@\n+fn a() {}\n+fn a2() {}".into(),
            invocation_id: Some("write-1".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        2_000,
        Event::DiffUpdated {
            path: "src/b.rs".into(),
            unified_diff: "@@".into(),
            invocation_id: Some("write-1".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        3_600,
        Event::ToolFinished {
            invocation_id: "write-1".into(),
            exit_code: 0,
            summary: Some("src/a.rs を作成（12 バイト）".into()),
        },
    );
    apply_at(
        &mut session,
        "turn",
        5_000,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    );

    let thread = &session.threads()[0];
    assert_eq!(thread.started_ms, Some(1_000));
    assert_eq!(thread.finished_ms, Some(5_000));
    assert_eq!(thread.duration_ms(), Some(4_000));
    assert_eq!(thread.changed_files(), ["src/a.rs", "src/b.rs"]);
    let activity = &thread.activities[0];
    assert_eq!(activity.tool.as_deref(), Some("write"));
    assert_eq!(activity.started_ms, Some(1_100));
    assert_eq!(activity.finished_ms, Some(3_600));
    assert_eq!(activity.duration_ms(), Some(2_500));
    assert_eq!(
        activity.approval,
        Some(ActivityApproval::Allowed("user".into()))
    );
    assert_eq!(activity.changed_paths, ["src/a.rs", "src/b.rs"]);
    assert_eq!(activity.result, "src/a.rs を作成（12 バイト）");
    let origin = session.diffs()[0].origin.clone().unwrap();
    assert_eq!(&*origin.turn_id, "turn");
    assert_eq!(origin.invocation_id.as_deref(), Some("write-1"));
}

/// 拒否の承認を記録し、ID が無い・不明な承認や差分はどの実行にも触れない。
#[test]
fn denied_approval_and_unmatched_links_leave_no_marks() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    start(&mut session, "turn");
    apply(&mut session, "turn", tool("rm", None));
    apply(
        &mut session,
        "turn",
        Event::ApprovalRequested {
            request_id: "r".into(),
            title: "rm".into(),
            executor: "shell".into(),
            command: None,
            details: serde_json::json!({}),
            invocation_id: Some("rm".into()),
        },
    );
    assert_eq!(
        session.threads()[0].activities[0].approval,
        Some(ActivityApproval::Pending)
    );
    apply(
        &mut session,
        "turn",
        Event::ApprovalDecided {
            request_id: "r".into(),
            accepted: false,
            source: "user".into(),
            invocation_id: Some("rm".into()),
        },
    );
    assert_eq!(
        session.threads()[0].activities[0].approval,
        Some(ActivityApproval::Denied("user".into()))
    );
    let revision = session.thread_revision();
    apply(
        &mut session,
        "turn",
        Event::ApprovalDecided {
            request_id: "r2".into(),
            accepted: true,
            source: "rule".into(),
            invocation_id: None,
        },
    );
    apply(
        &mut session,
        "turn",
        Event::ApprovalRequested {
            request_id: "r3".into(),
            title: "x".into(),
            executor: "shell".into(),
            command: None,
            details: serde_json::json!({}),
            invocation_id: Some("unknown".into()),
        },
    );
    apply(
        &mut session,
        "turn",
        Event::DiffUpdated {
            path: "orphan.rs".into(),
            unified_diff: "+x".into(),
            invocation_id: None,
        },
    );
    let activity = &session.threads()[0].activities[0];
    assert_eq!(
        activity.approval,
        Some(ActivityApproval::Denied("user".into()))
    );
    assert!(activity.changed_paths.is_empty());
    assert_eq!(session.thread_revision(), revision);
}

/// 上限で捨てられた実行への後続報告は panic せず捨て、残った実行には届く。
#[test]
fn evicted_activity_links_are_dropped_safely() {
    let mut session = SessionProjection::new(sid("s"), "".into());
    start(&mut session, "turn");
    for n in 0..=MAX_THREAD_ACTIVITIES {
        apply(&mut session, "turn", tool(&n.to_string(), None));
    }
    // "0" は先頭から捨てられた。DiffUpdated と承認決定が来てもクラッシュしない。
    apply(
        &mut session,
        "turn",
        Event::DiffUpdated {
            path: "gone.rs".into(),
            unified_diff: "+x".into(),
            invocation_id: Some("0".into()),
        },
    );
    apply(
        &mut session,
        "turn",
        Event::ApprovalDecided {
            request_id: "r".into(),
            accepted: true,
            source: "user".into(),
            invocation_id: Some("0".into()),
        },
    );
    let thread = session.threads().back().unwrap();
    assert!(thread.changed_files().is_empty());
    assert!(thread.activities.iter().all(|a| a.approval.is_none()));
    apply(
        &mut session,
        "turn",
        Event::DiffUpdated {
            path: "kept.rs".into(),
            unified_diff: "+y".into(),
            invocation_id: Some("1".into()),
        },
    );
    assert_eq!(
        session.threads().back().unwrap().changed_files(),
        ["kept.rs"]
    );
    complete(&mut session, "turn");
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
                summary: None,
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
                    exit_code: 0,
                    summary: None,
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

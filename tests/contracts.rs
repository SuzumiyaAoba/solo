use serde_json::json;
use solo::{
    event::{Decoded, Envelope, Event, SCHEMA_VERSION, Usage},
    projection::{
        Apply, CHAT_BLOCK_BYTES, Diff, DiffKind, MAX_CHAT_BLOCKS, MAX_LOG_ROWS, Session, Status,
    },
};

fn event(sequence: u64, payload: Event) -> Envelope {
    Envelope {
        schema_version: SCHEMA_VERSION,
        event_id: format!("event-{sequence}"),
        session_id: "s1".into(),
        sequence,
        timestamp_ms: 0,
        turn_id: Some("turn-1".into()),
        payload: serde_json::to_value(payload).unwrap(),
    }
}
fn session() -> Session {
    let mut session = Session::new("s1".into(), "test".into());
    session.apply(event(
        1,
        Event::TurnStarted {
            prompt: "表示を確認".into(),
        },
    ));
    session
}
fn delta(sequence: u64, text: &str) -> Envelope {
    event(
        sequence,
        Event::MessageDelta {
            message_id: "m1".into(),
            text: text.into(),
        },
    )
}
fn complete(sequence: u64) -> Envelope {
    event(
        sequence,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    )
}

#[test]
fn envelope_roundtrip_preserves_related_ids() {
    let original = delta(2, "日本語🙂");
    let restored: Envelope =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    assert_eq!(restored.event_id, original.event_id);
    assert_eq!(restored.turn_id, original.turn_id);
    assert_eq!(restored.payload, original.payload);
}

#[test]
fn unknown_kind_preserves_payload_and_is_visible() {
    let mut envelope = delta(2, "unused");
    envelope.payload = json!({"type":"future_event", "nested":{"key":42}});
    match envelope.decode().unwrap() {
        Decoded::Unknown { payload, .. } => assert_eq!(payload, envelope.payload),
        _ => panic!("unknown event was interpreted"),
    }
    let mut session = session();
    session.apply(envelope);
    assert_eq!(session.unknown, 1);
    assert!(session.logs.back().unwrap().text.contains("future_event"));
    assert_eq!(session.status, Status::Running);
}

#[test]
fn newer_schema_cannot_complete_a_turn() {
    let mut session = session();
    let mut terminal = complete(2);
    terminal.schema_version = 2;
    session.apply(terminal);
    assert_eq!(session.unknown, 1);
    session.transport_closed();
    assert_eq!(session.status, Status::Disconnected);
}

#[test]
fn malformed_known_event_is_rejected() {
    let mut session = session();
    let mut malformed = delta(2, "");
    malformed.payload = json!({"type":"message_delta", "text":42});
    assert_eq!(session.apply(malformed), Apply::Rejected);
    session.apply(complete(3));
    assert_eq!(session.status, Status::Disconnected);
}

#[test]
fn duplicate_id_never_appends_twice_even_with_new_sequence() {
    let mut session = session();
    let envelope = delta(2, "こんにちは");
    session.apply(envelope.clone());
    let mut duplicate = envelope;
    duplicate.sequence = 3;
    assert_eq!(session.apply(duplicate), Apply::Duplicate);
    assert_eq!(session.chat.back().unwrap().text, "こんにちは");
    assert_eq!(session.last_sequence, 2);
}

#[test]
fn reversed_sequence_and_other_session_do_not_mutate_chat() {
    let mut session = session();
    session.apply(delta(2, "A"));
    let mut late = delta(1, "B");
    late.event_id = "late".into();
    assert_eq!(session.apply(late), Apply::Rejected);
    let mut foreign = delta(3, "C");
    foreign.session_id = "s2".into();
    assert_eq!(session.apply(foreign), Apply::Rejected);
    assert_eq!(session.chat.back().unwrap().text, "A");
    assert_eq!(session.last_sequence, 2);
}

#[test]
fn gap_is_visible_and_prevents_unqualified_completion() {
    let mut session = session();
    session.apply(delta(3, "A"));
    assert!(session.logs.iter().any(|row| row.text.contains("欠落")));
    session.apply(complete(4));
    assert_eq!(session.status, Status::Disconnected);
}

#[test]
fn previous_turn_cannot_complete_current_turn() {
    let mut session = session();
    let mut wrong = complete(2);
    wrong.turn_id = Some("old-turn".into());
    assert_eq!(session.apply(wrong), Apply::Rejected);
    assert_eq!(session.status, Status::Running);
}

#[test]
fn terminal_state_cannot_be_overwritten_by_late_completion() {
    let mut session = session();
    session.apply(event(
        2,
        Event::TurnCancelled {
            reason: "cancelled".into(),
        },
    ));
    assert_eq!(session.apply(complete(3)), Apply::Rejected);
    assert_eq!(session.status, Status::Cancelled);
}

#[test]
fn completion_with_unfinished_tool_remains_uncertain() {
    let mut session = session();
    session.apply(event(
        2,
        Event::ToolStarted {
            invocation_id: "tool-1".into(),
            command: "fixture".into(),
            cwd: "/fixture".into(),
        },
    ));
    session.apply(complete(3));
    assert_eq!(session.status, Status::Disconnected);
    assert_eq!(session.tools["tool-1"], None);
}

#[test]
fn cancel_request_and_confirmation_are_separate() {
    let mut session = session();
    session.status = Status::Cancelling;
    session.apply(delta(2, "already queued"));
    assert_eq!(session.status, Status::Cancelling);
    session.apply(event(
        3,
        Event::TurnCancelled {
            reason: "confirmed".into(),
        },
    ));
    assert_eq!(session.status, Status::Cancelled);
}

#[test]
fn closing_transport_does_not_change_completed_turn() {
    let mut session = session();
    session.apply(complete(2));
    session.transport_closed();
    assert_eq!(session.status, Status::Completed);
    assert_eq!(session.usage, Usage::default());
    assert_eq!(
        serde_json::to_value(&session.usage).unwrap()["cost_usd"],
        serde_json::Value::Null
    );
}

#[test]
fn log_preview_retention_is_bounded() {
    let mut session = session();
    for i in 0..10_000 {
        session.apply(event(
            i + 2,
            Event::Log {
                level: "info".into(),
                preview: "preview".into(),
                offset: i * 4096,
                bytes: 4096,
            },
        ));
    }
    assert_eq!(session.logs.len(), MAX_LOG_ROWS);
    assert_eq!(session.logs_discarded, 10_000 - MAX_LOG_ROWS);
    assert_eq!(session.log_bytes, 10_000 * 4096);
    assert_eq!(session.logs.front().unwrap().offset, Some(9_000 * 4096));
}

#[test]
fn long_unicode_message_uses_bounded_blocks_without_losing_bytes() {
    let mut session = session();
    let text = "日本語🙂e\u{301}🧑🏽‍💻".repeat(10_000);
    session.apply(delta(2, &text));
    let joined = session
        .chat
        .iter()
        .skip(1)
        .map(|block| block.text.as_str())
        .collect::<String>();
    assert_eq!(joined, text);
    assert!(
        session
            .chat
            .iter()
            .all(|block| block.text.len() <= CHAT_BLOCK_BYTES)
    );
}

#[test]
fn chat_retention_limit_is_explicit() {
    let mut session = session();
    let text = "x".repeat(CHAT_BLOCK_BYTES);
    for i in 0..MAX_CHAT_BLOCKS + 10 {
        session.apply(delta(i as u64 + 2, &text));
    }
    assert_eq!(session.chat.len(), MAX_CHAT_BLOCKS);
    assert!(session.chat_discarded > 0);
}

#[test]
fn unified_diff_tracks_old_and_new_line_numbers() {
    let diff = Diff::parse(
        "sample.rs".into(),
        "--- a/sample.rs\n+++ b/sample.rs\n@@ -10,2 +20,2 @@\n-old\n+new\n same\n\\ No newline at end of file\n",
    );
    assert_eq!(diff.lines[3].kind, DiffKind::Removed);
    assert_eq!((diff.lines[3].old, diff.lines[3].new), (Some(10), None));
    assert_eq!((diff.lines[4].old, diff.lines[4].new), (None, Some(20)));
    assert_eq!((diff.lines[5].old, diff.lines[5].new), (Some(11), Some(21)));
    assert_eq!(diff.lines[6].kind, DiffKind::Header);
}

#[test]
fn overlapping_turn_does_not_clear_unfinished_tool() {
    let mut session = session();
    session.apply(event(
        2,
        Event::ToolStarted {
            invocation_id: "tool-1".into(),
            command: "fixture".into(),
            cwd: "fixture".into(),
        },
    ));
    let mut overlapping = event(
        3,
        Event::TurnStarted {
            prompt: "overlap".into(),
        },
    );
    overlapping.turn_id = Some("turn-2".into());
    assert_eq!(session.apply(overlapping), Apply::Rejected);
    assert_eq!(session.turn_id.as_deref(), Some("turn-1"));
    assert_eq!(session.tools["tool-1"], None);
}

#[test]
fn unmatched_tool_completion_cannot_be_success() {
    let mut session = session();
    assert_eq!(
        session.apply(event(
            2,
            Event::ToolFinished {
                invocation_id: "not-started".into(),
                exit_code: 0
            }
        )),
        Apply::Rejected
    );
    session.apply(complete(3));
    assert_eq!(session.status, Status::Disconnected);
}

#[test]
fn tool_completion_cannot_be_rewritten_with_new_id() {
    let mut session = session();
    session.apply(event(
        2,
        Event::ToolStarted {
            invocation_id: "tool-1".into(),
            command: "fixture".into(),
            cwd: "fixture".into(),
        },
    ));
    session.apply(event(
        3,
        Event::ToolFinished {
            invocation_id: "tool-1".into(),
            exit_code: 1,
        },
    ));
    assert_eq!(
        session.apply(event(
            4,
            Event::ToolFinished {
                invocation_id: "tool-1".into(),
                exit_code: 0
            }
        )),
        Apply::Rejected
    );
    assert_eq!(session.tools["tool-1"], Some(1));
}

#[test]
fn cancelling_before_next_turn_started_keeps_request_and_new_identity() {
    let mut session = session();
    session.apply(complete(2));
    session.status = Status::Cancelling;
    let mut started = event(
        3,
        Event::TurnStarted {
            prompt: "next".into(),
        },
    );
    started.turn_id = Some("turn-2".into());
    assert_eq!(session.apply(started), Apply::Applied);
    assert_eq!(session.status, Status::Cancelling);
    assert_eq!(session.turn_id.as_deref(), Some("turn-2"));
}

#![allow(dead_code)]

use solo::{
    event::{Envelope, Event, SCHEMA_VERSION, Usage},
    projection::Session,
};

pub fn envelope(session_id: &str, turn_id: &str, sequence: u64, payload: Event) -> Envelope {
    Envelope {
        schema_version: SCHEMA_VERSION,
        event_id: format!("event-{sequence}"),
        session_id: session_id.into(),
        sequence,
        timestamp_ms: 0,
        turn_id: Some(turn_id.into()),
        payload: serde_json::to_value(payload).unwrap(),
    }
}

pub fn event(sequence: u64, payload: Event) -> Envelope {
    envelope("s1", "turn-1", sequence, payload)
}

pub fn session() -> Session {
    let mut session = Session::new("s1".into(), "test".into());
    session.apply(event(
        1,
        Event::TurnStarted {
            prompt: "表示を確認".into(),
        },
    ));
    session
}

pub fn delta(sequence: u64, text: &str) -> Envelope {
    event(
        sequence,
        Event::MessageDelta {
            message_id: "m1".into(),
            text: text.into(),
        },
    )
}

pub fn complete(sequence: u64) -> Envelope {
    event(
        sequence,
        Event::TurnCompleted {
            reason: "done".into(),
            usage: Usage::default(),
        },
    )
}

#![allow(dead_code)]

use solo::{
    event::{Envelope, Event, SCHEMA_VERSION, SessionId, TurnId, Usage},
    projection::SessionProjection,
};

pub fn envelope(session_id: &str, turn_id: &str, sequence: u64, payload: Event) -> Envelope {
    Envelope {
        schema_version: SCHEMA_VERSION,
        event_id: format!("event-{sequence}"),
        session_id: SessionId::parse(session_id).unwrap(),
        sequence,
        timestamp_ms: 0,
        turn_id: Some(TurnId::from(turn_id.to_owned())),
        payload: serde_json::to_value(payload).unwrap(),
    }
}

pub fn event(sequence: u64, payload: Event) -> Envelope {
    envelope("s1", "turn-1", sequence, payload)
}

pub fn session() -> SessionProjection {
    let mut session = SessionProjection::new(SessionId::parse("s1").unwrap(), "test".into());
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

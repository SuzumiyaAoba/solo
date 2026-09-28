use super::{
    store::{MAX_DRAFT_BYTES, events_path, history_path, read_history, read_meta},
    *,
};
use crate::{
    event::{Envelope, SCHEMA_VERSION, SessionId, TurnId},
    harness::{Message, ToolCall, ToolResult},
    projection::{Speaker, Status},
};
use serde_json::json;
use std::{fs, io, time::Duration};

fn envelope(
    session_id: &str,
    sequence: u64,
    turn_id: Option<&str>,
    payload: serde_json::Value,
) -> Envelope {
    Envelope {
        schema_version: SCHEMA_VERSION,
        event_id: format!("{session_id}-{sequence}"),
        session_id: SessionId::parse(session_id).unwrap(),
        sequence,
        timestamp_ms: 1_700_000_000_000 + sequence,
        turn_id: turn_id.map(|t| TurnId::from(t.to_owned())),
        payload,
    }
}

fn store() -> (tempfile::TempDir, WorkspaceStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = WorkspaceStore::at_path(dir.path().join("workspace"));
    (dir, store)
}

fn sid(s: &str) -> SessionId {
    SessionId::parse(s).unwrap()
}

#[test]
fn replay_restores_chat_and_status() {
    let (_tmp, store) = store();
    {
        let mut file = store.create(&sid("s1")).expect("create");
        file.append(&envelope(
            "s1",
            1,
            None,
            json!({"type":"session_created","title":"テスト","workspace_id":"w","settings":{}}),
        ))
        .unwrap();
        file.append(&envelope(
            "s1",
            2,
            Some("t1"),
            json!({"type":"turn_started","prompt":"やって"}),
        ))
        .unwrap();
        file.append(&envelope(
            "s1",
            3,
            Some("t1"),
            json!({"type":"message_delta","message_id":"m1","text":"了解"}),
        ))
        .unwrap();
        file.append(&envelope(
            "s1",
            4,
            Some("t1"),
            json!({"type":"diff_updated","path":"a.rs","unified_diff":"@@ -1 +1 @@\n-a\n+b\n"}),
        ))
        .unwrap();
        file.append(&envelope(
            "s1",
            5,
            Some("t1"),
            json!({"type":"turn_completed","reason":"ok","usage":{"input_tokens":3}}),
        ))
        .unwrap();
        file.flush(true).unwrap();
    }
    let restored = store.load(&sid("s1")).expect("load");
    assert_eq!(restored.session.status(), Status::Completed);
    assert!(!restored.interrupted);
    assert_eq!(
        restored
            .session
            .chat()
            .iter()
            .filter(|block| block.speaker == Speaker::Assistant)
            .map(|block| block.text.as_str())
            .collect::<String>(),
        "了解"
    );
    assert_eq!(restored.session.diffs().len(), 1);
    assert_eq!(restored.session.title(), "テスト");
}

#[test]
fn open_turn_is_marked_recovered() {
    let (_tmp, store) = store();
    {
        let mut file = store.create(&sid("s2")).expect("create");
        file.append(&envelope(
            "s2",
            1,
            None,
            json!({"type":"session_created","title":"t","workspace_id":"w","settings":{}}),
        ))
        .unwrap();
        file.append(&envelope(
            "s2",
            2,
            Some("t1"),
            json!({"type":"turn_started","prompt":"go"}),
        ))
        .unwrap();
        file.flush(true).unwrap();
    }
    let restored = store.load(&sid("s2")).expect("load");
    assert_eq!(restored.session.status(), Status::Disconnected);
    assert!(restored.interrupted);
    assert!(restored.session.incomplete());
}

#[test]
fn truncated_tail_is_tolerated() {
    let (_tmp, store) = store();
    let dir = store.session_dir(&sid("s3")).expect("dir");
    fs::create_dir_all(&dir).unwrap();
    let good1 = serde_json::to_string(&envelope(
        "s3",
        1,
        None,
        json!({"type":"session_created","title":"t","workspace_id":"w","settings":{}}),
    ))
    .unwrap();
    let good2 = serde_json::to_string(&envelope(
        "s3",
        2,
        Some("t1"),
        json!({"type":"turn_started","prompt":"go"}),
    ))
    .unwrap();
    fs::write(
        events_path(&dir),
        format!("{good1}\n{good2}\n{{\"schema_version\":1,\"eve"),
    )
    .unwrap();
    let restored = store.load(&sid("s3")).expect("load");
    assert!(restored.truncated_tail);
    assert_eq!(restored.corrupted_lines, 0);
    assert_eq!(restored.session.status(), Status::Disconnected);
    assert_eq!(restored.session.turn_id().map(|id| &**id), Some("t1"));
}

#[test]
fn meta_roundtrip_and_sweep() {
    let (_tmp, store) = store();
    let file = store.create(&sid("s4")).expect("create");
    let meta = SessionMeta {
        session_id: sid("s4"),
        title: "締めた".into(),
        closed: true,
        ..SessionMeta::default()
    };
    file.save_meta(&meta).expect("save_meta");
    let loaded = read_meta(file.dir()).expect("load_meta");
    assert_eq!(loaded.title, "締めた");
    assert!(loaded.closed);
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert!(loaded.updated_ms > 0);
    assert!(store.session_ids().unwrap().contains(&sid("s4")));
    // closed=true は即削除。開いているハンドルは使用中とみなすので先に閉じる。
    drop(file);
    assert_eq!(store.sweep_orphans().unwrap(), 1);
    assert!(store.session_ids().unwrap().is_empty());

    // meta 無しの dir は mtime が新しい限り掃除しない(他ウィンドウの途中作成かもしれない)。
    let dir = store.session_dir(&sid("s5")).unwrap();
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(store.sweep_orphans().unwrap(), 0);
    assert!(dir.is_dir());
}

#[test]
fn history_is_capped_from_the_front() {
    let (_tmp, store) = store();
    let file = store.create(&sid("s6")).expect("create");
    let history: Vec<Message> = (0..16)
        .map(|_| Message::User {
            text: "x".repeat(600_000),
        })
        .collect();
    file.save_history(&history).expect("save_history");
    let loaded = read_history(file.dir()).expect("load_history");
    assert!(!loaded.is_empty());
    assert!(loaded.len() < history.len());
    let saved = fs::metadata(history_path(file.dir())).unwrap().len();
    assert!(saved <= MAX_HISTORY_BYTES);
}

#[test]
fn workspace_state_roundtrip() {
    let (_tmp, store) = store();
    assert!(store.load_workspace_state().unwrap().selected.is_none());
    let state = WorkspaceState {
        selected: Some(sid("s7")),
        queue: StoredQueue {
            paused: true,
            entries: vec![
                QueueEntry {
                    session_id: sid("s7"),
                    backend: BackendKind::Subscription,
                    prompt: "first".into(),
                },
                QueueEntry {
                    session_id: sid("s8"),
                    backend: BackendKind::Acp { id: "x".into() },
                    prompt: "second".into(),
                },
                QueueEntry {
                    session_id: sid("s9"),
                    backend: BackendKind::Mock {
                        key: "events-10k".into(),
                    },
                    prompt: "third".into(),
                },
            ],
        },
        ..WorkspaceState::default()
    };
    store.save_workspace_state(&state).expect("save");
    let loaded = store.load_workspace_state().expect("load");
    assert_eq!(loaded.selected.as_deref(), Some("s7"));
    assert_eq!(loaded.queue.entries.len(), 3);
    assert!(loaded.queue.paused);
    assert_eq!(
        loaded.queue.entries[1].backend,
        BackendKind::Acp { id: "x".into() }
    );
    assert_eq!(loaded.version, SCHEMA_VERSION);
}

#[test]
fn invalid_session_id_is_rejected() {
    // 検査は SessionId::parse に移った。store の全 API は &SessionId しか受けないため、
    // dir traversal 等の不正 id はそもそも生成できない。
    for raw in ["../evil", "", "with space", "bad/slash", &"x".repeat(41)] {
        let error = SessionId::parse(raw).expect_err("invalid id must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
    assert!(SessionId::parse("ok-session_1").is_ok());
}

#[test]
fn export_writes_events_state_and_transcript() {
    let (_tmp, store) = store();
    let session = {
        let mut file = store.create(&sid("s10")).expect("create");
        file.append(&envelope(
            "s10",
            1,
            None,
            json!({"type":"session_created","title":"出口","workspace_id":"w","settings":{}}),
        ))
        .unwrap();
        file.append(&envelope(
            "s10",
            2,
            Some("t1"),
            json!({"type":"turn_started","prompt":"go"}),
        ))
        .unwrap();
        file.append(&envelope(
            "s10",
            3,
            Some("t1"),
            json!({"type":"turn_completed","reason":"done","usage":{}}),
        ))
        .unwrap();
        file.flush(true).unwrap();
        file.save_meta(&SessionMeta {
            session_id: sid("s10"),
            title: "出口".into(),
            ..SessionMeta::default()
        })
        .unwrap();
        store.load(&sid("s10")).expect("load").session
    };
    let exports = tempfile::tempdir().unwrap();
    let dir = store.export(&session, exports.path()).expect("export");
    assert!(dir.join("events.jsonl").is_file());
    assert!(dir.join("state.json").is_file());
    let transcript = fs::read_to_string(dir.join("transcript.md")).unwrap();
    assert!(transcript.contains("## ユーザー"));
    assert!(transcript.contains("- title: 出口"));
    assert!(transcript.contains("- status: 完了"));
    // create→load→export と別の dir でも動く。
    assert_eq!(
        store.load(&sid("s10")).unwrap().session.status(),
        Status::Completed
    );
    store.remove(&sid("s10")).unwrap();
    assert!(store.session_ids().unwrap().is_empty());
    store.remove(&sid("s10")).unwrap();
}

#[test]
fn sweep_only_removes_eventless_stale_dirs() {
    let (_tmp, store) = store();
    // meta 無しでも events.jsonl に内容があれば記録を失わないよう残す。
    let recorded = store.session_dir(&sid("recorded")).unwrap();
    fs::create_dir_all(&recorded).unwrap();
    fs::write(events_path(&recorded), "{}\n").unwrap();
    // 未知 version・破損した state.json はダウングレードで消さない。
    let future = store.session_dir(&sid("future")).unwrap();
    fs::create_dir_all(&future).unwrap();
    fs::write(future.join("state.json"), "{\"version\":99}").unwrap();
    let broken = store.session_dir(&sid("broken")).unwrap();
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("state.json"), "{not json").unwrap();
    // SessionId として解釈できない dir 名はこのストアの管理外なので残す。
    fs::create_dir_all(store.root().join("not a session")).unwrap();
    // meta 無し・events.jsonl 空の残骸だけ消す。
    let empty = store.create(&sid("empty")).unwrap();
    drop(empty);
    assert_eq!(store.sweep(Duration::ZERO).unwrap(), 1);
    for dir in [&recorded, &future, &broken] {
        assert!(dir.is_dir(), "{dir:?} must be kept");
    }
    assert!(store.root().join("not a session").is_dir());
    assert!(store.load(&sid("future")).is_err());
    assert!(store.load(&sid("broken")).is_err());
    assert!(!store.session_dir(&sid("empty")).unwrap().exists());
}

#[test]
fn oversized_drafts_are_truncated_at_char_boundaries() {
    let (_tmp, store) = store();
    let file = store.create(&sid("draft")).unwrap();
    let draft = "あ".repeat(600_000);
    file.save_meta(&SessionMeta {
        session_id: sid("draft"),
        draft: draft.clone(),
        ..SessionMeta::default()
    })
    .expect("save_meta");
    let loaded = read_meta(file.dir()).expect("read_meta");
    assert!(loaded.draft.len() <= MAX_DRAFT_BYTES);
    assert!(draft.starts_with(&loaded.draft));
}

#[test]
fn history_trim_starts_at_a_user_message() {
    let (_tmp, store) = store();
    let file = store.create(&sid("hist")).unwrap();
    let history = vec![
        Message::User {
            text: "u".repeat(1024 * 1024),
        },
        Message::Assistant {
            text: "a".repeat(3 * 1024 * 1024),
            tool_calls: vec![ToolCall {
                id: "c".into(),
                name: "exec".into(),
                arguments: json!({}),
            }],
        },
        Message::Tool {
            call_id: "c".into(),
            result: ToolResult::ok("t".repeat(6 * 1024 * 1024)),
        },
        Message::User {
            text: "next".into(),
        },
        Message::Assistant {
            text: "ok".into(),
            tool_calls: vec![],
        },
    ];
    file.save_history(&history).expect("save_history");
    // 先頭の Tool 結果単体は API が拒否するため、直近の User から始まる。
    assert_eq!(
        read_history(file.dir()).expect("read_history"),
        history[3..].to_vec()
    );
}

#[test]
fn sweep_keeps_sessions_open_in_another_window() {
    let (_tmp, store) = store();
    // 開きっぱなしのハンドルは events.jsonl の shared lock で使用中を示す。
    let held = store.create(&sid("held")).unwrap();
    assert_eq!(store.sweep(Duration::ZERO).unwrap(), 0);
    assert!(held.dir().is_dir());
    drop(held);
    assert_eq!(store.sweep(Duration::ZERO).unwrap(), 1);
    assert!(!store.session_dir(&sid("held")).unwrap().exists());
}

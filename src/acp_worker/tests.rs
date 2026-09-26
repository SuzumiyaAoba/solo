use super::*;
use crate::projection::{Apply, Session, Status};
#[cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};

#[test]
fn permission_request_inherits_tool_metadata_and_updates_do_not_clear_it() {
    let mut bridge = Bridge::default();
    bridge.remember_tool(&json!({"toolCallId":"one","kind":"execute","name":"shell","rawInput":{"command":"pwd","cwd":"/tmp"}}));
    bridge
        .remember_tool(&json!({"toolCallId":"one","kind":null,"rawInput":null,"title":"Working"}));
    let params = bridge.permission_params(
        &json!({"sessionId":"s","toolCall":{"toolCallId":"one"},"options":[{"kind":"allow_once"}]}),
    );
    assert_eq!(params["toolCall"]["kind"], "execute");
    assert_eq!(params["toolCall"]["rawInput"]["command"], "pwd");
    assert_eq!(params["toolCall"]["title"], "Working");
    let other = bridge.permission_params(&json!({"toolCall":{"toolCallId":"two"}}));
    assert!(other["toolCall"]["rawInput"].is_null());
    let changed = bridge.permission_params(
        &json!({"toolCall":{"toolCallId":"one","rawInput":{"command":"whoami"}}}),
    );
    assert_eq!(changed["toolCall"]["rawInput"], json!({"command":"whoami"}));
}

#[test]
fn completed_tool_updates_preserve_the_outcome_and_accept_later_content() {
    let (sender, receiver) = async_channel::bounded(32);
    let mut emitter = Emitter::new("local".into(), 0, sender);
    emitter.begin_turn("inspect");
    let mut bridge = Bridge::default();
    bridge.handle(
        &json!({"sessionUpdate":"tool_call","toolCallId":"one","title":"read","status":"pending","rawInput":{"path":"a"}}),
        &mut emitter,
    );
    bridge.handle(
        &json!({"sessionUpdate":"tool_call_update","toolCallId":"one","status":"completed"}),
        &mut emitter,
    );
    let params = bridge.permission_params(&json!({"toolCall":{"toolCallId":"one"}}));
    assert!(
        params["toolCall"]["rawInput"].is_null(),
        "finished tool input must not authorize a later request"
    );
    for status in [Value::Null, json!("completed"), json!("failed")] {
        bridge.handle(
            &json!({"sessionUpdate":"tool_call_update","toolCallId":"one","status":status,"content":[{"type":"content","content":{"type":"text","text":"later output"}}]}),
            &mut emitter,
        );
    }
    emitter.emit(Event::TurnCompleted {
        reason: "done".into(),
        usage: Usage::default(),
    });
    drop(emitter);
    let mut session = Session::new("local".into(), "test".into());
    while let Ok(Delivery::Event(event)) = receiver.try_recv() {
        assert_eq!(session.apply(event), Apply::Applied);
    }
    assert_eq!(session.status, Status::Completed);
    assert_eq!(session.tool_activity.len(), 1);
    assert_eq!(session.tool_activity[0].exit_code, Some(0));
    assert_eq!(
        session
            .logs
            .iter()
            .filter(|log| log.text == "later output")
            .count(),
        3
    );
}

#[cfg(unix)]
#[test]
fn localized_authentication_errors_use_the_advertised_agent_method() {
    let (_dir, controller, receiver) = test_agent(
        r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"authMethods":[{"name":"missing ID"},{"id":"terminal","type":"terminal"},{"id":"agent-login","name":"Agent login"}]}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"認証が必要です"}}'
IFS= read -r line
case "$line" in *'"method":"authenticate"'*'"methodId":"agent-login"'*) ;; *) exit 3;; esac
echo '{"jsonrpc":"2.0","id":2,"result":{}}'
IFS= read -r line
case "$line" in *'"method":"session/new"'*) ;; *) exit 4;; esac
echo '{"jsonrpc":"2.0","id":3,"result":{"sessionId":"authenticated"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
"##,
    );
    controller.prompt("continue after login".into()).unwrap();
    let mut session = Session::new("local".into(), "test".into());
    let deadline = Instant::now() + Duration::from_secs(3);
    while session.status != Status::Completed && Instant::now() < deadline {
        match receiver.try_recv() {
            Ok(Delivery::Event(event)) => {
                session.apply(event);
            }
            Ok(Delivery::Error(error)) => panic!("{error}"),
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
    assert_eq!(session.status, Status::Completed);
}

#[cfg(unix)]
#[test]
fn authentication_words_in_other_errors_do_not_trigger_login() {
    let (_dir, _controller, receiver) = test_agent(
        r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"authMethods":[{"id":"agent-login","name":"Agent login"}]}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"authentication cache failure"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":2,"error":{"code":-32603,"message":"unexpected authentication attempt"}}'
IFS= read -r line
"##,
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match receiver.try_recv() {
            Ok(Delivery::Error(error)) => {
                assert_eq!(error, "session/new: authentication cache failure");
                return;
            }
            _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => panic!("the original error was not reported"),
        }
    }
}

#[cfg(unix)]
#[test]
fn request_cancelled_errors_end_the_turn_and_preserve_the_connection() {
    let (_dir, controller, receiver) = test_agent(
        r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"acp-1"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":2,"error":{"code":-32800,"message":"処理を中止しました"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
"##,
    );
    let mut session = Session::new("local".into(), "test".into());
    for expected in [Status::Cancelled, Status::Completed] {
        controller.prompt("run".into()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while session.status != expected && Instant::now() < deadline {
            match receiver.try_recv() {
                Ok(Delivery::Event(event)) => {
                    session.apply(event);
                }
                Ok(Delivery::Error(error)) => panic!("{error}"),
                _ => thread::sleep(Duration::from_millis(5)),
            }
        }
        assert_eq!(
            session.status, expected,
            "a protocol cancellation must allow a following turn"
        );
    }
}

#[cfg(unix)]
#[test]
fn cancelling_pending_permission_does_not_need_a_ui_reply_and_allows_another_turn() {
    let (_dir, controller, receiver) = test_agent(
        r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"acp-1"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":77,"method":"session/request_permission","params":{"sessionId":"acp-1","toolCall":{"toolCallId":"one"},"options":[{"optionId":"a","kind":"allow_once"}]}}'
IFS= read -r line
case "$line" in *'"method":"session/cancel"'*) ;; *) exit 3;; esac
IFS= read -r line
case "$line" in *'"outcome":"cancelled"'*) ;; *) exit 4;; esac
echo '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"cancelled"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":78,"method":"session/request_permission","params":{"sessionId":"acp-1","toolCall":{"toolCallId":"two"},"options":[{"optionId":"a","kind":"allow_once"}]}}'
IFS= read -r line
case "$line" in *'"optionId":"a"'*) ;; *) exit 5;; esac
echo '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
"##,
    );
    let mut session = Session::new("local".into(), "test".into());
    controller.prompt("first".into()).unwrap();
    let mut pending = None;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && pending.is_none() {
        match receiver.try_recv() {
            Ok(Delivery::Approval { reply, .. }) => pending = Some(reply),
            Ok(Delivery::Event(event)) => {
                session.apply(event);
            }
            Ok(Delivery::Error(error)) => panic!("{error}"),
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
    assert!(
        pending.is_some(),
        "permission request missing: status={:?}, sequence={}, reason={}",
        session.status,
        session.last_sequence,
        session.reason
    );
    controller.cancel();
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline && session.status != Status::Cancelled {
        if let Ok(Delivery::Event(event)) = receiver.try_recv() {
            session.apply(event);
        }
        thread::sleep(Duration::from_millis(5));
    }
    drop(pending);
    assert_eq!(
        session.status,
        Status::Cancelled,
        "a UI reply must not be needed to cancel"
    );
    controller.prompt("second".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && session.status != Status::Completed {
        match receiver.try_recv() {
            Ok(Delivery::Approval { reply, .. }) => reply.send_blocking(true).unwrap(),
            Ok(Delivery::Event(event)) => {
                session.apply(event);
            }
            Ok(Delivery::Error(error)) => panic!("{error}"),
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
    assert_eq!(
        session.status,
        Status::Completed,
        "cancellation must not affect the next turn"
    );
}

#[cfg(unix)]
#[test]
fn two_turns_share_one_agent_process_and_permission_is_forwarded() {
    let (_dir, controller, receiver) = test_agent(
        r##"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"agentCapabilities":{},"authMethods":[]}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"acp-1"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"tool_call","toolCallId":"tool-1","title":"read","status":"pending"}}}'
echo '{"jsonrpc":"2.0","id":77,"method":"session/request_permission","params":{"sessionId":"acp-1","toolCall":{"toolCallId":"tool-1"},"options":[{"optionId":"a","kind":"allow_once","name":"Allow"},{"optionId":"r","kind":"reject_once","name":"Reject"}]}}'
IFS= read -r line
case "$line" in *'"optionId":"a"'*) ;; *) exit 3;; esac
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"tool-1","status":"completed","content":[{"type":"diff","path":"/tmp/a","oldText":"old","newText":"new"}]}}}'
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"m1","content":{"type":"text","text":"first"}}}}'
echo '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"m2","content":{"type":"text","text":"second"}}}}'
echo '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
IFS= read -r line
"##,
    );
    let mut session = Session::new("local".into(), "test".into());
    for (turn, prompt) in ["first prompt", "second prompt"].into_iter().enumerate() {
        controller.prompt(prompt.into()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match receiver.try_recv() {
                Ok(Delivery::Event(event)) => {
                    assert_eq!(session.apply(event), Apply::Applied);
                    if session.status == Status::Completed
                        && session.accepted > if turn == 0 { 2 } else { 8 }
                    {
                        break;
                    }
                }
                Ok(Delivery::Approval { reply, .. }) => reply.send_blocking(true).unwrap(),
                Ok(Delivery::SessionId(id)) => assert_eq!(id, "acp-1"),
                Ok(Delivery::Error(error)) => panic!("{error}"),
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        assert_eq!(session.status, Status::Completed, "{}", session.reason);
    }
    assert!(session.chat.iter().any(|block| block.text == "first"));
    assert!(session.chat.iter().any(|block| block.text == "second"));
    assert_eq!(session.diffs.len(), 1);
    drop(controller);
}

#[cfg(unix)]
pub(super) fn test_agent(script: &str) -> (tempfile::TempDir, Controller, Receiver<Delivery>) {
    let dir = tempfile::tempdir().unwrap();
    let agent = dir.path().join("fake-acp");
    fs::write(&agent, script).unwrap();
    let mut permissions = fs::metadata(&agent).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&agent, permissions).unwrap();
    let (controller, receiver) = start(Config {
        local_session_id: "local".into(),
        title: "test".into(),
        workspace: dir.path().to_path_buf(),
        profile: AgentProfile {
            id: "a".into(),
            name: "Agent A".into(),
            command: agent.display().to_string(),
            args: vec![],
        },
        start_sequence: 0,
    })
    .unwrap();
    (dir, controller, receiver)
}

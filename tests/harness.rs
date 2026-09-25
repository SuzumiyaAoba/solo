use serde_json::json;
use solo::harness::{
    Cancellation, Limits, Message, Model, ModelOutput, StopReason, ToolCall, ToolExecutor,
    ToolResult, ToolSpec, Update, run, workspace::WorkspaceTools,
};
use std::collections::VecDeque;

struct ScriptedModel(VecDeque<ModelOutput>);

impl Model for ScriptedModel {
    fn complete(&mut self, _: &[Message], _: &[ToolSpec]) -> Result<ModelOutput, String> {
        self.0.pop_front().ok_or_else(|| "応答がありません".into())
    }
}

fn call(id: &str, name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments,
    }
}

#[test]
fn streamed_text_is_emitted_once_and_kept_in_history() {
    struct StreamingModel;
    impl Model for StreamingModel {
        fn complete(&mut self, _: &[Message], _: &[ToolSpec]) -> Result<ModelOutput, String> {
            unreachable!()
        }
        fn complete_with_updates(
            &mut self,
            _: &[Message],
            _: &[ToolSpec],
            on_delta: &mut dyn FnMut(&str),
        ) -> Result<ModelOutput, String> {
            on_delta("こん");
            on_delta("にちは");
            Ok(ModelOutput {
                text: "こんにちは".into(),
                tool_calls: vec![],
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut updates = Vec::new();
    let result = run(
        &mut StreamingModel,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User {
            text: "挨拶".into(),
        }],
        &Limits::default(),
        &Cancellation::default(),
        |update| updates.push(update),
    );
    assert_eq!(result.stop, StopReason::Completed);
    assert_eq!(
        updates
            .iter()
            .filter_map(|update| match update {
                Update::AssistantDelta(delta) => Some(delta.as_str()),
                _ => None,
            })
            .collect::<String>(),
        "こんにちは"
    );
    assert!(
        !updates
            .iter()
            .any(|update| matches!(update, Update::Assistant(_)))
    );
    assert!(matches!(&result.messages[1], Message::Assistant { text, .. } if text == "こんにちは"));
}

#[test]
fn tool_result_reaches_next_model_request() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "hello").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([
        ModelOutput {
            text: String::new(),
            tool_calls: vec![call("1", "read", json!({"path":"note.txt"}))],
        },
        ModelOutput {
            text: "done".into(),
            tool_calls: vec![],
        },
    ]));
    let mut updates = Vec::new();
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User {
            text: "read note".into(),
        }],
        &Limits::default(),
        &Cancellation::default(),
        |update| updates.push(update),
    );
    assert_eq!(result.stop, StopReason::Completed);
    assert_eq!(result.model_requests, 2);
    assert_eq!(result.tool_calls, 1);
    assert!(
        matches!(&result.messages[2], Message::Tool { result: ToolResult { content, is_error: false }, .. } if content == "hello")
    );
    assert!(matches!(
        updates.last(),
        Some(Update::Stopped(StopReason::Completed))
    ));
}

#[test]
fn denied_and_unknown_tools_are_not_executed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "old").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([
        ModelOutput {
            text: String::new(),
            tool_calls: vec![
                call(
                    "1",
                    "edit",
                    json!({"path":"note.txt","old":"old","new":"new"}),
                ),
                call("2", "missing", json!({})),
            ],
        },
        ModelOutput {
            text: "done".into(),
            tool_calls: vec![],
        },
    ]));
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| false,
        vec![Message::User {
            text: "change note".into(),
        }],
        &Limits::default(),
        &Cancellation::default(),
        |_| {},
    );
    assert_eq!(result.stop, StopReason::Completed);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.txt")).unwrap(),
        "old"
    );
    assert!(matches!(
        &result.messages[2],
        Message::Tool {
            result: ToolResult { is_error: true, .. },
            ..
        }
    ));
    assert!(matches!(
        &result.messages[3],
        Message::Tool {
            result: ToolResult { is_error: true, .. },
            ..
        }
    ));
}

#[test]
fn edit_checks_exact_match_and_workspace_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "old old").unwrap();
    std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = tools.execute(&call(
        "1",
        "edit",
        json!({"path":"note.txt","old":"old","new":"new"}),
    ));
    assert!(result.is_error);
    let result = tools.execute(&call(
        "2",
        "read",
        json!({"path":outside.path().join("secret.txt").to_str().unwrap()}),
    ));
    assert!(result.is_error);
    let result = tools.execute(&call(
        "3",
        "edit",
        json!({"path":"note.txt","old":"old old","new":"new"}),
    ));
    assert!(!result.is_error);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.txt")).unwrap(),
        "new"
    );
}

#[test]
fn limits_and_cancellation_stop_before_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([ModelOutput {
        text: String::new(),
        tool_calls: vec![call("1", "exec", json!({"command":"touch marker"}))],
    }]));
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User { text: "run".into() }],
        &Limits {
            max_tool_calls: 0,
            ..Limits::default()
        },
        &Cancellation::default(),
        |_| {},
    );
    assert_eq!(result.stop, StopReason::ToolLimit);
    assert!(!dir.path().join("marker").exists());

    let cancel = Cancellation::default();
    cancel.cancel();
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User { text: "run".into() }],
        &Limits::default(),
        &cancel,
        |_| {},
    );
    assert_eq!(result.stop, StopReason::Cancelled);
    assert_eq!(result.model_requests, 0);
}

#[test]
fn cancellation_during_tool_batch_skips_remaining_commands() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([ModelOutput {
        text: String::new(),
        tool_calls: vec![
            call("1", "exec", json!({"command":"touch first"})),
            call("2", "exec", json!({"command":"touch second"})),
        ],
    }]));
    let cancel = Cancellation::default();
    let request = cancel.clone();
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User { text: "run".into() }],
        &Limits::default(),
        &cancel,
        |update| {
            if matches!(update, Update::ToolFinished { ref call_id, .. } if call_id == "1") {
                request.cancel();
            }
        },
    );
    assert_eq!(result.stop, StopReason::Cancelled);
    assert!(dir.path().join("first").exists());
    assert!(!dir.path().join("second").exists());
    assert_eq!(result.tool_calls, 1);
    assert!(matches!(
        &result.messages[3],
        Message::Tool {
            result: ToolResult { is_error: true, .. },
            ..
        }
    ));
}

#[test]
fn duplicate_call_ids_are_rejected_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([ModelOutput {
        text: String::new(),
        tool_calls: vec![
            call("same", "exec", json!({"command":"touch first"})),
            call("same", "exec", json!({"command":"touch second"})),
        ],
    }]));
    let result = run(
        &mut model,
        &mut tools,
        &mut |_: &ToolCall| true,
        vec![Message::User { text: "run".into() }],
        &Limits::default(),
        &Cancellation::default(),
        |_| {},
    );
    assert!(matches!(result.stop, StopReason::InvalidResponse(_)));
    assert_eq!(result.tool_calls, 0);
    assert!(!dir.path().join("first").exists());
}

#[test]
fn cancellation_while_waiting_for_approval_prevents_execution() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut model = ScriptedModel(VecDeque::from([ModelOutput {
        text: String::new(),
        tool_calls: vec![call(
            "cancelled",
            "exec",
            json!({"command":"touch should-not-run"}),
        )],
    }]));
    let cancel = Cancellation::default();
    let while_awaiting = cancel.clone();
    let result = run(
        &mut model,
        &mut tools,
        &mut move |_: &ToolCall| {
            while_awaiting.cancel();
            true
        },
        vec![],
        &Limits::default(),
        &cancel,
        |_| {},
    );
    assert_eq!(result.stop, StopReason::Cancelled);
    assert!(!dir.path().join("should-not-run").exists());
}

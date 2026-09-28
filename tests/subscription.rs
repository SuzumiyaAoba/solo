use serde_json::json;
use solo::{
    codex::{Authentication, to_chat_messages},
    harness::{Message, ToolCall, ToolResult},
};

#[test]
fn subscription_auth_selects_a_rustls_crypto_provider() {
    let dir = tempfile::tempdir().unwrap();
    let _auth = Authentication::with_store(dir.path().join("auth.json")).unwrap();
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    let _config = rustls::ClientConfig::builder();
}

#[test]
fn preserves_tool_call_and_result_ids_for_next_model_request() {
    let messages = vec![
        Message::User {
            text: "read file".into(),
        },
        Message::Assistant {
            text: "確認します".into(),
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "read".into(),
                arguments: json!({"path":"README.md"}),
            }],
        },
        Message::Tool {
            call_id: "call-1".into(),
            result: ToolResult::ok("contents"),
        },
    ];
    let converted = to_chat_messages(&messages);
    assert_eq!(converted.len(), 3);
    let call = converted[1].content.tool_calls()[0];
    let result = converted[2].content.tool_responses()[0];
    assert_eq!(call.call_id, result.call_id);
    assert_eq!(call.fn_name, "read");
    assert_eq!(result.content, "contents");
}

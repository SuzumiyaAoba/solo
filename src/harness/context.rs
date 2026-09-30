//! モデルへ渡すメッセージ列の概算サイズと、予算へ収めるための決定的な省略。
//! 履歴本体(`run` の戻す `messages`)は変更せず、送信時のビューだけを作るため
//! 保存・監査される記録は省略前の完全な内容を保つ。
//!
//! 省略は「古い大きな tool 結果を先に退避する」方針に従い、最後の Assistant
//! メッセージより前にある Tool 結果だけを対象にする。それ以降の Tool 結果は
//! モデルがまだ受け取っていない最新の batch であり、省くと tool 呼出し自体が
//! 無駄になる。User や Assistant の本文は要約しないと意味が変わるため対象外。
use super::Message;
use serde_json::Value;
use std::borrow::Cow;

/// メッセージ列の送信ビュー。予算内なら借用で返し、複製は省略が必要な時だけ。
pub(crate) struct ContextView<'a> {
    pub messages: Cow<'a, [Message]>,
    /// 省略マーカーへ置き換えた tool 結果の件数。
    pub pruned_results: usize,
    /// 省略で削れた概算バイト数。
    pub saved_bytes: usize,
}

/// メッセージ 1 件の概算バイト数。role や JSON の枠組みは定数で足す。
/// 厳密なシリアライズ長・トークン数ではなく、履歴の肥大化を抑える目安。
pub(crate) fn message_bytes(message: &Message) -> usize {
    const FRAMING: usize = 32;
    let body = match message {
        Message::User { text } => text.len(),
        Message::Assistant { text, tool_calls } => {
            text.len()
                + tool_calls
                    .iter()
                    .map(|call| call.id.len() + call.name.len() + value_bytes(&call.arguments))
                    .sum::<usize>()
        }
        Message::Tool { call_id, result } => {
            call_id.len() + result.content.len() + result.summary.as_deref().map_or(0, str::len)
        }
    };
    FRAMING + body
}

/// `max_bytes` に収まる送信ビューを返す。超える場合は古い tool 結果の本文を
/// 先頭から順に省略マーカーへ置き換え、それでも収まらなければ None。
pub(crate) fn fit_context(messages: &[Message], max_bytes: usize) -> Option<ContextView<'_>> {
    let total = messages.iter().map(message_bytes).sum::<usize>();
    if total <= max_bytes {
        return Some(ContextView {
            messages: Cow::Borrowed(messages),
            pruned_results: 0,
            saved_bytes: 0,
        });
    }
    // 最後の Assistant 以降の Tool は「モデルがまだ見ていない最新 batch」なので保持。
    // Assistant が一度も無い履歴では省略対象なし（= 収まらなければ上限停止）。
    let boundary = messages
        .iter()
        .rposition(|message| matches!(message, Message::Assistant { .. }))
        .unwrap_or(0);
    let mut view = messages.to_vec();
    let mut remaining = total;
    let mut pruned_results = 0;
    let mut saved_bytes = 0;
    for (index, message) in view.iter_mut().enumerate() {
        if remaining <= max_bytes || index >= boundary {
            break;
        }
        let Message::Tool { result, .. } = message else {
            continue;
        };
        // is_error と summary は残し、呼出しの記録と何の結果かの手掛かりを保つ。
        let marker = format!(
            "[ツール結果を省略しました（{} バイト）。詳細が必要な場合は tool を再度呼び出してください]",
            result.content.len()
        );
        if result.content.len() > marker.len() {
            remaining = remaining - result.content.len() + marker.len();
            saved_bytes += result.content.len() - marker.len();
            result.content = marker;
            pruned_results += 1;
        }
    }
    (remaining <= max_bytes).then_some(ContextView {
        messages: Cow::Owned(view),
        pruned_results,
        saved_bytes,
    })
}

/// JSON 値の概算サイズ。文字列のエスケープは無視する概算で、実際の
/// シリアライズを避けるためのもの。
fn value_bytes(value: &Value) -> usize {
    match value {
        Value::Null | Value::Bool(_) => 4,
        Value::Number(number) => number.to_string().len(),
        Value::String(text) => text.len() + 2,
        Value::Array(items) => 2 + items.iter().map(value_bytes).sum::<usize>(),
        Value::Object(map) => {
            2 + map
                .iter()
                .map(|(key, value)| key.len() + 3 + value_bytes(value))
                .sum::<usize>()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{ToolCall, ToolResult};
    use serde_json::json;

    fn tool_result(call_id: &str, content: String) -> Message {
        Message::Tool {
            call_id: call_id.into(),
            result: ToolResult::ok(content),
        }
    }

    #[test]
    fn under_budget_returns_borrowed_view() {
        let messages = vec![Message::User { text: "hi".into() }];
        let view = fit_context(&messages, 1024).unwrap();
        assert!(matches!(view.messages, Cow::Borrowed(_)));
        assert_eq!(view.pruned_results, 0);
    }

    #[test]
    fn over_budget_prunes_oldest_tool_results_but_keeps_the_fresh_batch() {
        let big = "x".repeat(2000);
        let messages = vec![
            Message::User { text: "run".into() },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: "read".into(),
                    arguments: json!({}),
                }],
            },
            tool_result("1", big.clone()),
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "2".into(),
                    name: "read".into(),
                    arguments: json!({}),
                }],
            },
            tool_result("2", big.clone()),
        ];
        let total = messages.iter().map(message_bytes).sum::<usize>();
        // 古い方だけ削れば収まる上限にする。
        let view = fit_context(&messages, total - 1500).unwrap();
        assert_eq!(view.pruned_results, 1);
        let Message::Tool { result, .. } = &view.messages[2] else {
            panic!()
        };
        assert!(result.content.contains("省略"));
        // 最新 batch（最後の Assistant 以降）は省略しない。
        let Message::Tool { result, .. } = &view.messages[4] else {
            panic!()
        };
        assert_eq!(result.content, big);
        // 元の履歴は変更されない。
        let Message::Tool { result, .. } = &messages[2] else {
            panic!()
        };
        assert_eq!(result.content, big);
    }

    #[test]
    fn returns_none_when_only_the_fresh_batch_exceeds_the_budget() {
        let messages = vec![
            Message::User { text: "run".into() },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: "read".into(),
                    arguments: json!({}),
                }],
            },
            tool_result("1", "x".repeat(2000)),
        ];
        assert!(fit_context(&messages, 256).is_none());
    }
}

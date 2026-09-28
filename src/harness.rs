//! GPUI や特定の provider に依存しない、単一 agent の最小実行ループ。
//!
//! モデルと tool の実装はホストが渡す。各 tool 呼び出しの許可もホストが決める。
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

pub mod workspace;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        text: String,
    },
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        call_id: String,
        result: ToolResult,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// モデル応答に添えられるトークン使用量。未取得は None のまま残す。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelOutput {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
}

/// アダプターは provider 固有の形式を ModelOutput に変換する。
pub trait Model {
    fn complete(&mut self, messages: &[Message], tools: &[ToolSpec])
    -> Result<ModelOutput, String>;

    fn complete_with_updates(
        &mut self,
        messages: &[Message],
        tools: &[ToolSpec],
        _on_delta: &mut dyn FnMut(&str),
    ) -> Result<ModelOutput, String> {
        self.complete(messages, tools)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 実行器は workspace の境界、timeout、出力制限を適用する責任を持つ。
pub trait ToolExecutor {
    fn specs(&self) -> &[ToolSpec];
    fn execute(&mut self, call: &ToolCall) -> ToolResult;
}

/// `false` の場合、実行せず拒否結果をモデルへ返す。
pub trait Policy {
    fn allow(&mut self, call: &ToolCall) -> bool;
}

impl<F: FnMut(&ToolCall) -> bool> Policy for F {
    fn allow(&mut self, call: &ToolCall) -> bool {
        self(call)
    }
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_model_requests: usize,
    pub max_tool_calls: usize,
    pub max_result_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_model_requests: 64,
            max_tool_calls: 256,
            max_result_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, Default)]
pub struct Cancellation(CancellationToken);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }

    /// 実行の中止を I/O に伝える。I/O 側だけの中止は実行全体へ逆伝播しない。
    pub fn child_token(&self) -> CancellationToken {
        self.0.child_token()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    Completed,
    Cancelled,
    ModelLimit,
    ToolLimit,
    ModelError(String),
    InvalidResponse(String),
}

impl StopReason {
    /// ユーザー向けの停止理由。上限系の文言は実際の上限値を含める。
    pub fn message(&self, limits: &Limits) -> String {
        match self {
            Self::Completed => "実行完了".into(),
            Self::Cancelled => "実行を中止しました".into(),
            Self::ModelLimit => format!(
                "モデル呼び出しが上限の {} 回に達したため停止しました。続ける場合は、続きを依頼してください",
                limits.max_model_requests
            ),
            Self::ToolLimit => format!(
                "ツール呼び出しが上限の {} 回に達したため停止しました。続ける場合は、続きを依頼してください",
                limits.max_tool_calls
            ),
            Self::ModelError(error) => format!("モデルの呼び出しに失敗しました: {error}"),
            Self::InvalidResponse(error) => {
                format!("モデルの応答が不正なため停止しました: {error}")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Update {
    ModelRequested,
    Assistant(String),
    AssistantDelta(String),
    ToolProposed(ToolCall),
    ToolFinished { call_id: String, result: ToolResult },
    Stopped(StopReason),
}

#[derive(Clone, Debug)]
pub struct Run {
    pub messages: Vec<Message>,
    pub model_requests: usize,
    pub tool_calls: usize,
    pub stop: StopReason,
    /// usage を返した応答の合計。1 件も無ければ None。
    pub usage: Option<TokenUsage>,
}

/// 一回のユーザー入力を処理する。中止はモデル呼出しと tool 実行の間で確認する。
/// blocking な呼出しを即時停止する必要がある場合はアダプター自身も中止を実装する。
pub fn run<M, T, P, F>(
    model: &mut M,
    tools: &mut T,
    policy: &mut P,
    mut messages: Vec<Message>,
    limits: &Limits,
    cancellation: &Cancellation,
    mut on_update: F,
) -> Run
where
    M: Model,
    T: ToolExecutor,
    P: Policy,
    F: FnMut(Update),
{
    let mut requests = 0;
    let mut calls: usize = 0;
    let mut usage = None;
    let mut seen_ids = HashSet::new();
    let stop = loop {
        if cancellation.is_cancelled() {
            break StopReason::Cancelled;
        }
        if requests >= limits.max_model_requests {
            break StopReason::ModelLimit;
        }
        requests += 1;
        on_update(Update::ModelRequested);
        if cancellation.is_cancelled() {
            break StopReason::Cancelled;
        }
        let mut streamed_text = String::new();
        let output = match model.complete_with_updates(&messages, tools.specs(), &mut |delta| {
            streamed_text.push_str(delta);
            on_update(Update::AssistantDelta(delta.into()));
        }) {
            Ok(output) => output,
            Err(_) if cancellation.is_cancelled() => break StopReason::Cancelled,
            Err(error) => break StopReason::ModelError(error),
        };
        if cancellation.is_cancelled() {
            break StopReason::Cancelled;
        }
        if let Some(report) = output.usage {
            let total = usage.get_or_insert(TokenUsage::default());
            total.input_tokens = total.input_tokens.saturating_add(report.input_tokens);
            total.output_tokens = total.output_tokens.saturating_add(report.output_tokens);
        }
        if output.tool_calls.iter().any(|call| {
            call.id.is_empty() || call.name.is_empty() || !seen_ids.insert(call.id.clone())
        }) {
            break StopReason::InvalidResponse(
                "tool call ID または名前が空、あるいは ID が重複しています".into(),
            );
        }
        let done = output.tool_calls.is_empty();
        if !done && calls.saturating_add(output.tool_calls.len()) > limits.max_tool_calls {
            break StopReason::ToolLimit;
        }
        if streamed_text.is_empty() && !output.text.is_empty() {
            on_update(Update::Assistant(output.text.clone()));
        } else if let Some(suffix) = output.text.strip_prefix(&streamed_text)
            && !suffix.is_empty()
        {
            on_update(Update::AssistantDelta(suffix.into()));
        }
        messages.push(Message::Assistant {
            text: output.text,
            tool_calls: output.tool_calls.clone(),
        });
        if done {
            break StopReason::Completed;
        }
        for call in output.tool_calls {
            on_update(Update::ToolProposed(call.clone()));
            let mut result = if cancellation.is_cancelled() {
                ToolResult::error("中止されたため実行しませんでした")
            } else if !tools.specs().iter().any(|spec| spec.name == call.name) {
                calls += 1;
                ToolResult::error(format!("未知の tool: {}", call.name))
            } else if !policy.allow(&call) {
                calls += 1;
                ToolResult::error("tool の実行は許可されませんでした")
            } else if cancellation.is_cancelled() {
                ToolResult::error("承認待ちの間に中止されたため実行しませんでした")
            } else {
                calls += 1;
                tools.execute(&call)
            };
            result.content = truncate_utf8(result.content, limits.max_result_bytes);
            on_update(Update::ToolFinished {
                call_id: call.id.clone(),
                result: result.clone(),
            });
            messages.push(Message::Tool {
                call_id: call.id,
                result,
            });
        }
    };
    on_update(Update::Stopped(stop.clone()));
    Run {
        messages,
        model_requests: requests,
        tool_calls: calls,
        stop,
        usage,
    }
}

fn truncate_utf8(mut value: String, limit: usize) -> String {
    if value.len() > limit {
        value.truncate(value.floor_char_boundary(limit));
        value.push_str("\n[出力を省略]");
    }
    value
}

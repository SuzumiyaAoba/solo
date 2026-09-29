//! GPUI や特定の provider に依存しない、単一 agent の最小実行ループ。
//!
//! モデルと tool の実装はホストが渡す。各 tool 呼び出しの許可もホストが決める。
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
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
    /// 結果の一行要約。worker が ToolFinished の表示に回す。未取得は None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            summary: None,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            summary: None,
        }
    }

    /// 結果の一行要約を付ける。
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
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

/// Policy の判定。拒否理由はモデルへそのまま返るため、ルール名や承認元を
/// 添えるとモデルが再提案を調整できる。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyVerdict {
    Allow,
    /// モデルへ返す拒否理由。None は既定の文言にする。
    Deny(Option<String>),
}

impl PolicyVerdict {
    /// 理由付きで拒否する。
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::Deny(Some(reason.into()))
    }
}

/// `Deny` の場合、実行せず拒否結果をモデルへ返す。
/// 判定側で panic しても安全側(Deny)に倒す。
pub trait Policy {
    fn decide(&mut self, call: &ToolCall) -> PolicyVerdict;
}

impl<F: FnMut(&ToolCall) -> bool> Policy for F {
    fn decide(&mut self, call: &ToolCall) -> PolicyVerdict {
        if self(call) {
            PolicyVerdict::Allow
        } else {
            PolicyVerdict::Deny(None)
        }
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
/// 拡張側(model/policy/tool)の panic は実行ごと落とさない。モデルの panic は
/// ModelError、policy の panic は拒否、tool の panic はエラー結果として処理する。
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
        let output = match catch_unwind(AssertUnwindSafe(|| {
            model.complete_with_updates(&messages, tools.specs(), &mut |delta| {
                streamed_text.push_str(delta);
                on_update(Update::AssistantDelta(delta.into()));
            })
        })) {
            Ok(Ok(output)) => output,
            _ if cancellation.is_cancelled() => break StopReason::Cancelled,
            Ok(Err(error)) => break StopReason::ModelError(error),
            Err(payload) => {
                break StopReason::ModelError(format!(
                    "モデルの呼び出しが異常終了しました: {}",
                    panic_message(&payload)
                ));
            }
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
            } else {
                let verdict = match catch_unwind(AssertUnwindSafe(|| policy.decide(&call))) {
                    Ok(verdict) => verdict,
                    Err(payload) => PolicyVerdict::deny(format!(
                        "承認判定が異常終了しました: {}",
                        panic_message(&payload)
                    )),
                };
                match verdict {
                    PolicyVerdict::Deny(reason) => {
                        calls += 1;
                        ToolResult::error(
                            reason.unwrap_or_else(|| "tool の実行は許可されませんでした".into()),
                        )
                    }
                    PolicyVerdict::Allow if cancellation.is_cancelled() => {
                        ToolResult::error("承認待ちの間に中止されたため実行しませんでした")
                    }
                    PolicyVerdict::Allow => {
                        calls += 1;
                        match catch_unwind(AssertUnwindSafe(|| tools.execute(&call))) {
                            Ok(result) => result,
                            Err(payload) => ToolResult::error(format!(
                                "tool `{}` の実行が異常終了しました: {}",
                                call.name,
                                panic_message(&payload)
                            )),
                        }
                    }
                }
            };
            result.content = truncate_utf8(result.content, limits.max_result_bytes);
            result.summary = result
                .summary
                .map(|summary| truncate_utf8(summary, limits.max_result_bytes));
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

/// catch_unwind の payload から panic の説明を取り出す。
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("原因を取得できませんでした")
}

fn truncate_utf8(mut value: String, limit: usize) -> String {
    if value.len() > limit {
        value.truncate(value.floor_char_boundary(limit));
        value.push_str("\n[出力を省略]");
    }
    value
}

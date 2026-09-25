//! Solo の agent loop から ChatGPT Codex モデルを呼ぶアダプター。
//! OAuth と Responses 通信には公開ライブラリ genai-agentprism を使う。
use crate::harness::{Message, Model, ModelOutput, ToolCall, ToolSpec};
use futures::StreamExt;
use genai::{
    AssistantMessageEvent, StopReason as ProviderStopReason, StreamFn, StreamRequest,
    auth::genai_integration::CodexTokenResolver,
    auth::{
        CodexAuth, CodexConfig, CredentialStore, FileCredentialStore, OPENAI_CODEX_PROVIDER_ID,
    },
    chat::{ChatMessage, ContentPart, MessageContent, Tool, ToolResponse},
    codex::{CodexStreamFn, ResolverTokenSource},
    stream_fn::LlmContext,
};
use std::{io, path::PathBuf, sync::Arc};
use tokio::runtime::{Builder, Runtime};
use tokio_util::sync::CancellationToken;

pub const DEFAULT_MODEL: &str = "gpt-5.6-sol";

#[derive(Debug, Clone)]
pub struct DeviceLogin {
    pub verification_url: String,
    pub user_code: String,
}

pub struct Authentication {
    auth: Arc<CodexAuth>,
    store: Arc<FileCredentialStore>,
    runtime: Runtime,
}

impl Authentication {
    pub fn new() -> io::Result<Self> {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| io::Error::other("HOME が設定されていません"))?;
        Self::with_store(PathBuf::from(home).join(".solo").join("auth.json"))
    }

    pub fn with_store(path: PathBuf) -> io::Result<Self> {
        // genai の依存関係では aws-lc-rs と ring の両方が有効になるため、
        // rustls が最初の TLS 接続時に推測できるようにせず明示的に選択する。
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let config = CodexConfig {
            originator: "solo".into(),
            ..CodexConfig::default()
        };
        Ok(Self {
            auth: Arc::new(CodexAuth::with_config(config)),
            store: Arc::new(FileCredentialStore::new(path)),
            runtime,
        })
    }

    pub fn is_logged_in(&self) -> io::Result<bool> {
        self.store
            .load(OPENAI_CODEX_PROVIDER_ID)
            .map(|credential| credential.is_some())
            .map_err(auth_error)
    }

    pub fn login_device(
        &self,
        announce: impl FnOnce(DeviceLogin),
        cancel: &CancellationToken,
    ) -> io::Result<()> {
        let begin = self
            .runtime
            .block_on(self.auth.begin_device_login())
            .map_err(auth_error)?;
        announce(DeviceLogin {
            verification_url: begin.verification_uri.clone(),
            user_code: begin.user_code.clone(),
        });
        let credential = self.runtime.block_on(async {
            tokio::select! {
                result = self.auth.poll_device_login(&begin) => result.map_err(auth_error),
                _ = cancel.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "ログインを中止しました")),
            }
        })?;
        self.store
            .store(OPENAI_CODEX_PROVIDER_ID, &credential)
            .map_err(auth_error)
    }

    pub fn model(&self, name: String, cancel: CancellationToken) -> io::Result<CodexModel> {
        let resolver = Arc::new(CodexTokenResolver::new(
            self.auth.clone(),
            self.store.clone(),
            OPENAI_CODEX_PROVIDER_ID,
        ));
        let source = Arc::new(ResolverTokenSource::new(resolver));
        Ok(CodexModel {
            name,
            stream: CodexStreamFn::new(source)
                .with_originator("solo")
                .with_user_agent(concat!("solo/", env!("CARGO_PKG_VERSION"))),
            runtime: Builder::new_current_thread().enable_all().build()?,
            cancel,
        })
    }
}

fn auth_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

pub struct CodexModel {
    name: String,
    stream: CodexStreamFn,
    runtime: Runtime,
    cancel: CancellationToken,
}

impl CodexModel {
    fn complete_streaming(
        &mut self,
        messages: &[Message],
        tools: &[ToolSpec],
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ModelOutput, String> {
        let context = LlmContext {
            system_prompt: "あなたは Solo のコーディングエージェントです。必要に応じて提供された tool を使い、作業内容を日本語で報告してください。".into(),
            messages: to_chat_messages(messages),
            tools: tools.iter().map(|spec| {
                Tool::new(spec.name.clone())
                    .with_description(spec.description.clone())
                    .with_schema(spec.parameters.clone())
            }).collect(),
        };
        let request =
            StreamRequest::new(self.name.clone(), context).with_cancellation(self.cancel.clone());
        self.runtime.block_on(async {
            let mut stream = self.stream.stream(request).await;
            while let Some(event) = stream.next().await {
                match event {
                    AssistantMessageEvent::TextDelta { delta, .. } => on_delta(&delta),
                    AssistantMessageEvent::Done { message, .. } => {
                        if message.stop_reason == ProviderStopReason::Length {
                            return Err("モデルの出力上限に達しました".into());
                        }
                        return Ok(ModelOutput {
                            text: message.text(),
                            tool_calls: message
                                .tool_calls()
                                .map(|call| ToolCall {
                                    id: call.id.clone(),
                                    name: call.name.clone(),
                                    arguments: call.arguments.clone(),
                                })
                                .collect(),
                        });
                    }
                    AssistantMessageEvent::Error { error, .. } => {
                        return Err(error
                            .error_message
                            .unwrap_or_else(|| "モデル呼び出しに失敗しました".into()));
                    }
                    _ => {}
                }
            }
            Err("モデルの応答が途中で終了しました".into())
        })
    }
}

impl Model for CodexModel {
    fn complete(
        &mut self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<ModelOutput, String> {
        self.complete_streaming(messages, tools, &mut |_| {})
    }

    fn complete_with_updates(
        &mut self,
        messages: &[Message],
        tools: &[ToolSpec],
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ModelOutput, String> {
        self.complete_streaming(messages, tools, on_delta)
    }
}

pub fn to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|message| match message {
            Message::User { text } => ChatMessage::user(text.clone()),
            Message::Assistant { text, tool_calls } => {
                let mut parts = Vec::new();
                if !text.is_empty() {
                    parts.push(ContentPart::Text(text.clone()));
                }
                parts.extend(tool_calls.iter().map(|call| {
                    ContentPart::ToolCall(genai::chat::ToolCall {
                        call_id: call.id.clone(),
                        fn_name: call.name.clone(),
                        fn_arguments: call.arguments.clone(),
                        thought_signatures: None,
                    })
                }));
                ChatMessage::assistant(MessageContent::from_parts(parts))
            }
            Message::Tool { call_id, result } => {
                ChatMessage::tool(MessageContent::from_tool_responses(vec![
                    ToolResponse::new(call_id.clone(), result.content.clone()),
                ]))
            }
        })
        .collect()
}

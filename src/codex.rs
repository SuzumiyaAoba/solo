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
use std::{io, path::PathBuf, sync::Arc, time::Duration};
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
        Self::with_store(crate::storage::solo_dir("auth.json")?)
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
        let login = async {
            let begin = self.auth.begin_device_login().await.map_err(auth_error)?;
            announce(DeviceLogin {
                verification_url: begin.verification_uri.clone(),
                user_code: begin.user_code.clone(),
            });
            self.auth
                .poll_device_login(&begin)
                .await
                .map_err(auth_error)
        };
        let credential = self
            .runtime
            .block_on(cancel.run_until_cancelled(login))
            .unwrap_or_else(|| {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "ログインを中止しました",
                ))
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
            system_prompt: "あなたは Solo のコーディングエージェントです。必要に応じて提供された tool を使い、作業内容を日本語で報告してください。".into(),
            review_timeout: None,
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
    system_prompt: String,
    review_timeout: Option<Duration>,
}

impl CodexModel {
    pub fn with_review_settings(mut self, system_prompt: &str, timeout: Duration) -> Self {
        self.system_prompt = system_prompt.into();
        self.review_timeout = Some(timeout);
        self
    }

    fn complete_streaming(
        &mut self,
        messages: &[Message],
        tools: &[ToolSpec],
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ModelOutput, String> {
        let context = LlmContext {
            system_prompt: self.system_prompt.clone(),
            messages: to_chat_messages(messages),
            tools: tools
                .iter()
                .map(|spec| {
                    Tool::new(spec.name.clone())
                        .with_description(spec.description.clone())
                        .with_schema(spec.parameters.clone())
                })
                .collect(),
        };
        let mut request =
            StreamRequest::new(self.name.clone(), context).with_cancellation(self.cancel.clone());
        if self.review_timeout.is_some() {
            request = request
                .with_max_retries(0)
                .with_options(genai::chat::ChatOptions::default().with_max_tokens(2048));
        }
        let response = async {
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
        };
        self.runtime.block_on(async {
            match self.review_timeout {
                Some(timeout) => review_with_deadline(response, &self.cancel, timeout).await,
                None => response.await,
            }
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

async fn review_with_deadline<T>(
    response: impl std::future::Future<Output = Result<T, String>>,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<T, String> {
    tokio::select! {
        result = response => result,
        _ = cancel.cancelled() => Err("Auto 判定を中止しました".into()),
        _ = tokio::time::sleep(timeout) => Err("Auto 判定がタイムアウトしました。手動で確認してください。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Instant,
    };

    fn authentication_at(path: PathBuf, base_url: String) -> Authentication {
        let mut auth = Authentication::with_store(path).unwrap();
        auth.auth = Arc::new(CodexAuth::with_config(CodexConfig {
            base_url,
            ..CodexConfig::default()
        }));
        auth
    }

    #[test]
    fn cancelled_login_does_not_start_authentication() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let auth = authentication_at(path.clone(), "http://127.0.0.1:0".into());
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = auth.login_device(
            |_| panic!("cancelled login must not announce a code"),
            &cancel,
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(!path.exists());
    }

    #[test]
    fn cancelling_the_initial_login_request_does_not_wait_for_a_response() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let auth = authentication_at(
            path.clone(),
            format!("http://{}", listener.local_addr().unwrap()),
        );
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let announced = Arc::new(AtomicBool::new(false));
        let worker_announced = announced.clone();
        let (done, result) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = auth.login_device(
                |_| worker_announced.store(true, Ordering::Release),
                &worker_cancel,
            );
            let _ = done.send(result);
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut connection = loop {
            match listener.accept() {
                Ok((stream, _)) => break Some(stream),
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(_) => break None,
            }
        };
        let received_request = connection.as_mut().is_some_and(|stream| {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            stream.read(&mut [0; 4096]).is_ok_and(|bytes| bytes > 0)
        });
        cancel.cancel();
        let completed = result.recv_timeout(Duration::from_secs(1));
        // 旧実装でテストが失敗しても、待機中の通信を閉じて worker を回収する。
        drop(connection);
        drop(listener);
        worker.join().unwrap();
        assert!(
            received_request,
            "the mock server must receive the initial request"
        );
        assert_eq!(
            completed
                .expect("cancellation must end login before any HTTP response")
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(!announced.load(Ordering::Acquire));
        assert!(!path.exists());
    }

    #[test]
    fn deadline_and_cancellation_stop_a_waiting_review() {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let token = CancellationToken::new();
            let result = review_with_deadline(
                std::future::pending::<Result<(), String>>(),
                &token,
                Duration::from_millis(1),
            )
            .await;
            assert!(result.unwrap_err().contains("タイムアウト"));
            token.cancel();
            let result = review_with_deadline(
                std::future::pending::<Result<(), String>>(),
                &token,
                Duration::from_secs(30),
            )
            .await;
            assert!(result.unwrap_err().contains("中止"));
        });
    }
}

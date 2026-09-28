//! 実際のモデル、shell、workspace の書込みは一切呼び出さない。
//! worker がログ I/O を所有し、bounded channel で UI と接続する。
use crate::event::{Emitter, Envelope, Event, Sequencer, SessionId, Usage};
use crate::text::preview;
use async_channel::{Receiver, Sender, TrySendError};
use std::{
    fs,
    io::{self, BufWriter, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

pub const CHANNEL_CAPACITY: usize = 256;

/// UI のシナリオピッカーが並べる一覧。backend::MOCK_SCENARIO_KEYS と同順。
pub const SCENARIOS: [Scenario; 6] = [
    Scenario::Demo,
    Scenario::Threads,
    Scenario::Events10k,
    Scenario::Events100k,
    Scenario::Log100MiB,
    Scenario::Faults,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Demo,
    Threads,
    Events10k,
    Events100k,
    Log100MiB,
    Faults,
}

impl Scenario {
    /// backend::MOCK_SCENARIO_KEYS と同順の永続化キー。
    pub fn key(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::Threads => "threads",
            Self::Events10k => "events-10k",
            Self::Events100k => "events-100k",
            Self::Log100MiB => "log-100mib",
            Self::Faults => "faults",
        }
    }
    /// 永続化キーから復元する。未知キーは None。
    pub fn from_key(key: &str) -> Option<Scenario> {
        SCENARIOS
            .iter()
            .copied()
            .find(|scenario| scenario.key() == key)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Demo => "会話と差分",
            Self::Threads => "ツールとサブエージェント",
            Self::Events10k => "1万イベント",
            Self::Events100k => "10万イベント",
            Self::Log100MiB => "100 MiB ログ",
            Self::Faults => "未知イベント・切断",
        }
    }
    pub fn count(self) -> usize {
        match self {
            Self::Demo => 120,
            Self::Threads => 1,
            Self::Events10k => 10_000,
            Self::Events100k => 100_000,
            Self::Log100MiB => 25_600,
            Self::Faults => 32,
        }
    }
}

pub struct Config {
    pub session_id: SessionId,
    pub title: String,
    pub workspace: String,
    pub prompt: String,
    pub scenario: Scenario,
    pub start_sequence: u64,
    /// Some なら全文ログをこのディレクトリに残す(再起動後も参照可能)。
    /// None はシステムの一時領域で、セッション保存が無い環境向け。
    pub log_dir: Option<PathBuf>,
    pub delay: Duration,
}

impl Config {
    pub fn new(session_id: SessionId, scenario: Scenario) -> Self {
        Self {
            session_id,
            title: scenario.label().into(),
            workspace: "solo".into(),
            prompt: "調査結果をもとに、イベント表示の試作を確認してください。".into(),
            scenario,
            start_sequence: 0,
            log_dir: None,
            delay: if matches!(scenario, Scenario::Demo | Scenario::Faults) {
                Duration::from_millis(18)
            } else {
                Duration::ZERO
            },
        }
    }
}

#[derive(Debug)]
pub enum Delivery {
    Event(Envelope),
    /// 全文ログへのパス。log_dir 指定時はセッションの保存領域に残り、
    /// 未指定(一時領域)では明示的な削除はしない。
    LogOpened(Arc<PathBuf>),
    Error(String),
}

impl From<Envelope> for Delivery {
    fn from(envelope: Envelope) -> Self {
        Self::Event(envelope)
    }
}

/// 0=実行中、1=cancel、2=切断注入、3=window/session 廃棄。
pub struct Controller {
    signal: Arc<AtomicU8>,
    worker: Option<JoinHandle<()>>,
}

impl Controller {
    pub fn cancel(&self) {
        let _ = self
            .signal
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }
    pub fn disconnect(&self) {
        let _ = self
            .signal
            .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// テスト/CLI 専用。UI thread では join しない。
    pub fn shutdown_and_join(mut self) -> thread::Result<()> {
        self.signal.store(3, Ordering::SeqCst);
        self.worker.take().expect("worker exists").join()
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.signal.store(3, Ordering::SeqCst);
    }
}

pub fn start(config: Config) -> io::Result<(Controller, Receiver<Delivery>)> {
    let (sender, receiver) = async_channel::bounded(CHANNEL_CAPACITY);
    let signal = Arc::new(AtomicU8::new(0));
    let worker_signal = signal.clone();
    let worker = thread::Builder::new()
        .name(format!("solo-mock-{}", config.session_id))
        .spawn(move || {
            let mut producer = Producer {
                emitter: Emitter::new(
                    Sequencer::new(
                        config.session_id.clone(),
                        config.start_sequence,
                        format!("turn-{}", config.start_sequence + 1),
                    ),
                    sender,
                ),
                config,
                signal: worker_signal,
            };
            if let Err(error) = producer.run() {
                Producer::deliver(
                    &producer.signal,
                    producer.emitter.sender(),
                    Delivery::Error(format!("疑似ストリームの I/O エラー: {error}")),
                    false,
                );
            }
        })?;
    Ok((
        Controller {
            signal,
            worker: Some(worker),
        },
        receiver,
    ))
}

struct Producer {
    emitter: Emitter<Delivery>,
    config: Config,
    signal: Arc<AtomicU8>,
}

impl Producer {
    /// signal を監視しながら送信する。終了(3)、または interruptible 中の要求で打ち切る。
    fn deliver(
        signal: &AtomicU8,
        sender: &Sender<Delivery>,
        mut delivery: Delivery,
        interruptible: bool,
    ) -> bool {
        loop {
            let state = signal.load(Ordering::SeqCst);
            if state == 3 || (interruptible && state != 0) {
                return false;
            }
            match sender.try_send(delivery) {
                Ok(()) => return true,
                Err(TrySendError::Closed(_)) => return false,
                Err(TrySendError::Full(value)) => {
                    delivery = value;
                    thread::park_timeout(Duration::from_millis(1));
                }
            }
        }
    }

    fn emit(&mut self, event: Event, interruptible: bool) -> bool {
        self.emitter.send_with(event, |sender, delivery| {
            Self::deliver(&self.signal, sender, delivery, interruptible)
        })
    }

    fn emit_envelope(&mut self, event: Envelope, interruptible: bool) -> bool {
        if Self::deliver(
            &self.signal,
            self.emitter.sender(),
            Delivery::Event(event),
            interruptible,
        ) {
            self.emitter.commit();
            true
        } else {
            false
        }
    }

    /// emit に失敗したら中断通知を出す。呼出側は false で終了する。
    fn emit_or_quit(&mut self, event: Event, interruptible: bool) -> bool {
        let sent = self.emit(event, interruptible);
        if !sent {
            self.interrupted();
        }
        sent
    }

    /// 送信失敗時はログを flush してから中断する。Ok(false) = 中断済み。
    fn emit_or_quit_flushed(
        &mut self,
        event: Envelope,
        writer: &mut BufWriter<impl Write>,
    ) -> io::Result<bool> {
        if self.emit_envelope(event, true) {
            return Ok(true);
        }
        writer.flush()?;
        self.interrupted();
        Ok(false)
    }

    fn interrupted(&mut self) {
        let event = match self.signal.load(Ordering::SeqCst) {
            1 => Event::TurnCancelled {
                reason: "疑似ストリームの停止を確認しました".into(),
            },
            2 => Event::Disconnected {
                reason: "接続の切断を注入しました。実行結果は未確認です".into(),
            },
            _ => return,
        };
        self.emit(event, false);
    }

    fn run(&mut self) -> io::Result<()> {
        // SessionCreated と TurnStarted は中止要求より先に必ず順序を確定する。
        if self.config.start_sequence == 0 && !self.emit(Event::SessionCreated {
            title: self.config.title.clone(), workspace_id: self.config.workspace.clone(),
            settings: serde_json::json!({"backend": "mock", "scenario": self.config.scenario.key()}),
        }, false) { return Ok(()); }
        if !self.emit(
            Event::TurnStarted {
                prompt: self.config.prompt.clone(),
            },
            false,
        ) {
            return Ok(());
        }
        if !self.emit(
            Event::ModelRequestStarted {
                provider: "ローカル疑似プロバイダー".into(),
                model: "phase-0".into(),
                request_id: format!("request-{}", self.emitter.sequence()),
            },
            true,
        ) {
            self.interrupted();
            return Ok(());
        }

        if self.config.scenario == Scenario::Threads {
            self.run_thread_demo();
            return Ok(());
        }

        let (file, path) = if let Some(dir) = &self.config.log_dir {
            fs::create_dir_all(dir)?;
            let path = dir.join(format!(
                "log-{}-{}.log",
                self.config.start_sequence,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|at| at.as_millis())
                    .unwrap_or_default()
            ));
            (fs::File::create(&path)?, path)
        } else {
            let file = tempfile::Builder::new()
                .prefix("solo-phase0-")
                .suffix(".log")
                .tempfile()?;
            file.keep().map_err(|error| error.error)?
        };
        let mut writer = BufWriter::new(file);
        if !Self::deliver(
            &self.signal,
            self.emitter.sender(),
            Delivery::LogOpened(Arc::new(path)),
            true,
        ) {
            self.interrupted();
            return Ok(());
        }
        let invocation = format!("mock-check-{}", self.config.start_sequence);
        if !self.emit_or_quit(
            Event::ToolStarted {
                agent_id: None,
                tool: Some("exec".into()),
                invocation_id: invocation.clone(),
                command: "fixture::validate (simulation)".into(),
                cwd: self.config.workspace.clone(),
            },
            true,
        ) {
            return Ok(());
        }
        let message_id = format!("assistant-{}", self.config.start_sequence);
        let demo_text: Vec<&str> = "調査結果をもとに、Phase 0 の作業画面を用意しました。\n\n会話、差分、ログを同じセッションで確認できます。差分タブでは変更行と行番号を、ログタブでは実行状態と出力を確認してください。\n\n長いストリームは、上の「10万イベント」や「100 MiB ログ」から試せます。受信中も日本語の入力やセッションの切替ができます。\n\n未知イベントと切断は、診断を残して結果未確認として表示します。ここに表示する応答・差分・実行結果は、すべて検証用のサンプルです。".graphemes(true).collect();
        let mut offset = 0;
        // 100 MiB scenario は改行を含まない巨大なログを 4 KiB 単位で退避する。
        let large_chunk = "日本語🙂 long log / ".repeat(220);
        let chunk_end = large_chunk.floor_char_boundary(4096);
        let large_chunk = format!(
            "{}{}",
            &large_chunk[..chunk_end],
            " ".repeat(4096 - chunk_end)
        );
        for index in 0..self.config.scenario.count() {
            if self.signal.load(Ordering::SeqCst) != 0 {
                writer.flush()?;
                self.interrupted();
                return Ok(());
            }
            let event = if self.config.scenario == Scenario::Log100MiB || index % 4 == 0 {
                let text = if self.config.scenario == Scenario::Log100MiB {
                    large_chunk.clone()
                } else {
                    format!("[{index:06}] fixture を処理しています / 日本語・絵文字 🧑🏽‍💻\n")
                };
                writer.write_all(text.as_bytes())?;
                let event = Event::Log {
                    level: "info".into(),
                    preview: preview(text.trim_end(), 512),
                    offset,
                    bytes: text.len() as u64,
                };
                offset += text.len() as u64;
                event
            } else {
                let text = if self.config.scenario == Scenario::Demo {
                    let start = (index - index / 4 - 1) * 4;
                    demo_text
                        .get(start..(start + 4).min(demo_text.len()))
                        .map(|part| part.concat())
                        .unwrap_or_default()
                } else {
                    "長いストリームの表示を検証しています。日本語と emoji 🙂。\n".into()
                };
                Event::MessageDelta {
                    message_id: message_id.clone(),
                    text,
                }
            };
            let envelope = self.emitter.peek(event);
            if !self.emit_or_quit_flushed(envelope.clone(), &mut writer)? {
                return Ok(());
            }
            if self.config.scenario == Scenario::Faults && index == 8 {
                // 同一 ID の再送、順序逆転、未知 kind、新 schema、不完全な既知 payload。
                Self::deliver(
                    &self.signal,
                    self.emitter.sender(),
                    Delivery::Event(envelope.clone()),
                    true,
                );
                let mut reversed = envelope;
                reversed.event_id.push_str("-late");
                reversed.sequence -= 1;
                Self::deliver(
                    &self.signal,
                    self.emitter.sender(),
                    Delivery::Event(reversed),
                    true,
                );
                for (version, payload) in [
                    (
                        1,
                        serde_json::json!({"type":"future_tool","details":"保持された未知イベント"}),
                    ),
                    (
                        99,
                        serde_json::json!({"type":"turn_completed","future":true}),
                    ),
                    (1, serde_json::json!({"type":"message_delta","text":42})),
                ] {
                    let mut unknown = self.emitter.peek(Event::TurnFailed {
                        reason: String::new(),
                    });
                    unknown.schema_version = version;
                    unknown.payload = payload;
                    if !self.emit_or_quit_flushed(unknown, &mut writer)? {
                        return Ok(());
                    }
                }
            }
            if !self.config.delay.is_zero() {
                thread::park_timeout(self.config.delay);
            }
        }
        writer.flush()?;
        if self.config.scenario == Scenario::Faults {
            // terminal event なしで sender を落とす。UI が切断として検出する。
            return Ok(());
        }
        if !self.emit_or_quit(Event::DiffUpdated {
            invocation_id: Some(invocation.clone()),
            path: "src/example.rs (fixture)".into(),
            unified_diff: "--- a/src/example.rs\n+++ b/src/example.rs\n@@ -1,4 +1,5 @@\n fn greeting() -> &'static str {\n-    \"Hello\"\n+    // 日本語の表示を確認\n+    \"こんにちは、Solo 🙂\"\n }\n \n".into(),
        }, true) || !self.emit_or_quit(Event::ToolFinished {
            invocation_id: invocation,
            exit_code: 0,
            summary: Some("終了コード 0 · stdout 32 バイト / stderr 0 バイト".into()),
        }, true) {
            return Ok(());
        }
        if self.config.scenario == Scenario::Demo {
            // 分割ブロックの連結・Markdown・折りたたみの表示確認用。
            // 1,024 byte を超える応答を 1 メッセージとして送る。
            let mut markdown = String::from(
                "## 応答の確認\n\n\
                 - 見出し・リスト・コードフェンス・リンクをそのまま描画します\n\
                 - 手順の詳細は [ドキュメント](https://example.com/solo) を参照してください\n\n\
                 ```rust\n",
            );
            for index in 0..56 {
                markdown.push_str(&format!("fn step_{index:02}() -> usize {{ {index} }}\n"));
            }
            markdown.push_str("```\n\n`cargo test` と `cargo clippy` で確認します。\n");
            if !self.emit_or_quit(
                Event::MessageDelta {
                    message_id: format!("assistant-md-{}", self.config.start_sequence),
                    text: markdown,
                },
                true,
            ) {
                return Ok(());
            }
        }
        self.emit_or_quit(
            Event::TurnCompleted {
                reason: "疑似イベントの再生が完了しました。差分とログを確認できます。".into(),
                usage: Usage::default(),
            },
            true,
        );
        Ok(())
    }

    fn run_thread_demo(&mut self) {
        let events = vec![
            Event::MessageDelta { message_id: "thread-plan".into(), text: "画面構成と操作の流れを確認します。調査と検証の進み具合は、この依頼の実行スレッドで確認できます。\n\nこれは表示確認用の疑似シナリオです。".into() },
            Event::ToolStarted { invocation_id: "read".into(), command: "read src/ui/views.rs".into(), cwd: self.config.workspace.clone(), agent_id: None, tool: Some("read".into()) },
            Event::ToolFinished { invocation_id: "read".into(), exit_code: 0, summary: Some("210 行".into()) },
            Event::AgentStarted { agent_id: "research".into(), name: "UI リサーチ".into(), task: "チャンネルの構成とスレッドへの導線を調べる".into(), parent_agent_id: None },
            Event::ToolStarted { invocation_id: "search".into(), command: "search チャンネル src/ui".into(), cwd: self.config.workspace.clone(), agent_id: Some("research".into()), tool: Some("search".into()) },
            Event::ToolFinished { invocation_id: "search".into(), exit_code: 0, summary: Some("8 件の一致".into()) },
            Event::AgentFinished { agent_id: "research".into(), success: true, summary: "プロジェクト別の一覧と、依頼ごとのスレッドを確認しました。".into() },
            Event::AgentStarted { agent_id: "validation".into(), name: "表示の検証".into(), task: "テーマと小さいウィンドウでの表示を確認する".into(), parent_agent_id: None },
            Event::ToolStarted { invocation_id: "test".into(), command: "fixture::check_channel_layout (simulation)".into(), cwd: self.config.workspace.clone(), agent_id: Some("validation".into()), tool: Some("exec".into()) },
            Event::ApprovalRequested { request_id: "approve-test".into(), title: "fixture::check_channel_layout (simulation)".into(), executor: "shell".into(), command: None, details: serde_json::json!({}), invocation_id: Some("test".into()) },
            Event::ApprovalDecided { request_id: "approve-test".into(), accepted: true, source: "ユーザー".into(), invocation_id: Some("test".into()) },
            Event::DiffUpdated { path: "src/ui/views.rs (fixture)".into(), unified_diff: "--- a/src/ui/views.rs\n+++ b/src/ui/views.rs\n@@ -1,3 +1,4 @@\n mod chat;\n+mod thread;\n mod diff;\n".into(), invocation_id: Some("test".into()) },
            Event::ToolFinished { invocation_id: "test".into(), exit_code: 0, summary: Some("終了コード 0 · stdout 64 バイト / stderr 0 バイト".into()) },
            Event::AgentFinished { agent_id: "validation".into(), success: true, summary: "ライト・ダークとコンパクト表示の疑似検証が完了しました。".into() },
            Event::MessageDelta { message_id: "thread-result".into(), text: "チャンネルの会話と実行スレッドの表示を確認しました。\n\n次の依頼を送った後も、以前の依頼にある「スレッドを開く」から実行履歴を参照できます。".into() },
            Event::TurnCompleted { reason: "スレッド表示の疑似シナリオが完了しました。".into(), usage: Usage::default() },
        ];
        for event in events {
            if !self.emit_or_quit(event, true) {
                return;
            }
            if !self.config.delay.is_zero() {
                thread::park_timeout(self.config.delay);
            }
        }
    }
}

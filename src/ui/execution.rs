//! 実行先の選択、ワーカーの起動、セッションとの接続。
use super::*;
use solo::{acp_worker, codex_worker, mock};

/// 会話を開始した実行先。選択中の項目やワーカーの接続寿命とは独立して保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Backend {
    Subscription,
    Acp(String),
    Mock(Scenario),
}

impl Backend {
    pub(super) fn uses_workspace(&self) -> bool {
        !matches!(self, Self::Mock(_))
    }

    /// 保存済み BackendKind から実行中 backend を復元する(表示上の選択には使わない)。
    /// Mock 番号が範囲外になった場合のみ None。
    pub(super) fn from_kind(kind: &BackendKind) -> Option<Self> {
        match kind {
            BackendKind::Subscription => Some(Self::Subscription),
            BackendKind::Acp { id } => Some(Self::Acp(id.clone())),
            BackendKind::Mock { key } => Scenario::from_key(key).map(Self::Mock),
        }
    }
}

pub(super) enum UiController {
    Mock(mock::Controller),
    Subscription(codex_worker::Controller),
    Acp(acp_worker::Controller),
}

impl UiController {
    pub(super) fn cancel(&self) {
        match self {
            Self::Mock(c) => c.cancel(),
            Self::Subscription(c) => c.cancel(),
            Self::Acp(c) => c.cancel(),
        }
    }
    pub(super) fn disconnect(&self) {
        match self {
            Self::Mock(c) => c.disconnect(),
            Self::Subscription(c) => c.disconnect(),
            Self::Acp(c) => c.disconnect(),
        }
    }
}

impl SessionView {
    pub(super) fn uses_workspace(&self) -> bool {
        self.login_only || self.backend.as_ref().is_some_and(Backend::uses_workspace)
    }

    /// 実行開始。ドメイン status はイベントで進むため、ここでは UI 側の接続中表示だけ立てる。
    /// reason は次の TurnStarted/finish が上書きするので触らない。
    fn begin_run(&mut self, cx: &mut Context<Workspace>) {
        self.exec.connecting = true;
        self.metrics.started_at = Some(Instant::now());
        self.metrics.elapsed = None;
        self.unread_result = false;
        self.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
    }

    /// 実行 backend・永続化 kind・選択 index を一括で確定する(片方だけ更新する漏れを防ぐ)。
    fn assign_backend(&mut self, backend: Backend, kind: BackendKind, selected: usize) {
        self.backend = Some(backend);
        self.persist.meta.backend = Some(kind);
        self.selected_backend = selected;
    }

    fn attach_stream<D: Send + 'static>(
        &mut self,
        controller: UiController,
        receiver: async_channel::Receiver<D>,
        wrap: fn(D) -> UiDelivery,
        cx: &mut Context<Workspace>,
    ) {
        self.exec.controller = Some(controller);
        self.exec.stream_generation += 1;
        self.exec.task = Some(Workspace::listen(
            receiver,
            self.model.id.clone(),
            self.exec.stream_generation,
            wrap,
            cx,
        ));
    }
}

impl Workspace {
    pub(super) fn start_selected(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        if prompt.trim().is_empty()
            || self.session_at(index).display_status().is_active()
            || self
                .queue
                .position(&self.session_at(index).model.id)
                .is_some()
        {
            return;
        }
        let backend_kind = self
            .backend_kind_at(self.session_at(index).selected_backend)
            .unwrap_or(BackendKind::Subscription);
        let requested = Backend::from_kind(&backend_kind).unwrap_or(Backend::Subscription);
        if self.session_at(index).model.last_sequence() > 0
            && self.session_at(index).backend.as_ref() != Some(&requested)
        {
            self.start_failed(
                index,
                prompt,
                "実行先を切り替えるには新しいセッションを作成してください".into(),
                cx,
            );
            return;
        }
        // タイトルは各 start_* が config.title へ入れ、SessionCreated イベントで投影される。
        if self.picker_uses_workspace(self.session_at(index).selected_backend)
            && (self.workspace_busy() || !self.queue.is_empty() || self.queue.paused())
        {
            self.enqueue(index, prompt, cx);
            return;
        }
        self.run_backend(index, &backend_kind, prompt, cx);
    }

    /// キュー/選択の BackendKind から実行を起動する。永続化済みの識別なので
    /// index ではなく kind を受け取り、解決できない実行先はエラー扱いにする。
    pub(super) fn run_backend(
        &mut self,
        index: usize,
        kind: &BackendKind,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        self.session_at_mut(index).last_prompt = prompt.clone();
        match kind {
            BackendKind::Subscription => self.start_subscription(index, prompt, cx),
            BackendKind::Acp { id } => {
                match self
                    .acp_agents
                    .iter()
                    .find(|agent| &agent.id == id)
                    .cloned()
                {
                    Some(agent) => self.start_acp(index, agent, prompt, cx),
                    None => self.start_failed(
                        index,
                        prompt,
                        format!("ACP agent {id} が見つかりません"),
                        cx,
                    ),
                }
            }
            BackendKind::Mock { key } => match Scenario::from_key(key) {
                Some(scenario) => self.start_mock(index, scenario, prompt, cx),
                None => self.start_failed(
                    index,
                    prompt,
                    format!("Mock シナリオ {key} が見つかりません"),
                    cx,
                ),
            },
        }
    }

    /// 起動後の共通処理: メッセージを消し、セッション情報を保存して操作状態を更新する。
    fn finish_start(&mut self, index: usize, cx: &mut Context<Self>) {
        self.message.clear();
        self.save_session_meta(index, cx);
        self.sync_controls(cx);
        cx.notify();
    }

    /// 表示中のセッションが対象なら picker の選択を確定した実行先へ合わせて閉じる。
    fn sync_picker(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.selected {
            let selected = self.session_at(index).selected_backend;
            self.scenario_picker.update(cx, |picker, cx| {
                picker.selected = selected;
                picker.close(cx);
            });
        }
    }

    pub(super) fn start_mock(
        &mut self,
        index: usize,
        scenario: Scenario,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        let backend_kind = BackendKind::Mock {
            key: scenario.key().to_owned(),
        };
        let backend_index = self.backend_index(&backend_kind).unwrap_or(0);
        // session を保持しながら self.workspace_path/self.store/self.message に触れるため、
        // sessions の借用をフィールド分割する(session_at_mut だと &mut self 全体になり衝突する)。
        let sessions = &mut self.sessions;
        let session = &mut sessions[index];
        if session.display_status().is_active() {
            return;
        }
        // scenario() など run_backend を通らず直接呼ばれる経路があるため、last_prompt はここで設定する。
        session.last_prompt = prompt.clone();
        let mut config = mock::Config::new(session.model.id.clone(), scenario);
        // タイトルは SessionCreated イベントで投影へ反映されるため、config.title へだけ入れる。
        if session.model.last_sequence() == 0 {
            config.title = task_title(&prompt);
        } else {
            config.title = session.model.title().to_owned();
        }
        config.prompt = prompt.clone();
        config.start_sequence = session.model.last_sequence();
        config.workspace = self.workspace_path.clone();
        // 全文ログはセッションの保存領域へ書き、再起動後も参照できるようにする。
        if let Some(store) = self.store.clone() {
            if let Err(error) = session.open_or_create(&store)
                && self.message.is_empty()
            {
                self.message = format!("セッションを保存できません: {error}");
            }
            config.log_dir = session.persist.file.as_ref().map(|file| file.log_dir());
        }
        let (controller, receiver) = match mock::start(config) {
            Ok(stream) => stream,
            Err(error) => {
                self.start_failed(
                    index,
                    prompt,
                    format!("疑似ストリームを開始できませんでした: {error}"),
                    cx,
                );
                return;
            }
        };
        session.begin_run(cx);
        session.assign_backend(Backend::Mock(scenario), backend_kind, backend_index);
        session.attach_stream(
            UiController::Mock(controller),
            receiver,
            UiDelivery::Mock,
            cx,
        );
        self.sync_picker(index, cx);
        self.finish_start(index, cx);
    }

    fn start_subscription(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        // config 組み立てで self.workspace_path を読むため、session の借用前に取り出す。
        let workspace = PathBuf::from(&self.workspace_path);
        let session = self.session_at_mut(index);
        if session.display_status().is_active() {
            return;
        }
        let config = codex_worker::Config {
            session_id: session.model.id.clone(),
            title: if session.model.last_sequence() == 0 {
                task_title(&prompt)
            } else {
                session.model.title().to_owned()
            },
            workspace,
            prompt: prompt.clone(),
            history: session.history.clone(),
            start_sequence: session.model.last_sequence(),
            model: std::env::var("SOLO_MODEL").unwrap_or_default(),
        };
        let (controller, receiver) = match codex_worker::start(config) {
            Ok(stream) => stream,
            Err(error) => {
                self.start_failed(
                    index,
                    prompt,
                    format!("エージェントを開始できませんでした: {error}"),
                    cx,
                );
                return;
            }
        };
        session.begin_run(cx);
        session.login = None;
        session.login_only = false;
        session.approval = None;
        session.exec.provider_hint = Some("Codex / ChatGPT Subscription".into());
        session.assign_backend(Backend::Subscription, BackendKind::Subscription, 0);
        session.attach_stream(
            UiController::Subscription(controller),
            receiver,
            UiDelivery::Subscription,
            cx,
        );
        self.sync_picker(index, cx);
        self.finish_start(index, cx);
    }

    pub(super) fn start_login(&mut self, cx: &mut Context<Self>) {
        let session = self.session();
        if session.display_status().is_active()
            || self.queue.position(&session.model.id).is_some()
            || session.selected_backend != 0
            || session
                .backend
                .as_ref()
                .is_some_and(|backend| *backend != Backend::Subscription)
        {
            return;
        }
        let (controller, receiver) = match codex_worker::start_login() {
            Ok(stream) => stream,
            Err(error) => {
                self.message = format!("ChatGPT ログインを開始できませんでした: {error}");
                cx.notify();
                return;
            }
        };
        let session = self.session_mut();
        session.begin_run(cx);
        session.exec.provider_hint = Some("ChatGPT ログイン待ち".into());
        session.login = None;
        session.login_only = true;
        session.attach_stream(
            UiController::Subscription(controller),
            receiver,
            UiDelivery::Subscription,
            cx,
        );
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn start_acp(
        &mut self,
        index: usize,
        agent: AgentProfile,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        let backend_index = self
            .backend_index(&BackendKind::Acp {
                id: agent.id.clone(),
            })
            .unwrap_or(0);
        // config 組み立てで self.workspace_path を読むため、session の借用前に取り出す。
        let workspace = PathBuf::from(&self.workspace_path);
        let session = self.session_at_mut(index);
        if session.display_status().is_active() {
            return;
        }
        if let Some(UiController::Acp(controller)) = &session.exec.controller {
            if let Err(error) = controller.prompt(prompt.clone()) {
                self.start_failed(index, prompt, error.to_string(), cx);
                return;
            }
        } else {
            if session.model.turn_id().is_some() {
                self.start_failed(
                    index,
                    prompt,
                    "ACP の接続が終了しました。続けるには新しいチャンネルを作成してください".into(),
                    cx,
                );
                return;
            }
            let config = acp_worker::Config {
                local_session_id: session.model.id.clone(),
                title: if session.model.last_sequence() == 0 {
                    task_title(&prompt)
                } else {
                    session.model.title().to_owned()
                },
                workspace,
                profile: agent.clone(),
                start_sequence: session.model.last_sequence(),
            };
            let (controller, receiver) = match acp_worker::start(config) {
                Ok(stream) => stream,
                Err(error) => {
                    self.start_failed(
                        index,
                        prompt,
                        format!("ACP agent を起動できません: {error}"),
                        cx,
                    );
                    return;
                }
            };
            if let Err(error) = controller.prompt(prompt.clone()) {
                self.start_failed(index, prompt, error.to_string(), cx);
                return;
            }
            session.attach_stream(UiController::Acp(controller), receiver, UiDelivery::Acp, cx);
        }
        session.begin_run(cx);
        session.exec.provider_hint = Some(format!("ACP / {}", agent.name));
        session.assign_backend(
            Backend::Acp(agent.id.clone()),
            BackendKind::Acp {
                id: agent.id.clone(),
            },
            backend_index,
        );
        self.finish_start(index, cx);
    }

    pub(super) fn scenario(&mut self, scenario: Scenario, cx: &mut Context<Self>) {
        let session = self.session();
        if session.display_status().is_active() || self.queue.position(&session.model.id).is_some()
        {
            return;
        }
        if session.uses_workspace() {
            self.message = "デモを試すには新しいチャンネルを作成してください".into();
            cx.notify();
            return;
        }
        self.start_mock(
            self.selected,
            scenario,
            format!("{}の表示と操作を検証します。", scenario.label()),
            cx,
        );
    }

    fn start_failed(
        &mut self,
        index: usize,
        prompt: String,
        message: String,
        cx: &mut Context<Self>,
    ) {
        self.session_at_mut(index).composer.update(cx, |input, cx| {
            input.set_value(prompt, cx);
            cx.notify();
        });
        self.message = message;
        cx.notify();
    }
}

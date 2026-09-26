//! 実行先の選択、ワーカーの起動、セッションとの接続。
use super::*;
use solo::{acp_worker, mock, subscription_worker};

/// 会話を開始した実行先。選択中の項目やワーカーの接続寿命とは独立して保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Backend {
    Subscription,
    Acp(String),
    Mock,
}

impl Backend {
    pub(super) fn uses_workspace(&self) -> bool {
        !matches!(self, Self::Mock)
    }
}

pub(super) enum UiController {
    Mock(mock::Controller),
    Subscription(subscription_worker::Controller),
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

    fn begin_run(&mut self, cx: &mut Context<Workspace>) {
        self.model.status = Status::Connecting;
        self.model.reason.clear();
        self.started_at = Some(Instant::now());
        self.elapsed = None;
        self.unread_result = false;
        self.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
    }

    fn attach_stream<D: Send + 'static>(
        &mut self,
        controller: UiController,
        receiver: async_channel::Receiver<D>,
        wrap: fn(D) -> UiDelivery,
        cx: &mut Context<Workspace>,
    ) {
        self.controller = Some(controller);
        self.stream_generation += 1;
        self.task = Some(Workspace::listen(
            receiver,
            self.model.id.clone(),
            self.stream_generation,
            wrap,
            cx,
        ));
    }
}

impl Workspace {
    pub(super) fn start_selected(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        if prompt.trim().is_empty()
            || self.sessions[index].model.status.is_active()
            || self
                .queue
                .position(&self.sessions[index].model.id)
                .is_some()
        {
            return;
        }
        let selected = self.sessions[index].selected_backend;
        let requested = if selected == 0 {
            Backend::Subscription
        } else if let Some(agent) = self.acp_agents.get(selected - 1) {
            Backend::Acp(agent.id.clone())
        } else {
            Backend::Mock
        };
        if self.sessions[index].model.last_sequence > 0
            && self.sessions[index].backend.as_ref() != Some(&requested)
        {
            self.start_failed(
                index,
                prompt,
                "実行先を切り替えるには新しいセッションを作成してください".into(),
                cx,
            );
            return;
        }
        if self.sessions[index].model.last_sequence == 0 {
            self.sessions[index].model.title = task_title(&prompt);
        }
        if selected <= self.acp_agents.len()
            && (self.workspace_busy() || !self.queue.is_empty() || self.queue.paused)
        {
            self.enqueue(index, prompt, cx);
            return;
        }
        self.run_backend(index, selected, prompt, cx);
    }

    pub(super) fn run_backend(
        &mut self,
        index: usize,
        selected: usize,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        self.sessions[index].last_prompt = prompt.clone();
        if selected == 0 {
            self.start_subscription(index, prompt, cx);
        } else if let Some(agent) = self.acp_agents.get(selected - 1).cloned() {
            self.start_acp(index, agent, prompt, cx);
        } else {
            self.start_mock(
                index,
                SCENARIOS[selected - 1 - self.acp_agents.len()],
                prompt,
                cx,
            );
        }
    }

    pub(super) fn start_mock(
        &mut self,
        index: usize,
        scenario: Scenario,
        prompt: String,
        cx: &mut Context<Self>,
    ) {
        let session = &mut self.sessions[index];
        if session.model.status.is_active() {
            return;
        }
        session.last_prompt = prompt.clone();
        let mut config = mock::Config::new(&session.model.id, scenario);
        if session.model.last_sequence == 0 {
            session.model.title = task_title(&prompt);
        }
        config.title = session.model.title.clone();
        config.prompt = prompt.clone();
        config.start_sequence = session.model.last_sequence;
        config.workspace = self.workspace_path.clone();
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
        session.backend = Some(Backend::Mock);
        session.selected_backend = 1
            + self.acp_agents.len()
            + SCENARIOS
                .iter()
                .position(|item| *item == scenario)
                .unwrap_or(0);
        if index == self.selected {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.selected = session.selected_backend;
                picker.close(cx);
            });
        }
        session.attach_stream(
            UiController::Mock(controller),
            receiver,
            UiDelivery::Mock,
            cx,
        );
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn start_subscription(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        if session.model.status.is_active() {
            return;
        }
        let config = subscription_worker::Config {
            session_id: session.model.id.clone(),
            title: session.model.title.clone(),
            workspace: PathBuf::from(&self.workspace_path),
            prompt: prompt.clone(),
            history: session.history.clone(),
            start_sequence: session.model.last_sequence,
            model: std::env::var("SOLO_MODEL").unwrap_or_default(),
        };
        let (controller, receiver) = match subscription_worker::start(config) {
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
        session.backend = Some(Backend::Subscription);
        session.selected_backend = 0;
        if index == self.selected {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.selected = 0;
                picker.close(cx);
            });
        }
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

    pub(super) fn start_login(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if session.model.status.is_active()
            || self.queue.position(&session.model.id).is_some()
            || session.selected_backend != 0
            || session
                .backend
                .as_ref()
                .is_some_and(|backend| *backend != Backend::Subscription)
        {
            return;
        }
        let (controller, receiver) = match subscription_worker::start_login() {
            Ok(stream) => stream,
            Err(error) => {
                self.message = format!("ChatGPT ログインを開始できませんでした: {error}");
                cx.notify();
                return;
            }
        };
        session.begin_run(cx);
        session.model.provider = "ChatGPT ログイン待ち".into();
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
        let session = &mut self.sessions[index];
        if session.model.status.is_active() {
            return;
        }
        if let Some(UiController::Acp(controller)) = &session.controller {
            if let Err(error) = controller.prompt(prompt.clone()) {
                self.start_failed(index, prompt, error.to_string(), cx);
                return;
            }
        } else {
            if session.model.turn_id.is_some() {
                self.start_failed(
                    index,
                    prompt,
                    "ACP の接続が終了しました。続けるには新しいタスクを作成してください".into(),
                    cx,
                );
                return;
            }
            let config = acp_worker::Config {
                local_session_id: session.model.id.clone(),
                title: session.model.title.clone(),
                workspace: PathBuf::from(&self.workspace_path),
                profile: agent.clone(),
                start_sequence: session.model.last_sequence,
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
        session.model.provider = format!("ACP / {}", agent.name);
        session.backend = Some(Backend::Acp(agent.id.clone()));
        session.selected_backend = 1 + self
            .acp_agents
            .iter()
            .position(|profile| profile.id == agent.id)
            .unwrap_or(0);
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    pub(super) fn scenario(&mut self, scenario: Scenario, cx: &mut Context<Self>) {
        let session = &self.sessions[self.selected];
        if session.model.status.is_active() || self.queue.position(&session.model.id).is_some() {
            return;
        }
        if session.uses_workspace() {
            self.message = "デモを試すには新しいタスクを作成してください".into();
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
        self.sessions[index].composer.update(cx, |input, cx| {
            input.set_value(prompt, cx);
            cx.notify();
        });
        self.message = message;
        cx.notify();
    }
}

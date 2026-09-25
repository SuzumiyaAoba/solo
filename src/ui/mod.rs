mod smoke;
mod views;

use gpui_kit::component::message_scroller::MessageScrollerState;
use gpui_kit::{prelude::*, *};
use solo::design::{
    self as ds, Button, ButtonVariant, ColorScheme, ControlSize, DesignAssets, Icon, Select,
    Submitted, TextInput as Composer, ToastHost, Tone, space, typography,
};
use solo::{
    acp::{self, AgentProfile},
    acp_worker::{self, Delivery as AcpDelivery},
    codex_subscription::DeviceLogin,
    harness::Message,
    mock::{self, Config, Delivery as MockDelivery, FRAME_BATCH, FRAME_INTERVAL, Scenario},
    projection::{DiffKind, Session, Speaker, Status},
    subscription_worker::{self, Delivery as SubscriptionDelivery},
};
use std::{path::PathBuf, sync::Arc, time::Instant};
use tempfile::TempPath;

const SCENARIOS: [Scenario; 5] = [
    Scenario::Demo,
    Scenario::Events10k,
    Scenario::Events100k,
    Scenario::Log100MiB,
    Scenario::Faults,
];

enum UiController {
    Mock(mock::Controller),
    Subscription(subscription_worker::Controller),
    Acp(acp_worker::Controller),
}

impl UiController {
    fn cancel(&self) {
        match self {
            Self::Mock(c) => c.cancel(),
            Self::Subscription(c) => c.cancel(),
            Self::Acp(c) => c.cancel(),
        }
    }
    fn disconnect(&self) {
        match self {
            Self::Mock(c) => c.disconnect(),
            Self::Subscription(c) => c.disconnect(),
            Self::Acp(c) => c.disconnect(),
        }
    }
}

enum UiDelivery {
    Mock(MockDelivery),
    Subscription(SubscriptionDelivery),
    Acp(AcpDelivery),
}

struct PendingApproval {
    method: String,
    params: serde_json::Value,
    reply: async_channel::Sender<bool>,
}

actions!(
    solo_app,
    [
        Quit,
        CloseWindow,
        NewSession,
        ToggleTheme,
        ShowChat,
        ShowDiff,
        ShowLogs
    ]
);

pub fn run() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "Solo\n  --light     ライトテーマで起動\n  --compact   小さいウィンドウで起動\n  --smoke     実画面の検証後に終了"
        );
        return;
    }
    let smoke = args.iter().any(|arg| arg == "--smoke");
    let light = args.iter().any(|arg| arg == "--light");
    let compact = args.iter().any(|arg| arg == "--compact");
    gpui_kit::application()
        .with_assets(DesignAssets)
        .run(move |cx| {
            ds::init(cx);
            if light {
                ds::set_theme(ColorScheme::Light, cx);
            }
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-w", CloseWindow, None),
                KeyBinding::new("cmd-n", NewSession, None),
                KeyBinding::new("cmd-shift-l", ToggleTheme, None),
                KeyBinding::new("cmd-1", ShowChat, None),
                KeyBinding::new("cmd-2", ShowDiff, None),
                KeyBinding::new("cmd-3", ShowLogs, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let window_size = if compact {
                size(px(820.), px(620.))
            } else {
                size(px(1240.), px(840.))
            };
            let bounds = Bounds::centered(None, window_size, cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(820.), px(620.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Solo".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(16.), px(16.))),
                    }),
                    app_id: Some("dev.solo.app".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Workspace::new(smoke, window, cx));
                    cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
                },
            )
            .expect("Solo の window を開けませんでした");
            cx.activate(true);
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Chat,
    Diff,
    Logs,
}

struct SessionView {
    model: Session,
    composer: Entity<Composer>,
    chat_list: Entity<MessageScrollerState>,
    log_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    diff_index: usize,
    tab: Tab,
    follow_logs: bool,
    artifacts: Vec<Arc<TempPath>>,
    controller: Option<UiController>,
    history: Vec<Message>,
    login: Option<DeviceLogin>,
    login_only: bool,
    approval: Option<PendingApproval>,
    is_subscription: bool,
    is_acp: bool,
    backend_id: Option<String>,
    acp_session_id: Option<String>,
    selected_backend: usize,
    task: Option<Task<()>>,
    stream_generation: u64,
    _input_subscription: Subscription,
    batches: u64,
    max_batch_ms: f64,
}

struct Workspace {
    sessions: Vec<SessionView>,
    selected: usize,
    serial: u64,
    message: String,
    workspace_path: String,
    workspace_name: String,
    scenario_picker: Entity<Select>,
    acp_agents: Vec<AgentProfile>,
    _picker_subscription: Subscription,
    toast: Entity<ToastHost>,
    show_metrics: bool,
    rendered: usize,
}

impl Workspace {
    fn new(smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = std::env::current_dir().unwrap_or_default();
        let workspace_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Workspace".into());
        let (acp_agents, config_error) = match acp::load_agents(&path) {
            Ok(agents) => (agents, None),
            Err(error) => (
                Vec::new(),
                Some(format!(".solo/agents.json を読み込めません: {error}")),
            ),
        };
        let choices = std::iter::once("OpenAI Subscription".to_owned())
            .chain(
                acp_agents
                    .iter()
                    .map(|agent| format!("ACP: {}", agent.name)),
            )
            .chain(SCENARIOS.iter().map(|scenario| scenario.label().to_owned()))
            .collect::<Vec<_>>();
        let scenario_picker = cx.new(|cx| Select::new(choices, 0, window, cx));
        let picker_subscription = cx.subscribe(
            &scenario_picker,
            |this, _, selected: &solo::design::SelectionChanged, cx| {
                this.sessions[this.selected].selected_backend = selected.index;
                cx.notify();
            },
        );
        let mut this = Self {
            sessions: Vec::new(),
            selected: 0,
            serial: 0,
            message: String::new(),
            workspace_name,
            workspace_path: path.display().to_string(),
            scenario_picker,
            acp_agents,
            _picker_subscription: picker_subscription,
            toast: cx.new(ToastHost::new),
            show_metrics: false,
            rendered: 0,
        };
        this.new_session(window, cx);
        if let Some(error) = config_error {
            this.message = error;
        }
        window.focus(&this.sessions[0].composer.focus_handle(cx), cx);
        if smoke {
            this.start_mock(
                0,
                Scenario::Demo,
                "Phase 0 の表示を確認してください。".into(),
                cx,
            );
            smoke::start(window, cx);
        }
        this
    }

    fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sessions.len() >= 8 {
            self.message = "最大8セッションです。不要なセッションを閉じてください。".into();
            cx.notify();
            return;
        }
        self.serial += 1;
        let id = format!("session-{}", self.serial);
        let composer = cx.new(|cx| {
            Composer::multiline(window, cx)
                .control_size(ControlSize::Large)
                .clear_on_submit(true)
        });
        let callback_id = id.clone();
        let subscription = cx.subscribe(&composer, move |this, _, submitted: &Submitted, cx| {
            if let Some(index) = this.sessions.iter().position(|s| s.model.id == callback_id) {
                this.start_selected(index, submitted.0.clone(), cx);
            }
        });
        let mut model = Session::new(id, format!("新しいセッション {}", self.serial));
        model.provider = "Codex / ChatGPT Subscription".into();
        self.sessions.push(SessionView {
            model,
            composer,
            chat_list: cx.new(|cx| MessageScrollerState::new(0, cx)),
            log_scroll: UniformListScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_index: 0,
            tab: Tab::Chat,
            follow_logs: true,
            artifacts: Vec::new(),
            controller: None,
            history: Vec::new(),
            login: None,
            login_only: false,
            approval: None,
            is_subscription: false,
            is_acp: false,
            backend_id: None,
            acp_session_id: None,
            selected_backend: 0,
            task: None,
            stream_generation: 0,
            _input_subscription: subscription,
            batches: 0,
            max_batch_ms: 0.,
        });
        self.selected = self.sessions.len() - 1;
        self.scenario_picker.update(cx, |picker, cx| {
            picker.selected = 0;
            picker.close(cx);
        });
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn start_selected(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        let selected = self.scenario_picker.read(cx).selected;
        if selected <= self.acp_agents.len()
            && self.sessions.iter().enumerate().any(|(other, session)| {
                other != index
                    && session.model.status.is_active()
                    && (session.is_subscription || session.is_acp)
            })
        {
            self.sessions[index].composer.update(cx, |input, cx| {
                input.set_value(prompt.clone(), cx);
                cx.notify();
            });
            self.message =
                "この workspace では別の agent が実行中です。完了後に開始してください".into();
            cx.notify();
            return;
        }
        let requested = if selected == 0 {
            "subscription".to_owned()
        } else if let Some(agent) = self.acp_agents.get(selected - 1) {
            format!("acp:{}", agent.id)
        } else {
            "mock".to_owned()
        };
        if self.sessions[index].model.last_sequence > 0
            && self.sessions[index].backend_id.as_deref() != Some(requested.as_str())
        {
            self.sessions[index].composer.update(cx, |input, cx| {
                input.set_value(prompt.clone(), cx);
                cx.notify();
            });
            self.message = "実行先を切り替えるには新しいセッションを作成してください".into();
            cx.notify();
            return;
        }
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

    fn start_mock(
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
        let mut config = Config::new(&session.model.id, scenario);
        config.title = format!(
            "{} / {}",
            session.model.id.trim_start_matches("session-"),
            scenario.label()
        );
        config.prompt = prompt.clone();
        config.start_sequence = session.model.last_sequence;
        config.workspace = self.workspace_path.clone();
        let (controller, receiver) = match mock::start(config) {
            Ok(stream) => stream,
            Err(error) => {
                session.composer.update(cx, |input, cx| {
                    input.set_value(prompt.clone(), cx);
                    cx.notify();
                });
                self.message = format!("疑似ストリームを開始できませんでした: {error}");
                cx.notify();
                return;
            }
        };
        session.model.status = Status::Connecting;
        session.model.reason.clear();
        session.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
        session.controller = Some(UiController::Mock(controller));
        session.is_subscription = false;
        session.is_acp = false;
        session.backend_id = Some("mock".into());
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
        let id = session.model.id.clone();
        session.stream_generation += 1;
        session.task = Some(Self::listen(
            receiver,
            id,
            session.stream_generation,
            UiDelivery::Mock,
            cx,
        ));
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
                session.composer.update(cx, |input, cx| {
                    input.set_value(prompt.clone(), cx);
                    cx.notify();
                });
                self.message = format!("Solo ハーネスを開始できませんでした: {error}");
                cx.notify();
                return;
            }
        };
        session.model.status = Status::Connecting;
        session.model.reason.clear();
        session.login = None;
        session.login_only = false;
        session.approval = None;
        session.is_subscription = true;
        session.is_acp = false;
        session.backend_id = Some("subscription".into());
        session.selected_backend = 0;
        if index == self.selected {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.selected = 0;
                picker.close(cx);
            });
        }
        session.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
        session.controller = Some(UiController::Subscription(controller));
        let id = session.model.id.clone();
        session.stream_generation += 1;
        session.task = Some(Self::listen(
            receiver,
            id,
            session.stream_generation,
            UiDelivery::Subscription,
            cx,
        ));
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn start_login(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if session.model.status.is_active()
            || session.selected_backend != 0
            || session
                .backend_id
                .as_deref()
                .is_some_and(|id| id != "subscription")
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
        session.model.status = Status::Connecting;
        session.model.reason.clear();
        session.model.provider = "ChatGPT ログイン待ち".into();
        session.login = None;
        session.login_only = true;
        session.is_subscription = true;
        session.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
        session.controller = Some(UiController::Subscription(controller));
        let id = session.model.id.clone();
        session.stream_generation += 1;
        session.task = Some(Self::listen(
            receiver,
            id,
            session.stream_generation,
            UiDelivery::Subscription,
            cx,
        ));
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
                session.composer.update(cx, |input, cx| {
                    input.set_value(prompt.clone(), cx);
                    cx.notify();
                });
                self.message = error.to_string();
                cx.notify();
                return;
            }
        } else {
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
                    session.composer.update(cx, |input, cx| {
                        input.set_value(prompt.clone(), cx);
                        cx.notify();
                    });
                    self.message = format!("ACP agent を起動できません: {error}");
                    cx.notify();
                    return;
                }
            };
            if let Err(error) = controller.prompt(prompt.clone()) {
                self.message = error.to_string();
                cx.notify();
                return;
            }
            let id = session.model.id.clone();
            session.stream_generation += 1;
            session.task = Some(Self::listen(
                receiver,
                id,
                session.stream_generation,
                UiDelivery::Acp,
                cx,
            ));
            session.controller = Some(UiController::Acp(controller));
        }
        session.model.status = Status::Connecting;
        session.model.reason.clear();
        session.model.provider = format!("ACP / {}", agent.name);
        session.is_subscription = false;
        session.is_acp = true;
        session.backend_id = Some(format!("acp:{}", agent.id));
        session.selected_backend = 1 + self
            .acp_agents
            .iter()
            .position(|profile| profile.id == agent.id)
            .unwrap_or(0);
        session.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn listen<D: Send + 'static>(
        receiver: async_channel::Receiver<D>,
        id: String,
        generation: u64,
        wrap: fn(D) -> UiDelivery,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let first = match receiver.recv().await {
                    Ok(event) => event,
                    Err(_) => {
                        let _ = this.update(cx, |this, cx| {
                            this.consume(&id, generation, Vec::new(), true, cx)
                        });
                        break;
                    }
                };
                // 空のときは recv で待つ。入力や描画を阻害せず、delta をフレーム単位でまとめる。
                cx.background_executor().timer(FRAME_INTERVAL).await;
                let mut batch = Vec::with_capacity(FRAME_BATCH);
                batch.push(wrap(first));
                while batch.len() < FRAME_BATCH {
                    match receiver.try_recv() {
                        Ok(event) => batch.push(wrap(event)),
                        Err(_) => break,
                    }
                }
                if this
                    .update(cx, |this, cx| {
                        this.consume(&id, generation, batch, false, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn consume(
        &mut self,
        id: &str,
        generation: u64,
        batch: Vec<UiDelivery>,
        closed: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.model.id == id && session.stream_generation == generation)
        else {
            return;
        };
        let start = Instant::now();
        let old_len = session.model.chat.len();
        let old_discarded = session.model.chat_discarded;
        for delivery in batch {
            match delivery {
                UiDelivery::Mock(MockDelivery::Event(event))
                | UiDelivery::Subscription(SubscriptionDelivery::Event(event))
                | UiDelivery::Acp(AcpDelivery::Event(event)) => {
                    session.model.apply(event);
                }
                UiDelivery::Mock(MockDelivery::LogOpened(path)) => session.artifacts.push(path),
                UiDelivery::Subscription(SubscriptionDelivery::History(messages)) => {
                    session.history = messages;
                    session.login = None;
                }
                UiDelivery::Subscription(SubscriptionDelivery::Login(login)) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(login.user_code.clone()));
                    cx.open_url(&login.verification_url);
                    session.login = Some(login);
                    session.model.provider = "Codex / ChatGPT ログイン待ち".into();
                }
                UiDelivery::Subscription(SubscriptionDelivery::Authenticated) => {
                    if session.login_only {
                        session.model.status = Status::Idle;
                        session.model.reason.clear();
                        session.model.provider = "ChatGPT ログイン済み / Solo".into();
                        session.login = None;
                        session.login_only = false;
                        session.is_subscription = false;
                        session.controller = None;
                    } else {
                        session.login = None;
                        session.model.provider = "OpenAI Codex / Solo".into();
                    }
                }
                UiDelivery::Subscription(SubscriptionDelivery::LoginCancelled) => {
                    if session.login_only {
                        session.model.status = Status::Idle;
                        session.model.provider = "OpenAI Codex / Solo".into();
                        session.login = None;
                        session.login_only = false;
                        session.is_subscription = false;
                        session.controller = None;
                    }
                }
                UiDelivery::Subscription(SubscriptionDelivery::Approval {
                    method,
                    params,
                    reply,
                }) => {
                    session.approval = Some(PendingApproval {
                        method,
                        params,
                        reply,
                    });
                }
                UiDelivery::Acp(AcpDelivery::Approval {
                    method,
                    params,
                    reply,
                }) => {
                    session.approval = Some(PendingApproval {
                        method,
                        params,
                        reply,
                    });
                }
                UiDelivery::Acp(AcpDelivery::SessionId(id)) => session.acp_session_id = Some(id),
                UiDelivery::Mock(MockDelivery::Error(error))
                | UiDelivery::Subscription(SubscriptionDelivery::Error(error)) => {
                    session.model.status = Status::Failed;
                    session.model.reason = error;
                    if session.login_only {
                        session.login_only = false;
                        session.is_subscription = false;
                        session.controller = None;
                    }
                }
                UiDelivery::Acp(AcpDelivery::Error(error)) => {
                    session.model.status = if session.model.status == Status::Cancelling {
                        Status::Cancelled
                    } else {
                        Status::Failed
                    };
                    session.model.reason = error;
                    session.controller = None;
                }
            }
        }
        if closed {
            session.model.transport_closed();
            if session.is_acp {
                session.controller = None;
            }
        }
        if !session.model.status.is_active() {
            session.login = None;
            session.approval = None;
        }
        let discarded = session.model.chat_discarded - old_discarded;
        if discarded > 0 {
            session
                .chat_list
                .update(cx, |list, cx| list.splice(0..discarded.min(old_len), 0, cx));
        }
        let old_len = old_len.saturating_sub(discarded);
        let from = old_len.saturating_sub(1);
        session.chat_list.update(cx, |list, cx| {
            list.splice(from..old_len, session.model.chat.len() - from, cx)
        });
        if session.follow_logs && !session.model.logs.is_empty() {
            session
                .log_scroll
                .scroll_to_item(session.model.logs.len() - 1, ScrollStrategy::Bottom);
        }
        let can_submit = !session.model.status.is_active();
        if session.composer.read(cx).can_submit != can_submit {
            session.composer.update(cx, |input, cx| {
                input.can_submit = can_submit;
                cx.notify();
            });
        }
        session.batches += 1;
        session.max_batch_ms = session
            .max_batch_ms
            .max(start.elapsed().as_secs_f64() * 1000.);
        self.sync_controls(cx);
        cx.notify();
    }

    fn scenario(&mut self, scenario: Scenario, cx: &mut Context<Self>) {
        if self.sessions[self.selected].model.status.is_active() {
            return;
        }
        self.start_mock(
            self.selected,
            scenario,
            format!("{}の表示と操作を検証します。", scenario.label()),
            cx,
        );
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let active = self.sessions[self.selected].model.status.is_active();
        if self.scenario_picker.read(cx).disabled != active {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.disabled = active;
                picker.close(cx);
            });
        }
    }

    fn select_session(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.model.id == id)
        else {
            return;
        };
        self.selected = index;
        self.scenario_picker.update(cx, |picker, cx| {
            picker.selected = self.sessions[index].selected_backend;
            picker.close(cx);
        });
        self.sync_controls(cx);
        window.focus(&self.sessions[index].composer.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sessions.remove(self.selected);
        self.selected = self.selected.saturating_sub(1);
        if self.sessions.is_empty() {
            self.new_session(window, cx);
        }
        self.message.clear();
        let id = self.sessions[self.selected].model.id.clone();
        self.select_session(&id, window, cx);
    }

    fn show_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.sessions[self.selected].tab = tab;
        self.scenario_picker
            .update(cx, |picker, cx| picker.close(cx));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if session.model.status.is_active()
            && let Some(controller) = &session.controller
        {
            controller.cancel();
            if let Some(approval) = session.approval.take() {
                let _ = approval.reply.try_send(false);
            }
            session.model.status = Status::Cancelling;
            cx.notify();
        }
    }

    fn answer_approval(&mut self, accepted: bool, cx: &mut Context<Self>) {
        if let Some(approval) = self.sessions[self.selected].approval.take() {
            let _ = approval.reply.try_send(accepted);
            cx.notify();
        }
    }

    fn copy(&self, text: String, message: &'static str, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast
            .update(cx, |toast, cx| toast.push(message, Tone::Success, cx));
    }
}

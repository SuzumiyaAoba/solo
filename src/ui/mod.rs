mod approval_modes_smoke;
mod approvals;
mod command_rules;
mod execution;
mod overview;
mod projects;
mod projects_smoke;
mod smoke;
mod stream;
mod views;
mod workflow;
mod workflow_smoke;

use approvals::PendingApproval;
use execution::{Backend, UiController};
use stream::UiDelivery;

use gpui_kit::component::message_scroller::MessageScrollerState;
use gpui_kit::{prelude::*, *};
use solo::design::{
    self as ds, Button, ButtonVariant, ColorScheme, ControlSize, DesignAssets, Icon, Select,
    Submitted, TextInput as Composer, ToastHost, Tone, space, typography,
};
use solo::{
    acp::{self, AgentProfile},
    approval::{ApprovalPlan, ApprovalRequest},
    auto_approval::{self, Assessment, CodexReviewer, Reviewer},
    codex_subscription::DeviceLogin,
    command_rules::{Decision, RuleList, RuleStore},
    config::{ApprovalMode, ApprovalSettings, AutoSettings},
    harness::Message,
    mock::Scenario,
    orchestration::{QueuedRun, RunQueue, task_title},
    projection::{DiffKind, Session, Speaker, Status},
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

actions!(
    solo_app,
    [
        Quit,
        CloseWindow,
        NewSession,
        ToggleTheme,
        ShowChat,
        ShowDiff,
        ShowLogs,
        ShowOverview,
        NextAttention
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
                KeyBinding::new("cmd-0", ShowOverview, None),
                KeyBinding::new("cmd-shift-a", NextAttention, None),
                KeyBinding::new("cmd-shift-o", projects::AddProject, None),
                KeyBinding::new("cmd-shift-p", projects::ShowProjects, None),
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
            let window_title = std::env::current_dir()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "ワークスペース".into());
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(820.), px(620.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(window_title.into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(16.), px(16.))),
                    }),
                    app_id: Some("dev.solo.app".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| projects::ProjectManager::new(smoke, window, cx));
                    cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
                },
            )
            .expect("Solo の window を開けませんでした");
            cx.activate(true);
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
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
    backend: Option<Backend>,
    selected_backend: usize,
    task: Option<Task<()>>,
    stream_generation: u64,
    _input_subscription: Subscription,
    batches: u64,
    max_batch_ms: f64,
    started_at: Option<Instant>,
    elapsed: Option<std::time::Duration>,
    unread_result: bool,
    input_composing: bool,
    last_prompt: String,
    approval_note: Option<String>,
    _change_subscription: Subscription,
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
    command_rules: Result<RuleStore, String>,
    rule_editor: Entity<command_rules::CommandRuleEditor>,
    _rule_temp: Option<tempfile::TempDir>,
    approval_settings: Result<ApprovalSettings, String>,
    approval_serial: u64,
    approval_reviewer: Arc<dyn Reviewer>,
    _approval_settings_subscription: Subscription,
    show_metrics: bool,
    rendered: usize,
    queue: RunQueue,
    attention_only: bool,
    is_visible: bool,
    other_project_attention: usize,
}

impl Workspace {
    fn new(
        path: PathBuf,
        workspace_name: String,
        smoke: bool,
        toast: Entity<ToastHost>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        let rule_temp = smoke.then(|| tempfile::tempdir().expect("smoke rule store"));
        let command_rules = if let Some(dir) = &rule_temp {
            RuleStore::at_path(dir.path().join("config.yml"), &path)
        } else {
            RuleStore::for_workspace(&path)
        }
        .map_err(|error| error.to_string());
        approvals::init(cx);
        let approval_settings = approvals::settings(&command_rules);
        let approval_settings_subscription = cx
            .observe_global::<approvals::PolicyRevision>(|this, cx| this.reconsider_approvals(cx));
        let rule_editor =
            cx.new(|cx| command_rules::CommandRuleEditor::new(command_rules.clone(), window, cx));
        let mut this = Self {
            command_rules,
            rule_editor,
            _rule_temp: rule_temp,
            approval_settings,
            approval_serial: 0,
            approval_reviewer: Arc::new(CodexReviewer),
            _approval_settings_subscription: approval_settings_subscription,
            sessions: Vec::new(),
            selected: 0,
            serial: 0,
            message: String::new(),
            workspace_name,
            workspace_path: path.display().to_string(),
            scenario_picker,
            acp_agents,
            _picker_subscription: picker_subscription,
            toast,
            show_metrics: false,
            rendered: 0,
            queue: RunQueue::default(),
            attention_only: false,
            is_visible: true,
            other_project_attention: 0,
        };
        this.new_session(window, cx);
        if let Some(error) = config_error {
            this.message = error;
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
                .placeholder("依頼を入力…")
                .control_size(ControlSize::Large)
                .clear_on_submit(true)
        });
        let callback_id = id.clone();
        let subscription = cx.subscribe(&composer, move |this, _, submitted: &Submitted, cx| {
            if let Some(index) = this.sessions.iter().position(|s| s.model.id == callback_id) {
                this.start_selected(index, submitted.0.clone(), cx);
            }
        });
        let changed_id = id.clone();
        let change_subscription =
            cx.subscribe(&composer, move |this, _, change: &ds::InputChanged, cx| {
                if let Some(session) = this
                    .sessions
                    .iter_mut()
                    .find(|session| session.model.id == changed_id)
                {
                    session.input_composing = change.composing;
                }
                cx.notify();
            });
        let mut model = Session::new(id, format!("新しいタスク {}", self.serial));
        model.provider = "Codex / ChatGPT Subscription".into();
        self.sessions.push(SessionView {
            model,
            composer,
            chat_list: cx.new(|cx| MessageScrollerState::new(0, cx)),
            log_scroll: UniformListScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_index: 0,
            tab: Tab::Overview,
            follow_logs: true,
            artifacts: Vec::new(),
            controller: None,
            history: Vec::new(),
            login: None,
            login_only: false,
            approval: None,
            backend: None,
            selected_backend: 0,
            task: None,
            stream_generation: 0,
            _input_subscription: subscription,
            batches: 0,
            max_batch_ms: 0.,
            started_at: None,
            elapsed: None,
            unread_result: false,
            input_composing: false,
            last_prompt: String::new(),
            approval_note: None,
            _change_subscription: change_subscription,
        });
        self.selected = self.sessions.len() - 1;
        self.attention_only = false;
        self.scenario_picker.update(cx, |picker, cx| {
            picker.selected = 0;
            picker.close(cx);
        });
        self.message.clear();
        self.sync_controls(cx);
        cx.notify();
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let session = &self.sessions[self.selected];
        let locked = session.model.status.is_active()
            || self.queue.position(&session.model.id).is_some()
            || session.uses_workspace();
        if self.scenario_picker.read(cx).disabled != locked {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.disabled = locked;
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
        let session = &self.sessions[self.selected];
        self.queue.cancel(&session.model.id);
        if session.model.status.is_active() && session.uses_workspace() {
            self.queue.paused = true;
        }
        self.sessions.remove(self.selected);
        self.selected = self.selected.saturating_sub(1);
        if self.sessions.is_empty() {
            self.new_session(window, cx);
        }
        self.message.clear();
        let id = self.sessions[self.selected].model.id.clone();
        self.select_session(&id, window, cx);
        self.dispatch_queue(cx);
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
                approval.respond(false);
            }
            session.model.status = Status::Cancelling;
            cx.notify();
        }
    }

    fn copy(&self, text: String, message: &'static str, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast
            .update(cx, |toast, cx| toast.push(message, Tone::Success, cx));
    }
}

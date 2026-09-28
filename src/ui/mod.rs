#[cfg(debug_assertions)]
mod approval_modes_smoke;
mod approvals;
mod command_rules;
#[cfg(debug_assertions)]
mod command_rules_smoke;
mod execution;
mod overview;
mod persistence;
mod projects;
#[cfg(debug_assertions)]
mod projects_smoke;
#[cfg(debug_assertions)]
mod smoke;
mod stream;
#[cfg(debug_assertions)]
mod threads_smoke;
mod views;
#[cfg(feature = "gui-visual")]
mod visual;
mod workflow;
#[cfg(debug_assertions)]
mod workflow_smoke;

use approvals::PendingApproval;
use execution::{Backend, UiController};
use gpui_kit::component::message_scroller::MessageScrollerState;
use gpui_kit::{prelude::*, *};
use solo::design::{
    self as ds, Button, ButtonVariant, ColorScheme, ControlSize, DesignAssets, Icon, Select,
    Submitted, TextInput as Composer, ToastHost, Tone, glass, space, typography,
};
use solo::{
    acp::{self, AgentProfile},
    approval::{ApprovalPlan, ApprovalReply, ApprovalRequest},
    auto_approval::{self, Assessment, CodexReviewer, Reviewer},
    backend::BackendKind,
    codex::DeviceLogin,
    command_rules::{Decision, RuleList, RuleStore},
    config::{ApprovalMode, ApprovalSettings, AutoSettings},
    event::{SCHEMA_VERSION, SessionId},
    harness::Message,
    mock::{SCENARIOS, Scenario},
    orchestration::{QueuedRun, RunQueue},
    projection::{DiffKind, SessionProjection, Speaker, Status},
    session_store::{
        QueueEntry, RestoredSession, SessionFile, SessionMeta, StoredQueue, WorkspaceState,
        WorkspaceStore,
    },
    text::task_title,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use stream::UiDelivery;

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
        ShowThread,
        NextAttention
    ]
);

pub fn run() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "Solo\n  --light     ライトテーマで起動\n  --compact   小さいウィンドウで起動\n  --smoke     実画面の検証後に終了\n  --visual D  PNG で実描画を出力して終了（要 feature gui-visual）"
        );
        return;
    }
    let visual = args
        .iter()
        .position(|arg| arg == "--visual")
        .and_then(|i| args.get(i + 1).map(PathBuf::from))
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix("--visual=").map(PathBuf::from))
        });
    #[cfg(feature = "gui-visual")]
    if let Some(dir) = visual {
        visual::run(&dir);
        return;
    }
    #[cfg(not(feature = "gui-visual"))]
    if visual.is_some() {
        eprintln!(
            "--visual には feature gui-visual が必要です: cargo run --features gui-visual -- --visual DIR"
        );
        return;
    }
    // smoke 系モジュールは debug ビルドだけに含める。
    #[cfg(debug_assertions)]
    let smoke = args.iter().any(|arg| arg == "--smoke");
    #[cfg(not(debug_assertions))]
    let smoke = {
        if args.iter().any(|arg| arg == "--smoke") {
            eprintln!("--smoke は debug ビルドでのみ有効です");
            return;
        }
        false
    };
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
                KeyBinding::new("cmd-shift-t", ShowThread, None),
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
                    // Liquid Glass: 背面にシステムのブラー素材を敷き、半透明の層を透かせる。
                    window_background: WindowBackgroundAppearance::Blurred,
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

/// 表示の切り替えとスクロール位置。実行・永続化と独立して維持する。
struct ViewState {
    tab: Tab,
    diff_index: usize,
    /// 差分レビュー中のハンク位置。ファイル切替時に先頭へ戻す。
    diff_hunk: usize,
    selected_thread: Option<u64>,
    expanded_activity: Option<String>,
    follow_logs: bool,
    log_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    thread_scroll: ScrollHandle,
}

/// 実行系の接続状態。「接続中」はドメイン status ではなく UI の表示状態なので
/// SessionView 側に持ち、Connecting 相当の間だけ立てる。
struct ExecState {
    controller: Option<UiController>,
    task: Option<Task<()>>,
    stream_generation: u64,
    /// begin_run から最初の応答(TurnStarted)または終了までの接続中フラグ。
    connecting: bool,
    /// model.provider() が返す値が無い間の表示名フォールバック。
    provider_hint: Option<String>,
}

/// セッションの追記ストアと属性の保存状態。
struct PersistState {
    /// セッションの追記ストア。初回の保存要求で open/create する。
    file: Option<SessionFile>,
    /// 保存中のセッション属性。イベント由来の差分は sync_meta で都度反映する。
    meta: SessionMeta,
    /// 下書き保存のデバウンス世代。+1 で待機中の保存を無効化する。
    meta_save_gen: u64,
    /// append/保存の失敗を Workspace.message へ一度だけ伝えるための退避。
    persist_error: Option<String>,
}

/// バッチ処理と実行時間の計測値。
struct Metrics {
    batches: u64,
    max_batch_ms: f64,
    started_at: Option<Instant>,
    elapsed: Option<Duration>,
}

struct SessionView {
    model: SessionProjection,
    composer: Entity<Composer>,
    chat_list: Entity<MessageScrollerState>,
    /// 全文ログのパス。ストアありの実行ではセッションの logs/ を指し、再起動後も残る。
    artifacts: Vec<Arc<PathBuf>>,
    history: Vec<Message>,
    login: Option<DeviceLogin>,
    login_only: bool,
    approval: Option<PendingApproval>,
    backend: Option<Backend>,
    selected_backend: usize,
    _input_subscription: Subscription,
    unread_result: bool,
    input_composing: bool,
    last_prompt: String,
    approval_note: Option<String>,
    _change_subscription: Subscription,
    view: ViewState,
    exec: ExecState,
    persist: PersistState,
    metrics: Metrics,
}

struct Workspace {
    focus: FocusHandle,
    sessions: Vec<SessionView>,
    selected: usize,
    serial: u64,
    message: String,
    workspace_path: String,
    workspace_name: String,
    /// セッション・順番待ちの保存領域。None は保存なし(smoke 用フィクスチャでも tempdir 経由)。
    store: Option<WorkspaceStore>,
    /// store の一時領域。smoke 用。
    _store_temp: Option<tempfile::TempDir>,
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
    is_visible: bool,
    other_project_attention: usize,
}

impl Workspace {
    fn new(
        path: PathBuf,
        workspace_name: String,
        smoke: bool,
        store: Option<WorkspaceStore>,
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
        let mut store_temp = None;
        let store = store.or_else(|| Self::resolve_store(&path, smoke, &mut store_temp));
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
                this.session_mut().selected_backend = selected.index;
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
            focus: cx.focus_handle(),
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
            store,
            _store_temp: store_temp,
            scenario_picker,
            acp_agents,
            _picker_subscription: picker_subscription,
            toast,
            show_metrics: false,
            rendered: 0,
            queue: RunQueue::default(),
            is_visible: true,
            other_project_attention: 0,
        };
        // 前回のセッションを復元する。実行中だったものは「結果未確認」で戻る。
        this.restore_sessions(window, cx);
        if this.sessions.is_empty() {
            this.new_session(window, cx);
        } else {
            let id = this.session().model.id.clone();
            this.select_session(&id, window, cx);
        }
        if this.store.is_none() {
            this.report(
                "セッションの保存領域を用意できません。この起動中の変更は保存されません".into(),
            );
        }
        if let Some(error) = config_error {
            this.message = error;
        }
        this
    }

    /// 明示指定が無ければ ~/.solo/sessions/<workspace>/。smoke では一時領域を返す。
    fn resolve_store(
        path: &std::path::Path,
        smoke: bool,
        store_temp: &mut Option<tempfile::TempDir>,
    ) -> Option<WorkspaceStore> {
        if smoke {
            *store_temp = tempfile::tempdir().ok();
            store_temp
                .as_ref()
                .map(|dir| WorkspaceStore::at_path(dir.path().join("sessions")))
        } else {
            WorkspaceStore::for_workspace(path).ok()
        }
    }

    /// model.id → sessions の index。コールバックで頻出する検索。
    pub(super) fn session_index(&self, id: &SessionId) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| session.model.id == *id)
    }

    /// 選択中のセッション。
    pub(super) fn session(&self) -> &SessionView {
        &self.sessions[self.selected]
    }

    /// 選択中のセッション(可変)。
    /// `&mut self` 全体を借用するため、借用中に self の他フィールドへ触れる箇所は
    /// `let sessions = &mut self.sessions` で分割するか直接 index する。
    pub(super) fn session_mut(&mut self) -> &mut SessionView {
        &mut self.sessions[self.selected]
    }

    /// index 指定のセッション。
    pub(super) fn session_at(&self, index: usize) -> &SessionView {
        &self.sessions[index]
    }

    /// index 指定のセッション(可変)。session_mut と同じ借用上の注意。
    pub(super) fn session_at_mut(&mut self, index: usize) -> &mut SessionView {
        &mut self.sessions[index]
    }

    /// 通知領域が空のときだけメッセージを載せる(既存の通知を潰さない)。
    pub(super) fn report(&mut self, message: String) {
        if self.message.is_empty() {
            self.message = message;
        }
    }

    /// 永続化の失敗を一度だけ通知領域へ移す。
    pub(super) fn drain_persist_error(&mut self, index: usize) {
        if let Some(error) = self.session_at_mut(index).persist.persist_error.take() {
            self.report(error);
        }
    }

    /// composer・購読・ビュー状態を持つ SessionView を組み立てる。リストには入れない。
    /// `id` はストア採番済みの最終 ID(コールバックがこの ID で検索する)。
    fn build_session_view(
        &mut self,
        id: SessionId,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SessionView {
        let (composer, subscription, change_subscription) = self.build_composer(&id, window, cx);
        SessionView {
            model: SessionProjection::new(id, title),
            composer,
            chat_list: cx.new(|cx| MessageScrollerState::new(0, cx)),
            artifacts: Vec::new(),
            history: Vec::new(),
            login: None,
            login_only: false,
            approval: None,
            backend: None,
            selected_backend: 0,
            _input_subscription: subscription,
            unread_result: false,
            input_composing: false,
            last_prompt: String::new(),
            approval_note: None,
            _change_subscription: change_subscription,
            view: ViewState {
                tab: Tab::Chat,
                diff_index: 0,
                diff_hunk: 0,
                selected_thread: None,
                expanded_activity: None,
                follow_logs: true,
                log_scroll: UniformListScrollHandle::new(),
                diff_scroll: UniformListScrollHandle::new(),
                thread_scroll: ScrollHandle::new(),
            },
            exec: ExecState {
                controller: None,
                task: None,
                stream_generation: 0,
                connecting: false,
                provider_hint: None,
            },
            persist: PersistState {
                file: None,
                meta: SessionMeta::default(),
                meta_save_gen: 0,
                persist_error: None,
            },
            metrics: Metrics {
                batches: 0,
                max_batch_ms: 0.,
                started_at: None,
                elapsed: None,
            },
        }
    }

    /// セッションの composer と submit/change 購読を組み立てる。
    /// submit は `id` で宛先セッションを引き、change は下書き保存を予約する。
    fn build_composer(
        &mut self,
        id: &SessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<Composer>, Subscription, Subscription) {
        let composer = cx.new(|cx| {
            Composer::multiline(window, cx)
                .placeholder("このチャンネルに依頼を送る…")
                .control_size(ControlSize::Large)
                .appearance(false)
                .clear_on_submit(true)
        });
        let callback_id = id.clone();
        let subscription = cx.subscribe(&composer, move |this, _, submitted: &Submitted, cx| {
            if let Some(index) = this.session_index(&callback_id) {
                this.start_selected(index, submitted.0.clone(), cx);
            }
        });
        let changed_id = id.clone();
        let change_subscription =
            cx.subscribe(&composer, move |this, _, change: &ds::InputChanged, cx| {
                if let Some(index) = this.session_index(&changed_id) {
                    this.session_at_mut(index).input_composing = change.composing;
                    this.schedule_meta_save(index, cx);
                }
                cx.notify();
            });
        (composer, subscription, change_subscription)
    }

    fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sessions.len() >= 8 {
            self.message = "最大8セッションです。不要なセッションを閉じてください。".into();
            cx.notify();
            return;
        }
        self.serial += 1;
        // provision の失敗メッセージを消さないよう、クリアは採番の前に行う。
        self.message.clear();
        let (id, file, meta) = self.provision_session(self.serial);
        let mut view =
            self.build_session_view(id, format!("新しいセッション {}", self.serial), window, cx);
        view.persist.file = file;
        view.persist.meta = meta;
        // provider は ModelRequestStarted が来るまでフォールバックのヒントを表示する。
        view.exec.provider_hint = Some("Codex / ChatGPT Subscription".into());
        self.sessions.push(view);
        self.selected = self.sessions.len() - 1;
        self.scenario_picker.update(cx, |picker, cx| {
            picker.selected = 0;
            picker.close(cx);
        });
        self.sync_controls(cx);
        self.save_workspace_state(cx);
        cx.notify();
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let session = self.session();
        let locked = session.display_status().is_active()
            || self.queue.position(&session.model.id).is_some()
            || session.uses_workspace();
        if self.scenario_picker.read(cx).disabled != locked {
            self.scenario_picker.update(cx, |picker, cx| {
                picker.disabled = locked;
                picker.close(cx);
            });
        }
    }

    fn select_session(&mut self, id: &SessionId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.session_index(id) else {
            return;
        };
        if index != self.selected {
            // 離れるセッションの下書きを確定させてから切り替える。
            self.save_session_meta(self.selected, cx);
            self.session_mut().persist.meta_save_gen += 1;
        }
        self.selected = index;
        self.scenario_picker.update(cx, |picker, cx| {
            picker.selected = self.session_at(index).selected_backend;
            picker.close(cx);
        });
        self.sync_controls(cx);
        self.save_workspace_state(cx);
        let focus = self.content_focus(window, cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn content_focus(&self, window: &Window, cx: &App) -> FocusHandle {
        let session = self.session();
        if session.view.tab == Tab::Chat
            && session.view.selected_thread.is_some()
            && window.viewport_size().width < px(1120.)
        {
            self.focus.clone()
        } else {
            session.composer.focus_handle(cx)
        }
    }

    fn close_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // queue.cancel は &mut self.queue を取るため、先に ID を複写して session の借用を終える。
        let session_id = self.session().model.id.clone();
        self.queue.cancel(&session_id);
        if self.session().display_status().is_active() && self.session().uses_workspace() {
            self.queue.set_paused(true);
        }
        // チャンネルを閉じる = 会話・下書き・全文ログごと削除する。
        let mut removed = self.sessions.remove(self.selected);
        removed.persist.file = None; // 先にライターを閉じて flush させる
        if let Some(store) = self.store.clone() {
            // 途中で消えた場合に sweep 対象へ進めるよう、closed 印を書いてから削除する。
            removed.persist.meta.closed = true;
            let meta = removed.persist.meta.clone();
            let _ = removed
                .open_or_create(&store)
                .and_then(|file| file.save_meta(&meta));
            if let Err(error) = store.remove(&removed.model.id) {
                self.report(format!("保存済みセッションを削除できません: {error}"));
            }
        }
        self.selected = self.selected.saturating_sub(1);
        if self.sessions.is_empty() {
            self.new_session(window, cx);
        }
        self.save_workspace_state(cx);
        let id = self.session().model.id.clone();
        self.select_session(&id, window, cx);
        self.dispatch_queue(cx);
    }

    fn show_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.session_mut().view.tab = tab;
        self.scenario_picker
            .update(cx, |picker, cx| picker.close(cx));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if session.display_status().is_active()
            && let Some(controller) = &session.exec.controller
        {
            controller.cancel();
            if let Some(approval) = session.approval.take() {
                approval.respond(false);
            }
            session.model.request_cancel();
            cx.notify();
        }
    }

    fn copy(&self, text: String, message: &'static str, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast
            .update(cx, |toast, cx| toast.push(message, Tone::Success, cx));
    }
}

/// 「確認済み N / M ファイル」の共通インジケータ。概要と Diff タブで同じ表示に揃える。
fn review_indicator(
    id: impl Into<ElementId>,
    reviewed: usize,
    total: usize,
    cx: &App,
) -> Stateful<Div> {
    ds::indicator(
        id,
        Icon::CircleCheck,
        format!("{reviewed} / {total}"),
        format!("確認済み {reviewed} / {total} ファイル"),
        Tone::Neutral,
        cx,
    )
}

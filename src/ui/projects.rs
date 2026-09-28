mod dialogs;
mod navigation;
mod panel;

use super::*;
use gpui_kit::component::WindowExt;
use gpui_kit::component::{h_resizable, resizable_panel};
use solo::projects::{Catalog, Project, ProjectStore};
use std::collections::HashSet;

actions!(solo_projects, [AddProject, ShowProjects]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Activity {
    pub sessions: usize,
    pub running: usize,
    pub queued: usize,
    pub attention: usize,
}

impl Activity {
    fn read(workspace: &Workspace) -> Self {
        Self {
            sessions: workspace.sessions.len(),
            running: workspace
                .sessions
                .iter()
                .filter(|s| s.model.status.is_active())
                .count(),
            queued: workspace.queue.len(),
            attention: workspace
                .sessions
                .iter()
                .filter(|s| s.needs_attention())
                .count(),
        }
    }
    fn busy(self) -> bool {
        self.running > 0 || self.queued > 0
    }
}

struct OpenProject {
    id: u64,
    workspace: Entity<Workspace>,
    activity: Activity,
    _observation: Subscription,
}

pub(super) struct ProjectManager {
    pub(super) catalog: Catalog,
    pub(super) store: Result<ProjectStore, String>,
    opened: Vec<OpenProject>,
    pub(super) error: Option<String>,
    toast: Entity<ToastHost>,
    focus: FocusHandle,
    choosing_folder: bool,
    smoke: bool,
    collapsed_projects: HashSet<u64>,
    attention_only: bool,
    pub(super) temporary: Option<tempfile::TempDir>,
}

impl ProjectManager {
    pub(super) fn new(smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::init(smoke, smoke, window, cx)
    }

    /// フィクスチャの一時ストアで立ち上げ、自己検証タスクは走らせない(--visual 用)。
    /// Workspace への smoke フラグは維持し、モック系フィクスチャはそのまま使える。
    #[cfg(feature = "gui-visual")]
    pub(super) fn fixture(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::init(true, false, window, cx)
    }

    fn init(smoke: bool, run_smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let temporary = smoke.then(|| tempfile::tempdir().expect("project smoke store"));
        let store = if let Some(dir) = &temporary {
            Ok(ProjectStore::at_path(dir.path().join("projects.json")))
        } else {
            ProjectStore::user().map_err(|error| error.to_string())
        };
        let loaded = store
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|store| store.load().map_err(|error| error.to_string()));
        let mut error = loaded.as_ref().err().cloned();
        let mut catalog = loaded.unwrap_or_default();
        // 初回だけ起動ディレクトリを登録。以後は前回の選択（空の一覧も含む）を復元する。
        if catalog.revision == 0 && catalog.projects.is_empty() {
            match std::env::current_dir().and_then(|path| catalog.register(&path)) {
                Ok(_) => {
                    if error.is_none()
                        && let Err(problem) =
                            store.as_ref().map_err(Clone::clone).and_then(|store| {
                                store.save(&mut catalog).map_err(|error| error.to_string())
                            })
                    {
                        error = Some(problem);
                    }
                }
                Err(problem) => error = Some(problem.to_string()),
            }
        }
        let mut this = Self {
            catalog,
            store,
            opened: Vec::new(),
            error,
            toast: cx.new(ToastHost::new),
            focus: cx.focus_handle(),
            choosing_folder: false,
            smoke,
            collapsed_projects: HashSet::new(),
            attention_only: false,
            temporary,
        };
        this.activate(window, cx);
        if run_smoke {
            super::projects_smoke::start(window, cx);
        }
        this
    }

    pub(super) fn workspace(&self, id: u64) -> Option<Entity<Workspace>> {
        self.opened
            .iter()
            .find(|opened| opened.id == id)
            .map(|opened| opened.workspace.clone())
    }

    fn activity(&self, id: u64, cx: &App) -> Activity {
        self.workspace(id)
            .map(|view| Activity::read(view.read(cx)))
            .unwrap_or_default()
    }

    fn fail(&mut self, error: impl ToString, cx: &mut Context<Self>) {
        let error = error.to_string();
        self.toast
            .update(cx, |toast, cx| toast.push(error.clone(), Tone::Warning, cx));
        self.error = Some(error);
        cx.notify();
    }

    fn commit(&mut self, mut next: Catalog, cx: &mut Context<Self>) -> bool {
        let result = self
            .store
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|store| store.save(&mut next).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {
                self.catalog = next;
                self.error = None;
                cx.notify();
                true
            }
            Err(error) => {
                self.fail(error, cx);
                false
            }
        }
    }

    fn valid_folder(&mut self, project: &Project, cx: &mut Context<Self>) -> bool {
        match project.path.canonicalize() {
            Ok(path) if path.is_dir() && path == project.path => true,
            Ok(_) => {
                self.fail(
                    format!(
                        "フォルダの参照先が変更されています: {}",
                        project.path.display()
                    ),
                    cx,
                );
                false
            }
            Err(error) => {
                self.fail(format!("{}: {error}", project.path.display()), cx);
                false
            }
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.catalog.active;
        if let Some(project) = active.and_then(|id| self.catalog.get(id)).cloned()
            && self.workspace(project.id).is_none()
            && self.valid_folder(&project, cx)
        {
            let toast = self.toast.clone();
            let view = cx.new(|cx| {
                Workspace::new(
                    project.path.clone(),
                    project.name.clone(),
                    self.smoke,
                    None,
                    toast,
                    window,
                    cx,
                )
            });
            let activity = Activity::read(view.read(cx));
            let id = project.id;
            let observation = cx.observe(&view, move |this, view, cx| {
                let activity = Activity::read(view.read(cx));
                if let Some(opened) = this.opened.iter_mut().find(|opened| opened.id == id)
                    && opened.activity != activity
                {
                    opened.activity = activity;
                    this.sync_attention(cx);
                }
                cx.notify();
            });
            self.opened.push(OpenProject {
                id,
                workspace: view,
                activity,
                _observation: observation,
            });
        }
        for opened in &self.opened {
            let visible = active == Some(opened.id);
            let name = self
                .catalog
                .get(opened.id)
                .map(|project| project.name.clone());
            opened.workspace.update(cx, |workspace, cx| {
                workspace.is_visible = visible;
                if let Some(name) = name {
                    workspace.workspace_name = name;
                }
                cx.notify();
            });
        }
        if let Some(id) = active
            && let Some(view) = self.workspace(id)
        {
            let workspace = view.read(cx);
            window.set_window_title(&workspace.workspace_name);
        } else {
            window.set_window_title("プロジェクト");
        }
        self.focus_active(window, cx);
        self.sync_attention(cx);
        cx.notify();
    }

    fn sync_attention(&self, cx: &mut Context<Self>) {
        let total: usize = self
            .opened
            .iter()
            .map(|opened| opened.activity.attention)
            .sum();
        for opened in &self.opened {
            let other = total.saturating_sub(opened.activity.attention);
            if opened.workspace.read(cx).other_project_attention != other {
                opened.workspace.update(cx, |workspace, cx| {
                    workspace.other_project_attention = other;
                    cx.notify();
                });
            }
        }
    }

    fn focus_active(&self, window: &mut Window, cx: &mut App) {
        // 初期生成中は Kit の Root がまだ window に設定されていない。
        if window
            .root::<gpui_kit::component::Root>()
            .flatten()
            .is_some()
            && (window.has_active_sheet(cx) || window.has_active_dialog(cx))
        {
            return;
        }
        if let Some(view) = self.catalog.active.and_then(|id| self.workspace(id)) {
            let focus = view.read(cx).content_focus(window, cx);
            window.focus(&focus, cx);
        } else {
            window.focus(&self.focus, cx);
        }
    }

    pub(super) fn open_project(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.select_project(id, window, cx) {
            return false;
        }
        window.close_sheet(cx);
        self.focus_active(window, cx);
        true
    }

    pub(super) fn add_project(
        &mut self,
        path: &std::path::Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let mut next = self.catalog.clone();
        let id = match next.register(path) {
            Ok(id) => id,
            Err(error) => {
                self.fail(error, cx);
                return None;
            }
        };
        if next != self.catalog && !self.commit(next, cx) {
            return None;
        }
        self.activate(window, cx);
        Some(id)
    }

    pub(super) fn select_project(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(project) = self.catalog.get(id).cloned() else {
            return false;
        };
        if self.workspace(id).is_none() && !self.valid_folder(&project, cx) {
            return false;
        }
        if self.catalog.active != Some(id) {
            let mut next = self.catalog.clone();
            if next.select(id).is_err() || !self.commit(next, cx) {
                return false;
            }
        }
        self.activate(window, cx);
        true
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self
            .store
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|store| store.load().map_err(|error| error.to_string()));
        let next = match result {
            Ok(catalog) => catalog,
            Err(error) => {
                self.fail(error, cx);
                return;
            }
        };
        for opened in &self.opened {
            let same_path = next
                .get(opened.id)
                .zip(self.catalog.get(opened.id))
                .is_some_and(|(a, b)| a.path == b.path);
            let workspace = opened.workspace.read(cx);
            if !same_path
                && (!workspace.queue.is_empty()
                    || workspace.sessions.iter().any(|session| {
                        session.model.status.is_active()
                            || !session.model.chat.is_empty()
                            || !session.composer.read(cx).value(cx).is_empty()
                    }))
            {
                self.fail(
                    format!(
                        "{} のセッションを閉じてから再読み込みしてください",
                        workspace.workspace_name
                    ),
                    cx,
                );
                return;
            }
        }
        self.opened.retain(|opened| {
            next.get(opened.id)
                .zip(self.catalog.get(opened.id))
                .is_some_and(|(a, b)| a.path == b.path)
        });
        self.catalog = next;
        self.error = None;
        self.activate(window, cx);
    }

    fn with_active(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>),
    ) {
        if let Some(view) = self.catalog.active.and_then(|id| self.workspace(id)) {
            view.update(cx, |workspace, cx| f(workspace, window, cx));
        }
    }
}

impl Render for ProjectManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let view = self.catalog.active.and_then(|id| self.workspace(id));
        ds::root(cx)
            .relative()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .on_action(
                cx.listener(|this, _: &AddProject, window, cx| this.choose_folder(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ShowProjects, window, cx| this.show_projects(window, cx)),
            )
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &ToggleTheme, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| {
                this.attention_only = false;
                if let Some(id) = this.catalog.active {
                    this.collapsed_projects.remove(&id);
                }
                this.with_active(window, cx, |workspace, window, cx| {
                    workspace.new_session(window, cx);
                    window.focus(
                        &workspace.sessions[workspace.selected]
                            .composer
                            .focus_handle(cx),
                        cx,
                    );
                })
            }))
            .on_action(cx.listener(|this, _: &ShowOverview, window, cx| {
                this.with_active(window, cx, |workspace, _, cx| {
                    workspace.show_tab(Tab::Overview, cx)
                })
            }))
            .on_action(cx.listener(|this, _: &ShowChat, window, cx| {
                this.with_active(window, cx, |workspace, _, cx| {
                    workspace.show_tab(Tab::Chat, cx)
                })
            }))
            .on_action(cx.listener(|this, _: &ShowThread, window, cx| {
                this.with_active(window, cx, |workspace, window, cx| {
                    workspace.toggle_thread(window, cx)
                })
            }))
            .on_action(cx.listener(|this, _: &ShowDiff, window, cx| {
                this.with_active(window, cx, |workspace, _, cx| {
                    workspace.show_tab(Tab::Diff, cx)
                })
            }))
            .on_action(cx.listener(|this, _: &ShowLogs, window, cx| {
                this.with_active(window, cx, |workspace, _, cx| {
                    workspace.show_tab(Tab::Logs, cx)
                })
            }))
            .on_action(cx.listener(|this, _: &NextAttention, window, cx| {
                this.with_active(window, cx, |workspace, window, cx| {
                    workspace.next_attention(window, cx)
                })
            }))
            .child(self.titlebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.activity_bar(cx))
                    .child(
                        h_resizable("project-channel-layout")
                            .child(
                                resizable_panel()
                                    .size(px(244.))
                                    .size_range(px(210.)..px(330.))
                                    .child(self.navigation(window, cx)),
                            )
                            .child(
                                resizable_panel().size_range(px(480.)..px(5000.)).child(
                                    div()
                                        .relative()
                                        .size_full()
                                        .when_some(view.clone(), |v, view| v.child(view))
                                        .when(view.is_none(), |v| {
                                            v.child(
                                                div()
                                                    .size_full()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .h(px(46.))
                                                            .window_control_area(WindowControlArea::Drag),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .flex()
                                                            .flex_col()
                                                            .items_center()
                                                            .justify_center()
                                                            .gap_4()
                                                            .child(Icon::Folder.view(p.muted).size(px(32.)))
                                                            .child(
                                                                div()
                                                                    .text_color(rgb(p.muted))
                                                                    .child("プロジェクトなし"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .gap_2()
                                                                    .child(
                                                                        Button::icon(
                                                                            "empty-add-project",
                                                                            Icon::Plus,
                                                                            "プロジェクトを追加 · ⌘ ⇧ O",
                                                                        )
                                                                        .variant(ButtonVariant::Primary)
                                                                        .disabled(self.choosing_folder)
                                                                        .on_click(cx.listener(
                                                                            |this, _, window, cx| {
                                                                                this.choose_folder(
                                                                                    window, cx,
                                                                                )
                                                                            },
                                                                        )),
                                                                    )
                                                                    .child(
                                                                        Button::icon(
                                                                            "empty-project-list",
                                                                            Icon::Folder,
                                                                            "プロジェクト一覧 · ⌘ ⇧ P",
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            |this, _, window, cx| {
                                                                                this.show_projects(
                                                                                    window, cx,
                                                                                )
                                                                            },
                                                                        )),
                                                                    ),
                                                            ),
                                                    ),
                                            )
                                        }),
                                ),
                            ),
                        )
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .flex_shrink_0()
                        .p_3()
                        .bg(rgb(p.warning_soft))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Warning.view(p.warning))
                        .child(div().flex_1().text_size(px(12.)).child(error))
                        .child(
                            Button::icon(
                                "reload-project-error",
                                Icon::RotateCcw,
                                "プロジェクト一覧を再読み込み",
                            )
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
                        )
                        .child(
                            Button::icon("dismiss-project-error", Icon::Close, "通知を閉じる")
                                .control_size(ControlSize::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.toast.clone())
            .children(gpui_kit::component::Root::render_sheet_layer(window, cx))
            .children(gpui_kit::component::Root::render_dialog_layer(window, cx))
            .children(gpui_kit::component::Root::render_notification_layer(
                window, cx,
            ))
    }
}

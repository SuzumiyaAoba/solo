use super::super::views::{caption, status_icon};
use super::*;
use gpui_kit::component::{
    Sizable,
    sidebar::{SidebarItem, SidebarMenu, SidebarMenuItem},
    tab::{Tab as KitTab, TabBar},
};

impl ProjectManager {
    pub(super) fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        div()
            .h(px(44.))
            .flex_shrink_0()
            .pl(px(96.))
            .pr_4()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
            .border_b_1()
            .border_color(rgb(p.border))
            .flex()
            .items_center()
            .gap_2()
            .window_control_area(WindowControlArea::Drag)
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Solo"))
            .child(Icon::ChevronRight.view(p.disabled).size(px(12.)))
            .child(caption(
                self.catalog
                    .active
                    .and_then(|id| self.catalog.get(id))
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "プロジェクトを追加".into()),
                cx,
            ))
            .child(div().flex_1())
            .child(caption("エージェントワークスペース", cx))
    }

    /// VSCode 風の縦アイコンパネル。タブ・スレッド・要対応・テーマ・プロジェクト管理を並べる。
    pub(super) fn activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let workspace = self.catalog.active.and_then(|id| self.workspace(id));
        let session = workspace.as_ref().map(|w| {
            let w = w.read(cx);
            w.session().view.tab
        });
        let thread_open = workspace
            .as_ref()
            .map(|w| {
                let w = w.read(cx);
                let session = w.session();
                session.view.tab == Tab::Chat && session.view.selected_thread.is_some()
            })
            .unwrap_or(false);
        let dark = ds::scheme(cx) == ColorScheme::Dark;
        let row =
            |id: &'static str, icon: Icon, tooltip: &'static str, active: bool| -> Stateful<Div> {
                div()
                    .id(id)
                    .relative()
                    .w(px(40.))
                    .h(px(34.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(ds::radius::CONTROL))
                    .cursor_pointer()
                    .child(
                        icon.view(if active { p.accent_text } else { p.muted })
                            .size(px(16.)),
                    )
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tooltip).build(window, cx)
                    })
                    .when(active, |v| {
                        v.child(
                            div()
                                .absolute()
                                .left(px(-6.))
                                .top(px(6.))
                                .bottom(px(6.))
                                .w(px(2.))
                                .rounded(px(1.))
                                .bg(rgb(p.accent)),
                        )
                    })
                    .hover(move |style| style.bg(ds::glass(p.hover, ds::GLASS_HOVER)))
            };
        let on_action = |id, icon, tooltip, active, action: Box<dyn gpui_kit::Action>| {
            row(id, icon, tooltip, active)
                .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
        };
        div()
            .w(px(46.))
            .flex_shrink_0()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
            .border_r_1()
            .border_color(ds::glass(p.border, 0.6))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(6.))
            .pb_2()
            .gap(px(6.))
            .children([
                on_action(
                    "rail-chat",
                    Icon::MessageSquare,
                    "会話 · ⌘1",
                    session == Some(Tab::Chat),
                    Box::new(ShowChat),
                ),
                on_action(
                    "rail-diff",
                    Icon::FileDiff,
                    "変更 · ⌘2",
                    session == Some(Tab::Diff),
                    Box::new(ShowDiff),
                ),
                on_action(
                    "rail-logs",
                    Icon::Terminal,
                    "ログ · ⌘3",
                    session == Some(Tab::Logs),
                    Box::new(ShowLogs),
                ),
                on_action(
                    "rail-overview",
                    Icon::Activity,
                    "概要 · ⌘0",
                    session == Some(Tab::Overview),
                    Box::new(ShowOverview),
                ),
                on_action(
                    "rail-thread",
                    Icon::Layers,
                    "スレッド · ⌘⇧T",
                    thread_open,
                    Box::new(ShowThread),
                ),
                row(
                    "rail-attention",
                    Icon::Bell,
                    "要対応のチャンネル · ⌘⇧A",
                    self.attention_only,
                )
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.attention_only = !this.attention_only;
                    if this.attention_only {
                        this.collapsed_projects.clear();
                    }
                    cx.notify();
                })),
            ])
            .child(div().flex_1())
            .children([
                on_action(
                    "rail-theme",
                    if dark { Icon::Sun } else { Icon::Moon },
                    "テーマ切替 · ⌘⇧L",
                    false,
                    Box::new(ToggleTheme),
                ),
                on_action(
                    "rail-projects",
                    Icon::Sliders,
                    "プロジェクトを管理 · ⌘⇧P",
                    false,
                    Box::new(ShowProjects),
                ),
                on_action(
                    "rail-add-project",
                    Icon::Plus,
                    "プロジェクトを追加 · ⌘⇧O",
                    false,
                    Box::new(AddProject),
                ),
            ])
    }

    pub(in crate::ui) fn select_channel(
        &mut self,
        project: u64,
        session: &SessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.select_project(project, window, cx) {
            self.collapsed_projects.remove(&project);
            self.with_active(window, cx, |workspace, window, cx| {
                workspace.select_session(session, window, cx)
            });
        }
    }

    fn new_channel(&mut self, project: u64, window: &mut Window, cx: &mut Context<Self>) {
        let was_open = self.workspace(project).is_some();
        if self.select_project(project, window, cx) {
            self.collapsed_projects.remove(&project);
            self.attention_only = false;
            if was_open {
                self.with_active(window, cx, |workspace, window, cx| {
                    workspace.new_session(window, cx)
                });
            }
            self.focus_active(window, cx);
        }
    }

    pub(super) fn navigation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = ds::theme(cx);
        let attention: usize = self.opened.iter().map(|p| p.activity.attention).sum();
        let groups = self
            .catalog
            .projects
            .iter()
            .map(|project| {
                let id = project.id;
                let active = self.catalog.active == Some(id);
                let collapsed = self.collapsed_projects.contains(&id);
                let workspace = self.workspace(id);
                let project_attention = self.activity(id, cx).attention;
                let count = workspace
                    .as_ref()
                    .map(|w| w.read(cx).sessions.len())
                    .unwrap_or(0);
                let mut menu = SidebarMenu::new();
                if let Some(workspace) = &workspace {
                    let workspace = workspace.read(cx);
                    for (index, session) in workspace.sessions.iter().enumerate() {
                        let selected = active && index == workspace.selected;
                        if self.attention_only && !session.needs_attention() && !selected {
                            continue;
                        }
                        let session_id = session.model.id.clone();
                        let queued = workspace.queue.position(&session_id);
                        // state_label/state_tone/status_icon はそのまま使い、
                        // セッション状態に無い「要対応」「順番待ち」だけここで上書きする。
                        let tone = session.state_tone();
                        let state = queued
                            .map(|n| format!("順番待ち {n}"))
                            .unwrap_or_else(|| session.state_label().into());
                        let icon = if session.needs_attention() {
                            Icon::Bell
                        } else if queued.is_some() {
                            Icon::Clock
                        } else {
                            status_icon(session.model.status())
                        };
                        menu = menu.child(
                            SidebarMenuItem::new(format!("#  {}", session.model.title()))
                                .active(selected)
                                .suffix(move |_, cx| {
                                    ds::indicator(
                                        ("channel-state", index),
                                        icon,
                                        "",
                                        state.clone(),
                                        tone,
                                        cx,
                                    )
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_channel(id, &session_id, window, cx)
                                })),
                        );
                    }
                } else {
                    menu = menu.child(
                        SidebarMenuItem::new("チャンネルを開く")
                            .icon(Icon::MessageSquare.kit())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_project(id, window, cx);
                            })),
                    );
                }
                div()
                    .mb_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_1()
                            .child(
                                div().flex_shrink_0().pl_1().child(
                                    (if collapsed {
                                        Icon::ChevronRight
                                    } else {
                                        Icon::ChevronDown
                                    })
                                    .view(p.muted)
                                    .size(px(11.)),
                                ),
                            )
                            .child(
                                Button::new(("project-group", id as usize), project.name.clone())
                                    .variant(ButtonVariant::Ghost)
                                    .control_size(ControlSize::Small)
                                    .with_icon(Icon::Folder)
                                    .flex_1()
                                    .min_w_0()
                                    .align_start()
                                    .overflow_hidden()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .tooltip(project.path.display().to_string())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if this.workspace(id).is_none() {
                                            this.select_project(id, window, cx);
                                        } else if !this.collapsed_projects.remove(&id) {
                                            this.collapsed_projects.insert(id);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .when(project_attention > 0, |v| {
                                v.child(ds::indicator(
                                    ("project-attention-badge", id as usize),
                                    Icon::Bell,
                                    project_attention.to_string(),
                                    "要対応のチャンネル",
                                    Tone::Warning,
                                    cx,
                                ))
                            })
                            .child(
                                Button::icon(
                                    ("new-channel", id as usize),
                                    Icon::Plus,
                                    "このプロジェクトにチャンネルを作成",
                                )
                                .control_size(ControlSize::Small)
                                .disabled(count >= 8)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| this.new_channel(id, window, cx),
                                )),
                            ),
                    )
                    .when(!collapsed, |v| {
                        v.child(menu.render(("project-menu", id as usize), window, cx))
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("project-channel-sidebar")
            .size_full()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_2()
                    .pb_4()
                    .child(
                        div()
                            .h(px(36.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("チャンネル"),
                            )
                            .child(
                                Button::icon(
                                    "manage-projects",
                                    Icon::Sliders,
                                    "プロジェクトを管理 · ⌘ ⇧ P",
                                )
                                .control_size(ControlSize::Small)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.show_projects(window, cx)
                                    }),
                                ),
                            ),
                    )
                    .child(
                        TabBar::new("channel-filter")
                            .segmented()
                            .small()
                            .selected_index(usize::from(self.attention_only))
                            .child(KitTab::new().label("すべて"))
                            .child(KitTab::new().label(format!("要対応 {attention}")))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                this.attention_only = *index == 1;
                                if this.attention_only {
                                    this.collapsed_projects.clear();
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("project-groups")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .child(caption("プロジェクト", cx).px_2().pb_2())
                    .children(groups),
            )
            .child(
                div()
                    .w_full()
                    .p_2()
                    .border_t_1()
                    .border_color(rgb(p.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new("add-project", "プロジェクトを追加")
                            .with_icon(Icon::Plus)
                            .variant(ButtonVariant::Ghost)
                            .w_full()
                            .align_start()
                            .disabled(self.choosing_folder)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                            ),
                    )
                    .child(caption("⌘ N  新しいチャンネル", cx)),
            )
    }
}

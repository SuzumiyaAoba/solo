//! ワークスペースの共通レイアウトとナビゲーション。各ペインは子モジュールで描画する。
mod chat;
mod composer;
mod diff;
mod logs;

use super::*;
use gpui_kit::component::{
    Sizable, h_resizable, resizable_panel,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    tab::{Tab as KitTab, TabBar},
};

const TITLEBAR_HEIGHT: f32 = 46.;
const SIDEBAR_WIDTH: f32 = 224.;

impl SessionView {
    fn uses_openai_icon(&self) -> bool {
        match &self.backend {
            Some(backend) => *backend == Backend::Subscription,
            None => self.login_only || self.selected_backend == 0,
        }
    }

    fn agent_avatar(&self, cx: &App) -> gpui_kit::component::avatar::Avatar {
        if self.uses_openai_icon() {
            ds::icon_avatar(Icon::OpenAi, Tone::Accent, cx)
        } else {
            ds::icon_avatar(Icon::Layers, Tone::Accent, cx)
        }
    }
}

impl Workspace {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let attention = self.sessions.iter().filter(|s| s.needs_attention()).count();
        let running = self
            .sessions
            .iter()
            .filter(|s| s.model.status.is_active())
            .count();
        let menu = SidebarMenu::new().children(
            self.sessions
                .iter()
                .enumerate()
                .filter(|(_, session)| !self.attention_only || session.needs_attention())
                .map(|(index, session)| {
                    let id = session.model.id.clone();
                    let queued = self.queue.position(&id);
                    let label = queued
                        .map(|n| format!("順番待ち · {n}"))
                        .unwrap_or_else(|| session.state_label().to_owned());
                    let tone = session.state_tone();
                    let icon = if queued.is_some() {
                        Icon::Clock
                    } else if session.approval.is_some() {
                        Icon::Bell
                    } else if session.login.is_some() {
                        Icon::LogIn
                    } else if session.model.status == Status::Completed && session.needs_attention()
                    {
                        Icon::FileDiff
                    } else {
                        status_icon(session.model.status)
                    };
                    SidebarMenuItem::new(session.model.title.clone())
                        .icon(
                            if session.uses_openai_icon() {
                                Icon::OpenAi
                            } else {
                                Icon::Layers
                            }
                            .kit(),
                        )
                        .suffix(move |_, cx| {
                            ds::indicator(
                                ("task-state", index),
                                icon,
                                queued.map(|n| n.to_string()).unwrap_or_default(),
                                label.clone(),
                                tone,
                                cx,
                            )
                        })
                        .active(index == self.selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_session(&id, window, cx)
                        }))
                }),
        );
        let menu = if self.attention_only && attention == 0 {
            SidebarMenu::new().child(SidebarMenuItem::new("要対応なし").disable(true))
        } else {
            menu
        };
        Sidebar::new("workspace-sidebar")
            .w_full()
            .h_full()
            .collapsible(false)
            .header(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .pb_3()
                    .child(
                        div()
                            .h(px(TITLEBAR_HEIGHT))
                            .window_control_area(WindowControlArea::Drag),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_2()
                            .child(
                                Button::new("project-switcher", self.workspace_name.clone())
                                    .variant(ButtonVariant::Ghost)
                                    .with_icon(Icon::Folder)
                                    .trailing_icon(Icon::ChevronDown)
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .tooltip(format!(
                                        "プロジェクト · {} · ⌘ ⇧ P",
                                        self.workspace_path
                                    ))
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(projects::ShowProjects), cx)
                                    }),
                            )
                            .child(
                                Button::icon("new-session", Icon::Plus, "新しいタスク · ⌘ N")
                                    .variant(ButtonVariant::Primary)
                                    .disabled(self.sessions.len() >= 8)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.new_session(window, cx);
                                        window.focus(
                                            &this.sessions[this.selected].composer.focus_handle(cx),
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .flex()
                            .gap_4()
                            .child(ds::indicator(
                                "running-count",
                                Icon::Play,
                                running.to_string(),
                                format!("実行中 {running}"),
                                Tone::Accent,
                                cx,
                            ))
                            .child(ds::indicator(
                                "queued-count",
                                Icon::Clock,
                                self.queue.len().to_string(),
                                format!("順番待ち {}", self.queue.len()),
                                Tone::Neutral,
                                cx,
                            ))
                            .when(self.other_project_attention > 0, |v| {
                                v.child(
                                    Button::icon(
                                        "other-project-attention",
                                        Icon::Bell,
                                        format!(
                                            "他のプロジェクトに要対応 {} 件",
                                            self.other_project_attention
                                        ),
                                    )
                                    .control_size(ControlSize::Small)
                                    .text_color(rgb(ds::theme(cx).warning))
                                    .on_click(
                                        |_, window, cx| {
                                            window.dispatch_action(
                                                Box::new(projects::ShowProjects),
                                                cx,
                                            )
                                        },
                                    ),
                                )
                            }),
                    )
                    .child(
                        TabBar::new("session-filter")
                            .segmented()
                            .small()
                            .selected_index(usize::from(self.attention_only))
                            .child(KitTab::new().label("すべて"))
                            .child(KitTab::new().label(format!("要対応 {attention}")))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                this.attention_only = *index == 1;
                                cx.notify();
                            })),
                    ),
            )
            .child(SidebarGroup::new(format!("タスク  {} / 8", self.sessions.len())).child(menu))
            .footer(
                div()
                    .w_full()
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Button::icon("next-attention", Icon::Bell, "次の要対応へ · ⌘ ⇧ A")
                            .disabled(attention == 0)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.next_attention(window, cx)),
                            ),
                    )
                    .child(ds::indicator(
                        "storage-info",
                        Icon::Info,
                        "",
                        "履歴・順番待ち・レビュー記録はアプリ起動中のみ保持",
                        Tone::Neutral,
                        cx,
                    )),
            )
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let view = &self.sessions[self.selected];
        let session = &view.model;
        div()
            .h(px(TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(space::XL))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(space::MD))
            .border_b_1()
            .border_color(rgb(p.border))
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(Icon::Folder.view(p.muted))
                    .child(
                        caption(self.workspace_name.clone(), cx)
                            .max_w(px(120.))
                            .truncate(),
                    )
                    .child(Icon::ChevronRight.view(p.disabled).size(px(12.)))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(session.title.clone()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(ds::badge(view.state_label(), view.state_tone(), cx))
                    .child(self.approval_mode_button(cx))
                    .child(
                        Button::icon("command-rules", Icon::Sliders, "コマンド実行ルール")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_command_rules(window, cx)
                            })),
                    )
                    .child(
                        Button::icon(
                            "toggle-theme",
                            if ds::scheme(cx) == ColorScheme::Dark {
                                Icon::Sun
                            } else {
                                Icon::Moon
                            },
                            "テーマを切り替える · ⌘ ⇧ L",
                        )
                        .control_size(ControlSize::Small)
                        .on_click(|_, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx)),
                    )
                    .child(
                        Button::icon("close-session", Icon::Close, "このセッションを閉じる")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_close_session(window, cx)
                            })),
                    ),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected];
        let demo = session.selected_backend > self.acp_agents.len();
        div()
            .flex_shrink_0()
            .px(px(space::LG))
            .py(px(space::SM))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                TabBar::new("workspace-tabs")
                    .underline()
                    .small()
                    .selected_index(session.tab as usize)
                    .child(KitTab::new().label("概要"))
                    .child(KitTab::new().label("会話"))
                    .child(KitTab::new().label(format!("変更 {}", session.model.diffs.len())))
                    .child(KitTab::new().label("ログ"))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.show_tab([Tab::Overview, Tab::Chat, Tab::Diff, Tab::Logs][*index], cx);
                    })),
            )
            .when(demo, |v| {
                v.child(
                    Button::icon("replay", Icon::Play, "デモを再生")
                        .variant(ButtonVariant::Ghost)
                        .control_size(ControlSize::Small)
                        .disabled(session.model.status.is_active())
                        .on_click(cx.listener(|this, _, _, cx| {
                            let selected = this.sessions[this.selected].selected_backend;
                            if let Some(scenario) =
                                SCENARIOS.get(selected.saturating_sub(1 + this.acp_agents.len()))
                            {
                                this.scenario(*scenario, cx);
                            }
                        })),
                )
            })
    }

    fn notices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = &self.sessions[self.selected];
        let model = &session.model;
        let issue = matches!(model.status, Status::Failed | Status::Disconnected);
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .when(self.queue.paused, |v| {
                v.child(
                    notice(
                        Icon::Pause,
                        format!("順番待ち {} · 一時停止", self.queue.len()),
                        Tone::Warning,
                        cx,
                    )
                    .child(
                        Button::icon("resume-queue", Icon::Play, "順番待ちの自動実行を再開")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.queue.paused = false;
                                this.dispatch_queue(cx);
                                cx.notify();
                            })),
                    ),
                )
            })
            .when(
                (session.approval.is_some() || session.login.is_some())
                    && session.tab != Tab::Overview,
                |v| {
                    v.child(
                        notice(Icon::Bell, session.state_label(), Tone::Warning, cx).child(
                            Button::icon("open-attention", Icon::ChevronRight, "要求を確認")
                                .control_size(ControlSize::Small)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.show_tab(Tab::Overview, cx)),
                                ),
                        ),
                    )
                },
            )
            .when(!self.message.is_empty(), |v| {
                v.child(
                    notice(Icon::Warning, self.message.clone(), Tone::Warning, cx).child(
                        Button::icon("dismiss-notice", Icon::Close, "通知を閉じる")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.message.clear();
                                cx.notify();
                            })),
                    ),
                )
            })
            .when(model.unknown > 0 || model.rejected > 0, |v| {
                v.child(
                    notice(
                        Icon::Warning,
                        format!("未対応 {} · 不正 {}", model.unknown, model.rejected),
                        Tone::Warning,
                        cx,
                    )
                    .child(
                        Button::icon("event-issues", Icon::Terminal, "イベントの詳細をログで確認")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| this.show_tab(Tab::Logs, cx))),
                    ),
                )
            })
            .when(issue && !model.reason.is_empty(), |v| {
                v.child(notice(
                    Icon::Warning,
                    model.reason.clone(),
                    status_tone(model.status),
                    cx,
                ))
            })
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let s = &self.sessions[self.selected];
        div()
            .flex_shrink_0()
            .px(px(space::LG))
            .py(px(space::XS))
            .bg(rgb(p.surface))
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .when(self.show_metrics, |v| {
                v.child(ds::card(cx).p(px(space::MD)).child(caption(
                    format!(
                        "表示更新 {} 回 · 最大 {:.2} ms · 重複 {} 件 · 未対応 {} 件 · 不正 {} 件",
                        s.batches,
                        s.max_batch_ms,
                        s.model.duplicates,
                        s.model.unknown,
                        s.model.rejected
                    ),
                    cx,
                )))
            })
            .child(
                StatusBar::new()
                    .left(caption(format!("受信 {} 件", s.model.accepted), cx))
                    .right(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(caption(
                                format!(
                                    "トークン {} / {} · コスト {}",
                                    number(s.model.usage.input_tokens),
                                    number(s.model.usage.output_tokens),
                                    s.model
                                        .usage
                                        .cost_usd
                                        .map(|n| format!("${n:.4}"))
                                        .unwrap_or_else(|| "不明".into())
                                ),
                                cx,
                            ))
                            .child(
                                Button::icon("metrics", Icon::Activity, "計測値を表示")
                                    .toggled(self.show_metrics)
                                    .variant(ButtonVariant::Ghost)
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_metrics = !this.show_metrics;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered += 1;
        let session = &self.sessions[self.selected];
        let pane = match session.tab {
            Tab::Overview => self.overview(cx),
            Tab::Chat => self.conversation(cx),
            Tab::Diff => self.diff(cx),
            Tab::Logs => self.logs(cx),
        };
        ds::root(cx)
            .relative()
            .flex()
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &ToggleTheme, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| {
                this.new_session(window, cx);
                window.focus(&this.sessions[this.selected].composer.focus_handle(cx), cx);
            }))
            .on_action(
                cx.listener(|this, _: &ShowOverview, _, cx| this.show_tab(Tab::Overview, cx)),
            )
            .on_action(
                cx.listener(|this, _: &NextAttention, window, cx| this.next_attention(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowChat, _, cx| this.show_tab(Tab::Chat, cx)))
            .on_action(cx.listener(|this, _: &ShowDiff, _, cx| this.show_tab(Tab::Diff, cx)))
            .on_action(cx.listener(|this, _: &ShowLogs, _, cx| this.show_tab(Tab::Logs, cx)))
            .child(
                h_resizable("workspace-layout")
                    .child(
                        resizable_panel()
                            .size(px(SIDEBAR_WIDTH))
                            .size_range(px(190.)..px(340.))
                            .child(self.sidebar(cx)),
                    )
                    .child(
                        resizable_panel().size_range(px(470.)..px(5000.)).child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .child(self.header(cx))
                                .child(self.toolbar(cx))
                                .child(
                                    div()
                                        .id("workspace-notices")
                                        .max_h(px(150.))
                                        .overflow_y_scroll()
                                        .flex_shrink_0()
                                        .child(self.notices(cx)),
                                )
                                .child(div().flex_1().min_h_0().overflow_hidden().child(pane))
                                .when(session.model.chat_discarded > 0, |v| {
                                    v.child(div().px(px(space::XL)).py(px(space::XS)).child(
                                        caption(
                                            format!("会話 {} 件省略", session.model.chat_discarded),
                                            cx,
                                        ),
                                    ))
                                })
                                .child(self.composer(cx))
                                .child(self.footer(cx)),
                        ),
                    ),
            )
    }
}

fn empty(icon: Icon, title: &'static str, cx: &App) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .p_4()
        .child(icon.view(ds::theme(cx).muted).size(px(28.)))
        .child(caption(title, cx))
}
fn notice(icon: Icon, text: impl Into<SharedString>, tone: Tone, cx: &App) -> Div {
    let (fg, bg) = ds::theme(cx).tone(tone);
    div()
        .mx_4()
        .mt_2()
        .px_3()
        .py_2()
        .rounded(px(ds::radius::CONTROL))
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .flex()
        .items_center()
        .gap_2()
        .child(icon.view(fg))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .child(text.into()),
        )
}
pub(super) fn caption(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(typography::CAPTION))
        .text_color(rgb(ds::theme(cx).muted))
        .child(text.into())
}
pub(super) fn status_tone(status: Status) -> Tone {
    match status {
        Status::Failed | Status::Disconnected => Tone::Danger,
        Status::Cancelling => Tone::Warning,
        Status::Connecting | Status::Running => Tone::Accent,
        Status::Completed => Tone::Success,
        _ => Tone::Neutral,
    }
}
pub(super) fn status_icon(status: Status) -> Icon {
    match status {
        Status::Failed | Status::Disconnected => Icon::Warning,
        Status::Connecting | Status::Running | Status::Cancelling => Icon::Spinner,
        Status::Completed => Icon::Check,
        _ => Icon::Layers,
    }
}
fn number(value: Option<u64>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "不明".into())
}

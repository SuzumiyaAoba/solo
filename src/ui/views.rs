//! ワークスペースの共通レイアウトとナビゲーション。各ペインは子モジュールで描画する。
mod chat;
mod composer;
mod diff;
mod logs;
mod thread;

use super::*;
use gpui_kit::component::{
    Sizable, h_resizable, resizable_panel,
    status_bar::StatusBar,
    tab::{Tab as KitTab, TabBar},
};

const CHANNEL_HEADER_HEIGHT: f32 = 60.;
/// ツールバーのタブと `Tab` の対応順。選択位置の復元とクリック先で共有する。
const TAB_ORDER: [Tab; 4] = [Tab::Chat, Tab::Diff, Tab::Overview, Tab::Logs];

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
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let view = self.session();
        let session = &view.model;
        div()
            .h(px(CHANNEL_HEADER_HEIGHT))
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
                    .child(div().text_size(px(24.)).text_color(rgb(p.muted)).child("#"))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(session.title().to_owned()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(ds::badge(view.state_label(), view.state_tone(), cx))
                    .child(
                        Button::icon("open-thread", Icon::MessageSquare, "実行スレッド · ⌘ ⇧ T")
                            .control_size(ControlSize::Small)
                            .toggled(
                                view.view.selected_thread.is_some() && view.view.tab == Tab::Chat,
                            )
                            .disabled(view.model.threads().is_empty())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_thread(window, cx)),
                            ),
                    )
                    .child(self.approval_mode_button(cx))
                    .child(
                        Button::icon(
                            "export-session",
                            Icon::ExternalLink,
                            "会話とイベントを書き出す",
                        )
                        .control_size(ControlSize::Small)
                        .disabled(view.model.accepted() == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.export_session(cx))),
                    )
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
        let session = self.session();
        let demo = !self.picker_uses_workspace(session.selected_backend);
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
                    .selected_index(
                        TAB_ORDER
                            .iter()
                            .position(|tab| *tab == session.view.tab)
                            .unwrap_or(0),
                    )
                    // ラベルは TAB_ORDER から生成し、表示とクリック先の対応がずれないようにする。
                    .children(TAB_ORDER.iter().map(|tab| {
                        KitTab::new().label(match tab {
                            Tab::Chat => "会話".into(),
                            Tab::Diff => format!("変更 {}", session.model.diffs().len()),
                            Tab::Overview => "概要".into(),
                            Tab::Logs => "ログ".into(),
                        })
                    }))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.show_tab(TAB_ORDER[*index], cx);
                    })),
            )
            .child(
                Button::icon("show-metrics", Icon::Activity, "実行の計測値を表示")
                    .control_size(ControlSize::Small)
                    .toggled(self.show_metrics)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_metrics = !this.show_metrics;
                        cx.notify();
                    })),
            )
            .when(demo, |v| {
                v.child(
                    Button::icon("replay", Icon::Play, "デモを再生")
                        .variant(ButtonVariant::Ghost)
                        .control_size(ControlSize::Small)
                        .disabled(session.display_status().is_active())
                        .on_click(cx.listener(|this, _, _, cx| {
                            let selected = this.session().selected_backend;
                            if let Some(scenario) = this.mock_scenario(selected) {
                                this.scenario(scenario, cx);
                            }
                        })),
                )
            })
    }

    fn notices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session();
        let model = &session.model;
        let issue = matches!(model.status(), Status::Failed | Status::Disconnected);
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .when(self.queue.paused(), |v| {
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
                                this.queue.set_paused(false);
                                this.save_workspace_state(cx);
                                this.dispatch_queue(cx);
                                cx.notify();
                            })),
                    ),
                )
            })
            .when(
                (session.approval.is_some() || session.login.is_some())
                    && session.view.tab != Tab::Overview,
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
            .when(model.unknown() > 0 || model.rejected() > 0, |v| {
                v.child(
                    notice(
                        Icon::Warning,
                        format!("未対応 {} · 不正 {}", model.unknown(), model.rejected()),
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
            .when(issue && !model.reason().is_empty(), |v| {
                v.child(notice(
                    Icon::Warning,
                    model.reason(),
                    status_tone(session.display_status()),
                    cx,
                ))
            })
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let s = self.session();
        div()
            .flex_shrink_0()
            .px(px(space::LG))
            .py(px(space::XS))
            .bg(ds::glass(p.surface, ds::GLASS_SURFACE))
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .when(self.show_metrics, |v| {
                v.child(ds::card(cx).p(px(space::MD)).child(caption(
                    format!(
                        "表示更新 {} 回 · 最大 {:.2} ms · 重複 {} 件 · 未対応 {} 件 · 不正 {} 件",
                        s.metrics.batches,
                        s.metrics.max_batch_ms,
                        s.model.duplicates(),
                        s.model.unknown(),
                        s.model.rejected()
                    ),
                    cx,
                )))
            })
            .child(
                StatusBar::new()
                    .left(caption(format!("受信 {} 件", s.model.accepted()), cx))
                    .right(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(caption(
                                format!(
                                    "トークン {} / {} · コスト {}",
                                    number(s.model.usage().input_tokens),
                                    number(s.model.usage().output_tokens),
                                    s.model
                                        .usage()
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered += 1;
        let session = self.session();
        let thread_open = session.view.tab == Tab::Chat && session.view.selected_thread.is_some();
        let compact_thread = thread_open && window.viewport_size().width < px(1120.);
        if compact_thread && session.composer.focus_handle(cx).is_focused(window) {
            window.focus(&self.focus, cx);
        }
        let pane = match session.view.tab {
            Tab::Overview => self.overview(cx),
            Tab::Chat => self.conversation(cx),
            Tab::Diff => self.diff(cx),
            Tab::Logs => self.logs(cx),
        };
        let channel = div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().overflow_hidden().child(pane))
            .when(session.model.chat_discarded() > 0, |v| {
                v.child(div().px(px(space::XL)).child(caption(
                    format!("会話 {} 件省略", session.model.chat_discarded()),
                    cx,
                )))
            })
            .child(self.composer(window, cx))
            .when(self.show_metrics, |v| v.child(self.footer(cx)));
        let body = if compact_thread {
            self.execution_thread(cx).into_any_element()
        } else if thread_open {
            h_resizable("conversation-thread-layout")
                .child(
                    resizable_panel()
                        .size_range(px(340.)..px(5000.))
                        .child(channel),
                )
                .child(
                    resizable_panel()
                        .size(px(350.))
                        .size_range(px(300.)..px(520.))
                        .child(self.execution_thread(cx)),
                )
                .into_any_element()
        } else {
            channel.into_any_element()
        };
        ds::root(cx)
            .track_focus(&self.focus)
            .relative()
            .flex()
            .flex_col()
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &ToggleTheme, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx))
            .on_action(
                cx.listener(|this, _: &ShowOverview, _, cx| this.show_tab(Tab::Overview, cx)),
            )
            .on_action(
                cx.listener(|this, _: &NextAttention, window, cx| this.next_attention(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowChat, _, cx| this.show_tab(Tab::Chat, cx)))
            .on_action(cx.listener(|this, _: &ShowDiff, _, cx| this.show_tab(Tab::Diff, cx)))
            .on_action(cx.listener(|this, _: &ShowLogs, _, cx| this.show_tab(Tab::Logs, cx)))
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
            .child(div().flex_1().min_h_0().overflow_hidden().child(body))
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
        Status::Running => Tone::Accent,
        Status::Completed => Tone::Success,
        _ => Tone::Neutral,
    }
}
/// サイドバーの各チャンネル右端に表示する状態アイコン。実行の段階ごとに形を変える。
pub(super) fn status_icon(status: Status) -> Icon {
    match status {
        Status::Idle => Icon::Layers,
        Status::Running => Icon::Spinner,
        Status::Cancelling => Icon::Pause,
        Status::Completed => Icon::CircleCheck,
        Status::Cancelled => Icon::Square,
        Status::Failed => Icon::Warning,
        Status::Disconnected => Icon::Unplug,
    }
}
fn number(value: Option<u64>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "不明".into())
}

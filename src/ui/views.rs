use super::*;
use gpui_kit::component::{
    Sizable, h_resizable,
    message::{Message as KitMessage, MessageContent, MessageHeader},
    message_scroller::MessageScroller,
    resizable_panel,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    tab::{Tab as KitTab, TabBar},
};

const TITLEBAR_HEIGHT: f32 = 46.;
const SIDEBAR_WIDTH: f32 = 220.;

impl Workspace {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let menu = SidebarMenu::new().children(self.sessions.iter().map(|session| {
            let id = session.model.id.clone();
            let active = session.model.id == self.sessions[self.selected].model.id;
            let status = session.model.status;
            SidebarMenuItem::new(session.model.title.clone())
                .icon(status_icon(status).kit())
                .active(active)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.select_session(&id, window, cx)),
                )
        }));
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
                            .child(ds::avatar("S", Tone::Accent, cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(typography::LEAD))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Solo"),
                                    )
                                    .child(caption(self.workspace_name.clone(), cx).truncate()),
                            ),
                    )
                    .child(
                        Button::new("new-session", "新しいセッション")
                            .with_icon(Icon::Plus)
                            .w_full()
                            .justify_start()
                            .tooltip("新しいセッション · ⌘ N")
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
                SidebarGroup::new(format!("セッション  {} / 8", self.sessions.len())).child(menu),
            )
            .footer(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(caption("テーマ", cx))
                            .child(ds::keycap("⌘ ⇧ L", cx)),
                    )
                    .child(
                        TabBar::new("appearance")
                            .segmented()
                            .small()
                            .selected_index(usize::from(ds::scheme(cx) == ColorScheme::Dark))
                            .child(KitTab::new().label("Light"))
                            .child(KitTab::new().label("Dark"))
                            .on_click(|index, _, cx| {
                                ds::set_theme(
                                    if *index == 0 {
                                        ColorScheme::Light
                                    } else {
                                        ColorScheme::Dark
                                    },
                                    cx,
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::Folder.view(p.muted))
                            .child(caption("Subscription / ACP / デモ", cx)),
                    ),
            )
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected].model;
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
                    .child(ds::badge(
                        session.status.label(),
                        status_tone(session.status),
                        cx,
                    ))
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
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_session(window, cx)),
                            ),
                    ),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected];
        div()
            .flex_shrink_0()
            .px(px(space::LG))
            .py(px(space::SM))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(space::SM))
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                TabBar::new("workspace-tabs")
                    .underline()
                    .small()
                    .selected_index(session.tab as usize)
                    .child(KitTab::new().label("会話"))
                    .child(KitTab::new().label(format!("差分 {}", session.model.diffs.len())))
                    .child(KitTab::new().label("ログ"))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.show_tab([Tab::Chat, Tab::Diff, Tab::Logs][*index], cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(div().w(px(184.)).child(self.scenario_picker.clone()))
                    .child(
                        Button::new(
                            "replay",
                            if self.scenario_picker.read(cx).selected <= self.acp_agents.len() {
                                "実行"
                            } else {
                                "再生"
                            },
                        )
                        .with_icon(Icon::ArrowRight)
                        .disabled(session.model.status.is_active())
                        .tooltip("選択した実行先で開始します")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let selected = this.scenario_picker.read(cx).selected;
                            if selected <= this.acp_agents.len() {
                                this.start_selected(
                                    this.selected,
                                    "この workspace の状況を確認してください。".into(),
                                    cx,
                                );
                            } else {
                                this.start_selected(
                                    this.selected,
                                    format!(
                                        "{}の表示と操作を検証します。",
                                        SCENARIOS[selected - 1 - this.acp_agents.len()].label()
                                    ),
                                    cx,
                                );
                            }
                        })),
                    ),
            )
    }

    fn notices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let model = &self.sessions[self.selected].model;
        let issue = matches!(model.status, Status::Failed | Status::Disconnected);
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .when(!self.message.is_empty(), |v| {
                v.child(
                    div().px(px(space::LG)).pt(px(space::SM)).child(
                        ds::alert(
                            "操作を完了できませんでした",
                            self.message.clone(),
                            Tone::Warning,
                            cx,
                        )
                        .child(
                            Button::icon("dismiss-notice", Icon::Close, "通知を閉じる")
                                .control_size(ControlSize::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message.clear();
                                    cx.notify();
                                })),
                        ),
                    ),
                )
            })
            .when(model.unknown > 0 || model.rejected > 0, |v| {
                v.child(div().px(px(space::LG)).pt(px(space::SM)).child(ds::alert(
                    "確認が必要なイベントがあります",
                    format!(
                        "未対応 {} 件 · 不正・順序違反 {} 件。ログで詳細を確認できます。",
                        model.unknown, model.rejected
                    ),
                    Tone::Warning,
                    cx,
                )))
            })
            .when(issue && !model.reason.is_empty(), |v| {
                v.child(div().px(px(space::LG)).pt(px(space::SM)).child(ds::alert(
                    model.status.label(),
                    model.reason.clone(),
                    status_tone(model.status),
                    cx,
                )))
            })
    }

    fn conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.chat.is_empty() {
            return empty(
                Icon::Layers,
                "次の作業を、ここから。",
                "メッセージを入力し、Subscription または ACP agent で実行できます。",
                cx,
            )
            .child(
                div()
                    .flex()
                    .gap(px(space::SM))
                    .child(
                        Button::new("empty-run", "選択した実行先で開始")
                            .with_icon(Icon::ArrowRight)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_selected(
                                    this.selected,
                                    "この workspace の状況を確認してください。".into(),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("empty-replay", "サンプルを再生").on_click(
                            cx.listener(|this, _, _, cx| this.scenario(Scenario::Demo, cx)),
                        ),
                    ),
            )
            .into_any_element();
        }
        let weak = cx.weak_entity();
        let session_id = session.model.id.clone();
        MessageScroller::new(
            "conversation",
            session.chat_list.clone(),
            move |row, _, cx| {
                let Some(view) = weak.upgrade() else {
                    return div().into_any_element();
                };
                let Some(block) = view
                    .read(cx)
                    .sessions
                    .iter()
                    .find(|s| s.model.id == session_id)
                    .and_then(|s| s.model.chat.get(row))
                else {
                    return div().into_any_element();
                };
                let p = ds::theme(cx);
                let copy = block.text.clone();
                let callback_view = weak.clone();
                let copy_button = Button::icon(("copy-chat", row), Icon::Copy, "この部分をコピー")
                    .control_size(ControlSize::Small)
                    .on_click(move |_, _, cx| {
                        let _ = callback_view.update(cx, |this, cx| {
                            this.copy(copy.clone(), "本文をコピーしました", cx)
                        });
                    });
                let (name, initial, tone) = match block.speaker {
                    Speaker::User => ("あなた", "You", Tone::Neutral),
                    Speaker::Assistant => ("Solo", "S", Tone::Accent),
                    Speaker::Notice => ("状態の更新", "", Tone::Neutral),
                };
                let body = if block.speaker == Speaker::Notice {
                    ds::alert(name, block.text.clone(), tone, cx)
                        .child(copy_button)
                        .into_any_element()
                } else {
                    KitMessage::new()
                        .w_full()
                        .avatar(ds::avatar(initial, tone, cx))
                        .header(
                            MessageHeader::new()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().font_weight(FontWeight::MEDIUM).child(name))
                                        .when(
                                            block.speaker == Speaker::Assistant
                                                && view
                                                    .read(cx)
                                                    .sessions
                                                    .iter()
                                                    .find(|s| s.model.id == session_id)
                                                    .is_some_and(|s| {
                                                        !s.is_subscription && !s.is_acp
                                                    }),
                                            |v| v.child(ds::badge("疑似応答", Tone::Neutral, cx)),
                                        ),
                                )
                                .child(copy_button),
                        )
                        .content(
                            MessageContent::new()
                                .w_full()
                                .text_size(px(typography::LEAD))
                                .line_height(px(typography::LEAD + space::SM))
                                .text_color(rgb(p.text))
                                .child(block.text.clone()),
                        )
                        .into_any_element()
                };
                div()
                    .w_full()
                    .px(px(space::XL))
                    .py(px(space::SM))
                    .child(div().w_full().max_w(px(840.)).mx_auto().child(body))
                    .into_any_element()
            },
        )
        .with_jump_button_label("最新のメッセージへ")
        .size_full()
        .into_any_element()
    }

    fn logs(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.logs.is_empty() {
            return empty(
                Icon::Text,
                "ログはまだありません",
                "実行すると、コマンドと出力がここに表示されます。",
                cx,
            )
            .into_any_element();
        }
        let p = ds::theme(cx);
        div()
            .id("log-container")
            .size_full()
            .flex()
            .flex_col()
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                let session = &mut this.sessions[this.selected];
                if session.follow_logs {
                    session.follow_logs = false;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::LG))
                    .py(px(space::SM))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(space::SM))
                    .child(caption(
                        format!(
                            "最新 {} 件 · 全文 {:.2} MiB",
                            session.model.logs.len(),
                            session.model.log_bytes as f64 / 1048576.
                        ),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::SM))
                            .child(
                                ds::Toggle::switch("follow-logs", "自動追従", session.follow_logs)
                                    .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                                        let session = &mut this.sessions[this.selected];
                                        session.follow_logs = *enabled;
                                        if *enabled {
                                            session.log_scroll.scroll_to_item(
                                                session.model.logs.len().saturating_sub(1),
                                                ScrollStrategy::Bottom,
                                            );
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::icon("log-tail", Icon::ChevronDown, "最新のログへ")
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let s = &mut this.sessions[this.selected];
                                        s.follow_logs = true;
                                        s.log_scroll.scroll_to_item(
                                            s.model.logs.len().saturating_sub(1),
                                            ScrollStrategy::Bottom,
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("log-path", "全文のパスをコピー")
                                    .with_icon(Icon::Copy)
                                    .control_size(ControlSize::Small)
                                    .disabled(session.artifacts.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let paths = this.sessions[this.selected]
                                            .artifacts
                                            .iter()
                                            .map(|path| path.display().to_string())
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        this.copy(paths, "ログのパスをコピーしました", cx);
                                    })),
                            ),
                    ),
            )
            .child(div().px(px(space::LG)).pb(px(space::SM)).child(caption(
                format!(
                    "行をクリックしてコピー{}",
                    if session.model.logs_discarded > 0 {
                        format!(
                            " · 表示範囲外 {} 件は全文ログで確認できます",
                            session.model.logs_discarded
                        )
                    } else {
                        String::new()
                    }
                ),
                cx,
            )))
            .child(ds::divider(cx))
            .child(
                uniform_list(
                    "log-rows",
                    session.model.logs.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = ds::theme(cx);
                        let s = &this.sessions[this.selected];
                        range
                            .filter_map(|i| {
                                s.model.logs.get(i).map(|row| {
                                    let text = row.text.clone();
                                    div()
                                        .id(i)
                                        .h(px(ControlSize::Small.height()))
                                        .px(px(space::LG))
                                        .flex()
                                        .items_center()
                                        .gap(px(space::MD))
                                        .font_family(typography::MONO)
                                        .text_size(px(typography::LABEL))
                                        .text_color(rgb(p.text))
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .bg(rgb(if i % 2 == 0 { p.canvas } else { p.surface }))
                                        .hover(move |style| style.bg(rgb(p.hover)))
                                        .child(
                                            div()
                                                .w(px(64.))
                                                .flex_shrink_0()
                                                .text_color(rgb(p.muted))
                                                .child(format!("{:06}", row.sequence)),
                                        )
                                        .child(
                                            div()
                                                .w(px(44.))
                                                .flex_shrink_0()
                                                .text_color(rgb(if row.level == "event" {
                                                    p.warning
                                                } else {
                                                    p.accent_text
                                                }))
                                                .child(row.level.clone()),
                                        )
                                        .child(
                                            div().flex_1().min_w_0().truncate().child(text.clone()),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy(text.clone(), "ログをコピーしました", cx)
                                        }))
                                })
                            })
                            .collect()
                    }),
                )
                .track_scroll(&session.log_scroll)
                .flex_1()
                .min_h_0(),
            )
            .bg(rgb(p.canvas))
            .into_any_element()
    }

    fn diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.sessions[self.selected];
        let Some(diff) = s.model.diffs.get(s.diff_index) else {
            return empty(
                Icon::Folder,
                "差分はまだありません",
                "変更があると、ここに差分が表示されます。",
                cx,
            )
            .into_any_element();
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::LG))
                    .py(px(space::MD))
                    .flex()
                    .flex_wrap()
                    .gap(px(space::SM))
                    .child(
                        TabBar::new("diff-files")
                            .underline()
                            .small()
                            .max_width(px(240.))
                            .selected_index(s.diff_index)
                            .children(
                                s.model
                                    .diffs
                                    .iter()
                                    .map(|diff| KitTab::new().label(diff.path.clone())),
                            )
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                let session = &mut this.sessions[this.selected];
                                session.diff_index = *index;
                                session.diff_scroll = UniformListScrollHandle::new();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::LG))
                    .pb(px(space::MD))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::SM))
                    .child(ds::badge("追加 +", Tone::Success, cx))
                    .child(ds::badge("削除 −", Tone::Danger, cx))
                    .child(caption(
                        format!("{} 行 · 行をクリックしてコピー", diff.lines.len()),
                        cx,
                    )),
            )
            .when(diff.truncated, |v| {
                v.child(div().px(px(space::LG)).pb(px(space::MD)).child(ds::alert(
                    "差分の表示上限に達しました",
                    "表示は先頭20,000行までです。",
                    Tone::Warning,
                    cx,
                )))
            })
            .child(ds::divider(cx))
            .child(
                uniform_list(
                    "diff-lines",
                    diff.lines.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = ds::theme(cx);
                        let s = &this.sessions[this.selected];
                        let diff = &s.model.diffs[s.diff_index];
                        range
                            .map(|i| {
                                let line = &diff.lines[i];
                                let (background, color) = match line.kind {
                                    DiffKind::Added => (p.success_soft, p.success),
                                    DiffKind::Removed => (p.danger_soft, p.danger),
                                    DiffKind::Header => (p.accent_soft, p.accent_text),
                                    DiffKind::Context => (p.canvas, p.text),
                                };
                                let text = line.text.clone();
                                div()
                                    .id(i)
                                    .h(px(ControlSize::Small.height()))
                                    .px(px(space::LG))
                                    .flex()
                                    .items_center()
                                    .gap(px(space::MD))
                                    .font_family(typography::MONO)
                                    .text_size(px(typography::LABEL))
                                    .bg(rgb(background))
                                    .text_color(rgb(color))
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .w(px(36.))
                                            .flex_shrink_0()
                                            .text_color(rgb(p.secondary))
                                            .child(
                                                line.old.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(36.))
                                            .flex_shrink_0()
                                            .text_color(rgb(p.secondary))
                                            .child(
                                                line.new.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(div().flex_1().min_w_0().truncate().child(text.clone()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.copy(text.clone(), "差分をコピーしました", cx)
                                    }))
                            })
                            .collect()
                    }),
                )
                .track_scroll(&s.diff_scroll)
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected];
        let active = session.model.status.is_active();
        let cancelling = session.model.status == Status::Cancelling;
        let composer = session.composer.clone();
        div()
            .flex_shrink_0()
            .px(px(space::XL))
            .py(px(space::LG))
            .border_t_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .max_w(px(840.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(space::MD))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(space::SM))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .items_center()
                                    .gap(px(space::SM))
                                    .child(ds::avatar("S", Tone::Accent, cx))
                                    .child(caption(session.model.provider.clone(), cx).truncate()),
                            )
                            .child(div().flex().items_center().gap(px(space::SM))
                                .when(session.selected_backend == 0
                                    && !active
                                    && session.backend_id.as_deref().is_none_or(|id| id == "subscription"), |v| {
                                    v.child(Button::new("chatgpt-login", "ChatGPT ログイン")
                                        .control_size(ControlSize::Small)
                                        .on_click(cx.listener(|this, _, _, cx| this.start_login(cx))))
                                })
                                .child(ds::badge(
                                    if session.login_only { "ログイン待ち" } else if active { "受信中も入力できます" } else { "入力を待っています" },
                                    if active { Tone::Accent } else { Tone::Neutral },
                                    cx,
                                ))),
                    )
                    .when_some(session.login.as_ref(), |v, login| {
                        let url = login.verification_url.clone();
                        let code = login.user_code.clone();
                        let browser_url = url.clone();
                        v.child(ds::card(cx).p(px(space::MD)).flex().flex_col().gap(px(space::SM))
                            .child(caption("認証ページを開き、コードをクリップボードにコピーしました。ブラウザーに貼り付けてください。", cx))
                            .child(caption(format!("認証コード: {}", code), cx))
                            .child(caption(url.clone(), cx))
                            .child(div().flex().gap(px(space::SM))
                                .child(Button::new("open-login-url", "ブラウザーで開く").variant(ButtonVariant::Primary).control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&browser_url))))
                                .child(Button::new("copy-login-url", "URL をコピー").control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |this, _, _, cx| this.copy(url.clone(), "URL をコピーしました", cx))))
                                .child(Button::new("copy-login-code", "コードをコピー").control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |this, _, _, cx| this.copy(code.clone(), "コードをコピーしました", cx)))))
                        )
                    })
                    .when_some(session.approval.as_ref(), |v, approval| {
                        let details = approval.params.to_string();
                        v.child(ds::card(cx).p(px(space::MD)).flex().flex_col().gap(px(space::SM))
                            .child(caption(format!("実行の承認要求: {}", approval.method), cx))
                            .child(caption(solo::event::preview(&details, 1024), cx))
                            .child(div().flex().gap(px(space::SM))
                                .child(Button::new("approval-copy", "詳細をコピー").control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |this, _, _, cx| this.copy(details.clone(), "承認要求の詳細をコピーしました", cx))))
                                .child(Button::new("approval-decline", "拒否").control_size(ControlSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| this.answer_approval(false, cx))))
                                .child(Button::new("approval-accept", "今回だけ許可").variant(ButtonVariant::Primary).control_size(ControlSize::Small)
                                    .disabled(approval.method == "session/request_permission" && !approval.params["options"].as_array().is_some_and(|options| options.iter().any(|option| option["kind"] == "allow_once")))
                                    .on_click(cx.listener(|this, _, _, cx| this.answer_approval(true, cx)))))
                        )
                    })
                    .child(composer.clone())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(px(space::SM))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::SM))
                                    .child(ds::keycap("⌘ ↵", cx))
                                    .child(caption("送信 · Enter で改行", cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::SM))
                                    .when(active, |v| {
                                        v.when(!session.is_subscription && !session.is_acp, |v| v.child(
                                            Button::new("disconnect", "切断を試す")
                                                .variant(ButtonVariant::Ghost)
                                                .control_size(ControlSize::Small)
                                                .disabled(cancelling)
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    if let Some(controller) =
                                                        &this.sessions[this.selected].controller
                                                    {
                                                        controller.disconnect();
                                                    }
                                                })),
                                        ))
                                        .child(
                                            Button::new(
                                                "cancel",
                                                if cancelling {
                                                    "停止要求中"
                                                } else {
                                                    "中止"
                                                },
                                            )
                                            .variant(ButtonVariant::Danger)
                                            .control_size(ControlSize::Small)
                                            .disabled(cancelling)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.cancel(cx)),
                                            ),
                                        )
                                    })
                                    .child(
                                        Button::new("submit", "送信")
                                            .variant(ButtonVariant::Primary)
                                            .trailing_icon(Icon::ArrowRight)
                                            .control_size(ControlSize::Small)
                                            .disabled(active)
                                            .on_click(move |_, window, cx| {
                                                composer.update(cx, |input, cx| input.submit(window, cx))
                                            }),
                                    ),
                            ),
                    ),
            )
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
                                Button::new("metrics", "計測")
                                    .with_icon(Icon::Sliders)
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
        let session = &self.sessions[self.selected];
        let pane = match session.tab {
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
                                .child(self.notices(cx))
                                .child(div().flex_1().min_h_0().overflow_hidden().child(pane))
                                .when(session.model.chat_discarded > 0, |v| {
                                    v.child(div().px(px(space::XL)).py(px(space::XS)).child(
                                        caption(
                                            format!(
                                                "表示上限のため古い会話を {} 件省略しています",
                                                session.model.chat_discarded
                                            ),
                                            cx,
                                        ),
                                    ))
                                })
                                .child(self.composer(cx))
                                .child(self.footer(cx)),
                        ),
                    ),
            )
            .child(self.toast.clone())
            .children(gpui_kit::component::Root::render_dialog_layer(window, cx))
            .children(gpui_kit::component::Root::render_notification_layer(
                window, cx,
            ))
    }
}

fn empty(icon: Icon, title: &'static str, description: &'static str, cx: &App) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::LG))
        .p(px(space::LG))
        .child(ds::empty_state(icon, title, description, cx))
}
fn caption(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(typography::CAPTION))
        .text_color(rgb(ds::theme(cx).muted))
        .child(text.into())
}
fn status_tone(status: Status) -> Tone {
    match status {
        Status::Failed | Status::Disconnected => Tone::Danger,
        Status::Cancelling => Tone::Warning,
        Status::Connecting | Status::Running => Tone::Accent,
        Status::Completed => Tone::Success,
        _ => Tone::Neutral,
    }
}
fn status_icon(status: Status) -> Icon {
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

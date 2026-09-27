use super::*;

impl Workspace {
    /// 入力欄・実行先・操作ボタンを一枚のカードにまとめるプロンプト入力。
    pub(super) fn composer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected];
        let active = session.model.status.is_active();
        let cancelling = session.model.status == Status::Cancelling;
        let queued = self.queue.position(&session.model.id).is_some();
        let will_queue = session.selected_backend <= self.acp_agents.len()
            && (self.workspace_busy() || !self.queue.is_empty() || self.queue.paused);
        let composer = session.composer.clone();
        let empty = composer.read(cx).value(cx).trim().is_empty();
        let focused = composer.focus_handle(cx).is_focused(window);
        // Subscription の初回ログインだけ促す。実行中・順番待ち・別 backend では出さない。
        let login = session.selected_backend == 0
            && !active
            && !queued
            && session
                .backend
                .as_ref()
                .is_none_or(|backend| *backend == Backend::Subscription);
        // 高さの半分の角丸で正方形のアイコンボタンを真円にする。
        let round = px(ControlSize::Medium.height() / 2.);
        div()
            .flex_shrink_0()
            .px(px(space::XL))
            .pt(px(space::XS))
            .pb(px(space::MD))
            .border_color(rgb(p.border))
            .child(
                div()
                    .max_w(px(840.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(space::SM))
                    // 状態チップは必要なときだけカードの上に出す。
                    .when(login || queued, |v| {
                        v.child(
                            div().flex().items_center().justify_between().gap_2().child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(login, |v| {
                                        v.child(
                                            Button::new("chatgpt-login", "ChatGPT にログイン")
                                                .with_icon(Icon::LogIn)
                                                .control_size(ControlSize::Small)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.start_login(cx);
                                                    this.show_tab(Tab::Overview, cx);
                                                })),
                                        )
                                    })
                                    .when(queued, |v| {
                                        v.child(ds::indicator(
                                            "composer-queued",
                                            Icon::Clock,
                                            "順番待ち",
                                            "順番待ち · 依頼を保存済み",
                                            Tone::Accent,
                                            cx,
                                        ))
                                    }),
                            ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(px(ds::radius::CARD))
                            .border_1()
                            .border_color(if focused {
                                Hsla::from(glass(p.accent, 0.9))
                            } else {
                                Hsla::from(glass(p.control_border, 0.7))
                            })
                            .bg(glass(p.surface, ds::GLASS_SURFACE))
                            .child(
                                div()
                                    .px(px(space::LG))
                                    .pt(px(space::MD))
                                    .child(composer.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px(px(space::MD))
                                    .pb(px(space::SM))
                                    .gap_2()
                                    .child(session.agent_avatar(cx))
                                    .child(div().w(px(200.)).child(self.scenario_picker.clone()))
                                    .child(div().flex_1())
                                    .child(ds::keycap("⌘ ↵", cx))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(space::SM))
                                            .when(active, |v| {
                                                v.when(!session.uses_workspace(), |v| {
                                                    v.child(
                                                        Button::icon(
                                                            "disconnect",
                                                            Icon::Unplug,
                                                            "デモの接続を切断",
                                                        )
                                                        .rounded(round)
                                                        .on_click(cx.listener(|this, _, _, _| {
                                                            if let Some(controller) = &this.sessions
                                                                [this.selected]
                                                                .controller
                                                            {
                                                                controller.disconnect();
                                                            }
                                                        })),
                                                    )
                                                })
                                                .child(
                                                    Button::icon(
                                                        "cancel",
                                                        if cancelling {
                                                            Icon::Spinner
                                                        } else {
                                                            Icon::Square
                                                        },
                                                        if cancelling {
                                                            "停止要求中"
                                                        } else {
                                                            "実行を中止"
                                                        },
                                                    )
                                                    .variant(ButtonVariant::Danger)
                                                    .rounded(round)
                                                    .disabled(cancelling)
                                                    .on_click(
                                                        cx.listener(|this, _, _, cx| {
                                                            this.cancel(cx)
                                                        }),
                                                    ),
                                                )
                                            })
                                            .when(queued, |v| {
                                                v.child(
                                                    Button::icon(
                                                        "cancel-queue",
                                                        Icon::Close,
                                                        "順番待ちを取り消す",
                                                    )
                                                    .rounded(round)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.cancel_queued(cx)
                                                    })),
                                                )
                                            })
                                            .when(!queued, |v| {
                                                v.child(
                                                    Button::icon(
                                                        "submit",
                                                        if will_queue && !active {
                                                            Icon::ListPlus
                                                        } else {
                                                            Icon::ArrowUp
                                                        },
                                                        if will_queue && !active {
                                                            "順番待ちに追加 · ⌘ Enter"
                                                        } else {
                                                            "送信 · ⌘ Enter"
                                                        },
                                                    )
                                                    .variant(ButtonVariant::Primary)
                                                    .rounded(round)
                                                    .disabled(
                                                        active || empty || session.input_composing,
                                                    )
                                                    .on_click(move |_, window, cx| {
                                                        composer.update(cx, |input, cx| {
                                                            input.submit(window, cx)
                                                        })
                                                    }),
                                                )
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}

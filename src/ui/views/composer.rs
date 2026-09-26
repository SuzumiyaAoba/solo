use super::*;

impl Workspace {
    pub(super) fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ds::theme(cx);
        let session = &self.sessions[self.selected];
        let active = session.model.status.is_active();
        let cancelling = session.model.status == Status::Cancelling;
        let queued = self.queue.position(&session.model.id).is_some();
        let will_queue = session.selected_backend <= self.acp_agents.len()
            && (self.workspace_busy() || !self.queue.is_empty() || self.queue.paused);
        let composer = session.composer.clone();
        let empty = composer.read(cx).value(cx).trim().is_empty();
        div()
            .flex_shrink_0()
            .px(px(space::XL))
            .py(px(space::MD))
            .border_t_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .max_w(px(840.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(session.agent_avatar(cx))
                                    .child(div().w(px(220.)).child(self.scenario_picker.clone())),
                            )
                            .when(
                                session.selected_backend == 0
                                    && !active
                                    && !queued
                                    && session
                                        .backend
                                        .as_ref()
                                        .is_none_or(|backend| *backend == Backend::Subscription),
                                |v| {
                                    v.child(
                                        Button::icon(
                                            "chatgpt-login",
                                            Icon::LogIn,
                                            "ChatGPT にログイン",
                                        )
                                        .control_size(ControlSize::Small)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.start_login(cx);
                                                this.show_tab(Tab::Overview, cx);
                                            }),
                                        ),
                                    )
                                },
                            )
                            .when(queued, |v| {
                                v.child(ds::indicator(
                                    "composer-queued",
                                    Icon::Clock,
                                    "",
                                    "順番待ち · 依頼を保存済み",
                                    Tone::Accent,
                                    cx,
                                ))
                            }),
                    )
                    .child(composer.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(ds::keycap("⌘ ↵", cx))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(active, |v| {
                                        v.when(!session.uses_workspace(), |v| {
                                            v.child(
                                                Button::icon(
                                                    "disconnect",
                                                    Icon::Unplug,
                                                    "デモの接続を切断",
                                                )
                                                .control_size(ControlSize::Small)
                                                .disabled(cancelling)
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    if let Some(controller) =
                                                        &this.sessions[this.selected].controller
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
                                            .disabled(cancelling)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.cancel(cx)),
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
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.cancel_queued(cx)
                                                }),
                                            ),
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
                                            .disabled(active || empty || session.input_composing)
                                            .on_click(
                                                move |_, window, cx| {
                                                    composer.update(cx, |input, cx| {
                                                        input.submit(window, cx)
                                                    })
                                                },
                                            ),
                                        )
                                    }),
                            ),
                    ),
            )
    }
}

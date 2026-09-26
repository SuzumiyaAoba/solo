use super::*;

impl Workspace {
    pub(super) fn logs(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.logs.is_empty() {
            return empty(Icon::Terminal, "ログなし", cx).into_any_element();
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
                                Button::icon("follow-logs", Icon::Pin, "ログの自動追従")
                                    .toggled(session.follow_logs)
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let session = &mut this.sessions[this.selected];
                                        session.follow_logs = !session.follow_logs;
                                        if session.follow_logs {
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
                                Button::icon("log-path", Icon::Copy, "全文ログのパスをコピー")
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
            .when(session.model.logs_discarded > 0, |v| {
                v.child(div().px_4().pb_2().child(ds::indicator(
                    "discarded-logs",
                    Icon::Info,
                    format!("{} 件省略", session.model.logs_discarded),
                    "表示範囲外のログは全文ファイルに保持",
                    Tone::Neutral,
                    cx,
                )))
            })
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
}

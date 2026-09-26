use super::*;

impl Workspace {
    pub(super) fn diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.sessions[self.selected];
        let Some(diff) = s.model.diffs.get(s.diff_index) else {
            return empty(Icon::FileDiff, "変更なし", cx).into_any_element();
        };
        let (added, removed) = diff.line_counts();
        let unreviewed = s.model.unreviewed_count();
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
                            .children(s.model.diffs.iter().map(|diff| {
                                KitTab::new().label(format!(
                                    "{}{}",
                                    if diff.reviewed { "✓ " } else { "" },
                                    diff.path
                                ))
                            }))
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
                    .child(ds::badge(format!("+{added}"), Tone::Success, cx))
                    .child(ds::badge(format!("−{removed}"), Tone::Danger, cx))
                    .child(ds::indicator(
                        "diff-review-count",
                        Icon::CircleCheck,
                        format!(
                            "{} / {}",
                            s.model.diffs.len() - unreviewed,
                            s.model.diffs.len()
                        ),
                        format!(
                            "確認済み {} / {} ファイル",
                            s.model.diffs.len() - unreviewed,
                            s.model.diffs.len()
                        ),
                        Tone::Neutral,
                        cx,
                    ))
                    .child(
                        Button::icon(
                            "mark-reviewed",
                            if diff.reviewed {
                                Icon::CircleCheck
                            } else {
                                Icon::Check
                            },
                            if diff.reviewed {
                                "確認済みを解除"
                            } else {
                                "確認済みにする"
                            },
                        )
                        .toggled(diff.reviewed)
                        .control_size(ControlSize::Small)
                        .disabled(s.model.status.is_active() || diff.truncated)
                        .when(s.model.status.is_active(), |b| {
                            b.tooltip("実行終了後に確認できます")
                        })
                        .when(diff.truncated, |b| b.tooltip("差分省略のためレビュー不可"))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_review(cx))),
                    )
                    .child(
                        Button::icon("next-unreviewed", Icon::ArrowRight, "次の未確認ファイル")
                            .control_size(ControlSize::Small)
                            .disabled(unreviewed == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let session = &mut this.sessions[this.selected];
                                let len = session.model.diffs.len();
                                if let Some(index) = (1..=len)
                                    .map(|n| (session.diff_index + n) % len)
                                    .find(|&index| !session.model.diffs[index].reviewed)
                                {
                                    session.diff_index = index;
                                    session.diff_scroll = UniformListScrollHandle::new();
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .when(diff.truncated, |v| {
                v.child(notice(
                    Icon::Warning,
                    "差分省略 · レビュー不可",
                    Tone::Warning,
                    cx,
                ))
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
                                    .w_full()
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
}

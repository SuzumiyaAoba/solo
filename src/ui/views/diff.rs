use super::*;

impl Workspace {
    pub(super) fn diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let s = &self.sessions[self.selected];
        if s.model.diffs.is_empty() {
            return empty(Icon::FileDiff, "変更なし", cx).into_any_element();
        }
        let Some(diff) = s.model.diffs.get(s.diff_index) else {
            return empty(Icon::FileDiff, "変更なし", cx).into_any_element();
        };
        if diff.rows.is_empty() {
            // ヘッダのみ(メタ情報だけ)の差分は内容なしとして扱う。
            return empty(Icon::FileDiff, "表示できる変更行がありません", cx).into_any_element();
        }
        let (added, removed) = diff.line_counts();
        let unreviewed = s.model.unreviewed_count();
        let running = s.model.status.is_active();
        let hunk_rows = diff.hunk_rows();
        let hunk_total = hunk_rows.len();
        // 表示は 1 始まり。未訪問なら 1 ハンク目に置く。
        let hunk_index = s.diff_hunk.min(hunk_total.saturating_sub(1)) + 1;

        let file_rail = div()
            .id("diff-file-rail")
            .w(px(208.))
            .flex_shrink_0()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
            .border_r_1()
            .border_color(ds::glass(p.border, 0.6))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .px(px(space::MD))
                    .pt(px(space::MD))
                    .pb(px(space::XS))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(caption("変更", cx))
                    .child(div().flex_1())
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
                    )),
            )
            .child(
                div()
                    .id("diff-file-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(s.model.diffs.iter().enumerate().map(|(index, file)| {
                        let selected = index == s.diff_index;
                        let (dir_part, base) = file
                            .path
                            .rsplit_once('/')
                            .unwrap_or(("", file.path.as_str()));
                        let (file_added, file_removed) = file.line_counts();
                        let path = file.path.clone();
                        div()
                            .id(("diff-file", index))
                            .relative()
                            .w_full()
                            .px_2()
                            .py(px(5.))
                            .rounded(px(ds::radius::CONTROL))
                            .cursor_pointer()
                            .when(selected, |v| {
                                v.bg(ds::glass(p.accent_soft, 0.5)).child(
                                    div()
                                        .absolute()
                                        .left(px(-4.))
                                        .top(px(6.))
                                        .bottom(px(6.))
                                        .w(px(2.))
                                        .rounded(px(1.))
                                        .bg(rgb(p.accent)),
                                )
                            })
                            .hover(move |style| style.bg(ds::glass(p.hover, ds::GLASS_HOVER)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .when(file.reviewed, |v| {
                                        v.child(Icon::CircleCheck.view(p.success).size(px(12.)))
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(12.))
                                            .font_weight(if selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(base.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(p.success))
                                            .child(format!("+{file_added}")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(p.danger))
                                            .child(format!("−{file_removed}")),
                                    ),
                            )
                            .child(
                                div()
                                    .pl(px(if file.reviewed { 16. } else { 0. }))
                                    .text_size(px(10.))
                                    .text_color(rgb(p.secondary))
                                    .truncate()
                                    .child(dir_part.to_string()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let session = &mut this.sessions[this.selected];
                                session.diff_index = index;
                                session.diff_hunk = 0;
                                session.diff_scroll = UniformListScrollHandle::new();
                                cx.notify();
                            }))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(path.clone())
                                    .build(window, cx)
                            })
                    })),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::MD))
                    .py(px(space::SM))
                    .border_t_1()
                    .border_color(ds::glass(p.border, 0.6))
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(ds::badge(format!("+{added}"), Tone::Success, cx))
                    .child(ds::badge(format!("−{removed}"), Tone::Danger, cx))
                    .child(div().flex_1())
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
                                    session.diff_hunk = 0;
                                    session.diff_scroll = UniformListScrollHandle::new();
                                    cx.notify();
                                }
                            })),
                    ),
            );

        let detail = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::LG))
                    .py(px(space::MD))
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(typography::MONO)
                            .text_size(px(typography::LABEL))
                            .font_weight(FontWeight::MEDIUM)
                            .child(diff.path.clone()),
                    )
                    .when(hunk_total > 0, |v| {
                        v.child(caption(format!("{hunk_index} / {hunk_total} ハンク"), cx))
                            .child(
                                Button::icon("prev-hunk", Icon::ChevronRight, "前のハンク")
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.step_hunk(hunk_total, -1, cx)
                                    })),
                            )
                            .child(
                                Button::icon("next-hunk", Icon::ArrowRight, "次のハンク")
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.step_hunk(hunk_total, 1, cx)
                                    })),
                            )
                    })
                    .child(
                        Button::icon("copy-diff-path", Icon::Copy, "パスをコピー")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let session = &this.sessions[this.selected];
                                let path = session
                                    .model
                                    .diffs
                                    .get(session.diff_index)
                                    .map(|diff| diff.path.clone())
                                    .unwrap_or_default();
                                this.copy(path, "パスをコピーしました", cx)
                            })),
                    )
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
                        .disabled(running || diff.truncated)
                        .when(running, |b| b.tooltip("実行終了後に確認できます"))
                        .when(diff.truncated, |b| b.tooltip("差分省略のためレビュー不可"))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_review(cx))),
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
                    diff.rows.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = ds::theme(cx);
                        let s = &this.sessions[this.selected];
                        let diff = &s.model.diffs[s.diff_index];
                        range
                            .map(|row| {
                                let line = &diff.lines[diff.rows[row]];
                                let (background, color, strong) = match line.kind {
                                    DiffKind::Added => {
                                        (p.success_soft, p.success, ds::glass(p.success, 0.28))
                                    }
                                    DiffKind::Removed => {
                                        (p.danger_soft, p.danger, ds::glass(p.danger, 0.28))
                                    }
                                    DiffKind::Header => {
                                        (p.accent_soft, p.accent_text, ds::glass(p.accent, 0.))
                                    }
                                    DiffKind::Context => (p.canvas, p.text, ds::glass(p.text, 0.)),
                                };
                                let sign = match line.kind {
                                    DiffKind::Added => "+",
                                    DiffKind::Removed => "−",
                                    _ => " ",
                                };
                                // 行の先頭記号を符号列で表示し、本文からは外す。
                                let body = if matches!(
                                    line.kind,
                                    DiffKind::Added | DiffKind::Removed | DiffKind::Context
                                ) {
                                    line.text.get(1..).unwrap_or("").to_string()
                                } else {
                                    line.text.clone()
                                };
                                // 隣接する削除/追加行の語レベル差分だけを濃くする。
                                let body_el: AnyElement = match &line.changed {
                                    Some(range) if !body.is_empty() => {
                                        StyledText::new(SharedString::from(body.clone()))
                                            .with_highlights([(
                                                range.clone(),
                                                HighlightStyle {
                                                    color: None,
                                                    font_weight: Some(FontWeight::SEMIBOLD),
                                                    font_style: None,
                                                    background_color: Some(strong.into()),
                                                    underline: None,
                                                    strikethrough: None,
                                                    fade_out: None,
                                                },
                                            )])
                                            .into_any_element()
                                    }
                                    _ => body.clone().into_any_element(),
                                };
                                let text = line.text.clone();
                                div()
                                    .id(row)
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
                                            .text_right()
                                            .text_color(rgb(p.secondary))
                                            .child(
                                                line.old.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(36.))
                                            .flex_shrink_0()
                                            .text_right()
                                            .text_color(rgb(p.secondary))
                                            .child(
                                                line.new.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(12.))
                                            .flex_shrink_0()
                                            .text_color(rgb(color))
                                            .child(sign),
                                    )
                                    .child(div().flex_1().min_w_0().truncate().child(body_el))
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
            );

        div()
            .size_full()
            .flex()
            .child(file_rail)
            .child(detail)
            .into_any_element()
    }

    /// ハンク間を `dir` 方向に循環し、ハンク先頭の表示行へスクロールする。
    fn step_hunk(&mut self, _total: usize, dir: i64, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        let Some(diff) = session.model.diffs.get(session.diff_index) else {
            return;
        };
        let hunk_rows = diff.hunk_rows();
        if hunk_rows.is_empty() {
            return;
        }
        let total = hunk_rows.len();
        let next = ((session.diff_hunk as i64 + dir).rem_euclid(total as i64)) as usize;
        session.diff_hunk = next;
        session
            .diff_scroll
            .scroll_to_item(hunk_rows[next], ScrollStrategy::Top);
        cx.notify();
    }
}

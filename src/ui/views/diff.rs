use super::super::{SessionView, Tab, Workspace, review_indicator};
use super::{caption, empty, notice};
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, HighlightStyle, ScrollStrategy, SharedString, Stateful,
    StyledText, UniformListScrollHandle, Window, div, prelude::*, px, rgb, uniform_list,
};
use solo::projection::{ActivityKind, Diff};
use solo::{
    design::{self as ds, Button, ControlSize, Icon, Tone, space, typography},
    projection::DiffKind,
};

impl SessionView {
    /// 差分の選択を切り替える。diff_index・diff_hunk・diff_scroll は一体で更新する
    /// (前ファイルのハンク位置やスクロールが残ると誤表示になる)。
    pub(in crate::ui) fn select_diff(&mut self, index: usize) {
        self.view.diff_index = index;
        self.view.diff_hunk = 0;
        self.view.diff_scroll = UniformListScrollHandle::new();
    }

    /// diff_index から末尾→先頭へラップしながら未確認の差分を探す。
    fn next_unreviewed_diff(&self) -> Option<usize> {
        let len = self.model.diffs().len();
        (1..=len)
            .map(|n| (self.view.diff_index + n) % len)
            .find(|&index| !self.model.diffs()[index].reviewed)
    }
}

impl Workspace {
    /// 差分が起きたファイルを選んで「変更」タブを開く。実行スレッドの変更ファイルから飛ぶ導線。
    pub(in crate::ui) fn open_diff(&mut self, path: &str, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if let Some(index) = session
            .model
            .diffs()
            .iter()
            .position(|diff| diff.path == path)
        {
            session.select_diff(index);
        }
        self.show_tab(Tab::Diff, cx);
    }

    /// 差分を起こした実行スレッドを開き、特定できていれば該当の実行を展開する。
    /// スレッドが保存上限で捨てられた場合は何もしない。
    pub(in crate::ui) fn open_diff_origin(
        &mut self,
        diff_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(origin) = self
            .session()
            .model
            .diffs()
            .get(diff_index)
            .and_then(|diff| diff.origin.clone())
        else {
            return;
        };
        let Some(thread) = self
            .session()
            .model
            .threads()
            .iter()
            .find(|thread| thread.turn_id == origin.turn_id)
        else {
            return;
        };
        // 差分を出した実行が分かるときは、その行を展開した状態でスレッドを開く。
        let expand_key = origin.invocation_id.map(|invocation_id| {
            let kind = thread
                .activities
                .iter()
                .find(|activity| activity.id == invocation_id)
                .map(|activity| activity.kind)
                .unwrap_or(ActivityKind::Tool);
            format!("{kind:?}:{invocation_id}")
        });
        let thread_id = thread.id;
        self.open_thread(thread_id, window, cx);
        self.session_mut().view.expanded_activity = expand_key;
    }

    pub(super) fn diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = self.session();
        if s.model.diffs().is_empty() {
            return empty(Icon::FileDiff, "変更なし", cx).into_any_element();
        }
        let Some(diff) = s.model.diffs().get(s.view.diff_index) else {
            return empty(Icon::FileDiff, "変更なし", cx).into_any_element();
        };
        if diff.rows.is_empty() {
            // ヘッダのみ(メタ情報だけ)の差分は内容なしとして扱う。
            return empty(Icon::FileDiff, "表示できる変更行がありません", cx).into_any_element();
        }
        div()
            .size_full()
            .flex()
            .child(self.diff_file_rail(diff, cx))
            .child(self.diff_detail(diff, cx))
            .into_any_element()
    }

    /// 左レール: ファイル一覧と合計増減・次の未確認ファイルへの導線。
    fn diff_file_rail(&self, diff: &Diff, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = ds::theme(cx);
        let s = self.session();
        let (added, removed) = diff.line_counts();
        let unreviewed = s.model.unreviewed_count();
        // 最新スレッド由来のファイルに印を付ける。
        let current_turn = s
            .model
            .threads()
            .back()
            .map(|thread| thread.turn_id.clone());
        div()
            .id("diff-file-rail")
            .w(px(208.))
            .flex_shrink_0()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
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
                    .child(review_indicator(
                        "diff-review-count",
                        s.model.diffs().len() - unreviewed,
                        s.model.diffs().len(),
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
                    .children(s.model.diffs().iter().enumerate().map(|(index, file)| {
                        let selected = index == s.view.diff_index;
                        let (dir_part, base) = file
                            .path
                            .rsplit_once('/')
                            .unwrap_or(("", file.path.as_str()));
                        let (file_added, file_removed) = file.line_counts();
                        let from_current = file
                            .origin
                            .as_ref()
                            .is_some_and(|origin| Some(&origin.turn_id) == current_turn.as_ref());
                        let marks = usize::from(file.reviewed) + usize::from(from_current);
                        let path = file.path.clone();
                        div()
                            .id(("diff-file", index))
                            .relative()
                            .w_full()
                            .px_2()
                            .py_1()
                            .rounded(px(ds::radius::CONTROL))
                            .cursor_pointer()
                            .when(selected, |v| {
                                v.bg(ds::glass(p.accent_soft, 0.5)).child(
                                    div()
                                        .absolute()
                                        .left(px(-4.))
                                        .top(px(space::SM))
                                        .bottom(px(space::SM))
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
                                    .when(from_current, |v| {
                                        v.child(ds::indicator(
                                            ("diff-current-turn", index),
                                            Icon::Activity,
                                            "",
                                            "最新の実行で変更されたファイル",
                                            Tone::Accent,
                                            cx,
                                        ))
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(typography::LABEL))
                                            .font_weight(if selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(base.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(typography::CAPTION))
                                            .text_color(rgb(p.success))
                                            .child(format!("+{file_added}")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(typography::CAPTION))
                                            .text_color(rgb(p.danger))
                                            .child(format!("−{file_removed}")),
                                    ),
                            )
                            .child(
                                div()
                                    .pl(px(marks as f32 * 16.))
                                    .text_size(px(typography::CAPTION))
                                    .text_color(rgb(p.secondary))
                                    .truncate()
                                    .child(dir_part.to_string()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.session_mut().select_diff(index);
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
                                let session = this.session_mut();
                                if let Some(index) = session.next_unreviewed_diff() {
                                    session.select_diff(index);
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    /// 右ペイン: パスとハンク移動、レビュー操作、差分行のリスト。
    fn diff_detail(&self, diff: &Diff, cx: &mut Context<Self>) -> Div {
        let s = self.session();
        let running = s.display_status().is_active();
        // スレッドが保存上限で破棄された差分は、辿る先が無いので導線を出さない。
        let has_origin = diff.origin.as_ref().is_some_and(|origin| {
            s.model
                .threads()
                .iter()
                .any(|thread| thread.turn_id == origin.turn_id)
        });
        let hunk_rows = diff.hunk_rows();
        let hunk_total = hunk_rows.len();
        // 表示は 1 始まり。未訪問なら 1 ハンク目に置く。
        let hunk_index = s.view.diff_hunk.min(hunk_total.saturating_sub(1)) + 1;
        div()
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
                                Button::icon("prev-hunk", Icon::ArrowLeft, "前のハンク")
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
                    .when(has_origin, |v| {
                        v.child(
                            Button::icon("diff-origin", Icon::MessageSquare, "変更元の実行を表示")
                                .control_size(ControlSize::Small)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let index = this.session().view.diff_index;
                                    this.open_diff_origin(index, window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::icon("copy-diff-patch", Icon::FileDiff, "パッチをコピー")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let session = this.session();
                                let patch = session
                                    .model
                                    .diffs()
                                    .get(session.view.diff_index)
                                    .map(|diff| {
                                        diff.lines
                                            .iter()
                                            .map(|line| line.text.as_str())
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    })
                                    .unwrap_or_default();
                                this.copy(patch, "パッチをコピーしました", cx)
                            })),
                    )
                    .child(
                        Button::icon("copy-diff-path", Icon::Copy, "パスをコピー")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let session = this.session();
                                let path = session
                                    .model
                                    .diffs()
                                    .get(session.view.diff_index)
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
            .child(
                uniform_list(
                    "diff-lines",
                    diff.rows.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range.map(|row| this.diff_row(row, cx)).collect()
                    }),
                )
                .track_scroll(&s.view.diff_scroll)
                .flex_1()
                .min_h_0(),
            )
    }

    /// uniform_list の 1 行。行種別で色と先頭記号を決め、語レベル差分は強調する。
    fn diff_row(&self, row: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = ds::theme(cx);
        let s = self.session();
        let diff = &s.model.diffs()[s.view.diff_index];
        let line = &diff.lines[diff.rows[row]];
        let (background, color, strong) = match line.kind {
            DiffKind::Added => (p.success_soft, p.success, ds::glass(p.success, 0.28)),
            DiffKind::Removed => (p.danger_soft, p.danger, ds::glass(p.danger, 0.28)),
            DiffKind::Header => (p.accent_soft, p.accent_text, ds::glass(p.accent, 0.)),
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
            Some(range) if !body.is_empty() => StyledText::new(SharedString::from(body.clone()))
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
                .into_any_element(),
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
                    .child(line.old.map(|n| n.to_string()).unwrap_or_default()),
            )
            .child(
                div()
                    .w(px(36.))
                    .flex_shrink_0()
                    .text_right()
                    .text_color(rgb(p.secondary))
                    .child(line.new.map(|n| n.to_string()).unwrap_or_default()),
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
    }

    /// ハンク間を `dir` 方向に循環し、ハンク先頭の表示行へスクロールする。
    fn step_hunk(&mut self, _total: usize, dir: i64, cx: &mut Context<Self>) {
        let session = self.session_mut();
        let Some(diff) = session.model.diffs().get(session.view.diff_index) else {
            return;
        };
        let hunk_rows = diff.hunk_rows();
        if hunk_rows.is_empty() {
            return;
        }
        let total = hunk_rows.len();
        let next = ((session.view.diff_hunk as i64 + dir).rem_euclid(total as i64)) as usize;
        session.view.diff_hunk = next;
        session
            .view
            .diff_scroll
            .scroll_to_item(hunk_rows[next], ScrollStrategy::Top);
        cx.notify();
    }
}

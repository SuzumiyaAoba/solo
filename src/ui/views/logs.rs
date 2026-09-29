use super::*;
use solo::projection::LogRow;
use std::collections::{BTreeMap, VecDeque};

/// 検索語とレベルチップに合う行の index。uniform_list の表示 index→実 index の写像。
/// sequence・level・text に対する大小区別しない部分一致で、level は完全一致。
pub(in crate::ui) fn filtered_indices(
    logs: &VecDeque<LogRow>,
    query: &str,
    level: Option<&str>,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    logs.iter()
        .enumerate()
        .filter(|(_, row)| level.is_none_or(|level| row.level == level))
        .filter(|(_, row)| {
            query.is_empty()
                || row.text.to_lowercase().contains(&query)
                || row.level.to_lowercase().contains(&query)
                || row.sequence.to_string().contains(&query)
        })
        .map(|(index, _)| index)
        .collect()
}

/// 「表示分をコピー」の書式。フィルタ適用後の全行を seq/level/text のタブ区切りで連結する。
pub(in crate::ui) fn filtered_log_text(
    logs: &VecDeque<LogRow>,
    query: &str,
    level: Option<&str>,
) -> String {
    filtered_indices(logs, query, level)
        .into_iter()
        .filter_map(|index| logs.get(index))
        .map(|row| format!("{}\t{}\t{}", row.sequence, row.level, row.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 出現しているレベルと件数。チップの並びを安定させるため名前順にソートする。
fn level_counts(logs: &VecDeque<LogRow>) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in logs {
        *counts.entry(row.level.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(level, count)| (level.to_owned(), count))
        .collect()
}

/// ログ行のレベル表示色。既知の error/event/tool/plan 以外は secondary で統一する。
fn level_color(level: &str, p: &ds::Palette) -> u32 {
    match level {
        "error" => p.danger,
        "event" => p.warning,
        "tool" => p.accent_text,
        "plan" => p.muted,
        _ => p.secondary,
    }
}

impl Workspace {
    pub(super) fn logs(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = self.session();
        if session.model.logs().is_empty() {
            return empty(Icon::Terminal, "ログなし", cx).into_any_element();
        }
        let p = ds::theme(cx);
        let logs_len = session.model.logs().len();
        let discarded = session.model.logs_discarded();
        let level = session.view.log_level.clone();
        let filtered = filtered_indices(
            session.model.logs(),
            &session.view.log_filter,
            level.as_deref(),
        );
        let filtered_len = filtered.len();
        let filtering = !session.view.log_filter.trim().is_empty() || level.is_some();
        let levels = level_counts(session.model.logs());
        // 詳細パネルの対象行。破棄で指す行が消えた/ずれたときは畳む(stream 側でも解除する)。
        let selected = session
            .view
            .log_selected
            .and_then(|global| global.checked_sub(discarded))
            .and_then(|index| session.model.logs().get(index))
            .map(|row| (row.sequence, row.level.clone(), row.text.clone()));
        div()
            .id("log-container")
            .size_full()
            .flex()
            .flex_col()
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                let session = this.session_mut();
                if session.view.follow_logs {
                    session.view.follow_logs = false;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(space::LG))
                    .py(px(space::SM))
                    .flex()
                    .flex_col()
                    .gap(px(space::SM))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(px(space::SM))
                            .child(caption(
                                if filtering {
                                    format!(
                                        "{filtered_len} / {logs_len} 件 · 全文 {:.2} MiB",
                                        session.model.log_bytes() as f64 / 1048576.
                                    )
                                } else {
                                    format!(
                                        "最新 {logs_len} 件 · 全文 {:.2} MiB",
                                        session.model.log_bytes() as f64 / 1048576.
                                    )
                                },
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::SM))
                                    .child(
                                        Button::icon("follow-logs", Icon::Pin, "ログの自動追従")
                                            .toggled(session.view.follow_logs)
                                            .control_size(ControlSize::Small)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let session = this.session_mut();
                                                session.view.follow_logs =
                                                    !session.view.follow_logs;
                                                if session.view.follow_logs {
                                                    let tail = filtered_indices(
                                                        session.model.logs(),
                                                        &session.view.log_filter,
                                                        session.view.log_level.as_deref(),
                                                    )
                                                    .len();
                                                    if tail > 0 {
                                                        session.view.log_scroll.scroll_to_item(
                                                            tail - 1,
                                                            ScrollStrategy::Bottom,
                                                        );
                                                    }
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::icon("log-tail", Icon::ChevronDown, "最新のログへ")
                                            .control_size(ControlSize::Small)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let s = this.session_mut();
                                                s.view.follow_logs = true;
                                                let tail = filtered_indices(
                                                    s.model.logs(),
                                                    &s.view.log_filter,
                                                    s.view.log_level.as_deref(),
                                                )
                                                .len();
                                                if tail > 0 {
                                                    s.view.log_scroll.scroll_to_item(
                                                        tail - 1,
                                                        ScrollStrategy::Bottom,
                                                    );
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::icon(
                                            "log-copy-visible",
                                            Icon::Copy,
                                            "表示中のログをコピー",
                                        )
                                        .control_size(ControlSize::Small)
                                        .disabled(filtered_len == 0)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                let session = this.session();
                                                let text = filtered_log_text(
                                                    session.model.logs(),
                                                    &session.view.log_filter,
                                                    session.view.log_level.as_deref(),
                                                );
                                                if !text.is_empty() {
                                                    this.copy(
                                                        text,
                                                        "表示中のログをコピーしました",
                                                        cx,
                                                    );
                                                }
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::icon(
                                            "log-path",
                                            Icon::Folder,
                                            "全文ログのパスをコピー",
                                        )
                                        .control_size(ControlSize::Small)
                                        .disabled(session.artifacts.is_empty())
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                let paths = this
                                                    .session()
                                                    .artifacts
                                                    .iter()
                                                    .map(|path| path.display().to_string())
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                this.copy(paths, "ログのパスをコピーしました", cx);
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(space::SM))
                            .child(div().w(px(200.)).child(session.log_search.clone()))
                            .child(
                                Button::new("log-level-all", format!("すべて {logs_len}"))
                                    .control_size(ControlSize::Small)
                                    .toggled(level.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.session_mut().view.log_level = None;
                                        cx.notify();
                                    })),
                            )
                            .children(levels.iter().enumerate().map(|(index, (name, count))| {
                                let name = name.clone();
                                let toggled = level.as_deref() == Some(name.as_str());
                                Button::new(("log-level", index), format!("{name} {count}"))
                                    .control_size(ControlSize::Small)
                                    .toggled(toggled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let view = &mut this.session_mut().view;
                                        view.log_level =
                                            if toggled { None } else { Some(name.clone()) };
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
            .when_some(selected, |v, (sequence, level, text)| {
                let copied = text.clone();
                v.child(
                    div()
                        .flex_shrink_0()
                        .mx(px(space::LG))
                        .mb(px(space::SM))
                        .rounded(px(ds::radius::CONTROL))
                        .border_1()
                        .border_color(rgb(p.border))
                        .bg(glass(p.surface, ds::GLASS_SURFACE))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px(px(space::SM))
                                .pt(px(space::SM))
                                .pb(px(space::XS))
                                .child(caption(format!("{sequence:06} · {level}"), cx))
                                .child(
                                    Button::new("log-detail-copy", "コピー")
                                        .control_size(ControlSize::Small)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy(copied.clone(), "ログをコピーしました", cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("log-detail")
                                .max_h(px(160.))
                                .overflow_y_scroll()
                                .px(px(space::SM))
                                .pb(px(space::SM))
                                .font_family(typography::MONO)
                                .text_size(px(typography::LABEL))
                                .text_color(rgb(p.text))
                                .child(text),
                        ),
                )
            })
            .when(session.model.logs_discarded() > 0, |v| {
                v.child(div().px_4().pb_2().child(ds::indicator(
                    "discarded-logs",
                    Icon::Info,
                    format!("{} 件省略", session.model.logs_discarded()),
                    "表示範囲外のログは全文ファイルに保持",
                    Tone::Neutral,
                    cx,
                )))
            })
            .child(
                uniform_list(
                    "log-rows",
                    filtered_len,
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        let p = ds::theme(cx);
                        let s = this.session();
                        let discarded = s.model.logs_discarded();
                        range
                            .filter_map(|i| {
                                let row_index = *filtered.get(i)?;
                                s.model.logs().get(row_index).map(|row| {
                                    let text = row.text.clone();
                                    let global = discarded + row_index;
                                    let selected = s.view.log_selected == Some(global);
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
                                        .bg(rgb(if selected {
                                            p.accent_soft
                                        } else if i % 2 == 0 {
                                            p.canvas
                                        } else {
                                            p.surface
                                        }))
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
                                                .text_color(rgb(level_color(&row.level, &p)))
                                                .child(row.level.clone()),
                                        )
                                        .child(
                                            div().flex_1().min_w_0().truncate().child(text.clone()),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let view = &mut this.session_mut().view;
                                            view.log_selected = if view.log_selected == Some(global)
                                            {
                                                None
                                            } else {
                                                Some(global)
                                            };
                                            cx.notify();
                                        }))
                                })
                            })
                            .collect()
                    }),
                )
                .track_scroll(&session.view.log_scroll)
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }
}

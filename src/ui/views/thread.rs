use super::*;
use solo::projection::{
    ActivityApproval, ActivityKind, ActivityState, ExecutionActivity, ExecutionThread,
};

impl Workspace {
    pub(in crate::ui) fn open_thread(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session_mut();
        if session.view.selected_thread != Some(id) {
            session.view.thread_scroll = ScrollHandle::new();
            session.view.expanded_activity = None;
        }
        session.view.selected_thread = Some(id);
        window.focus(&self.focus, cx);
        self.show_tab(Tab::Chat, cx);
    }

    pub(in crate::ui) fn toggle_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if session.view.tab == Tab::Chat && session.view.selected_thread.is_some() {
            session.view.selected_thread = None;
            window.focus(&session.composer.focus_handle(cx), cx);
            cx.notify();
        } else if let Some(id) = session.model.threads().back().map(|thread| thread.id) {
            self.open_thread(id, window, cx);
        }
    }

    pub(super) fn execution_thread(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let session = self.session();
        let thread = session
            .model
            .threads()
            .iter()
            .find(|thread| Some(thread.id) == session.view.selected_thread);
        let header = div()
            .h(px(52.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::MessageSquare.view(p.accent_text))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("実行スレッド"),
                    ),
            )
            .child(
                Button::icon("close-thread", Icon::Close, "スレッドを閉じて会話へ戻る")
                    .control_size(ControlSize::Small)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.session_mut().view.selected_thread = None;
                        window.focus(&this.session().composer.focus_handle(cx), cx);
                        cx.notify();
                    })),
            );
        let root = div()
            .size_full()
            .min_w_0()
            .bg(ds::glass(p.sidebar, ds::GLASS_SIDEBAR))
            .flex()
            .flex_col()
            .child(header);
        let Some(thread) = thread else {
            return root
                .child(empty(
                    Icon::Clock,
                    "この依頼の実行履歴は保存上限により省略されました",
                    cx,
                ))
                .into_any_element();
        };
        let current = session
            .model
            .threads()
            .back()
            .is_some_and(|last| last.id == thread.id);
        let thread_status = if current && session.model.status() == Status::Cancelling {
            Status::Cancelling
        } else {
            thread.status
        };
        root.child(
            div()
                .flex_shrink_0()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(ds::avatar("You", Tone::Neutral, cx))
                        .child(div().font_weight(FontWeight::MEDIUM).child("あなたの依頼"))
                        .child(div().flex_1())
                        .child(ds::badge(
                            thread_status.label(),
                            status_tone(thread_status),
                            cx,
                        )),
                )
                .child(
                    div()
                        .id("thread-prompt")
                        .max_h(px(96.))
                        .overflow_y_scroll()
                        .text_size(px(typography::BODY))
                        .child(thread.prompt.clone()),
                )
                .child(caption(thread_meta(thread), cx)),
        )
        .when(current && session.approval.is_some(), |v| {
            v.child(
                div().p_3().child(
                    Button::new("thread-approval", "承認待ちの操作を確認")
                        .with_icon(Icon::Bell)
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| this.show_tab(Tab::Overview, cx))),
                ),
            )
        })
        .child(
            div()
                .id(("execution-activities", thread.id as usize))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&session.view.thread_scroll)
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .when(thread.discarded > 0, |v| {
                    v.child(caption(format!("先頭の {} 件を省略", thread.discarded), cx))
                })
                .when(thread.activities.is_empty(), |v| {
                    v.child(
                        div()
                            .py_6()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Icon::Activity.view(p.muted))
                            .child(caption(
                                if thread.status.is_active() {
                                    "実行が始まると、ここに表示されます"
                                } else {
                                    "この依頼ではツールやサブエージェントの実行はありません"
                                },
                                cx,
                            )),
                    )
                })
                .children(
                    thread
                        .activities
                        .iter()
                        .map(|activity| self.activity_row(thread, activity, cx)),
                )
                .when(!thread.reason.is_empty(), |v| {
                    v.child(div().pt_3().child(caption(thread.reason.clone(), cx)))
                }),
        )
        .child(
            div().flex_shrink_0().px_4().py_2().child(
                Button::new("thread-logs", "チャンネルのログを開く")
                    .with_icon(Icon::Terminal)
                    .variant(ButtonVariant::Ghost)
                    .control_size(ControlSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| this.show_tab(Tab::Logs, cx))),
            ),
        )
        .into_any_element()
    }

    /// スレッド内の 1 アクティビティ行。タップで詳細パネルを開閉する。
    fn activity_row(
        &self,
        thread: &ExecutionThread,
        activity: &ExecutionActivity,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = ds::theme(cx);
        let session = self.session();
        let key = format!("{:?}:{}", activity.kind, activity.id);
        let toggle_key = key.clone();
        let expanded = session.view.expanded_activity.as_ref() == Some(&key);
        let tone = activity_tone(activity.state);
        let (color, _) = p.tone(tone);
        let agent = activity.kind == ActivityKind::Agent;
        let owner = activity.parent_agent_id.as_ref().map(|id| {
            thread
                .activities
                .iter()
                .find(|a| a.kind == ActivityKind::Agent && &a.id == id)
                .map(|a| a.title.clone())
                .unwrap_or_else(|| id.clone())
        });
        let duration = activity.duration_ms().map(duration_label);
        let copy = format!(
            "{}\n{}\n{}",
            activity.title, activity.detail, activity.result
        );
        div()
            .min_w_0()
            .border_l_2()
            .border_color(rgb(color))
            .pl_3()
            .py_1()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(match &activity.tool {
                        Some(tool) => ds::badge(tool.clone(), Tone::Neutral, cx).into_any_element(),
                        None => caption(
                            if agent {
                                "サブエージェント"
                            } else {
                                "ツール呼び出し"
                            },
                            cx,
                        )
                        .into_any_element(),
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when_some(duration, |v, duration| v.child(caption(duration, cx)))
                            .child(ds::badge(activity.state.label(), tone, cx)),
                    ),
            )
            .child(
                Button::new(
                    SharedString::from(format!("activity-{key}")),
                    activity.title.clone(),
                )
                .with_icon(if agent { Icon::Layers } else { Icon::Terminal })
                .variant(ButtonVariant::Ghost)
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .align_start()
                .tooltip(activity.title.clone())
                .control_size(ControlSize::Small)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.session_mut().view.expanded_activity = if expanded {
                        None
                    } else {
                        Some(toggle_key.clone())
                    };
                    cx.notify();
                })),
            )
            .when_some(activity.approval.as_ref(), |v, approval| {
                let (label, tone) = match approval {
                    ActivityApproval::Pending => ("承認待ち".into(), Tone::Warning),
                    ActivityApproval::Allowed(source) => {
                        (format!("許可 · {source}"), Tone::Neutral)
                    }
                    ActivityApproval::Denied(_) => ("拒否".into(), Tone::Danger),
                };
                v.child(ds::badge(label, tone, cx))
            })
            .when_some(owner, |v, owner| {
                v.child(caption(format!("担当: {owner}"), cx))
            })
            .when(!activity.detail.is_empty() && !expanded, |v| {
                v.child(caption(activity.detail.clone(), cx).truncate())
            })
            .when(expanded, |v| {
                v.child(
                    div()
                        .p_3()
                        .bg(ds::glass(p.canvas, 0.5))
                        .rounded(px(ds::radius::CONTROL))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(typography::LABEL))
                                .font_family(typography::MONO)
                                .child(activity.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(typography::LABEL))
                                .text_color(rgb(p.secondary))
                                .child(activity.detail.clone()),
                        )
                        .child(
                            Button::icon(
                                SharedString::from(format!("copy-activity-{}", activity.id)),
                                Icon::Copy,
                                "実行内容をコピー",
                            )
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.copy(copy.clone(), "実行内容をコピーしました", cx)
                                },
                            )),
                        ),
                )
            })
            .when(!activity.result.is_empty(), |v| {
                v.child(caption(activity.result.clone(), cx))
            })
            .when(!activity.changed_paths.is_empty(), |v| {
                v.child(self.changed_paths_row(&key, activity, cx))
            })
    }

    /// 変更ファイルのチップ。押すと Diff タブの該当ファイルを開く。
    fn changed_paths_row(
        &self,
        key: &str,
        activity: &ExecutionActivity,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = ds::theme(cx);
        const SHOWN: usize = 3;
        let rest = activity.changed_paths.len().saturating_sub(SHOWN);
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .children(
                activity
                    .changed_paths
                    .iter()
                    .take(SHOWN)
                    .enumerate()
                    .map(|(index, path)| {
                        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
                        let file = path.clone();
                        let tip = path.clone();
                        div()
                            .id(SharedString::from(format!("{key}-file-{index}")))
                            .px_2()
                            .py_1()
                            .rounded(px(ds::radius::CONTROL))
                            .bg(ds::glass(p.accent_soft, 0.6))
                            .text_color(rgb(p.accent_text))
                            .text_size(px(typography::CAPTION))
                            .font_family(typography::MONO)
                            .cursor_pointer()
                            .child(name)
                            .hover(move |style| style.bg(ds::glass(p.hover, ds::GLASS_HOVER)))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                    .build(window, cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.open_diff(&file, cx)))
                    }),
            )
            .when(rest > 0, |v| v.child(caption(format!("他 {rest} 件"), cx)))
    }
}

/// ヘッダの補足行。「2 件の実行 · 所要 3.2 秒 · 変更ファイル 1 件」のように並べる。
fn thread_meta(thread: &ExecutionThread) -> String {
    let mut parts = vec![format!("{} 件の実行", thread.activity_count())];
    if let Some(duration) = thread.duration_ms() {
        parts.push(format!("所要 {}", duration_label(duration)));
    }
    let changed = thread.changed_files();
    if !changed.is_empty() {
        parts.push(format!("変更ファイル {} 件", changed.len()));
    }
    parts.join(" · ")
}

/// 実行時間の表示。1 秒未満は切り捨てず明示し、長い場合は荒く丸める。
fn duration_label(ms: u64) -> String {
    if ms < 1_000 {
        "1 秒未満".into()
    } else if ms < 10_000 {
        format!("{}.{} 秒", ms / 1_000, ms % 1_000 / 100)
    } else if ms < 60_000 {
        format!("{} 秒", ms / 1_000)
    } else {
        let minutes = ms / 60_000;
        let seconds = ms % 60_000 / 1_000;
        if seconds == 0 {
            format!("{minutes} 分")
        } else {
            format!("{minutes} 分 {seconds} 秒")
        }
    }
}

fn activity_tone(state: ActivityState) -> Tone {
    match state {
        ActivityState::Running => Tone::Accent,
        ActivityState::Succeeded => Tone::Success,
        ActivityState::Failed => Tone::Danger,
        ActivityState::Cancelled | ActivityState::Interrupted => Tone::Warning,
    }
}

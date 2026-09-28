use super::views::{caption, status_icon};
use super::*;

impl Workspace {
    pub(super) fn overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = self.session();
        let p = ds::theme(cx);
        if let Some(approval) = self.approval_card(cx) {
            return div()
                .size_full()
                .min_h_0()
                .p_3()
                .child(approval)
                .into_any_element();
        }
        let fresh = s.model.chat().is_empty()
            && s.display_status() == Status::Idle
            && self.queue.position(&s.model.id).is_none();
        if fresh {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_6()
                .p_4()
                .child(Icon::Layers.view(p.accent_text).size(px(32.)))
                .child(self.starters(cx))
                .into_any_element();
        }
        div()
            .id(("task-overview", self.selected))
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .max_w(px(900.))
                    .mx_auto()
                    .p(px(space::XL))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(s.model.title().to_owned()),
                            )
                            .child(caption(s.provider_display().unwrap_or_default(), cx)),
                    )
                    .when_some(s.approval_note.clone(), |v, note| {
                        v.child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(p.secondary))
                                .child(note),
                        )
                    })
                    .when_some(s.login.as_ref(), |v, login| {
                        v.child(self.login_card(login, cx))
                    })
                    .child(self.run_summary(cx))
                    .when(!s.model.tool_activity().is_empty(), |v| {
                        v.child(self.tool_list(cx))
                    })
                    .when(!s.model.diffs().is_empty(), |v| v.child(self.diff_list(cx))),
            )
            .into_any_element()
    }

    /// ChatGPT デバイスログインの案内カード。コード表示と認証ページへの導線。
    fn login_card(&self, login: &DeviceLogin, cx: &mut Context<Self>) -> Div {
        let p = ds::theme(cx);
        let url = login.verification_url.clone();
        let code = login.user_code.clone();
        ds::card(cx)
            .p_4()
            .gap_3()
            .border_color(rgb(p.warning))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::OpenAi.view(p.text))
                    .child("ChatGPT")
                    .child(ds::badge("ログイン待ち", Tone::Warning, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .font_family(typography::MONO)
                            .text_size(px(20.))
                            .child(code.clone()),
                    )
                    .child(
                        Button::icon("copy-login-code", Icon::Copy, "認証コードをコピー").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.copy(code.clone(), "コードをコピーしました", cx)
                            }),
                        ),
                    )
                    .child(
                        Button::icon("open-login-url", Icon::ExternalLink, "認証ページを開く")
                            .variant(ButtonVariant::Primary)
                            .tooltip(format!("認証ページ · {url}"))
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            )
    }

    /// 直近のツール実行を最大12件だけ新しい順で並べる。
    fn tool_list(&self, cx: &mut Context<Self>) -> Div {
        let s = self.session();
        let p = ds::theme(cx);
        let active = s.display_status().is_active();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("操作"))
                    .child(
                        Button::icon("activity-logs", Icon::Terminal, "ログを開く")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| this.show_tab(Tab::Logs, cx))),
                    ),
            )
            .child(
                ds::card(cx).overflow_hidden().children(
                    s.model
                        .tool_activity()
                        .iter()
                        .rev()
                        .take(12)
                        .map(|activity| {
                            let (label, tone, icon) = match activity.exit_code {
                                Some(0) => ("完了".to_owned(), Tone::Success, Icon::Check),
                                Some(code) => {
                                    (format!("exit {code}"), Tone::Warning, Icon::Warning)
                                }
                                None if active => ("実行中".into(), Tone::Accent, Icon::Spinner),
                                None => ("結果未確認".into(), Tone::Warning, Icon::Warning),
                            };
                            div()
                                .px_4()
                                .py_3()
                                .border_b_1()
                                .border_color(rgb(p.border))
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(icon.view(p.tone(tone).0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .font_family(typography::MONO)
                                                .text_size(px(12.))
                                                .truncate()
                                                .child(activity.command.clone()),
                                        )
                                        .child(caption(activity.cwd.clone(), cx).truncate()),
                                )
                                .child(ds::badge(label, tone, cx))
                        }),
                ),
            )
    }

    /// 変更ファイル一覧。行クリックで Diff タブの該当ファイルへ飛ぶ。
    fn diff_list(&self, cx: &mut Context<Self>) -> Div {
        let s = self.session();
        let p = ds::theme(cx);
        let unreviewed = s.model.unreviewed_count();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("変更"))
                    .child(review_indicator(
                        "review-count",
                        s.model.diffs().len() - unreviewed,
                        s.model.diffs().len(),
                        cx,
                    )),
            )
            .child(
                ds::card(cx)
                    .overflow_hidden()
                    .children(s.model.diffs().iter().enumerate().map(|(index, diff)| {
                        let (added, removed) = diff.line_counts();
                        div()
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(p.border))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                if diff.reviewed {
                                    Icon::CircleCheck
                                } else {
                                    Icon::FileDiff
                                }
                                .view(if diff.reviewed {
                                    p.success
                                } else {
                                    p.muted
                                }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(typography::MONO)
                                    .text_size(px(12.))
                                    .child(diff.path.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(p.success))
                                    .child(format!("+{added}")),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(p.danger))
                                    .child(format!("−{removed}")),
                            )
                            .child(
                                Button::icon(
                                    ("review-file", index),
                                    Icon::ChevronRight,
                                    format!("差分を開く · {}", diff.path),
                                )
                                .control_size(ControlSize::Small)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.session_mut().select_diff(index);
                                        this.show_tab(Tab::Diff, cx);
                                    },
                                )),
                            )
                    })),
            )
    }

    fn run_summary(&self, cx: &mut Context<Self>) -> Div {
        let s = self.session();
        let p = ds::theme(cx);
        let queued = self.queue.position(&s.model.id);
        let active = s.display_status().is_active();
        let unreviewed = s.model.unreviewed_count();
        let finished = s
            .model
            .tools()
            .values()
            .filter(|code| code.is_some())
            .count();
        let failed = s
            .model
            .tools()
            .values()
            .filter(|code| code.is_some_and(|code| code != 0))
            .count();
        let issue = matches!(
            s.model.status(),
            Status::Failed | Status::Disconnected | Status::Cancelled
        );
        ds::card(cx)
            .p_4()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                if queued.is_some() {
                                    Icon::Clock
                                } else {
                                    status_icon(s.display_status())
                                }
                                .view(p.tone(s.state_tone()).0),
                            )
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(
                                if let Some(position) = queued {
                                    format!("順番待ち · {position}")
                                } else {
                                    s.state_label().into()
                                },
                            ))
                            .when(!self.picker_uses_workspace(s.selected_backend), |v| {
                                v.child(ds::badge("デモ", Tone::Neutral, cx))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(queued.is_some(), |v| {
                                v.child(
                                    Button::icon(
                                        "overview-cancel-queue",
                                        Icon::Pencil,
                                        "順番待ちを取り消して編集",
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.cancel_queued(cx);
                                            window.focus(
                                                &this.session().composer.focus_handle(cx),
                                                cx,
                                            );
                                        },
                                    )),
                                )
                            })
                            .when(!active && unreviewed > 0, |v| {
                                v.child(
                                    Button::icon(
                                        "overview-review",
                                        Icon::FileDiff,
                                        "変更をレビュー",
                                    )
                                    .variant(ButtonVariant::Primary)
                                    .on_click(cx.listener(|this, _, _, cx| this.review_next(cx))),
                                )
                            })
                            .when(!s.model.chat().is_empty(), |v| {
                                v.child(
                                    Button::icon(
                                        "overview-chat",
                                        Icon::MessageSquare,
                                        "会話を開く",
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.session_mut().unread_result = false;
                                            this.show_tab(Tab::Chat, cx);
                                        },
                                    )),
                                )
                            })
                            .when(issue, |v| {
                                v.child(
                                    Button::icon("overview-recovery", Icon::Terminal, "ログを確認")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.session_mut().unread_result = false;
                                            this.show_tab(Tab::Logs, cx);
                                        })),
                                )
                                .when(
                                    !s.last_prompt.is_empty(),
                                    |v| {
                                        v.child(
                                            Button::icon(
                                                "restore-request",
                                                Icon::RotateCcw,
                                                "依頼を下書きに戻す",
                                            )
                                            .on_click(
                                                cx.listener(|this, _, window, cx| {
                                                    let prompt = this.session().last_prompt.clone();
                                                    this.fill_prompt(&prompt, window, cx);
                                                }),
                                            ),
                                        )
                                    },
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_6()
                    .child(ds::indicator(
                        "summary-tools",
                        Icon::Terminal,
                        format!("{finished} / {}", s.model.tools().len()),
                        format!("完了した操作 {finished} / {}", s.model.tools().len()),
                        Tone::Neutral,
                        cx,
                    ))
                    .child(ds::indicator(
                        "summary-files",
                        Icon::FileDiff,
                        s.model.diffs().len().to_string(),
                        format!("変更ファイル {} 件", s.model.diffs().len()),
                        Tone::Neutral,
                        cx,
                    ))
                    .child(review_indicator(
                        "summary-reviewed",
                        s.model.diffs().len() - unreviewed,
                        s.model.diffs().len(),
                        cx,
                    ))
                    .when_some(s.metrics.elapsed, |v, elapsed| {
                        v.child(ds::indicator(
                            "summary-time",
                            Icon::Clock,
                            format!("{}s", elapsed.as_secs()),
                            format!("実行時間 {} 秒", elapsed.as_secs()),
                            Tone::Neutral,
                            cx,
                        ))
                    })
                    .when(failed > 0, |v| {
                        v.child(ds::indicator(
                            "summary-failures",
                            Icon::Warning,
                            failed.to_string(),
                            format!("終了コードが 0 以外の操作 {failed} 件"),
                            Tone::Warning,
                            cx,
                        ))
                    }),
            )
    }

    fn starters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let templates = [
            (
                Icon::Search,
                "調査",
                "コードの構成と開発手順を調べる",
                "このリポジトリの構成、主要な処理、開発手順を調べて要約してください。ファイルの変更は不要です。",
            ),
            (
                Icon::Layers,
                "計画",
                "改善の優先順位と実装計画を作る",
                "このアプリケーションの改善点を調べ、優先順位と完了条件を含む実装計画を提案してください。まずは調査と計画のみ行ってください。",
            ),
            (
                Icon::FileDiff,
                "レビュー",
                "未コミットの変更をレビューする",
                "現在の未コミットの変更をレビューし、不具合や検証が不足している箇所を根拠とともに示してください。ファイルの変更は不要です。",
            ),
        ];
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap_2()
            .children(templates.into_iter().enumerate().map(
                |(index, (icon, title, hint, prompt))| {
                    Button::new(("starter", index), title)
                        .with_icon(icon)
                        .tooltip(format!("{hint} · 下書きに追加"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.fill_prompt(prompt, window, cx)
                        }))
                },
            ))
            .child(
                Button::new("overview-demo", "デモ")
                    .with_icon(Icon::Play)
                    .variant(ButtonVariant::Ghost)
                    .tooltip("デモを再生")
                    .on_click(cx.listener(|this, _, _, cx| this.scenario(Scenario::Demo, cx))),
            )
    }
}

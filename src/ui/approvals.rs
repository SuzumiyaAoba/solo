use super::*;
use gpui_kit::component::WindowExt;

pub(super) struct PendingApproval {
    request: ApprovalRequest,
    reply: async_channel::Sender<bool>,
    details_open: bool,
    pub(super) policy_error: Option<String>,
    serial: u64,
    review: Option<RunningReview>,
    pub(super) review_note: Option<String>,
}

impl PendingApproval {
    pub(super) fn respond(self, accepted: bool) {
        let _ = self.reply.try_send(accepted);
    }

    pub(super) fn is_reviewing(&self) -> bool {
        self.review.is_some()
    }
}

/// 判定に使った設定とその判定を止める token を同じ寿命で管理する。
struct RunningReview {
    settings: AutoSettings,
    cancel: tokio_util::sync::CancellationToken,
}

impl Drop for RunningReview {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub(super) struct PolicyRevision(u64);
impl Global for PolicyRevision {}
pub(super) fn init(cx: &mut App) {
    if cx.try_global::<PolicyRevision>().is_none() {
        cx.set_global(PolicyRevision::default());
    }
}
pub(super) fn changed(cx: &mut App) {
    let revision = cx
        .try_global::<PolicyRevision>()
        .map_or(0, |revision| revision.0);
    cx.set_global(PolicyRevision(revision.wrapping_add(1)));
    cx.refresh_windows();
}
pub(super) fn settings(store: &Result<RuleStore, String>) -> Result<ApprovalSettings, String> {
    store
        .as_ref()
        .map_err(Clone::clone)?
        .approval_policy()
        .map(|(settings, _)| settings)
        .map_err(|error| error.to_string())
}
pub(super) fn plan(
    store: &Result<RuleStore, String>,
    request: &ApprovalRequest,
) -> Result<ApprovalPlan, String> {
    request
        .plan(store.as_ref().map_err(Clone::clone)?)
        .map_err(|error| error.to_string())
}

impl Workspace {
    pub(super) fn receive_approval(
        &mut self,
        session_id: &str,
        generation: u64,
        request: ApprovalRequest,
        reply: async_channel::Sender<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.sessions.iter().position(|session| {
            session.model.id == session_id && session.stream_generation == generation
        }) else {
            let _ = reply.try_send(false);
            return;
        };
        if reply.is_closed() {
            return;
        }
        if !self.sessions[index].model.status.is_active()
            || self.sessions[index].model.status == Status::Cancelling
        {
            let _ = reply.try_send(false);
            return;
        }
        self.approval_settings = settings(&self.command_rules);
        let decision = plan(&self.command_rules, &request);
        self.approval_serial = self.approval_serial.wrapping_add(1);
        let serial = self.approval_serial;
        self.sessions[index].approval = Some(PendingApproval {
            request,
            reply,
            details_open: false,
            policy_error: decision.as_ref().err().cloned(),
            serial,
            review: None,
            review_note: None,
        });
        match decision {
            Ok(ApprovalPlan::Allow(source)) => {
                self.finish_approval(index, true, format!("{source} · 許可"), cx)
            }
            Ok(ApprovalPlan::Deny(source)) => {
                self.finish_approval(index, false, format!("{source} · 拒否"), cx)
            }
            Ok(ApprovalPlan::Auto(settings)) => self.start_auto_review(index, settings, cx),
            _ => {
                if !self.is_visible || self.selected != index {
                    let title = self.sessions[index].model.title.clone();
                    self.toast.update(cx, |toast, cx| {
                        toast.push(
                            format!("{} · 承認待ち · {title}", self.workspace_name),
                            Tone::Warning,
                            cx,
                        )
                    });
                }
            }
        }
        cx.notify();
    }
    fn finish_approval(
        &mut self,
        index: usize,
        accepted: bool,
        note: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(approval) = self.sessions[index].approval.take() {
            approval.respond(accepted);
            self.sessions[index].approval_note = Some(note.clone());
            self.toast.update(cx, |toast, cx| {
                toast.push(
                    note,
                    if accepted {
                        Tone::Neutral
                    } else {
                        Tone::Warning
                    },
                    cx,
                )
            });
        }
        cx.notify();
    }
    fn start_auto_review(&mut self, index: usize, settings: AutoSettings, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        let Some(pending) = session.approval.as_mut() else {
            return;
        };
        let cancel = tokio_util::sync::CancellationToken::new();
        let input = auto_approval::ReviewInput::new(
            &pending.request,
            &session.last_prompt,
            std::path::Path::new(&self.workspace_path),
        )
        .with_user_history(&session.history);
        let serial = pending.serial;
        let session_id = session.model.id.clone();
        let generation = session.stream_generation;
        pending.review = Some(RunningReview {
            settings: settings.clone(),
            cancel: cancel.clone(),
        });
        let reviewer = self.approval_reviewer.clone();
        let worker_settings = settings.clone();
        let (sender, receiver) = async_channel::bounded(1);
        if let Err(error) = std::thread::Builder::new()
            .name("solo-auto-approval".into())
            .spawn(move || {
                let result =
                    auto_approval::review(reviewer.as_ref(), &worker_settings, &input, &cancel);
                let _ = sender.send_blocking(result);
            })
        {
            pending.review = None;
            pending.review_note = Some(format!(
                "Auto を開始できません。手動で確認してください: {error}"
            ));
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = receiver
                .recv()
                .await
                .unwrap_or_else(|_| Err("Auto 判定の応答を受信できませんでした".into()));
            let _ = this.update(cx, |this, cx| {
                this.complete_auto(&session_id, generation, serial, &settings, result, cx)
            });
        })
        .detach();
    }
    fn complete_auto(
        &mut self,
        session_id: &str,
        generation: u64,
        serial: u64,
        started: &AutoSettings,
        result: Result<Assessment, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.sessions.iter().position(|session| {
            session.model.id == session_id
                && session.stream_generation == generation
                && session
                    .approval
                    .as_ref()
                    .is_some_and(|approval| approval.serial == serial)
        }) else {
            return;
        };
        let current = plan(
            &self.command_rules,
            &self.sessions[index].approval.as_ref().unwrap().request,
        );
        let accepted = current
            .as_ref()
            .ok()
            .and_then(|plan| auto_approval::resolve_result(plan, started, result.as_ref().ok()));
        if let Some(accepted) = accepted {
            let note = match &current {
                Ok(ApprovalPlan::Allow(source) | ApprovalPlan::Deny(source)) => {
                    format!("{source} · {}", if accepted { "許可" } else { "拒否" })
                }
                _ => format!(
                    "Auto · {} · {} · {}",
                    started.model,
                    if accepted { "許可" } else { "拒否" },
                    result
                        .as_ref()
                        .map(|result| result.reason.as_str())
                        .unwrap_or("")
                ),
            };
            self.finish_approval(index, accepted, note, cx);
        } else {
            let approval = self.sessions[index].approval.as_mut().unwrap();
            approval.review = None;
            approval.policy_error = current.as_ref().err().cloned();
            approval.review_note = Some(match current {
                Ok(ApprovalPlan::Auto(settings)) if &settings == started => match result {
                    Ok(result) => format!("Auto · {} · 手動確認: {}", started.model, result.reason),
                    Err(error) => format!("Auto · {} · {error}", started.model),
                },
                _ => "承認設定が変更されました。手動で確認するか再判定してください。".into(),
            });
            cx.notify();
        }
    }
    pub(super) fn reconsider_approvals(&mut self, cx: &mut Context<Self>) {
        self.approval_settings = settings(&self.command_rules);
        let pending: Vec<_> = self
            .sessions
            .iter_mut()
            .filter_map(|session| {
                let approval = session.approval.as_ref()?;
                if let (Some(started), Ok(ApprovalPlan::Auto(current))) = (
                    &approval.review,
                    plan(&self.command_rules, &approval.request),
                ) && started.settings == current
                {
                    return None;
                }
                session.approval.take().map(|approval| {
                    (
                        session.model.id.clone(),
                        session.stream_generation,
                        approval.request,
                        approval.reply,
                    )
                })
            })
            .collect();
        for (id, generation, request, reply) in pending {
            self.receive_approval(&id, generation, request, reply, cx);
        }
        cx.notify();
    }

    pub(super) fn approval_mode_button(&self, cx: &mut Context<Self>) -> Button {
        let (label, tooltip, tone) = match &self.approval_settings {
            Ok(settings) => (
                settings.mode.label(),
                match settings.mode {
                    ApprovalMode::Manual => "Manual · 未登録の要求を手動確認".into(),
                    ApprovalMode::Bypass => "Bypass · 承認とリスト判定を省略".into(),
                    ApprovalMode::Auto => format!("Auto · 判定モデル: {}", settings.auto.model),
                },
                if settings.mode == ApprovalMode::Bypass {
                    Tone::Warning
                } else {
                    Tone::Accent
                },
            ),
            Err(error) => ("設定エラー", error.clone(), Tone::Danger),
        };
        Button::new("approval-mode", label)
            .variant(ButtonVariant::Ghost)
            .control_size(ControlSize::Small)
            .text_color(rgb(ds::theme(cx).tone(tone).0))
            .tooltip(tooltip)
            .on_click(cx.listener(|this, _, window, cx| {
                this.rule_editor.update(cx, |editor, cx| {
                    editor.show_settings = true;
                    cx.notify();
                });
                this.open_command_rules(window, cx);
            }))
    }

    pub(super) fn open_command_rules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.approval_settings = settings(&self.command_rules);
        self.rule_editor.update(cx, |editor, cx| editor.reload(cx));
        cx.notify();
        let editor = self.rule_editor.clone();
        window.open_sheet(cx, move |sheet, _, _| {
            sheet
                .title("実行の承認設定")
                .size(px(620.))
                .child(editor.clone())
        });
    }

    pub(super) fn answer_approval(&mut self, accepted: bool, cx: &mut Context<Self>) {
        let Some(approval) = self.sessions[self.selected].approval.as_mut() else {
            return;
        };
        if accepted {
            if !approval.request.can_allow {
                return;
            }
            // Recheck at confirmation: another window may have blacklisted this command.
            match plan(&self.command_rules, &approval.request) {
                Ok(ApprovalPlan::Deny(_)) => {
                    approval.policy_error = Some(
                        "Blacklist に一致します。実行するにはルールを削除してください。".into(),
                    );
                    cx.notify();
                    return;
                }
                Err(error) => {
                    approval.policy_error = Some(error);
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }
        self.finish_approval(
            self.selected,
            accepted,
            format!("Manual · {}", if accepted { "許可" } else { "拒否" }),
            cx,
        );
    }

    pub(super) fn remember_approval(&mut self, list: RuleList, cx: &mut Context<Self>) {
        let Some(approval) = self.sessions[self.selected].approval.as_mut() else {
            return;
        };
        if list == RuleList::Whitelist && !approval.request.can_allow {
            return;
        }
        let Some(command) = &approval.request.command else {
            return;
        };
        let result = self
            .command_rules
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|store| {
                store
                    .add(list, command.clone())
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            approval.policy_error = Some(format!("ルールを保存できません: {error}"));
            cx.notify();
            return;
        }
        self.rule_editor.update(cx, |editor, cx| editor.reload(cx));
        self.answer_approval(list == RuleList::Whitelist, cx);
        changed(cx);
    }

    pub(super) fn approval_card(&self, cx: &mut Context<Self>) -> Option<Div> {
        let approval = self.sessions[self.selected].approval.as_ref()?;
        let request = &approval.request;
        let p = ds::theme(cx);
        let details = serde_json::to_string_pretty(&request.details)
            .unwrap_or_else(|_| request.details.to_string());
        let can_remember = request.command.is_some();
        let body = div()
            .id("approval-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .when_some(approval.review.as_ref(), |v, review| {
                v.child(ds::alert(
                    "Auto 判定中",
                    format!("判定モデル: {}", review.settings.model),
                    Tone::Accent,
                    cx,
                ))
            })
            .when_some(approval.review_note.clone(), |v, note| {
                v.child(ds::alert("承認判定", note, Tone::Warning, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .text_color(rgb(p.secondary))
                    .child(request.executor.clone())
                    .when(request.is_command, |v| {
                        v.child(Icon::Folder.view(p.muted)).child(
                            request
                                .cwd
                                .as_ref()
                                .map(|cwd| cwd.display().to_string())
                                .unwrap_or_else(|| "未提供".into()),
                        )
                    }),
            )
            .when_some(request.display_command.clone(), |v, command| {
                let copy = command.clone();
                v.child(
                    div().flex().items_center().justify_end().child(
                        Button::icon("copy-command", Icon::Copy, "コマンド全文をコピー")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy(copy.clone(), "コマンドをコピーしました", cx)
                            })),
                    ),
                )
                .child(command_block("approval-command", &command, cx))
            })
            .when(
                request.is_command && request.display_command.is_none(),
                |v| v.child(ds::badge("コマンド未提供", Tone::Warning, cx)),
            )
            .when_some(approval.policy_error.clone(), |v, error| {
                v.child(ds::alert("ルールエラー", error, Tone::Danger, cx))
            })
            .when(!request.can_allow, |v| {
                v.child(ds::badge("今回のみの許可に未対応", Tone::Warning, cx))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::icon(
                            "approval-details",
                            Icon::Info,
                            if approval.details_open {
                                "要求の詳細を閉じる"
                            } else {
                                "要求の詳細"
                            },
                        )
                        .toggled(approval.details_open)
                        .control_size(ControlSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(approval) = this.sessions[this.selected].approval.as_mut() {
                                approval.details_open = !approval.details_open;
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        Button::icon("approval-copy", Icon::Copy, "要求の詳細をコピー")
                            .control_size(ControlSize::Small)
                            .on_click(cx.listener({
                                let details = details.clone();
                                move |this, _, _, cx| {
                                    this.copy(details.clone(), "承認要求の詳細をコピーしました", cx)
                                }
                            })),
                    ),
            )
            .when(
                approval.details_open || !request.is_command || request.display_command.is_none(),
                |v| v.child(command_block("approval-details-json", &details, cx)),
            );
        let footer = div().flex_shrink_0().flex().flex_col().gap_2().pt_2().border_t_1().border_color(rgb(p.border))
            .when(can_remember, |v| v.child(div().flex().flex_wrap().items_center().gap_2()
                .child(Button::new("approval-blacklist", "登録して拒否").with_icon(Icon::Close).control_size(ControlSize::Small)
                    .tooltip("Blacklist に登録して今回の要求を拒否")
                    .on_click(cx.listener(|this, _, _, cx| this.remember_approval(RuleList::Blacklist, cx))))
                .child(Button::new("approval-whitelist", "登録して許可").with_icon(Icon::Check).control_size(ControlSize::Small)
                    .tooltip("Whitelist に登録して今回の要求を許可").disabled(!request.can_allow)
                    .on_click(cx.listener(|this, _, _, cx| this.remember_approval(RuleList::Whitelist, cx))))
                .child(ds::indicator("approval-rule-scope", Icon::Info, "", "今回は完全一致で登録。ワイルドカードはコマンド実行ルール画面で編集できます", Tone::Neutral, cx))))
            .child(div().flex().flex_wrap().items_center().justify_between().gap_2()
                .child(Button::icon("retry-auto-approval", Icon::RotateCcw, "設定を読み直して再判定")
                    .control_size(ControlSize::Small).on_click(cx.listener(|this, _, _, cx| this.reconsider_approvals(cx))))
                .when(request.is_command, |v| v.child(Button::icon("approval-rules", Icon::Sliders, "コマンド実行ルール").control_size(ControlSize::Small)
                    .on_click(cx.listener(|this, _, window, cx| this.open_command_rules(window, cx)))))
                .child(div().flex().gap_2()
                    .child(Button::new("approval-decline", "拒否").control_size(ControlSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| this.answer_approval(false, cx))))
                    .child(Button::new("approval-accept", if request.is_command { "今回のみ実行" } else { "今回のみ許可" })
                        .variant(ButtonVariant::Primary).control_size(ControlSize::Small).disabled(!request.can_allow)
                        .on_click(cx.listener(|this, _, _, cx| this.answer_approval(true, cx))))));
        Some(
            ds::card(cx)
                .min_h_0()
                .h_full()
                .p_3()
                .gap_3()
                .border_color(rgb(p.warning))
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            if request.is_command {
                                Icon::Terminal
                            } else {
                                Icon::Warning
                            }
                            .view(p.warning),
                        )
                        .child(
                            div()
                                .flex_1()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(request.title.clone()),
                        )
                        .child(ds::badge(
                            if approval.policy_error.is_some() {
                                "ルール要確認"
                            } else {
                                "承認待ち"
                            },
                            Tone::Warning,
                            cx,
                        )),
                )
                .child(body)
                .child(footer),
        )
    }
}

pub(super) fn command_block(id: impl Into<ElementId>, text: &str, cx: &App) -> Stateful<Div> {
    let p = ds::theme(cx);
    let truncated = text.len() > 16 * 1024;
    div()
        .id(id)
        .w_full()
        .flex_shrink_0()
        .max_h(px(140.))
        .overflow_y_scroll()
        .p_3()
        .rounded(px(ds::radius::CONTROL))
        .bg(rgb(p.canvas))
        .border_1()
        .border_color(rgb(p.border))
        .font_family(typography::MONO)
        .text_size(px(12.))
        .line_height(px(18.))
        .child(solo::event::preview(text, 16 * 1024))
        .when(truncated, |v| {
            v.child(div().text_color(rgb(p.warning)).child("表示省略"))
        })
}

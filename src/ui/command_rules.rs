use super::approvals::{command_block, rule_store};
use super::*;
use gpui_kit::component::{
    Sizable, WindowExt,
    tab::{Tab as KitTab, TabBar},
};
use solo::command_rules::{CommandInvocation, CommandRule, Matching, Rules};
use solo::harness::workspace::{TOOL_NAMES, is_read_only_tool, tool_description};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Commands,
    Tools,
    Settings,
}

pub(super) struct CommandRuleEditor {
    pub(super) store: Result<RuleStore, String>,
    pub(super) rules: Rules,
    pub(super) error: Option<String>,
    pub(super) list: RuleList,
    pub(super) draft: Entity<Composer>,
    pub(super) matching: Matching,
    editing: Option<CommandRule>,
    pub(super) page: Page,
    pub(super) approval: ApprovalSettings,
    mode_picker: Entity<Select>,
    tool_pickers: Vec<Entity<Select>>,
    pub(super) auto_model: Entity<Composer>,
    _subscriptions: Vec<Subscription>,
}
impl CommandRuleEditor {
    pub(super) fn new(
        store: Result<RuleStore, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = cx.new(|cx| Composer::multiline(window, cx).placeholder("例: cargo test *"));
        let mode_picker = cx.new(|cx| Select::new(["Manual", "Bypass", "Auto"], 0, window, cx));
        let auto_model =
            cx.new(|cx| Composer::new(window, cx).placeholder(solo::codex::DEFAULT_MODEL));
        let mut subscriptions = vec![cx.subscribe(
            &mode_picker,
            |this, _, selected: &ds::SelectionChanged, cx| {
                this.approval.mode = [
                    ApprovalMode::Manual,
                    ApprovalMode::Bypass,
                    ApprovalMode::Auto,
                ][selected.index];
                this.auto_model.update(cx, |input, cx| {
                    input.disabled = this.approval.mode != ApprovalMode::Auto;
                    cx.notify();
                });
                cx.notify();
            },
        )];
        let mut tool_pickers = Vec::new();
        for name in TOOL_NAMES {
            let picker = cx.new(|cx| Select::new(["既定", "Allow", "Deny", "Ask"], 0, window, cx));
            let tool = (*name).to_string();
            subscriptions.push(cx.subscribe(
                &picker,
                move |this, _, selected: &ds::SelectionChanged, cx| {
                    let decision = match selected.index {
                        1 => Some(Decision::Allow),
                        2 => Some(Decision::Deny),
                        3 => Some(Decision::Ask),
                        _ => None,
                    };
                    this.set_tool_decision(&tool, decision, cx);
                },
            ));
            tool_pickers.push(picker);
        }
        let mut this = Self {
            store,
            rules: Rules::default(),
            error: None,
            list: RuleList::Allow,
            draft,
            matching: Matching::Wildcard,
            editing: None,
            page: Page::Commands,
            approval: ApprovalSettings::default(),
            mode_picker,
            tool_pickers,
            auto_model,
            _subscriptions: subscriptions,
        };
        this.reload(cx);
        this
    }
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        match rule_store(&self.store)
            .and_then(|store| store.approval_policy().map_err(|error| error.to_string()))
        {
            Ok((approval, rules)) => {
                self.mode_picker.update(cx, |picker, cx| {
                    picker.selected = approval.mode as usize;
                    picker.close(cx);
                });
                self.auto_model.update(cx, |input, cx| {
                    input.set_value(approval.auto.model.clone(), cx);
                    input.disabled = approval.mode != ApprovalMode::Auto;
                    cx.notify();
                });
                self.approval = approval;
                self.rules = rules;
                self.error = None;
                for (picker, name) in self.tool_pickers.iter().zip(TOOL_NAMES) {
                    let selected = match self.rules.tool_decision(name) {
                        Some(Decision::Allow) => 1,
                        Some(Decision::Deny) => 2,
                        Some(Decision::Ask) => 3,
                        None => 0,
                    };
                    picker.update(cx, |picker, cx| {
                        picker.selected = selected;
                        picker.close(cx);
                    });
                }
            }
            Err(error) => {
                self.error = Some(error);
                self.rules = Rules::default();
            }
        }
        cx.notify();
    }
    pub(super) fn add(&mut self, cx: &mut Context<Self>) {
        let result = rule_store(&self.store).and_then(|store| {
            let text = self.draft.read(cx).value(cx).to_string();
            let mut rule = if let Some(old) = &self.editing {
                let mut rule = old.clone();
                rule.command = text;
                rule
            } else {
                CommandRule::from(
                    CommandInvocation::shell(text, store.workspace())
                        .map_err(|error| error.to_string())?,
                )
            };
            rule.matching = self.matching;
            if let Some(old) = &self.editing {
                store.replace(self.list, old, rule)
            } else {
                store.add(self.list, rule)
            }
            .map_err(|error| error.to_string())
        });
        self.applied(
            result,
            |this, cx| {
                this.cancel_edit(cx);
                this.reload(cx);
            },
            cx,
        );
    }
    pub(super) fn edit(&mut self, rule: CommandRule, cx: &mut Context<Self>) {
        self.draft
            .update(cx, |input, cx| input.set_value(rule.command.clone(), cx));
        self.matching = rule.matching;
        self.editing = Some(rule);
        self.error = None;
        cx.notify();
    }
    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.matching = Matching::Wildcard;
        self.draft.update(cx, |input, cx| input.set_value("", cx));
        cx.notify();
    }
    pub(super) fn set_tool_decision(
        &mut self,
        tool: &str,
        decision: Option<Decision>,
        cx: &mut Context<Self>,
    ) {
        let result = rule_store(&self.store).and_then(|store| {
            store
                .set_tool(tool, decision)
                .map_err(|error| error.to_string())
        });
        self.applied(
            result,
            |this, cx| {
                this.rules.set_tool(tool, decision);
                this.error = None;
                cx.notify();
            },
            cx,
        );
    }
    pub(super) fn remove(&mut self, command: &CommandRule, cx: &mut Context<Self>) {
        let result = rule_store(&self.store).and_then(|store| {
            store
                .remove(self.list, command)
                .map_err(|error| error.to_string())
        });
        self.applied(result, Self::reload, cx);
    }
    pub(super) fn save_settings(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.approval.clone();
        settings.auto.model = self.auto_model.read(cx).value(cx).trim().to_owned();
        let result = rule_store(&self.store).and_then(|store| {
            store
                .config_store()
                .ok_or_else(|| "承認モードは config.yml に保存してください".to_owned())?
                .set_approval(settings)
                .map_err(|error| error.to_string())
        });
        self.applied(result, Self::reload, cx);
    }

    /// ルール保存後の共通処理。成功なら画面を再反映してポリシー変更を通知し、
    /// 失敗ならエラーを保持する。
    fn applied(
        &mut self,
        result: Result<(), String>,
        refresh: impl FnOnce(&mut Self, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                refresh(self, cx);
                super::approvals::changed(cx);
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    /// コマンドルールページ。登録フォームと Allow/Deny リストの編集。
    fn commands_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let entries = self.rules.entries(self.list);
        let list = self.list;
        div()
            .id("command-rules-page")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(ds::indicator(
                        "rules-policy",
                        Icon::Info,
                        "",
                        "* のワイルドカードに対応 · Manual / Auto では Deny 優先",
                        Tone::Neutral,
                        cx,
                    )),
            )
            .when_some(self.store.as_ref().ok(), |v, store| {
                v.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .text_color(rgb(p.muted))
                        .child(Icon::Folder.view(p.muted))
                        .child(store.workspace().display().to_string()),
                )
            })
            .child(
                TabBar::new("rule-lists")
                    .segmented()
                    .small()
                    .selected_index(usize::from(self.list == RuleList::Deny))
                    .child(
                        KitTab::new()
                            .label(format!("Allow {}", self.rules.allow.len()))
                            .disabled(self.editing.is_some()),
                    )
                    .child(
                        KitTab::new()
                            .label(format!("Deny {}", self.rules.deny.len()))
                            .disabled(self.editing.is_some()),
                    )
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.list = if *index == 0 {
                            RuleList::Allow
                        } else {
                            RuleList::Deny
                        };
                        cx.notify();
                    })),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(ds::alert("ルールを更新できません", error, Tone::Danger, cx))
            })
            .child(
                ds::card(cx)
                    .p_3()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(Icon::Terminal.view(p.muted))
                            .child(ds::indicator(
                                "manual-rule-scope",
                                Icon::Info,
                                "",
                                "手入力はローカルの shell 実行に適用。ACP のルールは承認画面から登録",
                                Tone::Neutral,
                                cx,
                            )),
                    )
                    .child(self.draft.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                ds::Toggle::switch(
                                    "wildcard-matching",
                                    "ワイルドカード",
                                    self.matching == Matching::Wildcard,
                                )
                                .on_change(cx.listener(
                                    |this, value: &bool, _, cx| {
                                        this.matching = if *value {
                                            Matching::Wildcard
                                        } else {
                                            Matching::Exact
                                        };
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(ds::indicator(
                                "wildcard-syntax",
                                Icon::Info,
                                "",
                                if self.matching == Matching::Wildcard {
                                    r"* は任意の文字列。文字としての * は \*"
                                } else {
                                    "空白・改行・* を含むコマンドの完全一致"
                                },
                                Tone::Neutral,
                                cx,
                            )),
                    )
                    .when_some(self.editing.as_ref(), |v, rule| {
                        v.child(div().text_size(px(11.)).text_color(rgb(p.secondary)).child(
                            format!(
                                "編集中 · {} · {}",
                                rule.executor_label(),
                                rule.cwd.display()
                            ),
                        ))
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .when(self.editing.is_some(), |v| {
                                v.child(
                                    Button::icon(
                                        "cancel-rule-edit",
                                        Icon::Close,
                                        "編集をキャンセル",
                                    )
                                    .control_size(ControlSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_edit(cx))),
                                )
                            })
                            .child(
                                Button::icon(
                                    "add-command-rule",
                                    if self.editing.is_some() {
                                        Icon::Check
                                    } else {
                                        Icon::Plus
                                    },
                                    if self.editing.is_some() {
                                        "変更を保存".into()
                                    } else {
                                        format!(
                                            "{} に追加",
                                            if list == RuleList::Allow {
                                                "Allow"
                                            } else {
                                                "Deny"
                                            }
                                        )
                                    },
                                )
                                .variant(if list == RuleList::Allow {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Danger
                                })
                                .control_size(ControlSize::Small)
                                .disabled(self.store.is_err())
                                .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .id("command-rule-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(entries.is_empty(), |v| {
                        v.child(ds::empty_state(Icon::Terminal, "未登録", "", cx))
                    })
                    .children(entries.iter().cloned().enumerate().map(|(index, command)| {
                        let remove = command.clone();
                        let edit = command.clone();
                        let copy = command.command.clone();
                        let details = serde_json::to_string_pretty(&command.conditions())
                            .expect("rule serialization");
                        ds::card(cx)
                            .p_3()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(ds::badge(
                                        command.executor_label(),
                                        if list == RuleList::Allow {
                                            Tone::Success
                                        } else {
                                            Tone::Danger
                                        },
                                        cx,
                                    ))
                                    .child(ds::badge(command.matching.label(), Tone::Neutral, cx))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .child(
                                                Button::icon(
                                                    ("edit-rule", index),
                                                    Icon::Sliders,
                                                    "このルールを編集",
                                                )
                                                .control_size(ControlSize::Small)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.edit(edit.clone(), cx)
                                                })),
                                            )
                                            .child(
                                                Button::icon(
                                                    ("copy-rule", index),
                                                    Icon::Copy,
                                                    "コマンド全文をコピー",
                                                )
                                                .control_size(ControlSize::Small)
                                                .on_click(move |_, window, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(copy.clone()),
                                                    );
                                                    window.push_notification(
                                                        "コマンドをコピーしました",
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(
                                                Button::icon(
                                                    ("rule-details", index),
                                                    Icon::Info,
                                                    "ルールの一致条件",
                                                )
                                                .control_size(ControlSize::Small)
                                                .on_click(move |_, window, cx| {
                                                    let details = details.clone();
                                                    window.open_dialog(cx, move |dialog, _, cx| {
                                                        let copy = details.clone();
                                                        dialog
                                                            .title("ルールの一致条件")
                                                            .w(px(540.))
                                                            .child(command_block(
                                                                "rule-details-json",
                                                                &details,
                                                                cx,
                                                            ))
                                                            .child(
                                                                Button::icon(
                                                                    "copy-rule-details",
                                                                    Icon::Copy,
                                                                    "条件の全文をコピー",
                                                                )
                                                                .on_click(move |_, window, cx| {
                                                                    cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            copy.clone(),
                                                                        ),
                                                                    );
                                                                    window.push_notification(
                                                                        "一致条件をコピーしました",
                                                                        cx,
                                                                    );
                                                                }),
                                                            )
                                                    });
                                                }),
                                            )
                                            .child(
                                                Button::icon(
                                                    ("remove-rule", index),
                                                    Icon::Trash,
                                                    "このルールを削除",
                                                )
                                                .control_size(ControlSize::Small)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.remove(&remove, cx)
                                                })),
                                            ),
                                    ),
                            )
                            .child(command_block(("rule-command", index), &command.command, cx))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(11.))
                                    .text_color(rgb(p.muted))
                                    .child(Icon::Folder.view(p.muted))
                                    .child(command.cwd.display().to_string()),
                            )
                            .when(
                                list == RuleList::Allow
                                    && self.rules.deny.contains(&command),
                                |v| v.child(ds::badge("Deny 優先", Tone::Warning, cx)),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::icon(
                            "reload-command-rules",
                            Icon::RotateCcw,
                            "ルールを再読み込み",
                        )
                        .control_size(ControlSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                    )
                    .when_some(self.store.as_ref().ok(), |v, store| {
                        let path = store.path().display().to_string();
                        v.child(
                            Button::icon(
                                "copy-rules-path",
                                Icon::Copy,
                                "設定ファイルのパスをコピー",
                            )
                            .control_size(ControlSize::Small)
                            .on_click(move |_, window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                                window.push_notification("設定ファイルのパスをコピーしました", cx);
                            }),
                        )
                    }),
            )
            .into_any_element()
    }

    fn tools_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        div()
            .id("tool-rules")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                ds::card(cx)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("基本ツール · このプロジェクト"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.secondary))
                            .child("Allow は確認を省略し、Deny は常に拒否、Ask はコマンドルールと同じ承認確認に戻します。既定はツールごとの安全な初期値です。"),
                    )
                    .children(TOOL_NAMES.iter().enumerate().map(|(index, name)| {
                        let decision = self.rules.tool_decision(name);
                        let default = if is_read_only_tool(name) { "許可" } else { "確認" };
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(56.))
                                    .flex_shrink_0()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(*name),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .text_color(rgb(p.secondary))
                                    .truncate()
                                    .child(format!("{default} · {}", tool_description(name))),
                            )
                            .child(
                                div()
                                    .w(px(148.))
                                    .flex_shrink_0()
                                    .child(self.tool_pickers[index].clone()),
                            )
                            .when_some(decision, |v, decision| {
                                v.child(ds::badge(
                                    match decision {
                                        Decision::Allow => "許可",
                                        Decision::Deny => "拒否",
                                        Decision::Ask => "確認",
                                    },
                                    match decision {
                                        Decision::Allow => Tone::Success,
                                        Decision::Deny => Tone::Danger,
                                        Decision::Ask => Tone::Neutral,
                                    },
                                    cx,
                                ))
                            })
                    })),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(ds::alert("ツール設定を保存できません", error, Tone::Danger, cx))
            })
            .into_any_element()
    }
    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let description = match self.approval.mode {
            ApprovalMode::Manual => "リストで決まらない要求を、実行前に確認します。",
            ApprovalMode::Bypass => {
                "承認確認と Allow / Deny の判定を省略し、すべての要求を許可します。"
            }
            ApprovalMode::Auto => {
                "Deny / Allow を優先し、未登録の要求を指定モデルで判定します。判断できない場合は手動確認に戻ります。"
            }
        };
        div()
            .id("approval-settings-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                ds::card(cx)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("承認モード · 全プロジェクト共通"),
                    )
                    .child(self.mode_picker.clone())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.secondary))
                            .child(description),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Auto の判定モデル"),
                    )
                    .child(self.auto_model.clone())
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(p.muted))
                            .child(format!(
                                "ChatGPT ログインで利用できるモデル名 · 制限時間 {} 秒",
                                self.approval.auto.timeout_seconds,
                            )),
                    )
                    .child(
                        Button::new("save-approval-settings", "承認設定を保存")
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_settings(cx);
                                if this.error.is_none() {
                                    window.push_notification("承認設定を保存しました", cx);
                                }
                            })),
                    ),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(ds::alert("設定を保存できません", error, Tone::Danger, cx))
            })
            .when_some(self.store.as_ref().ok(), |v, store| {
                v.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(p.muted))
                        .child(format!("設定ファイル: {}", store.path().display())),
                )
            })
            .into_any_element()
    }
}
impl Render for CommandRuleEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let navigation = TabBar::new("approval-settings-pages")
            .segmented()
            .small()
            .selected_index(match self.page {
                Page::Commands => 0,
                Page::Tools => 1,
                Page::Settings => 2,
            })
            .child(KitTab::new().label("コマンドルール"))
            .child(KitTab::new().label("ツール"))
            .child(KitTab::new().label("承認モード"))
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.page = match *index {
                    1 => Page::Tools,
                    2 => Page::Settings,
                    _ => Page::Commands,
                };
                cx.notify();
            }));
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .gap_3()
            .child(navigation)
            .child(match self.page {
                Page::Commands => self.commands_view(cx),
                Page::Tools => self.tools_view(cx),
                Page::Settings => self.settings_view(cx),
            })
            .into_any_element()
    }
}

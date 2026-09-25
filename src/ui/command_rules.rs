use super::approvals::command_block;
use super::*;
use gpui_kit::component::{
    Sizable, WindowExt,
    tab::{Tab as KitTab, TabBar},
};
use solo::command_rules::{CommandInvocation, CommandRule, Matching, Rules};

pub(super) struct CommandRuleEditor {
    store: Result<RuleStore, String>,
    rules: Rules,
    error: Option<String>,
    list: RuleList,
    draft: Entity<Composer>,
    matching: Matching,
    editing: Option<CommandRule>,
    pub(super) show_settings: bool,
    approval: ApprovalSettings,
    mode_picker: Entity<Select>,
    auto_model: Entity<Composer>,
    _mode_subscription: Subscription,
}
impl CommandRuleEditor {
    pub(super) fn new(
        store: Result<RuleStore, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = cx.new(|cx| Composer::multiline(window, cx).placeholder("例: cargo test *"));
        let mode_picker = cx.new(|cx| Select::new(["Manual", "Bypass", "Auto"], 0, window, cx));
        let auto_model = cx.new(|cx| {
            Composer::new(window, cx).placeholder(solo::codex_subscription::DEFAULT_MODEL)
        });
        let mode_subscription = cx.subscribe(
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
        );
        let mut this = Self {
            store,
            rules: Rules::default(),
            error: None,
            list: RuleList::Whitelist,
            draft,
            matching: Matching::Wildcard,
            editing: None,
            show_settings: false,
            approval: ApprovalSettings::default(),
            mode_picker,
            auto_model,
            _mode_subscription: mode_subscription,
        };
        this.reload(cx);
        this
    }
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        match self
            .store
            .as_ref()
            .map_err(Clone::clone)
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
            }
            Err(error) => {
                self.error = Some(error);
                self.rules = Rules::default();
            }
        }
        cx.notify();
    }
    fn add(&mut self, cx: &mut Context<Self>) {
        let result = self.store.as_ref().map_err(Clone::clone).and_then(|store| {
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
        match result {
            Ok(()) => {
                self.cancel_edit(cx);
                self.reload(cx);
                super::approvals::changed(cx);
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn edit(&mut self, rule: CommandRule, cx: &mut Context<Self>) {
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
    fn remove(&mut self, command: &CommandRule, cx: &mut Context<Self>) {
        let result = self.store.as_ref().map_err(Clone::clone).and_then(|store| {
            store
                .remove(self.list, command)
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => {
                self.reload(cx);
                super::approvals::changed(cx);
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.approval.clone();
        settings.auto.model = self.auto_model.read(cx).value(cx).trim().to_owned();
        let result = self.store.as_ref().map_err(Clone::clone).and_then(|store| {
            store
                .config_store()
                .ok_or_else(|| "承認モードは config.yml に保存してください".to_owned())?
                .set_approval(settings)
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => {
                self.reload(cx);
                super::approvals::changed(cx);
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let description = match self.approval.mode {
            ApprovalMode::Manual => "リストで決まらない要求を、実行前に確認します。",
            ApprovalMode::Bypass => {
                "承認確認と Whitelist / Blacklist の判定を省略し、すべての要求を許可します。"
            }
            ApprovalMode::Auto => {
                "Blacklist / Whitelist を優先し、未登録の要求を指定モデルで判定します。判断できない場合は手動確認に戻ります。"
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
        let p = ds::theme(cx);
        let entries = self.rules.entries(self.list);
        let list = self.list;
        let navigation = TabBar::new("approval-settings-pages")
            .segmented()
            .small()
            .selected_index(usize::from(self.show_settings))
            .child(KitTab::new().label("コマンドルール"))
            .child(KitTab::new().label("承認モード"))
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.show_settings = *index == 1;
                cx.notify();
            }));
        if self.show_settings {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .gap_3()
                .child(navigation)
                .child(self.settings_view(cx))
                .into_any_element();
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .gap_3()
            .child(navigation)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(ds::indicator(
                        "rules-policy",
                        Icon::Info,
                        "",
                        "* のワイルドカードに対応 · Manual / Auto では Blacklist 優先",
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
                    .selected_index(usize::from(self.list == RuleList::Blacklist))
                    .child(
                        KitTab::new()
                            .label(format!("Whitelist {}", self.rules.whitelist.len()))
                            .disabled(self.editing.is_some()),
                    )
                    .child(
                        KitTab::new()
                            .label(format!("Blacklist {}", self.rules.blacklist.len()))
                            .disabled(self.editing.is_some()),
                    )
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.list = if *index == 0 {
                            RuleList::Whitelist
                        } else {
                            RuleList::Blacklist
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
                                            if list == RuleList::Whitelist {
                                                "Whitelist"
                                            } else {
                                                "Blacklist"
                                            }
                                        )
                                    },
                                )
                                .variant(if list == RuleList::Whitelist {
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
                                        if list == RuleList::Whitelist {
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
                                list == RuleList::Whitelist
                                    && self.rules.blacklist.contains(&command),
                                |v| v.child(ds::badge("Blacklist 優先", Tone::Warning, cx)),
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
}

/// Exercise the actual editor callbacks using the smoke test's temporary store.
pub(super) fn smoke(editor: &Entity<CommandRuleEditor>, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        let store = editor.store.as_ref().unwrap().clone();
        editor.list = RuleList::Whitelist;
        editor.matching = Matching::Wildcard;
        editor
            .draft
            .update(cx, |input, cx| input.set_value("cargo test *", cx));
        editor.add(cx);
        assert!(editor.error.is_none());
        let probe = CommandInvocation::shell("cargo test --release", store.workspace()).unwrap();
        assert_eq!(store.evaluate(&probe).unwrap(), Decision::Allow);
        let added = editor
            .rules
            .whitelist
            .iter()
            .find(|rule| rule.command == "cargo test *")
            .unwrap()
            .clone();
        editor.edit(added, cx);
        editor
            .draft
            .update(cx, |input, cx| input.set_value("cargo test --locked*", cx));
        editor.add(cx);
        assert!(editor.error.is_none());
        assert_eq!(store.evaluate(&probe).unwrap(), Decision::Ask);
        let edited = editor
            .rules
            .whitelist
            .iter()
            .find(|rule| rule.command == "cargo test --locked*")
            .unwrap()
            .clone();
        editor.remove(&edited, cx);
        assert!(editor.error.is_none());
        editor
            .draft
            .update(cx, |input, cx| input.set_value("git *", cx));
        editor.add(cx);
        assert!(editor.error.is_none());
    });
}

pub(super) fn settings_smoke(editor: &Entity<CommandRuleEditor>, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        let config = editor
            .store
            .as_ref()
            .unwrap()
            .config_store()
            .unwrap()
            .clone();
        editor.approval.mode = ApprovalMode::Auto;
        editor
            .auto_model
            .update(cx, |input, cx| input.set_value("selected-in-ui", cx));
        editor.save_settings(cx);
        assert!(editor.error.is_none());
        assert_eq!(config.load().unwrap().approval.auto.model, "selected-in-ui");
        assert_eq!(config.load().unwrap().approval.mode, ApprovalMode::Auto);
        editor
            .auto_model
            .update(cx, |input, cx| input.set_value("", cx));
        editor.save_settings(cx);
        assert!(editor.error.is_some());
        assert_eq!(
            config.load().unwrap().approval.auto.model,
            "selected-in-ui",
            "invalid model overwrote the config"
        );
        editor.approval.mode = ApprovalMode::Manual;
        editor
            .auto_model
            .update(cx, |input, cx| input.set_value("selected-in-ui", cx));
        editor.save_settings(cx);
        assert!(editor.error.is_none());
    });
}

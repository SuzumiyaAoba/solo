use super::super::approval_flow::command_block;
use super::CommandRuleEditor;
use gpui_kit::component::{
    Sizable, WindowExt,
    tab::{Tab as KitTab, TabBar},
};
use gpui_kit::{AnyElement, ClipboardItem, Context, div, prelude::*, px, rgb};
use solo::{
    command_rules::{Matching, RuleList},
    design::{self as ds, Button, ButtonVariant, ControlSize, Icon, Tone, typography},
};

impl CommandRuleEditor {
    /// コマンドルールページ。登録フォームと Allow/Deny リストの編集。
    pub(super) fn commands_view(&self, cx: &mut Context<Self>) -> AnyElement {
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
                        .text_size(px(typography::CAPTION))
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
                        v.child(div().text_size(px(typography::CAPTION)).text_color(rgb(p.secondary)).child(
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
                                    .text_size(px(typography::CAPTION))
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
}

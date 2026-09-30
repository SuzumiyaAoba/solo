//! コマンドルールと承認モードの編集エディタ。各ページの描画は子モジュールに分ける。
mod commands;
mod settings;
mod tools;

use super::approval_flow::rule_store;
use gpui_kit::component::{
    Sizable,
    tab::{Tab as KitTab, TabBar},
};
use gpui_kit::{Context, Entity, Subscription, Window, div, prelude::*};
use solo::command_rules::{
    CommandInvocation, CommandRule, Decision, Matching, RuleList, RuleStore, Rules,
};
use solo::config::{ApprovalMode, ApprovalSettings};
use solo::design::{self as ds, Select, TextInput as Composer};
use solo::harness::workspace::tool_names;

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
        for name in tool_names() {
            let picker = cx.new(|cx| Select::new(["既定", "Allow", "Deny", "Ask"], 0, window, cx));
            let tool = name.to_string();
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
                for (picker, name) in self.tool_pickers.iter().zip(tool_names()) {
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
                super::approval_flow::changed(cx);
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
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

use super::super::rule_editor::CommandRuleEditor;
use gpui_kit::{App, Entity};
use solo::command_rules::{CommandInvocation, Decision, Matching, RuleList};
use solo::config::ApprovalMode;

/// Exercise the actual editor callbacks using the smoke test's temporary store.
pub(super) fn smoke(editor: &Entity<CommandRuleEditor>, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        let store = editor.store.as_ref().unwrap().clone();
        editor.list = RuleList::Allow;
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
            .allow
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
            .allow
            .iter()
            .find(|rule| rule.command == "cargo test --locked*")
            .unwrap()
            .clone();
        editor.remove(&edited, cx);
        assert!(editor.error.is_none());
        // ツール判定: Deny / Ask / 既定(Allow) への往復と保存を確認する。
        assert!(store.load().unwrap().tool_decision("exec").is_none());
        editor.set_tool_decision("exec", Some(Decision::Deny), cx);
        assert!(editor.error.is_none());
        assert_eq!(
            store.load().unwrap().tool_decision("exec"),
            Some(Decision::Deny)
        );
        editor.set_tool_decision("exec", Some(Decision::Ask), cx);
        assert_eq!(
            store.load().unwrap().tool_decision("exec"),
            Some(Decision::Ask)
        );
        editor.set_tool_decision("read", Some(Decision::Ask), cx);
        assert_eq!(
            store.load().unwrap().tool_decision("read"),
            Some(Decision::Ask)
        );
        editor.set_tool_decision("exec", None, cx);
        assert!(store.load().unwrap().tool_decision("exec").is_none());
        assert_eq!(
            store.load().unwrap().tool_decision("read"),
            Some(Decision::Ask)
        );

        // ツールタブの実描画まで通す。
        editor.page = super::super::rule_editor::Page::Tools;
        cx.notify();

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

use serde_json::json;
use solo::{
    acp::AgentProfile,
    approval::ApprovalRequest,
    command_rules::{CommandInvocation, CommandRule, Decision, Matching, RuleList, RuleStore},
    harness::{
        self, Cancellation, Limits, Message, Model, ModelOutput, StopReason, ToolCall, ToolSpec,
        workspace::WorkspaceTools,
    },
};
use std::{collections::VecDeque, fs, path::Path};

fn store(workspace: &Path) -> RuleStore {
    RuleStore::at_path(workspace.join("private/rules.json"), workspace).unwrap()
}
fn command(workspace: &Path, text: &str) -> CommandInvocation {
    CommandInvocation::shell(text, workspace).unwrap()
}
fn agent() -> AgentProfile {
    AgentProfile {
        id: "demo".into(),
        name: "Demo".into(),
        command: "demo-agent".into(),
        args: vec!["--acp".into()],
    }
}

#[test]
fn exact_matching_never_allows_extra_shell_operations_or_different_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let safe = command(dir.path(), "git status");
    assert_eq!(store.evaluate(&safe).unwrap(), Decision::Ask);
    store.add(RuleList::Whitelist, safe.clone()).unwrap();
    assert_eq!(store.evaluate(&safe).unwrap(), Decision::Allow);
    for text in [
        "git status --short",
        "git status; touch marker",
        "git status && touch marker",
        "git status\ntouch marker",
        "git status$(touch marker)",
        "git status ",
        "GIT status",
    ] {
        assert_eq!(
            store.evaluate(&command(dir.path(), text)).unwrap(),
            Decision::Ask,
            "{text}"
        );
    }
    fs::create_dir(dir.path().join("other")).unwrap();
    assert_eq!(
        store
            .evaluate(&command(&dir.path().join("other"), "git status"))
            .unwrap(),
        Decision::Ask
    );
}

#[test]
fn blacklist_wins_and_rules_survive_reload_and_removal() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let item = command(dir.path(), "cargo test");
    store.add(RuleList::Blacklist, item.clone()).unwrap();
    store.add(RuleList::Whitelist, item.clone()).unwrap();
    store.add(RuleList::Whitelist, item.clone()).unwrap();
    let reopened = RuleStore::at_path(store.path(), dir.path()).unwrap();
    assert_eq!(reopened.load().unwrap().whitelist.len(), 1);
    assert_eq!(reopened.evaluate(&item).unwrap(), Decision::Deny);
    reopened.remove(RuleList::Blacklist, &item).unwrap();
    assert_eq!(store.evaluate(&item).unwrap(), Decision::Allow);
    reopened.remove(RuleList::Whitelist, &item).unwrap();
    assert_eq!(store.evaluate(&item).unwrap(), Decision::Ask);
}

#[test]
fn workspaces_and_other_writers_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let a = store(dir.path());
    let stale = a.clone();
    let b = RuleStore::at_path(a.path(), other.path()).unwrap();
    let item = command(dir.path(), "pwd");
    a.add(RuleList::Whitelist, item.clone()).unwrap();
    assert_eq!(b.evaluate(&item).unwrap(), Decision::Ask);
    b.add(RuleList::Blacklist, command(other.path(), "false"))
        .unwrap();
    stale
        .add(RuleList::Whitelist, command(dir.path(), "ls"))
        .unwrap();
    assert_eq!(a.load().unwrap().whitelist.len(), 2);
    assert_eq!(b.load().unwrap().blacklist.len(), 1);
}

#[test]
fn malformed_or_future_configuration_cannot_grant_or_be_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    let item = command(dir.path(), "pwd");
    for source in [
        "{",
        r#"{"version":3,"workspaces":{}}"#,
        r#"{"version":1,"workspaces":{},"typo":true}"#,
    ] {
        fs::write(store.path(), source).unwrap();
        assert!(store.evaluate(&item).is_err());
        assert!(store.add(RuleList::Whitelist, item.clone()).is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), source);
    }
}

#[test]
fn busy_store_never_loses_an_update() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let item = command(dir.path(), "pwd");
    store.add(RuleList::Whitelist, item.clone()).unwrap();
    let lock = fs::File::options()
        .write(true)
        .open(store.path().with_extension("json.lock"))
        .unwrap();
    lock.lock().unwrap();
    assert!(store.add(RuleList::Blacklist, item.clone()).is_err());
    assert_eq!(store.evaluate(&item).unwrap(), Decision::Allow);
    lock.unlock().unwrap();
    store.add(RuleList::Blacklist, item.clone()).unwrap();
    assert_eq!(store.evaluate(&item).unwrap(), Decision::Deny);
}

#[test]
fn empty_and_invalid_rules_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    for text in ["", " \n ", "abc\0def"] {
        assert!(CommandInvocation::shell(text, dir.path()).is_err());
    }
    let mut item = command(dir.path(), "pwd");
    item.cwd = "relative".into();
    assert!(store(dir.path()).add(RuleList::Whitelist, item).is_err());
}

#[test]
fn command_approval_uses_actual_arguments_and_preserves_multiline_text() {
    let dir = tempfile::tempdir().unwrap();
    let text = "printf '日本語'\nprintf 'second line'";
    let request = ApprovalRequest::tool(
        &ToolCall {
            id: "1".into(),
            name: "exec".into(),
            arguments: json!({"command":text}),
        },
        dir.path(),
    );
    assert!(request.is_command);
    assert_eq!(request.display_command.as_deref(), Some(text));
    assert_eq!(
        request.command.unwrap().cwd,
        dir.path().canonicalize().unwrap()
    );
    let edit = ApprovalRequest::tool(
        &ToolCall {
            id: "2".into(),
            name: "edit".into(),
            arguments: json!({"command":"pwd","path":"note.txt"}),
        },
        dir.path(),
    );
    assert!(!edit.is_command);
    assert!(edit.command.is_none());
}

#[test]
fn acp_does_not_treat_titles_or_incomplete_metadata_as_authority() {
    let dir = tempfile::tempdir().unwrap();
    let base = json!({"toolCall":{"toolCallId":"tool-1","kind":"execute","title":"git status"},"options":[{"optionId":"allow","kind":"allow_once"}]});
    let mut params = base.clone();
    assert!(
        ApprovalRequest::acp(params.clone(), &agent(), dir.path())
            .command
            .is_none()
    );
    params["toolCall"]["rawInput"] = json!({"command":"git status"});
    let incomplete = ApprovalRequest::acp(params.clone(), &agent(), dir.path());
    assert_eq!(incomplete.display_command.as_deref(), Some("git status"));
    assert!(incomplete.command.is_none());
    params["toolCall"]["rawInput"]["cwd"] = json!(dir.path());
    assert!(
        ApprovalRequest::acp(params.clone(), &agent(), dir.path())
            .command
            .is_some()
    );
    params["toolCall"]["kind"] = json!("edit");
    assert!(
        ApprovalRequest::acp(params.clone(), &agent(), dir.path())
            .command
            .is_none()
    );
    params["options"] = json!([{"kind":"allow_always"}]);
    assert!(!ApprovalRequest::acp(params, &agent(), dir.path()).can_allow);
}

#[test]
fn acp_rules_include_agent_executable_arguments_and_entire_raw_input() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let params = json!({"toolCall":{"toolCallId":"1","name":"shell","kind":"execute","title":"Read status","rawInput":{"command":"git status","cwd":dir.path(),"env":{"MODE":"safe"}}},"options":[{"optionId":"allow","kind":"allow_once"}]});
    let original = ApprovalRequest::acp(params.clone(), &agent(), dir.path())
        .command
        .unwrap();
    store.add(RuleList::Whitelist, original.clone()).unwrap();
    assert_eq!(store.evaluate(&original).unwrap(), Decision::Allow);
    let mut changed = params.clone();
    changed["toolCall"]["rawInput"]["env"]["MODE"] = json!("different");
    let changed = ApprovalRequest::acp(changed, &agent(), dir.path())
        .command
        .unwrap();
    assert_eq!(store.evaluate(&changed).unwrap(), Decision::Ask);
    let mut different_agent = agent();
    different_agent.command = "other-agent".into();
    let changed = ApprovalRequest::acp(params, &different_agent, dir.path())
        .command
        .unwrap();
    assert_eq!(store.evaluate(&changed).unwrap(), Decision::Ask);
    assert_eq!(
        store.evaluate(&command(dir.path(), "git status")).unwrap(),
        Decision::Ask
    );
}

#[test]
fn denied_commands_never_reach_the_shell() {
    struct Script(VecDeque<ModelOutput>);
    impl Model for Script {
        fn complete(&mut self, _: &[Message], _: &[ToolSpec]) -> Result<ModelOutput, String> {
            self.0.pop_front().ok_or("unexpected request".into())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let allowed = command(dir.path(), "touch allowed");
    let denied = command(dir.path(), "touch denied");
    store.add(RuleList::Whitelist, allowed).unwrap();
    store.add(RuleList::Whitelist, denied.clone()).unwrap();
    store.add(RuleList::Blacklist, denied).unwrap();
    let mut model = Script(VecDeque::from([
        ModelOutput {
            text: "".into(),
            tool_calls: ["touch allowed", "touch denied", "touch unknown"]
                .into_iter()
                .enumerate()
                .map(|(id, command)| ToolCall {
                    id: id.to_string(),
                    name: "exec".into(),
                    arguments: json!({"command":command}),
                })
                .collect(),
        },
        ModelOutput {
            text: "done".into(),
            tool_calls: vec![],
        },
    ]));
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let mut confirmations = 0;
    let mut policy = |call: &ToolCall| {
        let request = ApprovalRequest::tool(call, dir.path());
        match store.evaluate(request.command.as_ref().unwrap()).unwrap() {
            Decision::Allow => true,
            Decision::Deny => false,
            Decision::Ask => {
                confirmations += 1;
                false
            }
        }
    };
    let result = harness::run(
        &mut model,
        &mut tools,
        &mut policy,
        vec![],
        &Limits::default(),
        &Cancellation::default(),
        |_| {},
    );
    assert_eq!(result.stop, StopReason::Completed);
    assert!(dir.path().join("allowed").exists());
    assert!(!dir.path().join("denied").exists());
    assert!(!dir.path().join("unknown").exists());
    assert_eq!(confirmations, 1);
}

#[test]
fn incomplete_commands_still_fail_closed_when_rules_cannot_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(store.path(), "{").unwrap();
    let request = ApprovalRequest::acp(
        json!({"toolCall":{"kind":"execute","rawInput":{"command":"pwd"}},"options":[{"optionId":"allow","kind":"allow_once"}]}),
        &agent(),
        dir.path(),
    );
    assert!(request.command.is_none());
    assert!(request.decision(&store).is_err());
    let request = ApprovalRequest::tool(
        &ToolCall {
            id: "edit".into(),
            name: "edit".into(),
            arguments: json!({}),
        },
        dir.path(),
    );
    assert_eq!(request.decision(&store).unwrap(), Decision::Ask);
}

#[test]
fn acp_requires_a_usable_allow_once_option() {
    let dir = tempfile::tempdir().unwrap();
    for options in [
        json!([{"kind":"allow_once"}]),
        json!([{"kind":"allow_once","optionId":""}]),
        json!([{"kind":"allow_always","optionId":"always"}]),
    ] {
        assert!(
            !ApprovalRequest::acp(
                json!({"toolCall":{"kind":"execute"}, "options":options}),
                &agent(),
                dir.path()
            )
            .can_allow
        );
    }
    assert!(ApprovalRequest::acp(json!({"toolCall":{"kind":"execute"}, "options":[{"kind":"allow_once","optionId":"once"}]}), &agent(), dir.path()).can_allow);
}

fn wildcard(workspace: &Path, pattern: &str) -> CommandRule {
    CommandRule::from(command(workspace, pattern)).with_matching(Matching::Wildcard)
}

#[test]
fn wildcard_matches_variable_arguments_unicode_and_multiple_stars() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "cargo test *"))
        .unwrap();
    for text in [
        "cargo test ",
        "cargo test --locked",
        "cargo test --package ui -- --nocapture",
        "cargo test 日本語🙂",
    ] {
        assert_eq!(
            store.evaluate(&command(dir.path(), text)).unwrap(),
            Decision::Allow,
            "{text}"
        );
    }
    for text in [
        "cargo test",
        "cargo tests --locked",
        "other cargo test --locked",
        "CARGO test --locked",
    ] {
        assert_eq!(
            store.evaluate(&command(dir.path(), text)).unwrap(),
            Decision::Ask,
            "{text}"
        );
    }
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "git * -- *rs"))
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "git diff -- src/main.rs"))
            .unwrap(),
        Decision::Allow
    );
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "git diff -- README.md"))
            .unwrap(),
        Decision::Ask
    );
    fs::create_dir(dir.path().join("subdir")).unwrap();
    assert_eq!(
        store
            .evaluate(&command(&dir.path().join("subdir"), "cargo test --locked"))
            .unwrap(),
        Decision::Ask
    );
}

#[test]
fn wildcard_blacklist_overrides_exact_and_wildcard_whitelists() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "git *"))
        .unwrap();
    store
        .add(
            RuleList::Whitelist,
            command(dir.path(), "git push origin main"),
        )
        .unwrap();
    store
        .add(RuleList::Blacklist, wildcard(dir.path(), "git push *"))
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "git status --short"))
            .unwrap(),
        Decision::Allow
    );
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "git push origin main"))
            .unwrap(),
        Decision::Deny
    );
    store
        .add(RuleList::Blacklist, wildcard(dir.path(), "*touch marker*"))
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(
                dir.path(),
                "echo before; touch marker; echo after"
            ))
            .unwrap(),
        Decision::Deny
    );
}

#[test]
fn wildcard_does_not_swallow_new_shell_operators_or_expansions() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "echo *"))
        .unwrap();
    for text in [
        "echo ok; touch marker",
        "echo ok && touch marker",
        "echo ok || touch marker",
        "echo ok | sh",
        "echo ok & touch marker",
        "echo ok\ntouch marker",
        "echo ok > marker",
        "echo $(touch marker)",
        "echo `touch marker`",
        "echo \"$(touch marker)\"",
        "echo $HOME",
        "echo <(touch marker)",
        "echo ok # comment",
        "echo 'unclosed",
        "echo ok <<EOF\ntouch marker\nEOF",
    ] {
        assert_eq!(
            store.evaluate(&command(dir.path(), text)).unwrap(),
            Decision::Ask,
            "{text}"
        );
    }
    for text in [
        "echo 'a;b|$(literal)'",
        "echo \"a;b|literal\"",
        r"echo literal\;value",
        r"echo literal\$value",
    ] {
        assert_eq!(
            store.evaluate(&command(dir.path(), text)).unwrap(),
            Decision::Allow,
            "{text}"
        );
    }
    store
        .add(
            RuleList::Whitelist,
            wildcard(dir.path(), "echo * && printf *"),
        )
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "echo hello && printf world"))
            .unwrap(),
        Decision::Allow
    );
    assert_eq!(
        store
            .evaluate(&command(
                dir.path(),
                "echo hello && printf world; touch marker"
            ))
            .unwrap(),
        Decision::Ask
    );
}

#[test]
fn wildcard_escaping_keeps_literal_stars_and_backslashes() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), r"echo \*"))
        .unwrap();
    assert_eq!(
        store.evaluate(&command(dir.path(), "echo *")).unwrap(),
        Decision::Allow
    );
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "echo anything"))
            .unwrap(),
        Decision::Ask
    );
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), r"echo \\*"))
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), r"echo \literal"))
            .unwrap(),
        Decision::Allow
    );
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "echo [abc]?"))
        .unwrap();
    assert_eq!(
        store.evaluate(&command(dir.path(), "echo [abc]?")).unwrap(),
        Decision::Allow
    );
    assert_eq!(
        store.evaluate(&command(dir.path(), "echo ax")).unwrap(),
        Decision::Ask
    );
}

#[test]
fn existing_star_rules_remain_exact_across_v1_upgrade_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let exact = command(dir.path(), "echo *");
    let workspace = store.workspace().to_str().unwrap();
    let legacy = json!({"version":1,"workspaces":{workspace:{"whitelist":[exact],"blacklist":[]}}});
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(store.load().unwrap().whitelist[0].matching, Matching::Exact);
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "echo expanded"))
            .unwrap(),
        Decision::Ask
    );
    store
        .add(RuleList::Whitelist, wildcard(dir.path(), "cargo test *"))
        .unwrap();
    let file: serde_json::Value = serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(file["version"], 2);
    let reopened = RuleStore::at_path(store.path(), dir.path()).unwrap();
    assert_eq!(
        reopened
            .evaluate(&command(dir.path(), "cargo test --locked"))
            .unwrap(),
        Decision::Allow
    );
    assert_eq!(
        reopened
            .evaluate(&command(dir.path(), "echo expanded"))
            .unwrap(),
        Decision::Ask
    );
    assert_eq!(
        reopened.evaluate(&command(dir.path(), "echo *")).unwrap(),
        Decision::Allow
    );
}

#[test]
fn editing_a_rule_is_atomic_and_detects_stale_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let exact = CommandRule::from(command(dir.path(), "cargo test --locked"));
    store.add(RuleList::Whitelist, exact.clone()).unwrap();
    let pattern = wildcard(dir.path(), "cargo test *");
    store
        .replace(RuleList::Whitelist, &exact, pattern.clone())
        .unwrap();
    assert_eq!(
        store.load().unwrap().whitelist.as_slice(),
        std::slice::from_ref(&pattern)
    );
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "cargo test --release"))
            .unwrap(),
        Decision::Allow
    );
    assert!(
        store
            .replace(RuleList::Whitelist, &exact, wildcard(dir.path(), "*"))
            .is_err()
    );
    assert_eq!(
        store.load().unwrap().whitelist.as_slice(),
        std::slice::from_ref(&pattern)
    );
    store.remove(RuleList::Whitelist, &pattern).unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), "cargo test --release"))
            .unwrap(),
        Decision::Ask
    );
}

#[test]
fn acp_patterns_vary_commands_but_not_other_input_or_input_representation() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let params = json!({"toolCall":{"toolCallId":"1","name":"shell","kind":"execute","rawInput":{"command":"git status","cwd":dir.path(),"env":{"MODE":"safe"}}},"options":[{"kind":"allow_once","optionId":"once"}]});
    let original = ApprovalRequest::acp(params.clone(), &agent(), dir.path())
        .command
        .unwrap();
    let mut pattern = CommandRule::from(original).with_matching(Matching::Wildcard);
    pattern.command = "git *".into();
    store.add(RuleList::Whitelist, pattern).unwrap();
    let mut changed = params.clone();
    changed["toolCall"]["rawInput"]["command"] = json!("git diff --stat");
    let request = ApprovalRequest::acp(changed.clone(), &agent(), dir.path());
    assert_eq!(request.decision(&store).unwrap(), Decision::Allow);
    let mut forged = request.command.unwrap();
    if let solo::command_rules::Executor::Acp { input, .. } = &mut forged.executor {
        input["command"] = json!("touch marker");
    }
    assert_eq!(store.evaluate(&forged).unwrap(), Decision::Ask);
    changed["toolCall"]["rawInput"]["env"]["MODE"] = json!("different");
    assert_eq!(
        ApprovalRequest::acp(changed, &agent(), dir.path())
            .decision(&store)
            .unwrap(),
        Decision::Ask
    );
    let mut argv = params.clone();
    argv["toolCall"]["rawInput"]["command"] = json!(["git", "status"]);
    assert_eq!(
        ApprovalRequest::acp(argv.clone(), &agent(), dir.path())
            .decision(&store)
            .unwrap(),
        Decision::Ask
    );
    let mut pattern = CommandRule::from(
        ApprovalRequest::acp(argv.clone(), &agent(), dir.path())
            .command
            .unwrap(),
    )
    .with_matching(Matching::Wildcard);
    pattern.command = "git *".into();
    store.add(RuleList::Whitelist, pattern).unwrap();
    argv["toolCall"]["rawInput"]["command"] = json!(["git", "diff", "--stat"]);
    assert_eq!(
        ApprovalRequest::acp(argv, &agent(), dir.path())
            .decision(&store)
            .unwrap(),
        Decision::Allow
    );
}

#[test]
fn wildcard_handles_long_nonmatches_without_recursive_backtracking() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .add(
            RuleList::Blacklist,
            wildcard(dir.path(), &format!("*{}b*", "a".repeat(20_000))),
        )
        .unwrap();
    assert_eq!(
        store
            .evaluate(&command(dir.path(), &"a".repeat(60_000)))
            .unwrap(),
        Decision::Ask
    );
}

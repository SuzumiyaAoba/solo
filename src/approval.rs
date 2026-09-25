//! Normalize tool approval data without treating a display title as an executable command.
use crate::{
    acp::AgentProfile,
    command_rules::{CommandInvocation, Decision, Executor, RuleStore, command_text},
    config::{ApprovalMode, AutoSettings},
    harness::ToolCall,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalPlan {
    Allow(&'static str),
    Deny(&'static str),
    Manual,
    Auto(AutoSettings),
}

#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub title: String,
    pub details: Value,
    pub is_command: bool,
    pub display_command: Option<String>,
    pub cwd: Option<PathBuf>,
    pub executor: String,
    pub command: Option<CommandInvocation>,
    pub can_allow: bool,
}
impl ApprovalRequest {
    pub fn plan(&self, store: &RuleStore) -> std::io::Result<ApprovalPlan> {
        let (settings, rules) = store.approval_policy()?;
        if settings.mode == ApprovalMode::Bypass {
            return Ok(if self.can_allow {
                ApprovalPlan::Allow("Bypass")
            } else {
                ApprovalPlan::Manual
            });
        }
        if let Some(command) = &self.command {
            match rules.evaluate(command) {
                Decision::Allow if self.can_allow => return Ok(ApprovalPlan::Allow("Whitelist")),
                Decision::Allow => return Ok(ApprovalPlan::Manual),
                Decision::Deny => return Ok(ApprovalPlan::Deny("Blacklist")),
                Decision::Ask => {}
            }
        }
        if self.can_allow
            && settings.mode == ApprovalMode::Auto
            && (!self.is_command || self.command.is_some())
        {
            Ok(ApprovalPlan::Auto(settings.auto))
        } else {
            Ok(ApprovalPlan::Manual)
        }
    }

    pub fn decision(&self, store: &RuleStore) -> std::io::Result<Decision> {
        if !self.is_command {
            return Ok(Decision::Ask);
        }
        let rules = store.load()?;
        Ok(self
            .command
            .as_ref()
            .map(|command| rules.evaluate(command))
            .unwrap_or(Decision::Ask))
    }
    pub fn tool(call: &ToolCall, workspace: &Path) -> Self {
        let is_command = call.name == "exec";
        let display_command = is_command
            .then(|| call.arguments["command"].as_str().map(str::to_owned))
            .flatten();
        let command = display_command
            .as_ref()
            .and_then(|command| CommandInvocation::shell(command.clone(), workspace).ok());
        Self {
            title: match call.name.as_str() {
                "exec" => "コマンド実行の確認".into(),
                "edit" => "ファイル編集の確認".into(),
                _ => format!("{} の実行確認", call.name),
            },
            details: json!({"tool":call.name,"arguments":call.arguments,"cwd":workspace}),
            is_command,
            display_command,
            cwd: Some(workspace.to_path_buf()),
            executor: "ローカル · sh -c".into(),
            command,
            can_allow: true,
        }
    }
    pub fn acp(params: Value, profile: &AgentProfile, workspace: &Path) -> Self {
        let tool = &params["toolCall"];
        let is_command = tool["kind"] == "execute";
        let raw = &tool["rawInput"];
        let display_command = if is_command { command_text(raw) } else { None };
        // ACP leaves rawInput provider-defined. An explicit, existing cwd is required
        // before a complete request can become an automatic rule.
        let cwd = raw
            .get("cwd")
            .or_else(|| raw.get("workdir"))
            .or_else(|| raw.get("working_directory"))
            .and_then(Value::as_str)
            .and_then(|cwd| {
                let path = Path::new(cwd);
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    workspace.join(path)
                };
                path.canonicalize().ok().filter(|path| path.is_dir())
            });
        let command = display_command
            .as_ref()
            .zip(cwd.as_ref())
            .filter(|_| raw.is_object())
            .map(|(command, cwd)| CommandInvocation {
                command: command.clone(),
                cwd: cwd.clone(),
                executor: Executor::Acp {
                    agent_id: profile.id.clone(),
                    program: profile.command.clone(),
                    args: profile.args.clone(),
                    tool: tool["name"]
                        .as_str()
                        .or_else(|| tool["title"].as_str())
                        .unwrap_or("execute")
                        .into(),
                    input: raw.clone(),
                },
            });
        let command = command.filter(|command| command.validate().is_ok());
        let can_allow = params["options"].as_array().is_some_and(|options| {
            options.iter().any(|option| {
                option["kind"] == "allow_once"
                    && option["optionId"].as_str().is_some_and(|id| !id.is_empty())
            })
        });
        Self {
            title: if is_command {
                "コマンド実行の確認".into()
            } else {
                tool["title"].as_str().unwrap_or("ツール実行の確認").into()
            },
            details: params,
            is_command,
            display_command,
            cwd,
            executor: format!("ACP · {}", profile.name),
            command,
            can_allow,
        }
    }
}

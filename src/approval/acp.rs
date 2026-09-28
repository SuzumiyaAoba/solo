//! ACP toolCall の承認要求への正規化。rawInput は provider 任意のため、
//! ルール化は実行ディレクトリとコマンドが揃った完全な要求に限る。
use super::ApprovalRequest;
use crate::{
    acp::AgentProfile,
    command_rules::{CommandInvocation, Executor, command_text},
};
use serde_json::Value;
use std::path::Path;

impl ApprovalRequest {
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
        let executor = Executor::Acp {
            agent_id: profile.id.clone(),
            program: profile.command.clone(),
            args: profile.args.clone(),
            tool: tool["name"]
                .as_str()
                .or_else(|| tool["title"].as_str())
                .unwrap_or("execute")
                .into(),
            input: raw.clone(),
        };
        let command = display_command
            .as_ref()
            .zip(cwd.as_ref())
            .filter(|_| raw.is_object())
            .map(|(command, cwd)| CommandInvocation {
                command: command.clone(),
                cwd: cwd.clone(),
                executor: executor.clone(),
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
            executor: executor.label(),
            command,
            can_allow,
        }
    }
}

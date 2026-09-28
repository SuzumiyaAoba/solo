//! 承認要求の正規化と、worker→UI の往路チャネル。表示タイトルを実行可能な
//! コマンドとして扱わない。判定結果は ApprovalReply で往路へ戻る。
mod acp;

use crate::{
    command_rules::{CommandInvocation, Decision, Executor, RuleStore, Rules},
    config::{ApprovalMode, AutoSettings},
    harness::{
        ToolCall,
        workspace::{TOOL_EDIT, TOOL_EXEC, is_read_only_tool},
    },
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// 決定の出どころ。worker が ApprovalDecided の `source` として記録する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalSource {
    /// ユーザーが承認 UI で選択した。
    User,
    /// auto_approval のモデル判定。
    Auto,
    /// コマンド・ツールのルール一致。
    Rule,
    /// Bypass モードによる即時許可。
    Bypass,
}

impl ApprovalSource {
    /// ApprovalDecided の `source` に入れる安定した文字列。
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Auto => "auto",
            Self::Rule => "rule",
            Self::Bypass => "bypass",
        }
    }
}

/// UI から worker への返答。切断・中止はこの型ではなくチャネル切断で届く。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalReply {
    pub accepted: bool,
    pub source: ApprovalSource,
}

impl ApprovalReply {
    pub fn user(accepted: bool) -> Self {
        Self {
            accepted,
            source: ApprovalSource::User,
        }
    }
    pub fn auto(accepted: bool) -> Self {
        Self {
            accepted,
            source: ApprovalSource::Auto,
        }
    }
}

/// UI が返答を保持したままでも中止できる。切断と中止は許可として扱わない。
pub(crate) fn wait_for_reply(
    answer: async_channel::Receiver<ApprovalReply>,
    cancellation: &crate::harness::Cancellation,
) -> Option<ApprovalReply> {
    let cancel = cancellation.child_token();
    futures::executor::block_on(cancel.run_until_cancelled(answer.recv())).and_then(Result::ok)
}

/// ワーカー→UI への承認要求と返答待ちをひとまとめにする。
/// UI が閉じて送信できない場合のみ None。切断・中止は拒否として扱う。
pub(crate) fn ask<D>(
    sender: &async_channel::Sender<D>,
    request: ApprovalRequest,
    delivery: impl FnOnce(Box<ApprovalRequest>, async_channel::Sender<ApprovalReply>) -> D,
    cancellation: &crate::harness::Cancellation,
) -> Option<ApprovalReply>
where
    D: Send,
{
    let (reply, answer) = async_channel::bounded(1);
    sender
        .send_blocking(delivery(Box::new(request), reply))
        .ok()?;
    wait_for_reply(answer, cancellation)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalPlan {
    Allow(&'static str),
    Deny(&'static str),
    Manual,
    Auto(AutoSettings),
}

/// Bypass 判定の表示ラベル。返答 source の判定でもこの文字列だけを見る。
const BYPASS: &str = "Bypass";

impl ApprovalPlan {
    /// Allow/Deny を UI の確認なしに即決する場合の返答。Manual/Auto は None。
    pub fn reply(&self) -> Option<ApprovalReply> {
        match self {
            Self::Allow(label) => Some(ApprovalReply {
                accepted: true,
                source: if *label == BYPASS {
                    ApprovalSource::Bypass
                } else {
                    ApprovalSource::Rule
                },
            }),
            Self::Deny(_) => Some(ApprovalReply {
                accepted: false,
                source: ApprovalSource::Rule,
            }),
            Self::Manual | Self::Auto(_) => None,
        }
    }
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
                ApprovalPlan::Allow(BYPASS)
            } else {
                ApprovalPlan::Manual
            });
        }
        if let Some(command) = &self.command {
            match rules.evaluate(command) {
                Decision::Allow if self.can_allow => return Ok(ApprovalPlan::Allow("Allow")),
                Decision::Allow => return Ok(ApprovalPlan::Manual),
                Decision::Deny => return Ok(ApprovalPlan::Deny("Deny")),
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
        let is_command = call.name == TOOL_EXEC;
        let display_command = is_command
            .then(|| call.arguments["command"].as_str().map(str::to_owned))
            .flatten();
        let command = display_command
            .as_ref()
            .and_then(|command| CommandInvocation::shell(command.clone(), workspace).ok());
        Self {
            title: match call.name.as_str() {
                TOOL_EXEC => "コマンド実行の確認".into(),
                TOOL_EDIT => "ファイル編集の確認".into(),
                _ => format!("{} の実行確認", call.name),
            },
            details: json!({"tool":call.name,"arguments":call.arguments,"cwd":workspace}),
            is_command,
            display_command,
            cwd: Some(workspace.to_path_buf()),
            executor: Executor::Shell.label(),
            command,
            can_allow: true,
        }
    }
}

/// worker と CLI が共有する承認前の判定。ルールと読み取り専用ツールだけを見て、
/// Some((許可, 理由)) を返す。None は承認フローへ進むことを意味する。
pub fn precheck(call: &ToolCall, workspace: &Path) -> Option<(bool, &'static str)> {
    let rules = RuleStore::for_workspace(workspace)
        .and_then(|store| store.load())
        .ok();
    precheck_with(call, workspace, rules.as_ref())
}

/// `precheck` の純粋な判定部。rules は呼出し側が読み込み済みのものを渡す。
pub fn precheck_with(
    call: &ToolCall,
    workspace: &Path,
    rules: Option<&Rules>,
) -> Option<(bool, &'static str)> {
    match rules.and_then(|rules| rules.tool_decision(&call.name)) {
        // ツール単位の Allow は確認の省略のみ。コマンドの Deny に一致する要求は
        // 承認フロー(plan)へ回し、Deny を優先させる。
        Some(Decision::Allow)
            if rules.is_some_and(|rules| denied_command(call, workspace, rules)) =>
        {
            None
        }
        Some(Decision::Allow) => Some((true, "rule")),
        Some(Decision::Deny) => Some((false, "rule")),
        // Ask は読み取り専用ツールでも承認フローへ回す。
        Some(Decision::Ask) => None,
        None => is_read_only_tool(&call.name).then_some((true, "read-only")),
    }
}

/// exec 要求のコマンドが Deny ルールに一致するか。コマンドを特定できない要求は対象外。
fn denied_command(call: &ToolCall, workspace: &Path, rules: &Rules) -> bool {
    ApprovalRequest::tool(call, workspace)
        .command
        .is_some_and(|command| rules.evaluate(&command) == Decision::Deny)
}

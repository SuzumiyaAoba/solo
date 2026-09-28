//! User-owned, workspace-scoped command rules with explicit exact / wildcard matching.
mod matcher;
use crate::storage::{FileTransaction, invalid, read_optional};
pub(crate) use matcher::command_text;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

const MAX_STORE_BYTES: u64 = 1024 * 1024;
const MAX_COMMAND_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Executor {
    Shell,
    Acp {
        agent_id: String,
        program: String,
        args: Vec<String>,
        tool: String,
        input: Value,
    },
}

impl Executor {
    /// 承認画面とルール一覧で共有する実行元の表示名。
    pub fn label(&self) -> String {
        match self {
            Self::Shell => "ローカル · sh -c".into(),
            Self::Acp { agent_id, .. } => format!("ACP · {agent_id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandInvocation {
    pub command: String,
    pub cwd: PathBuf,
    pub executor: Executor,
}
impl CommandInvocation {
    pub fn shell(command: impl Into<String>, cwd: &Path) -> io::Result<Self> {
        let this = Self {
            command: command.into(),
            cwd: cwd.canonicalize()?,
            executor: Executor::Shell,
        };
        this.validate()?;
        Ok(this)
    }
    pub fn executor_label(&self) -> String {
        self.executor.label()
    }
    pub(crate) fn validate(&self) -> io::Result<()> {
        if self.command.trim().is_empty()
            || self.command.contains('\0')
            || self.command.len() > MAX_COMMAND_BYTES
        {
            return Err(invalid(
                "コマンドは空白だけにせず、NUL を含めず、64 KiB 以下にしてください",
            ));
        }
        if !self.cwd.is_absolute() {
            return Err(invalid("作業ディレクトリには絶対パスが必要です"));
        }
        if let Executor::Acp {
            agent_id,
            program,
            tool,
            input,
            ..
        } = &self.executor
            && (agent_id.is_empty() || program.is_empty() || tool.is_empty() || !input.is_object())
        {
            return Err(invalid("ACP ルールには実行元と完全な入力情報が必要です"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Matching {
    #[default]
    Exact,
    Wildcard,
}
impl Matching {
    fn is_exact(&self) -> bool {
        *self == Self::Exact
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "完全一致",
            Self::Wildcard => "ワイルドカード",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRule {
    pub command: String,
    pub cwd: PathBuf,
    pub executor: Executor,
    // Missing in v1 files: never turn an existing literal '*' into a wildcard.
    #[serde(default, skip_serializing_if = "Matching::is_exact")]
    pub matching: Matching,
}
impl From<CommandInvocation> for CommandRule {
    fn from(command: CommandInvocation) -> Self {
        Self {
            command: command.command,
            cwd: command.cwd,
            executor: command.executor,
            matching: Matching::Exact,
        }
    }
}
impl From<&CommandInvocation> for CommandRule {
    fn from(command: &CommandInvocation) -> Self {
        command.clone().into()
    }
}
impl From<&CommandRule> for CommandRule {
    fn from(rule: &CommandRule) -> Self {
        rule.clone()
    }
}
impl CommandRule {
    pub fn with_matching(mut self, matching: Matching) -> Self {
        self.matching = matching;
        self
    }
    pub fn executor_label(&self) -> String {
        self.executor.label()
    }
    fn invocation(&self) -> CommandInvocation {
        CommandInvocation {
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            executor: self.executor.clone(),
        }
    }
    fn validate(&self) -> io::Result<()> {
        self.invocation().validate()
    }
    pub fn conditions(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("rule serialization");
        value["matching"] = serde_json::to_value(self.matching).expect("matching serialization");
        if let Executor::Acp { input, .. } = &self.executor {
            value["executor"]["input"] =
                matcher::command_context(input).unwrap_or_else(|| input.clone());
        }
        value
    }
    fn matches(&self, command: &CommandInvocation, list: RuleList) -> bool {
        if self.cwd != command.cwd {
            return false;
        }
        let same_executor = match (&self.executor, &command.executor) {
            (Executor::Shell, Executor::Shell) => true,
            (
                Executor::Acp {
                    agent_id,
                    program,
                    args,
                    tool,
                    input,
                },
                Executor::Acp {
                    agent_id: other_id,
                    program: other_program,
                    args: other_args,
                    tool: other_tool,
                    input: other_input,
                },
            ) => {
                agent_id == other_id
                    && program == other_program
                    && args == other_args
                    && tool == other_tool
                    && command_text(other_input).as_deref() == Some(command.command.as_str())
                    && matcher::command_context(input).is_some_and(|context| {
                        Some(context) == matcher::command_context(other_input)
                    })
            }
            _ => false,
        };
        same_executor
            && match self.matching {
                Matching::Exact => self.command == command.command,
                Matching::Wildcard => matcher::wildcard_matches(
                    &self.command,
                    &command.command,
                    list == RuleList::Allow,
                ),
            }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Ask,
    Allow,
    Deny,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleList {
    Allow,
    Deny,
}
impl RuleList {
    pub fn label(self) -> &'static str {
        match self {
            Self::Allow => "Allow（自動許可）",
            Self::Deny => "Deny（自動拒否）",
        }
    }
}

/// 既定では version 1/2 の whitelist/blacklist キーは読まず破棄する。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rules {
    #[serde(default)]
    pub allow: Vec<CommandRule>,
    #[serde(default)]
    pub deny: Vec<CommandRule>,
    /// 基本ツール名ごとの明示判定。未定義なら既定（read/search は許可、他は確認）。
    #[serde(default)]
    pub tool_decisions: BTreeMap<String, Decision>,
}
impl Rules {
    pub(crate) fn validate(&self) -> io::Result<()> {
        for rule in self.allow.iter().chain(&self.deny) {
            rule.validate()?;
        }
        for name in self.tool_decisions.keys() {
            if name.trim().is_empty() {
                return Err(invalid("ツール名が空の判定があります"));
            }
        }
        Ok(())
    }
    /// ツールごとの明示判定。None は呼び出し側の既定（read/search 許可など）に委ねる。
    pub fn tool_decision(&self, tool: &str) -> Option<Decision> {
        self.tool_decisions.get(tool).copied()
    }
    /// ツール名ごとの明示判定を設定。None で既定へ戻す。
    pub fn set_tool(&mut self, tool: &str, decision: Option<Decision>) {
        match decision {
            Some(decision) => self.tool_decisions.insert(tool.into(), decision),
            None => self.tool_decisions.remove(tool),
        };
    }
    pub fn evaluate(&self, command: &CommandInvocation) -> Decision {
        if self
            .deny
            .iter()
            .any(|rule| rule.matches(command, RuleList::Deny))
        {
            Decision::Deny
        } else if self
            .allow
            .iter()
            .any(|rule| rule.matches(command, RuleList::Allow))
        {
            Decision::Allow
        } else {
            Decision::Ask
        }
    }
    pub fn entries(&self, list: RuleList) -> &[CommandRule] {
        match list {
            RuleList::Allow => &self.allow,
            RuleList::Deny => &self.deny,
        }
    }
    fn entries_mut(&mut self, list: RuleList) -> &mut Vec<CommandRule> {
        match list {
            RuleList::Allow => &mut self.allow,
            RuleList::Deny => &mut self.deny,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Store {
    version: u32,
    workspaces: BTreeMap<PathBuf, Rules>,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            version: 3,
            workspaces: BTreeMap::new(),
        }
    }
}

/// ルールの保存先。workspace 直下の JSON か、ユーザー共有の config.yml 内 workspaces[]。
#[derive(Clone, Debug)]
enum Backend {
    Json,
    Config(crate::config::ConfigStore),
}

/// Configuration is outside the repository; cloning a repository cannot grant execution rights.
#[derive(Clone, Debug)]
pub struct RuleStore {
    path: PathBuf,
    workspace: PathBuf,
    backend: Backend,
}
impl RuleStore {
    pub fn for_workspace(workspace: &Path) -> io::Result<Self> {
        let config = crate::config::ConfigStore::user()?;
        let mut store = Self::at_path(config.path(), workspace)?;
        store.backend = Backend::Config(config);
        Ok(store)
    }
    pub fn at_path(path: impl Into<PathBuf>, workspace: &Path) -> io::Result<Self> {
        let workspace = workspace.canonicalize()?;
        if !workspace.is_dir() {
            return Err(invalid("workspace はディレクトリである必要があります"));
        }
        let path = path.into();
        let backend = if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            Backend::Json
        } else {
            Backend::Config(crate::config::ConfigStore::at_path(&path))
        };
        Ok(Self {
            path,
            workspace,
            backend,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
    pub fn config_store(&self) -> Option<&crate::config::ConfigStore> {
        match &self.backend {
            Backend::Config(config) => Some(config),
            Backend::Json => None,
        }
    }
    pub fn approval_policy(&self) -> io::Result<(crate::config::ApprovalSettings, Rules)> {
        if let Backend::Config(store) = &self.backend {
            let mut config = store.load()?;
            return Ok((
                config.approval,
                config
                    .workspaces
                    .remove(&self.workspace)
                    .unwrap_or_default(),
            ));
        }
        Ok((crate::config::ApprovalSettings::default(), self.load()?))
    }
    pub fn load(&self) -> io::Result<Rules> {
        if let Backend::Config(config) = &self.backend {
            return Ok(config
                .load()?
                .workspaces
                .remove(&self.workspace)
                .unwrap_or_default());
        }
        Ok(self
            .read()?
            .workspaces
            .remove(&self.workspace)
            .unwrap_or_default())
    }
    pub fn evaluate(&self, command: &CommandInvocation) -> io::Result<Decision> {
        command.validate()?;
        Ok(self.load()?.evaluate(command))
    }
    pub fn add(&self, list: RuleList, command: impl Into<CommandRule>) -> io::Result<()> {
        let command = command.into();
        command.validate()?;
        self.modify(|rules| {
            let entries = rules.entries_mut(list);
            if !entries.contains(&command) {
                entries.push(command);
            }
            Ok(())
        })
    }
    pub fn remove(&self, list: RuleList, command: impl Into<CommandRule>) -> io::Result<()> {
        let command = command.into();
        self.modify(|rules| {
            rules.entries_mut(list).retain(|entry| entry != &command);
            Ok(())
        })
    }
    pub fn replace(&self, list: RuleList, old: &CommandRule, new: CommandRule) -> io::Result<()> {
        new.validate()?;
        self.modify(|rules| {
            let entries = rules.entries_mut(list);
            let Some(index) = entries.iter().position(|entry| entry == old) else {
                return Err(invalid(
                    "編集中にルールが変更されました。再読み込みしてください",
                ));
            };
            entries.remove(index);
            if !entries.contains(&new) {
                entries.insert(index.min(entries.len()), new);
            }
            Ok(())
        })
    }
    pub fn set_tool(&self, tool: &str, decision: Option<Decision>) -> io::Result<()> {
        self.modify(|rules| {
            rules.set_tool(tool, decision);
            Ok(())
        })
    }
    fn read(&self) -> io::Result<Store> {
        let Some(bytes) = read_optional(
            &self.path,
            MAX_STORE_BYTES,
            "コマンドルールのファイルが 1 MiB を超えています",
        )?
        else {
            return Ok(Store::default());
        };
        let store: Store = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("コマンドルールを読めません: {error}")))?;
        if !matches!(store.version, 1..=3) {
            return Err(invalid("未対応のコマンドルールの version です"));
        }
        for (workspace, rules) in &store.workspaces {
            if !workspace.is_absolute() {
                return Err(invalid("workspace は絶対パスで指定してください"));
            }
            for command in rules.allow.iter().chain(&rules.deny) {
                command.validate()?;
                if store.version == 1 && command.matching != Matching::Exact {
                    return Err(invalid(
                        "ワイルドカードを使う設定ファイルは version 2 にしてください",
                    ));
                }
            }
        }
        Ok(store)
    }
    fn modify(&self, edit: impl FnOnce(&mut Rules) -> io::Result<()>) -> io::Result<()> {
        if let Backend::Config(store) = &self.backend {
            return store.update(|config| {
                edit(config.workspaces.entry(self.workspace.clone()).or_default())
            });
        }
        let transaction = FileTransaction::begin(
            &self.path,
            "json.lock",
            "ルール保存先が不正です",
            "別の画面でルールを更新中です。再試行してください",
        )?;
        // Reload under the lock so other windows / CLI changes are not lost.
        let mut store = self.read()?;
        edit(store.workspaces.entry(self.workspace.clone()).or_default())?;
        store.version = 3;
        let bytes = serde_json::to_vec_pretty(&store)?;
        if bytes.len() as u64 > MAX_STORE_BYTES {
            return Err(invalid("コマンドルールの保存上限です"));
        }
        transaction.commit(&bytes)
    }
}

pub(crate) fn load_legacy_workspaces(path: &Path) -> io::Result<BTreeMap<PathBuf, Rules>> {
    RuleStore {
        path: path.into(),
        workspace: PathBuf::new(),
        backend: Backend::Json,
    }
    .read()
    .map(|store| store.workspaces)
}

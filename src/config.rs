//! Shared user configuration. Credentials and project history are separate data stores.
use crate::command_rules::Rules;
use crate::storage::{FileTransaction, read_optional};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    #[default]
    Manual,
    Bypass,
    Auto,
}
impl ApprovalMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Bypass => "Bypass",
            Self::Auto => "Auto",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutoSettings {
    pub model: String,
    pub timeout_seconds: u64,
}
impl Default for AutoSettings {
    fn default() -> Self {
        Self {
            model: String::new(),
            timeout_seconds: 30,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApprovalSettings {
    pub mode: ApprovalMode,
    pub auto: AutoSettings,
}
impl ApprovalSettings {
    pub fn validate(&self) -> io::Result<()> {
        if self.mode == ApprovalMode::Auto && self.auto.model.trim().is_empty() {
            return Err(invalid(
                "auto モードでは approval.auto.model を指定してください",
            ));
        }
        if self.auto.model != self.auto.model.trim()
            || self.auto.model.len() > 256
            || self.auto.model.chars().any(char::is_control)
        {
            return Err(invalid(
                "auto のモデル名は前後の空白・制御文字を含まない256 byte以下の名前にしてください",
            ));
        }
        if !(1..=300).contains(&self.auto.timeout_seconds) {
            return Err(invalid(
                "approval.auto.timeout_seconds は1〜300秒で指定してください",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub version: u32,
    pub approval: ApprovalSettings,
    pub workspaces: BTreeMap<PathBuf, Rules>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            approval: ApprovalSettings::default(),
            workspaces: BTreeMap::new(),
        }
    }
}
impl AppConfig {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(invalid("未対応の config.yml バージョンです"));
        }
        self.approval.validate()?;
        for (workspace, rules) in &self.workspaces {
            if !workspace.is_absolute() {
                return Err(invalid("workspaces には絶対パスを指定してください"));
            }
            rules.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    path: PathBuf,
    legacy_rules: Option<PathBuf>,
}
impl ConfigStore {
    pub fn user() -> io::Result<Self> {
        let user_home = std::env::var_os("HOME")
            .filter(|path| !path.is_empty())
            .ok_or_else(|| invalid("HOME が設定されていません"))?;
        let user_home = PathBuf::from(user_home);
        Ok(Self::at_path(user_home.join(".config/solo/config.yml"))
            .with_legacy_rules(user_home.join(".solo/command-rules.json")))
    }
    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            legacy_rules: None,
        }
    }
    pub fn with_legacy_rules(mut self, path: impl Into<PathBuf>) -> Self {
        self.legacy_rules = Some(path.into());
        self
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn load(&self) -> io::Result<AppConfig> {
        let Some(bytes) = read_optional(
            &self.path,
            MAX_CONFIG_BYTES,
            "config.yml は2 MiB以下にしてください",
        )?
        else {
            let mut config = AppConfig::default();
            if let Some(path) = &self.legacy_rules {
                config.workspaces = crate::command_rules::load_legacy_workspaces(path)?;
            }
            config.validate()?;
            return Ok(config);
        };
        let options = serde_saphyr::options! {
            budget: serde_saphyr::budget! { max_documents: 1, max_depth: 64, max_total_scalar_bytes: 8 * 1024 * 1024 },
            reject_unsupported_tags: true,
        };
        let config: AppConfig =
            serde_saphyr::from_slice_with_options(&bytes, options).map_err(|error| {
                invalid(format!(
                    "config.yml を読めません: {}",
                    error.without_snippet()
                ))
            })?;
        config.validate()?;
        Ok(config)
    }
    pub fn set_approval(&self, settings: ApprovalSettings) -> io::Result<()> {
        settings.validate()?;
        self.update(|config| {
            config.approval = settings;
            Ok(())
        })
    }
    pub fn update(&self, edit: impl FnOnce(&mut AppConfig) -> io::Result<()>) -> io::Result<()> {
        let transaction = FileTransaction::begin(
            &self.path,
            "yml.lock",
            "設定の保存先が不正です",
            "設定を別の画面で更新中です。再試行してください",
        )?;
        let mut config = self.load()?;
        edit(&mut config)?;
        config.validate()?;
        let yaml = serde_saphyr::to_string(&config).map_err(|error| invalid(error.to_string()))?;
        if yaml.len() as u64 > MAX_CONFIG_BYTES {
            return Err(invalid("config.yml の保存上限です"));
        }
        transaction.commit(yaml.as_bytes())
    }
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

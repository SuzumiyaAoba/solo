//! プロジェクトの登録情報。セッションや実行プロセスは UI 側でプロジェクトごとに保持する。
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_PROJECTS: usize = 16;
const MAX_CATALOG_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub revision: u64,
    pub next_id: u64,
    pub active: Option<u64>,
    pub projects: Vec<Project>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            next_id: 1,
            active: None,
            projects: Vec::new(),
        }
    }
}

impl Catalog {
    pub fn get(&self, id: u64) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn register(&mut self, path: &Path) -> io::Result<u64> {
        let path = path.canonicalize()?;
        if !path.is_dir() {
            return Err(invalid("プロジェクトにはフォルダを指定してください"));
        }
        if let Some(project) = self.projects.iter().find(|project| project.path == path) {
            let id = project.id;
            self.active = Some(id);
            return Ok(id);
        }
        if self.projects.len() >= MAX_PROJECTS {
            return Err(invalid("プロジェクトは最大16件です"));
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let name: String = name.chars().filter(|ch| !ch.is_control()).collect();
        let name = if name.trim().is_empty() {
            "プロジェクト".into()
        } else {
            crate::orchestration::task_title(&name)
        };
        validate_name(&name)?;
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| invalid("プロジェクト ID の上限です"))?;
        self.projects.push(Project { id, name, path });
        self.active = Some(id);
        Ok(id)
    }

    pub fn select(&mut self, id: u64) -> io::Result<()> {
        if self.get(id).is_none() {
            return Err(invalid("プロジェクトが見つかりません"));
        }
        self.active = Some(id);
        Ok(())
    }

    pub fn rename(&mut self, id: u64, name: &str) -> io::Result<()> {
        let name = name.trim();
        validate_name(name)?;
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or_else(|| invalid("プロジェクトが見つかりません"))?;
        project.name = name.to_owned();
        Ok(())
    }

    /// 登録だけを解除する。作業フォルダには書き込まない。
    pub fn remove(&mut self, id: u64) -> io::Result<()> {
        let index = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or_else(|| invalid("プロジェクトが見つかりません"))?;
        self.projects.remove(index);
        if self.active == Some(id) {
            self.active = self
                .projects
                .get(index.min(self.projects.len().saturating_sub(1)))
                .map(|project| project.id);
        }
        Ok(())
    }

    fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(invalid("未対応のプロジェクト設定バージョンです"));
        }
        if self.projects.len() > MAX_PROJECTS {
            return Err(invalid("プロジェクト数が上限を超えています"));
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        for project in &self.projects {
            validate_name(&project.name)?;
            if project.id == 0
                || project.id >= self.next_id
                || !ids.insert(project.id)
                || !paths.insert(&project.path)
                || !project.path.is_absolute()
                || project.path.components().any(|part| {
                    matches!(
                        part,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                })
            {
                return Err(invalid("プロジェクト設定の ID またはフォルダが不正です"));
            }
        }
        if self.next_id == 0 || self.active.is_some_and(|id| self.get(id).is_none()) {
            return Err(invalid("選択中のプロジェクトが不正です"));
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> io::Result<()> {
    use unicode_segmentation::UnicodeSegmentation;
    if name.trim().is_empty()
        || name.graphemes(true).count() > 64
        || name.chars().any(char::is_control)
    {
        return Err(invalid(
            "プロジェクト名は改行なしの1〜64文字で入力してください",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Clone, Debug)]
pub struct ProjectStore {
    path: PathBuf,
}

impl ProjectStore {
    pub fn user() -> io::Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .ok_or_else(|| invalid("ユーザーディレクトリを取得できません"))?;
        Ok(Self::at_path(
            PathBuf::from(home).join(".solo/projects.json"),
        ))
    }

    pub fn at_path(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> io::Result<Catalog> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Catalog::default()),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CATALOG_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(invalid("プロジェクト設定が大きすぎます"));
        }
        let catalog: Catalog =
            serde_json::from_slice(&bytes).map_err(|error| invalid(&error.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// 保存完了まで呼び出し元の状態を変えない。古いウィンドウからの上書きも拒否する。
    pub fn save(&self, catalog: &mut Catalog) -> io::Result<()> {
        catalog.validate()?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| invalid("プロジェクト設定の保存先が不正です"))?;
        fs::create_dir_all(parent)?;
        let lock_path = self.path.with_extension("json.lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        lock.try_lock().map_err(|error| {
            io::Error::other(format!("プロジェクト設定を別の処理が更新中です: {error}"))
        })?;
        let current = self.load()?;
        if current.revision != catalog.revision {
            return Err(invalid(
                "プロジェクト一覧が別のウィンドウで更新されました。再読み込みしてください",
            ));
        }
        let mut next = catalog.clone();
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("設定の更新回数が上限に達しました"))?;
        let bytes =
            serde_json::to_vec_pretty(&next).map_err(|error| invalid(&error.to_string()))?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(invalid("プロジェクト設定が大きすぎます"));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        *catalog = next;
        Ok(())
    }
}

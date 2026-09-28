//! セッションの追記型イベントストアと、クラッシュ後の復元・掃除・エクスポート。
//! レイアウト: `~/.solo/sessions/<workspace-dir>/<session-id>/{state.json,events.jsonl,history.json,logs/}`。
use crate::{
    event::{Envelope, SCHEMA_VERSION},
    harness::Message,
    projection::{Session, Speaker},
    storage::{FileTransaction, read_optional},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// events.jsonl の追記上限。超過分は欠落として復元時に記録する。
pub const MAX_EVENT_STORE_BYTES: u64 = 512 * 1024 * 1024;
/// history.json の保存・読み取り上限。超過分は先頭の古いやり取りから捨てる。
pub const MAX_HISTORY_BYTES: u64 = 8 * 1024 * 1024;
/// workspace.json の読み取り上限。
const MAX_WORKSPACE_BYTES: u64 = 1024 * 1024;
/// state.json の読み取り上限。
const MAX_META_BYTES: u64 = 256 * 1024;
/// dir 名の衝突時に試す候補数。
const MAX_CREATE_ATTEMPTS: usize = 16;
/// 直前に作成された dir は他ウィンドウの途中作成と区別がつかないので残す。
const SWEEP_GRACE: std::time::Duration = std::time::Duration::from_secs(60);

/// $HOME/.solo/<sub>
fn solo_dir(sub: &str) -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| invalid("ユーザーディレクトリを取得できません"))?;
    Ok(PathBuf::from(home).join(".solo").join(sub))
}

/// $HOME/.solo/sessions
pub fn user_sessions_root() -> io::Result<PathBuf> {
    solo_dir("sessions")
}

/// $HOME/.solo/exports
pub fn exports_root() -> io::Result<PathBuf> {
    solo_dir("exports")
}

/// canonicalize 失敗時は与えられたパスで hash。dir 名 = slug(file_name or "workspace")-{fnv64 hex}
pub fn workspace_dir_name(workspace: &Path) -> String {
    let canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let slug = canonical
        .file_name()
        .map(|name| slugify(&name.to_string_lossy()))
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| "workspace".to_owned());
    format!(
        "{slug}-{:016x}",
        fnv64(canonical.as_os_str().as_encoded_bytes())
    )
}

/// ディレクトリ名に使える文字だけ残す。長すぎる名前は UTF-8 境界で切る。
fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches(&['-', '.'][..]);
    let end = slug.len().min(40);
    slug[..slug.floor_char_boundary(end)].to_owned()
}

/// FNV-1a 64bit。乱数でなくパスの安定した識別子として使う。
fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

/// dir 名の衝突回避に使う見た目上の乱数。時刻・プロセス・試行回数を混ぜる。
fn random_suffix(seed: &str, attempt: usize) -> String {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(seed.as_bytes());
    bytes.extend_from_slice(
        &SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&std::process::id().to_le_bytes());
    bytes.extend_from_slice(&attempt.to_le_bytes());
    format!("{:08x}", fnv64(&bytes) as u32)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// セッション ID は dir traversal 防止のため [A-Za-z0-9_-] のみ。
fn validate_id(session_id: &str) -> io::Result<()> {
    if session_id.is_empty()
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid("セッション ID が不正です"));
    }
    Ok(())
}

fn meta_path(dir: &Path) -> PathBuf {
    dir.join("state.json")
}
fn events_path(dir: &Path) -> PathBuf {
    dir.join("events.jsonl")
}
fn history_path(dir: &Path) -> PathBuf {
    dir.join("history.json")
}

fn read_meta(dir: &Path) -> io::Result<SessionMeta> {
    let bytes = read_optional(
        &meta_path(dir),
        MAX_META_BYTES,
        "セッション状態が大きすぎます",
    )?
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "セッション状態がありません"))?;
    let meta: SessionMeta =
        serde_json::from_slice(&bytes).map_err(|error| invalid(&error.to_string()))?;
    if meta.version != SCHEMA_VERSION {
        return Err(invalid("セッション状態のバージョンが違います"));
    }
    Ok(meta)
}

fn write_meta(dir: &Path, meta: &SessionMeta) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(meta).map_err(|error| invalid(&error.to_string()))?;
    FileTransaction::begin(
        &meta_path(dir),
        "json.lock",
        "セッション状態の保存先が不正です",
        "セッション状態を別の処理が更新中です",
    )?
    .commit(&bytes)
}

fn read_history(dir: &Path) -> io::Result<Vec<Message>> {
    let Some(bytes) = read_optional(
        &history_path(dir),
        MAX_HISTORY_BYTES,
        "会話履歴が大きすぎます",
    )?
    else {
        return Ok(Vec::new());
    };
    serde_json::from_slice(&bytes).map_err(|error| invalid(&error.to_string()))
}

/// JSONL 本文を Envelope へ分解する。壊れた行は数え、末尾の不完全な行だけは
/// 追記途中の自然な状態(truncated_tail)として区別する。
fn parse_events(bytes: &[u8]) -> (Vec<Envelope>, usize, bool) {
    // cap 境界でマルチバイト文字の途中まで読むことがあるので lossy に寄せる。
    let text = String::from_utf8_lossy(bytes);
    let mut envelopes = Vec::new();
    let mut corrupted = 0usize;
    let mut truncated_tail = false;
    let last_line = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, _)| index)
        .last();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Envelope>(line) {
            Ok(envelope) => envelopes.push(envelope),
            Err(_) if Some(index) == last_line => truncated_tail = true,
            Err(_) => corrupted += 1,
        }
    }
    (envelopes, corrupted, truncated_tail)
}

/// 全文ログは復元の対象外なので、参照用にパスだけ集める。
fn collect_log_paths(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir.join("logs")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    paths.sort();
    paths
}

#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    root: PathBuf,
}

impl WorkspaceStore {
    pub fn for_workspace(workspace: &Path) -> io::Result<Self> {
        Ok(Self::at_path(
            user_sessions_root()?.join(workspace_dir_name(workspace)),
        ))
    }
    pub fn at_path(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// id は [A-Za-z0-9_-] のみ許可(dir traversal 防止)。不正なら InvalidData。
    pub fn session_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        validate_id(session_id)?;
        Ok(self.root.join(session_id))
    }

    /// workspace.json。不在は Default。破損は Err。
    pub fn load_workspace_state(&self) -> io::Result<WorkspaceState> {
        let Some(bytes) = read_optional(
            &self.root.join("workspace.json"),
            MAX_WORKSPACE_BYTES,
            "ワークスペース状態が大きすぎます",
        )?
        else {
            return Ok(WorkspaceState::default());
        };
        let state: WorkspaceState =
            serde_json::from_slice(&bytes).map_err(|error| invalid(&error.to_string()))?;
        if state.version != SCHEMA_VERSION {
            return Err(invalid("ワークスペース状態のバージョンが違います"));
        }
        Ok(state)
    }

    /// FileTransaction でアトミック保存(version/revision なし、単純上書きでよい)。
    pub fn save_workspace_state(&self, state: &WorkspaceState) -> io::Result<()> {
        let bytes =
            serde_json::to_vec_pretty(state).map_err(|error| invalid(&error.to_string()))?;
        FileTransaction::begin(
            &self.root.join("workspace.json"),
            "json.lock",
            "ワークスペース状態の保存先が不正です",
            "ワークスペース状態を別の処理が更新中です",
        )?
        .commit(&bytes)
    }

    /// state.json が無い/パース不能/closed=true のセッション dir を削除(その中の logs ごと)。
    /// closed=true は明示的に閉じられたので即削除し、それ以外は mtime が
    /// SWEEP_GRACE 以内の dir を他ウィンドウの途中作成として残す。削除数を返す。
    pub fn sweep_orphans(&self) -> io::Result<usize> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut removed = 0;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let dir = entry.path();
            let orphan = match read_meta(&dir) {
                Ok(meta) => meta.closed,
                Err(_) => {
                    // エントリ自体の stat で時刻を見る。失敗したら消さずに残す。
                    entry
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .is_ok_and(|modified| {
                            SystemTime::now()
                                .duration_since(modified)
                                .is_ok_and(|age| age > SWEEP_GRACE)
                        })
                }
            };
            if orphan && fs::remove_dir_all(&dir).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// 直下の dir 名の一覧(id 検証済みのものだけ)。順序は問わない。
    pub fn session_ids(&self) -> io::Result<Vec<String>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            if let Some(id) = entry.file_name().to_str()
                && validate_id(id).is_ok()
            {
                ids.push(id.to_owned());
            }
        }
        Ok(ids)
    }

    /// preferred_id の dir が既にあれば "{preferred}-w{8hex乱数}" を試す(最大16回)。
    /// dir 作成済みの SessionFile を返す。meta は書かない(呼出側)。
    pub fn create(&self, preferred_id: &str) -> io::Result<SessionFile> {
        validate_id(preferred_id)?;
        fs::create_dir_all(&self.root)?;
        let candidates = std::iter::once(preferred_id.to_owned())
            .chain((0..MAX_CREATE_ATTEMPTS).map(|attempt| {
                format!("{preferred_id}-w{}", random_suffix(preferred_id, attempt))
            }));
        for id in candidates {
            let dir = self.root.join(&id);
            match fs::create_dir(&dir) {
                Ok(()) => return SessionFile::open_dir(id, dir),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(invalid("セッション ID を一意に決められませんでした"))
    }

    /// 既存 dir を append 追記用に開く。events.jsonl が無ければ新規。bytes は既存サイズから。
    pub fn open(&self, session_id: &str) -> io::Result<SessionFile> {
        let dir = self.session_dir(session_id)?;
        if !dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "セッションが見つかりません",
            ));
        }
        SessionFile::open_dir(session_id.to_owned(), dir)
    }

    /// events.jsonl を各行 Envelope として読み、Session::apply に再適用する。
    /// 途中の壊れた行は数えて flag_incomplete、末尾の不完全な行は truncated_tail。
    /// どちらも再生は続行する。
    pub fn load(&self, session_id: &str) -> io::Result<RestoredSession> {
        let dir = self.session_dir(session_id)?;
        let meta = read_meta(&dir).ok();

        // 上限を超えた追記は読み切らず、切り詰めた分を不完全として記録する。
        let file = File::open(events_path(&dir))?;
        let mut bytes = Vec::new();
        io::Read::take(&file, MAX_EVENT_STORE_BYTES.saturating_add(1)).read_to_end(&mut bytes)?;
        let store_capped = bytes.len() as u64 > MAX_EVENT_STORE_BYTES;
        let (envelopes, corrupted_lines, mut truncated_tail) = parse_events(&bytes);
        truncated_tail |= store_capped;

        // セッション ID は meta 優先。壊れたストアではイベント側の ID を採用し、
        // それも無ければ dir 名にする(apply は ID 不一致を拒否するため)。
        let id = meta
            .as_ref()
            .map(|meta| meta.session_id.clone())
            .filter(|id| !id.is_empty())
            .or_else(|| {
                envelopes
                    .first()
                    .map(|envelope| envelope.session_id.clone())
                    .filter(|id| !id.is_empty())
            })
            .unwrap_or_else(|| session_id.to_owned());
        let title = meta
            .as_ref()
            .map(|meta| meta.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| id.clone());
        let mut session = Session::new(id, title);
        for envelope in envelopes {
            session.apply(envelope);
        }
        // turn_open は「実行中のはず」に限るので、活性 status の検査で十分。
        let interrupted = session.status.is_active();
        if interrupted {
            session.mark_recovered("アプリの終了で実行状況を確認できませんでした");
        }
        if corrupted_lines > 0 {
            session.flag_incomplete(format!(
                "保存イベントのうち {corrupted_lines} 行が破損していました"
            ));
        }
        let store_capped = store_capped || meta.as_ref().is_some_and(|meta| meta.store_capped);
        if store_capped {
            session.flag_incomplete("イベント保存が上限に達し、一部は復元対象外です");
        }
        let history = match read_history(&dir) {
            Ok(history) => history,
            Err(error) => {
                session.flag_incomplete(format!("会話履歴を復元できませんでした: {error}"));
                Vec::new()
            }
        };
        Ok(RestoredSession {
            session,
            meta,
            history,
            log_paths: collect_log_paths(&dir),
            interrupted,
            corrupted_lines,
            truncated_tail,
        })
    }

    /// dir ごと削除。存在しなくても Ok。
    pub fn remove(&self, session_id: &str) -> io::Result<()> {
        let dir = self.session_dir(session_id)?;
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// exports_root/<session_id>-<unix_ms>/ に events.jsonl・state.json(あれば)・transcript.md を書く。
    pub fn export(&self, session: &Session, exports_root: &Path) -> io::Result<PathBuf> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let target = exports_root.join(format!("{}-{stamp}", session.id));
        fs::create_dir_all(&target)?;
        if let Ok(dir) = self.session_dir(&session.id) {
            let events = events_path(&dir);
            if events.is_file() {
                fs::copy(&events, target.join("events.jsonl"))?;
            }
            let meta = meta_path(&dir);
            if meta.is_file() {
                fs::copy(&meta, target.join("state.json"))?;
            }
        }
        fs::write(target.join("transcript.md"), transcript_markdown(session))?;
        Ok(target)
    }
}

/// events.jsonl への追記を保持するハンドル。Drop で flush(best-effort)。
#[derive(Debug)]
pub struct SessionFile {
    session_id: String,
    dir: PathBuf,
    writer: BufWriter<File>,
    bytes: u64,
    capped: bool,
}

impl SessionFile {
    fn open_dir(session_id: String, dir: PathBuf) -> io::Result<Self> {
        let file = File::options()
            .append(true)
            .create(true)
            .open(events_path(&dir))?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            session_id,
            dir,
            writer: BufWriter::new(file),
            bytes,
            capped: bytes >= MAX_EVENT_STORE_BYTES,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    /// dir/logs。作成は呼出側(必要時に create_dir_all)。
    pub fn log_dir(&self) -> PathBuf {
        self.dir.join("logs")
    }
    pub fn capped(&self) -> bool {
        self.capped
    }

    /// Envelope を1行 JSON で追記。capped なら書かず Ok。
    /// cap を超えるイベントは欠落扱いにして capped=true(書き込み済みの行は残る)。
    /// fsync はしない(flush() の責務)。
    pub fn append(&mut self, envelope: &Envelope) -> io::Result<()> {
        if self.capped {
            return Ok(());
        }
        let line = serde_json::to_vec(envelope).map_err(|error| invalid(&error.to_string()))?;
        let size = line.len() as u64 + 1;
        if self.bytes.saturating_add(size) > MAX_EVENT_STORE_BYTES {
            self.capped = true;
            return Ok(());
        }
        self.writer.write_all(&line)?;
        self.writer.write_all(b"\n")?;
        self.bytes += size;
        Ok(())
    }

    /// BufWriter flush。sync=true なら file.sync_data()。
    pub fn flush(&mut self, sync: bool) -> io::Result<()> {
        self.writer.flush()?;
        if sync {
            self.writer.get_ref().sync_data()?;
        }
        Ok(())
    }

    /// state.json を FileTransaction で保存。updated_ms はここで採番する。
    pub fn save_meta(&self, meta: &SessionMeta) -> io::Result<()> {
        let mut meta = meta.clone();
        meta.touch();
        write_meta(&self.dir, &meta)
    }

    /// 先頭から捨てて直列化サイズを MAX_HISTORY_BYTES 以内に収めてから保存。
    /// (tool call/result の対応は末尾側を優先するため先頭捨て)
    pub fn save_history(&self, history: &[Message]) -> io::Result<()> {
        // 各メッセージの直列化サイズは変わらないので、上限に収まる最大の末尾側を選ぶ。
        let item_bytes = history
            .iter()
            .map(serde_json::to_vec)
            .collect::<Result<Vec<Vec<u8>>, _>>()
            .map_err(|error| invalid(&error.to_string()))?;
        let mut total: u64 = 2; // "[" 相当
        let mut start = item_bytes.len();
        for (index, bytes) in item_bytes.iter().enumerate().rev() {
            // 要素ごとのバイト数と区切りコンマ(先頭要素では不要)。
            let item = bytes.len() as u64 + u64::from(start < item_bytes.len());
            if total.saturating_add(item) > MAX_HISTORY_BYTES {
                break;
            }
            total += item;
            start = index;
        }
        // 収まる分だけ末尾から採用する(1件も収まらなければ空)。
        let kept = &history[start..];
        let bytes = serde_json::to_vec(kept).map_err(|error| invalid(&error.to_string()))?;
        FileTransaction::begin(
            &history_path(&self.dir),
            "json.lock",
            "会話履歴の保存先が不正です",
            "会話履歴を別の処理が更新中です",
        )?
        .commit(&bytes)
    }
}

impl Drop for SessionFile {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}

#[derive(Debug)]
pub struct RestoredSession {
    pub session: Session,
    pub meta: Option<SessionMeta>,
    pub history: Vec<Message>,
    /// `<dir>/logs/*.log`。復元の対象外だが全文参照用に列挙する(ファイル名ソート)。
    pub log_paths: Vec<PathBuf>,
    pub interrupted: bool,
    pub corrupted_lines: usize,
    pub truncated_tail: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionMeta {
    pub version: u32,
    pub session_id: String,
    pub title: String,
    pub serial: u64,
    pub backend: Option<BackendKind>,
    pub provider: String,
    pub draft: String,
    pub unread_result: bool,
    /// reviewed==true の diff.path。
    pub reviewed: Vec<String>,
    pub store_capped: bool,
    pub closed: bool,
    pub updated_ms: u64,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            session_id: String::new(),
            title: String::new(),
            serial: 0,
            backend: None,
            provider: String::new(),
            draft: String::new(),
            unread_result: false,
            reviewed: Vec::new(),
            store_capped: false,
            closed: false,
            updated_ms: 0,
        }
    }
}

impl SessionMeta {
    pub fn touch(&mut self) {
        self.updated_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendKind {
    Subscription,
    Acp { id: String },
    Mock { scenario: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceState {
    pub version: u32,
    /// 選択中の session_id。
    pub selected: Option<String>,
    pub queue: StoredQueue,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            selected: None,
            queue: StoredQueue::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StoredQueue {
    pub paused: bool,
    pub entries: Vec<QueueEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueEntry {
    pub session_id: String,
    pub backend: BackendKind,
    pub prompt: String,
}

/// Speaker を "## ユーザー / ## アシスタント / ## 通知" の見出しに、ブロックを順に連結。
/// 同一 message_id+speaker の連続ブロックは結合して1段落に( block 分割は内部都合)。
/// 末尾にタイトル・status・usage のフッタを付ける(None は「不明」)。
pub(crate) fn transcript_markdown(session: &Session) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut speaker: Option<&Speaker> = None;
    let mut paragraph = String::new();
    let mut paragraph_key: Option<(&str, &Speaker)> = None;
    let flush_paragraph = |out: &mut String, paragraph: &mut String| {
        if !paragraph.is_empty() {
            let _ = writeln!(out, "{paragraph}");
            paragraph.clear();
        }
    };
    for block in &session.chat {
        let key = (block.message_id.as_str(), &block.speaker);
        if paragraph_key != Some(key) {
            flush_paragraph(&mut out, &mut paragraph);
            if speaker != Some(&block.speaker) {
                let heading = match block.speaker {
                    Speaker::User => "## ユーザー",
                    Speaker::Assistant => "## アシスタント",
                    Speaker::Notice => "## 通知",
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                let _ = writeln!(out, "{heading}\n");
                speaker = Some(&block.speaker);
            }
            paragraph_key = Some(key);
        }
        paragraph.push_str(&block.text);
    }
    flush_paragraph(&mut out, &mut paragraph);
    let format_option =
        |value: Option<u64>| value.map_or_else(|| "不明".to_owned(), |value| value.to_string());
    let _ = write!(
        out,
        "\n---\n- title: {}\n- status: {}\n- input_tokens: {}\n- output_tokens: {}\n- cost_usd: {}\n",
        session.title,
        session.status.label(),
        format_option(session.usage.input_tokens),
        format_option(session.usage.output_tokens),
        session
            .usage
            .cost_usd
            .map_or_else(|| "不明".to_owned(), |cost| format!("{cost:.4}")),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Envelope, SCHEMA_VERSION};
    use crate::projection::{Speaker, Status};
    use serde_json::json;

    fn envelope(
        session_id: &str,
        sequence: u64,
        turn_id: Option<&str>,
        payload: serde_json::Value,
    ) -> Envelope {
        Envelope {
            schema_version: SCHEMA_VERSION,
            event_id: format!("{session_id}-{sequence}"),
            session_id: session_id.to_owned(),
            sequence,
            timestamp_ms: 1_700_000_000_000 + sequence,
            turn_id: turn_id.map(str::to_owned),
            payload,
        }
    }

    fn store() -> (tempfile::TempDir, WorkspaceStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = WorkspaceStore::at_path(dir.path().join("workspace"));
        (dir, store)
    }

    #[test]
    fn replay_restores_chat_and_status() {
        let (_tmp, store) = store();
        {
            let mut file = store.create("s1").expect("create");
            file.append(&envelope(
                "s1",
                1,
                None,
                json!({"type":"session_created","title":"テスト","workspace_id":"w","settings":{}}),
            ))
            .unwrap();
            file.append(&envelope(
                "s1",
                2,
                Some("t1"),
                json!({"type":"turn_started","prompt":"やって"}),
            ))
            .unwrap();
            file.append(&envelope(
                "s1",
                3,
                Some("t1"),
                json!({"type":"message_delta","message_id":"m1","text":"了解"}),
            ))
            .unwrap();
            file.append(&envelope(
                "s1",
                4,
                Some("t1"),
                json!({"type":"diff_updated","path":"a.rs","unified_diff":"@@ -1 +1 @@\n-a\n+b\n"}),
            ))
            .unwrap();
            file.append(&envelope(
                "s1",
                5,
                Some("t1"),
                json!({"type":"turn_completed","reason":"ok","usage":{"input_tokens":3}}),
            ))
            .unwrap();
            file.flush(true).unwrap();
        }
        let restored = store.load("s1").expect("load");
        assert_eq!(restored.session.status, Status::Completed);
        assert!(!restored.interrupted);
        assert_eq!(
            restored
                .session
                .chat
                .iter()
                .filter(|block| block.speaker == Speaker::Assistant)
                .map(|block| block.text.as_str())
                .collect::<String>(),
            "了解"
        );
        assert_eq!(restored.session.diffs.len(), 1);
        assert_eq!(restored.session.title, "テスト");
    }

    #[test]
    fn open_turn_is_marked_recovered() {
        let (_tmp, store) = store();
        {
            let mut file = store.create("s2").expect("create");
            file.append(&envelope(
                "s2",
                1,
                None,
                json!({"type":"session_created","title":"t","workspace_id":"w","settings":{}}),
            ))
            .unwrap();
            file.append(&envelope(
                "s2",
                2,
                Some("t1"),
                json!({"type":"turn_started","prompt":"go"}),
            ))
            .unwrap();
            file.flush(true).unwrap();
        }
        let restored = store.load("s2").expect("load");
        assert_eq!(restored.session.status, Status::Disconnected);
        assert!(restored.interrupted);
        assert!(restored.session.incomplete);
    }

    #[test]
    fn truncated_tail_is_tolerated() {
        let (_tmp, store) = store();
        let dir = store.session_dir("s3").expect("dir");
        fs::create_dir_all(&dir).unwrap();
        let good1 = serde_json::to_string(&envelope(
            "s3",
            1,
            None,
            json!({"type":"session_created","title":"t","workspace_id":"w","settings":{}}),
        ))
        .unwrap();
        let good2 = serde_json::to_string(&envelope(
            "s3",
            2,
            Some("t1"),
            json!({"type":"turn_started","prompt":"go"}),
        ))
        .unwrap();
        fs::write(
            events_path(&dir),
            format!("{good1}\n{good2}\n{{\"schema_version\":1,\"eve"),
        )
        .unwrap();
        let restored = store.load("s3").expect("load");
        assert!(restored.truncated_tail);
        assert_eq!(restored.corrupted_lines, 0);
        assert_eq!(restored.session.status, Status::Disconnected);
        assert_eq!(restored.session.turn_id.as_deref(), Some("t1"));
    }

    #[test]
    fn meta_roundtrip_and_sweep() {
        let (_tmp, store) = store();
        let file = store.create("s4").expect("create");
        let meta = SessionMeta {
            session_id: "s4".into(),
            title: "締めた".into(),
            closed: true,
            ..SessionMeta::default()
        };
        file.save_meta(&meta).expect("save_meta");
        let loaded = read_meta(file.dir()).expect("load_meta");
        assert_eq!(loaded.title, "締めた");
        assert!(loaded.closed);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert!(loaded.updated_ms > 0);
        assert!(store.session_ids().unwrap().contains(&"s4".to_owned()));
        // closed=true は即削除。
        assert_eq!(store.sweep_orphans().unwrap(), 1);
        assert!(store.session_ids().unwrap().is_empty());

        // meta 無しの dir は mtime が新しい限り掃除しない(他ウィンドウの途中作成かもしれない)。
        let dir = store.session_dir("s5").unwrap();
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(store.sweep_orphans().unwrap(), 0);
        assert!(dir.is_dir());
    }

    #[test]
    fn history_is_capped_from_the_front() {
        let (_tmp, store) = store();
        let file = store.create("s6").expect("create");
        let history: Vec<Message> = (0..16)
            .map(|_| Message::User {
                text: "x".repeat(600_000),
            })
            .collect();
        file.save_history(&history).expect("save_history");
        let loaded = read_history(file.dir()).expect("load_history");
        assert!(!loaded.is_empty());
        assert!(loaded.len() < history.len());
        let saved = fs::metadata(history_path(file.dir())).unwrap().len();
        assert!(saved <= MAX_HISTORY_BYTES);
    }

    #[test]
    fn workspace_state_roundtrip() {
        let (_tmp, store) = store();
        assert!(store.load_workspace_state().unwrap().selected.is_none());
        let state = WorkspaceState {
            selected: Some("s7".into()),
            queue: StoredQueue {
                paused: true,
                entries: vec![
                    QueueEntry {
                        session_id: "s7".into(),
                        backend: BackendKind::Subscription,
                        prompt: "first".into(),
                    },
                    QueueEntry {
                        session_id: "s8".into(),
                        backend: BackendKind::Acp { id: "x".into() },
                        prompt: "second".into(),
                    },
                    QueueEntry {
                        session_id: "s9".into(),
                        backend: BackendKind::Mock { scenario: 2 },
                        prompt: "third".into(),
                    },
                ],
            },
            ..WorkspaceState::default()
        };
        store.save_workspace_state(&state).expect("save");
        let loaded = store.load_workspace_state().expect("load");
        assert_eq!(loaded.selected.as_deref(), Some("s7"));
        assert_eq!(loaded.queue.entries.len(), 3);
        assert!(loaded.queue.paused);
        assert_eq!(
            loaded.queue.entries[1].backend,
            BackendKind::Acp { id: "x".into() }
        );
        assert_eq!(loaded.version, SCHEMA_VERSION);
    }

    #[test]
    fn invalid_session_id_is_rejected() {
        let (_tmp, store) = store();
        for result in [
            store.session_dir("../evil").map(|dir| dir.to_path_buf()),
            store.create("../evil").map(|file| file.dir().to_path_buf()),
            store.open("../evil").map(|file| file.dir().to_path_buf()),
            store.load("../evil").map(|_| PathBuf::new()),
            store.remove("../evil").map(|()| PathBuf::new()),
        ] {
            let error = result.expect_err("invalid id must fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn export_writes_events_state_and_transcript() {
        let (_tmp, store) = store();
        let session = {
            let mut file = store.create("s10").expect("create");
            file.append(&envelope(
                "s10",
                1,
                None,
                json!({"type":"session_created","title":"出口","workspace_id":"w","settings":{}}),
            ))
            .unwrap();
            file.append(&envelope(
                "s10",
                2,
                Some("t1"),
                json!({"type":"turn_started","prompt":"go"}),
            ))
            .unwrap();
            file.append(&envelope(
                "s10",
                3,
                Some("t1"),
                json!({"type":"turn_completed","reason":"done","usage":{}}),
            ))
            .unwrap();
            file.flush(true).unwrap();
            file.save_meta(&SessionMeta {
                session_id: "s10".into(),
                title: "出口".into(),
                ..SessionMeta::default()
            })
            .unwrap();
            store.load("s10").expect("load").session
        };
        let exports = tempfile::tempdir().unwrap();
        let dir = store.export(&session, exports.path()).expect("export");
        assert!(dir.join("events.jsonl").is_file());
        assert!(dir.join("state.json").is_file());
        let transcript = fs::read_to_string(dir.join("transcript.md")).unwrap();
        assert!(transcript.contains("## ユーザー"));
        assert!(transcript.contains("- title: 出口"));
        assert!(transcript.contains("- status: 完了"));
        // create→load→export と別の dir でも動く。
        assert_eq!(store.load("s10").unwrap().session.status, Status::Completed);
        store.remove("s10").unwrap();
        assert!(store.session_ids().unwrap().is_empty());
        store.remove("s10").unwrap();
    }
}

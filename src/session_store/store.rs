use super::{
    model::{RestoredSession, SessionMeta, WorkspaceState},
    transcript_markdown,
};
use crate::{
    event::{Envelope, SCHEMA_VERSION, SessionId},
    harness::Message,
    projection::SessionProjection,
    storage::{FileTransaction, invalid, read_optional, solo_dir},
};
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
/// state.json の読み取り・保存上限。超過した状態は復元せず、掃除でも削除しない。
const MAX_META_BYTES: u64 = 4 * 1024 * 1024;
/// meta.draft の保存上限。切り詰めてでも状態は読めるようにする。
pub const MAX_DRAFT_BYTES: usize = 1024 * 1024;
/// dir 名の衝突時に試す候補数。
const MAX_CREATE_ATTEMPTS: usize = 16;
/// 直前に作成された dir は他ウィンドウの途中作成と区別がつかないので残す。
const SWEEP_GRACE: std::time::Duration = std::time::Duration::from_secs(60);

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

fn meta_path(dir: &Path) -> PathBuf {
    dir.join("state.json")
}
pub(super) fn events_path(dir: &Path) -> PathBuf {
    dir.join("events.jsonl")
}
pub(super) fn history_path(dir: &Path) -> PathBuf {
    dir.join("history.json")
}

pub(super) fn read_meta(dir: &Path) -> io::Result<SessionMeta> {
    let bytes = read_optional(
        &meta_path(dir),
        MAX_META_BYTES,
        "セッション状態が大きすぎます",
    )?
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "セッション状態がありません"))?;
    let meta: SessionMeta =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    if meta.version != SCHEMA_VERSION {
        return Err(invalid("セッション状態のバージョンが違います"));
    }
    Ok(meta)
}

fn write_meta(dir: &Path, meta: &SessionMeta) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(meta).map_err(|error| invalid(error.to_string()))?;
    // 読み取り上限を超える state.json は書かない(書けば読めない保存になる)。
    if bytes.len() as u64 > MAX_META_BYTES {
        return Err(invalid("セッション状態が保存上限を超えています"));
    }
    FileTransaction::begin(
        &meta_path(dir),
        "json.lock",
        "セッション状態の保存先が不正です",
        "セッション状態を別の処理が更新中です",
    )?
    .commit(&bytes)
}

pub(super) fn read_history(dir: &Path) -> io::Result<Vec<Message>> {
    let Some(bytes) = read_optional(
        &history_path(dir),
        MAX_HISTORY_BYTES,
        "会話履歴が大きすぎます",
    )?
    else {
        return Ok(Vec::new());
    };
    serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))
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

/// meta(state.json) を読む。無い dir は None、壊れた meta は Err のまま返し、
/// 次の保存で読めないファイルを上書きしない。
fn meta_or_none(dir: &Path) -> io::Result<Option<SessionMeta>> {
    match read_meta(dir) {
        Ok(meta) => Ok(Some(meta)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// events.jsonl の読み取り結果。
struct ParsedEvents {
    envelopes: Vec<Envelope>,
    /// 途中の壊れた行数(末尾の不完全な行とは別に数える)。
    corrupted_lines: usize,
    /// 末尾行が途中で切れていた。
    truncated_tail: bool,
    /// 保存上限を超えて読み切らなかった分がある。
    store_capped: bool,
}

/// events.jsonl を上限+1バイトまで読み、Envelope 行へ分解する。
/// 上限超過は store_capped として返す(cap 境界でマルチバイト文字を割ることがある)。
fn read_events(dir: &Path) -> io::Result<ParsedEvents> {
    let file = File::open(events_path(dir))?;
    let mut bytes = Vec::new();
    io::Read::take(&file, MAX_EVENT_STORE_BYTES.saturating_add(1)).read_to_end(&mut bytes)?;
    let (envelopes, corrupted_lines, truncated_tail) = parse_events(&bytes);
    Ok(ParsedEvents {
        envelopes,
        corrupted_lines,
        truncated_tail,
        store_capped: bytes.len() as u64 > MAX_EVENT_STORE_BYTES,
    })
}

/// 復元するセッションの id/title。meta 優先、壊れたストアではイベント先頭の ID、
/// それも無ければ dir 名(apply は ID 不一致を拒否するため)。title が無ければ id。
fn session_identity(
    meta: Option<&SessionMeta>,
    envelopes: &[Envelope],
    fallback: &SessionId,
) -> (SessionId, String) {
    let id = meta
        .map(|meta| meta.session_id.clone())
        .filter(|id| !id.is_empty())
        .or_else(|| {
            envelopes
                .first()
                .map(|envelope| envelope.session_id.clone())
                .filter(|id| !id.is_empty())
        })
        .unwrap_or_else(|| fallback.clone());
    let title = meta
        .map(|meta| meta.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| id.to_string());
    (id, title)
}

/// sweep の対象判定: closed meta は即削除。meta が無い dir は mtime が grace を
/// 過ぎ、かつ events.jsonl が無い/空のものだけ消す。読めない meta は残す
/// (復元側が Err として報告する)。
fn is_orphan(entry: &fs::DirEntry, grace: std::time::Duration) -> bool {
    let dir = entry.path();
    match read_meta(&dir) {
        Ok(meta) => meta.closed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let stale = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .is_ok_and(|modified| {
                    SystemTime::now()
                        .duration_since(modified)
                        .is_ok_and(|age| age >= grace)
                });
            // events.jsonl は「無い/空」のときだけ空とみなす。他のエラーは残す。
            stale
                && match fs::metadata(events_path(&dir)) {
                    Ok(meta) => meta.len() == 0,
                    Err(error) => error.kind() == io::ErrorKind::NotFound,
                }
        }
        Err(_) => false,
    }
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

    /// session_id の形式は型(SessionId)が保証する(dir traversal 防止)。
    pub fn session_dir(&self, session_id: &SessionId) -> io::Result<PathBuf> {
        Ok(self.root.join(&**session_id))
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
            serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
        if state.version != SCHEMA_VERSION {
            return Err(invalid("ワークスペース状態のバージョンが違います"));
        }
        Ok(state)
    }

    /// FileTransaction でアトミック保存(version/revision なし、単純上書きでよい)。
    pub fn save_workspace_state(&self, state: &WorkspaceState) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| invalid(error.to_string()))?;
        FileTransaction::begin(
            &self.root.join("workspace.json"),
            "json.lock",
            "ワークスペース状態の保存先が不正です",
            "ワークスペース状態を別の処理が更新中です",
        )?
        .commit(&bytes)
    }

    /// closed=true のセッションと、イベントを持たない残骸の dir を削除(その中の logs ごと)。
    /// state.json が読めない dir(別 version・破損・読み取り上限超過)は中身を失わないよう残し、
    /// state.json が無い dir も events.jsonl に内容があれば残す。他ウィンドウが
    /// events.jsonl を開いている(shared lock)dir も使用中として残す。削除数を返す。
    pub fn sweep_orphans(&self) -> io::Result<usize> {
        self.sweep(SWEEP_GRACE)
    }

    /// grace より古い残骸だけを消す sweep の本体。テストは Duration::ZERO で呼ぶ。
    pub(super) fn sweep(&self, grace: std::time::Duration) -> io::Result<usize> {
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
            // セッション ID として解釈できない名前はこのストアの管理外なので残す。
            if entry
                .file_name()
                .to_str()
                .and_then(|name| SessionId::parse(name).ok())
                .is_none()
            {
                continue;
            }
            let dir = entry.path();
            if !is_orphan(&entry, grace) {
                continue;
            }
            // 別ウィンドウが開いているセッション(events.jsonl への shared lock)は消さない。
            // 開けない・ロックの失敗が競合以外なら使用中ではないとして扱う。
            // guard は削除を試みるまで保持し、判定と削除の間の取りこぼしを防ぐ。
            let guard = File::open(events_path(&dir)).ok();
            let in_use = guard
                .as_ref()
                .and_then(|file| file.try_lock().err())
                .is_some_and(|error| matches!(error, std::fs::TryLockError::WouldBlock));
            if !in_use && fs::remove_dir_all(&dir).is_ok() {
                removed += 1;
            }
            drop(guard);
        }
        Ok(removed)
    }

    /// 直下の dir 名の一覧(id 検証済みのものだけ)。順序は問わない。
    pub fn session_ids(&self) -> io::Result<Vec<SessionId>> {
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
            if let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|name| SessionId::parse(name).ok())
            {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    /// preferred_id の dir が既にあれば "{preferred}-w{8hex乱数}" を試す(最大16回)。
    /// dir 作成済みの SessionFile を返す。meta は書かない(呼出側)。
    pub fn create(&self, preferred_id: &SessionId) -> io::Result<SessionFile> {
        fs::create_dir_all(&self.root)?;
        let candidates = std::iter::once(preferred_id.clone()).chain(
            (0..MAX_CREATE_ATTEMPTS).filter_map(|attempt| {
                SessionId::parse(format!(
                    "{preferred_id}-w{}",
                    random_suffix(preferred_id, attempt)
                ))
                .ok()
            }),
        );
        for id in candidates {
            let dir = self.root.join(&*id);
            match fs::create_dir(&dir) {
                Ok(()) => return SessionFile::open_dir(id, dir),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(invalid("セッション ID を一意に決められませんでした"))
    }

    /// 既存 dir を append 追記用に開く。events.jsonl が無ければ新規。bytes は既存サイズから。
    pub fn open(&self, session_id: &SessionId) -> io::Result<SessionFile> {
        let dir = self.session_dir(session_id)?;
        if !dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "セッションが見つかりません",
            ));
        }
        SessionFile::open_dir(session_id.clone(), dir)
    }

    /// events.jsonl を各行 Envelope として読み、SessionProjection::apply に再適用する。
    /// 途中の壊れた行は数えて flag_incomplete、末尾の不完全な行は truncated_tail。
    /// どちらも再生は続行する。
    pub fn load(&self, session_id: &SessionId) -> io::Result<RestoredSession> {
        let dir = self.session_dir(session_id)?;
        let meta = meta_or_none(&dir)?;

        // 切り詰めた分は不完全として記録する(行単位の不完全と上限超過を区別しない)。
        let events = read_events(&dir)?;
        let corrupted_lines = events.corrupted_lines;
        let truncated_tail = events.truncated_tail || events.store_capped;
        let (id, title) = session_identity(meta.as_ref(), &events.envelopes, session_id);
        let mut session = SessionProjection::new(id, title);
        for envelope in events.envelopes {
            session.apply(envelope);
        }
        // turn_open は「実行中のはず」に限るので、活性 status の検査で十分。
        let interrupted = session.status().is_active();
        if interrupted {
            session.mark_recovered("アプリの終了で実行状況を確認できませんでした");
        }
        if corrupted_lines > 0 {
            session.flag_incomplete(format!(
                "保存イベントのうち {corrupted_lines} 行が破損していました"
            ));
        }
        let store_capped =
            events.store_capped || meta.as_ref().is_some_and(|meta| meta.store_capped);
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

    pub fn remove(&self, session_id: &SessionId) -> io::Result<()> {
        let dir = self.session_dir(session_id)?;
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// exports_root/<session_id>-<unix_ms>/ に events.jsonl・state.json(あれば)・transcript.md を書く。
    pub fn export(&self, session: &SessionProjection, exports_root: &Path) -> io::Result<PathBuf> {
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
    session_id: SessionId,
    dir: PathBuf,
    writer: BufWriter<File>,
    bytes: u64,
    capped: bool,
}

impl SessionFile {
    fn open_dir(session_id: SessionId, dir: PathBuf) -> io::Result<Self> {
        let file = File::options()
            .append(true)
            .create(true)
            .open(events_path(&dir))?;
        // ハンドルを開いている間、shared lock が他ウィンドウの sweep に使用中を伝える。
        let _ = file.try_lock_shared();
        let bytes = file.metadata()?.len();
        Ok(Self {
            session_id,
            dir,
            writer: BufWriter::new(file),
            bytes,
            capped: bytes >= MAX_EVENT_STORE_BYTES,
        })
    }

    pub fn session_id(&self) -> &SessionId {
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
        let line = serde_json::to_vec(envelope).map_err(|error| invalid(error.to_string()))?;
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
    /// draft は MAX_DRAFT_BYTES に切り詰める(読めない state.json を書かないため)。
    pub fn save_meta(&self, meta: &SessionMeta) -> io::Result<()> {
        let mut meta = meta.clone();
        meta.draft
            .truncate(meta.draft.floor_char_boundary(MAX_DRAFT_BYTES));
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
            .map_err(|error| invalid(error.to_string()))?;
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
        // tool 結果や tool call が先頭に残ると次の model 要求を拒否されるため、
        // 先頭は直近の User メッセージに揃える(なければ空)。
        let start = history[start..]
            .iter()
            .position(|message| matches!(message, Message::User { .. }))
            .map(|offset| start + offset)
            .unwrap_or(history.len());
        // 収まる分だけ末尾から採用する(1件も収まらなければ空)。
        let kept = &history[start..];
        let bytes = serde_json::to_vec(kept).map_err(|error| invalid(error.to_string()))?;
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

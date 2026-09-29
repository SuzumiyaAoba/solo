//! ローカル workspace 用の基本 tool。exec の実行権限は呼出し側 Policy が決める。
mod command;
mod files;
mod prompt;
mod search;

pub use prompt::system_prompt;

use super::{Cancellation, ToolCall, ToolExecutor, ToolResult, ToolSpec};
use serde_json::json;
use std::{
    collections::HashMap,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

/// 副作用のない組込み tool。承認 policy は read/list/search を確認なしで許可してよい。
pub const TOOL_READ: &str = "read";
pub const TOOL_LIST: &str = "list";
pub const TOOL_SEARCH: &str = "search";
pub const TOOL_EDIT: &str = "edit";
pub const TOOL_WRITE: &str = "write";
pub const TOOL_EXEC: &str = "exec";

/// (description, parameters.properties, required) の組。
type Schema = (&'static str, serde_json::Value, &'static [&'static str]);

/// 組込み tool の定義一覧。spec・1行説明・実行・読み取り専用判定はすべて
/// この表から導く。tool の追加は表へ 1 行と実装関数を足すだけでよい。
struct ToolDef {
    name: &'static str,
    /// ツールタブにそのまま表示する 1 行説明。
    summary: &'static str,
    /// 副作用のないツールは承認を省略できる。
    read_only: bool,
    /// モデルへ渡す description と parameters。
    schema: fn() -> Schema,
    run: fn(&mut WorkspaceTools, &ToolCall) -> io::Result<ToolResult>,
}

impl ToolDef {
    fn spec(&self) -> ToolSpec {
        let (description, properties, required) = (self.schema)();
        ToolSpec {
            name: self.name.into(),
            description: description.into(),
            parameters: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        }
    }
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: TOOL_READ,
        summary: "workspace 内の UTF-8 ファイルを読む（行範囲の指定可）",
        read_only: true,
        schema: read_schema,
        run: |tools, call| {
            tools.read(
                required_string(call, "path")?,
                optional_u64(call, "offset")?,
                optional_u64(call, "limit")?,
            )
        },
    },
    ToolDef {
        name: TOOL_LIST,
        summary: "workspace 内のディレクトリを一覧・glob で検索する",
        read_only: true,
        schema: list_schema,
        run: |tools, call| {
            tools.list(
                optional_string(call, "path")?,
                optional_string(call, "pattern")?,
            )
        },
    },
    ToolDef {
        name: TOOL_SEARCH,
        summary: "workspace 内の UTF-8 ファイルから文字列・正規表現を探す",
        read_only: true,
        schema: search_schema,
        run: |tools, call| {
            tools.search(
                required_string(call, "query")?,
                optional_string(call, "path")?,
                optional_string(call, "glob")?,
                optional_bool(call, "regex")?,
            )
        },
    },
    ToolDef {
        name: TOOL_EDIT,
        summary: "既存ファイルの一致する箇所を置換する",
        read_only: false,
        schema: edit_schema,
        run: |tools, call| {
            tools.edit(
                required_string(call, "path")?,
                required_string(call, "old")?,
                required_string(call, "new")?,
                optional_bool(call, "replace_all")?,
            )
        },
    },
    ToolDef {
        name: TOOL_WRITE,
        summary: "ファイルを新規作成・全体を上書きする",
        read_only: false,
        schema: write_schema,
        run: |tools, call| {
            tools.write(
                required_string(call, "path")?,
                required_string(call, "content")?,
            )
        },
    },
    ToolDef {
        name: TOOL_EXEC,
        summary: "workspace で shell command を実行する",
        read_only: false,
        schema: exec_schema,
        run: |tools, call| {
            tools.exec(
                required_string(call, "command")?,
                optional_u64(call, "timeout_seconds")?,
            )
        },
    },
];

/// 基本ツールの表示順。`WorkspaceTools::specs` の配列と一致する。
pub fn tool_names() -> impl Iterator<Item = &'static str> + Clone {
    TOOLS.iter().map(|def| def.name)
}

/// 副作用のない組込み tool なら true。承認 policy が確認を省略できる。
pub fn is_read_only_tool(name: &str) -> bool {
    TOOLS.iter().any(|def| def.name == name && def.read_only)
}

/// 基本ツールの1行説明。ツールタブにそのまま表示する。
pub fn tool_description(name: &str) -> &'static str {
    TOOLS
        .iter()
        .find(|def| def.name == name)
        .map(|def| def.summary)
        .unwrap_or("外部ツール")
}

/// 探索・差分追跡から外すディレクトリ。ベンダーやビルド成果物は対象外にする。
pub(crate) fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | ".next" | "dist" | "build" | ".venv" | "__pycache__"
    )
}

/// 範囲指定の `read` が一度に読むファイルの上限。
const MAX_READ_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub struct WorkspaceTools {
    root: PathBuf,
    specs: Vec<ToolSpec>,
    cancellation: Option<Cancellation>,
    pub timeout: Duration,
    /// `timeout_seconds` 引数の上限。`timeout` 自体の既定はこの値より小さい。
    pub max_timeout: Duration,
    pub max_output_bytes: usize,
    /// `search`/`list` が走査するエントリ数の上限。超過時は部分結果と注記を返す。
    pub max_search_entries: usize,
    /// read/edit/write が最後に観測したファイルの (len, hash)。write の上書きガード用。
    observed: HashMap<PathBuf, (u64, u64)>,
}

impl WorkspaceTools {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = canonical_workspace(root.as_ref())?;
        Ok(Self {
            root,
            cancellation: None,
            timeout: Duration::from_secs(120),
            max_timeout: Duration::from_secs(600),
            max_output_bytes: 64 * 1024,
            max_search_entries: 20_000,
            observed: HashMap::new(),
            specs: TOOLS.iter().map(ToolDef::spec).collect(),
        })
    }

    pub fn with_cancellation(mut self, cancellation: Cancellation) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    /// 観測したファイルの内容を記録する。write の上書き前確認に使う。
    fn observe(&mut self, path: &Path, bytes: &[u8]) {
        self.observed
            .insert(path.to_path_buf(), (bytes.len() as u64, hash(bytes)));
    }

    /// 現在のファイル内容が最後の観測と一致するか。未観測は None。
    fn observation(&self, path: &Path) -> Option<bool> {
        self.observed
            .get(path)
            .map(|expected| file_state(path).is_ok_and(|state| state == *expected))
    }

    fn execute_call(&mut self, call: &ToolCall) -> io::Result<ToolResult> {
        let Some(def) = TOOLS.iter().find(|def| def.name == call.name) else {
            return Err(io::Error::other("未知の tool"));
        };
        (def.run)(self, call)
    }
}

impl ToolExecutor for WorkspaceTools {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn execute(&mut self, call: &ToolCall) -> ToolResult {
        self.execute_call(call)
            .unwrap_or_else(|error| ToolResult::error(error.to_string()))
    }
}

/// 引数オブジェクトから必須の文字列を取る。
fn required_string<'a>(call: &'a ToolCall, key: &str) -> io::Result<&'a str> {
    optional_string(call, key)?.ok_or_else(|| invalid(format!("{key} が必要です")))
}

/// 任意の文字列引数。型が合わない場合はエラー。
fn optional_string<'a>(call: &'a ToolCall, key: &str) -> io::Result<Option<&'a str>> {
    match call.arguments.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} は文字列で指定してください"))),
    }
}

/// 任意の非負整数引数。文字列や負数などの型不一致はエラー。
fn optional_u64(call: &ToolCall, key: &str) -> io::Result<Option<u64>> {
    match call.arguments.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} は 1 以上の整数で指定してください"))),
    }
}

/// 任意の真偽値引数。
fn optional_bool(call: &ToolCall, key: &str) -> io::Result<Option<bool>> {
    match call.arguments.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} は true または false で指定してください"))),
    }
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// ファイルの (バイト長, 内容ハッシュ)。write の上書き前の再確認に使う。
/// 上限を超える場合は「観測時と明らかに違う」としてエラー（=不一致）で返す。
fn file_state(path: &Path) -> io::Result<(u64, u64)> {
    let bytes = crate::storage::read_bytes(
        path,
        MAX_READ_FILE_BYTES,
        "比較対象のファイルが大きすぎます",
    )?;
    Ok((bytes.len() as u64, hash(&bytes)))
}

fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// 各 tool の description と JSON schema。ToolDef::spec が ToolSpec へ組み立てる。
fn read_schema() -> Schema {
    (
        "workspace 内の UTF-8 テキストファイルを読む。offset と limit を省略するとファイル全体（64 KiB まで）を返す。大きなファイルや長いファイルは offset と limit で行範囲を指定する。結果に行番号は付かない。",
        json!({
            "path": {"type":"string","description":"workspace ルートからの相対パス"},
            "offset": {"type":"integer","minimum":1,"description":"読み始める行（1 始まり）。limit だけ指定した場合は 1"},
            "limit": {"type":"integer","minimum":1,"description":"読む最大行数。offset だけ指定した場合は 2000"},
        }),
        &["path"],
    )
}

fn list_schema() -> Schema {
    (
        "workspace 内のディレクトリを一覧する。pattern を省略すると path 直下の項目を返す（ディレクトリは末尾に /）。pattern を指定すると path 以下を再帰的に探し、glob に一致するファイルを返す。.git・target・node_modules などは再帰の対象外。",
        json!({
            "path": {"type":"string","description":"一覧するディレクトリ（workspace ルートからの相対パス）。省略時はルート"},
            "pattern": {"type":"string","description":"path からの相対パスに対する glob。例: \"**/*.rs\"、\"src/**/mod.rs\"、\"*.toml\""},
        }),
        &[],
    )
}

fn search_schema() -> Schema {
    (
        "workspace 内の UTF-8 テキストファイルから query を含む行を探し、`パス:行番号:行` の形式で返す。既定は文字列の完全一致。.git・target・node_modules などは対象外。",
        json!({
            "query": {"type":"string","description":"探す文字列。regex が true の場合は正規表現（Rust の regex 構文）"},
            "path": {"type":"string","description":"検索するディレクトリまたはファイル（workspace ルートからの相対パス）。省略時はルート"},
            "glob": {"type":"string","description":"対象ファイルを絞り込む glob。/ を含まない場合はファイル名、含む場合は path からの相対パスに一致させる。例: \"*.rs\"、\"src/**/*.ts\""},
            "regex": {"type":"boolean","description":"true の場合、query を正規表現として扱う"},
        }),
        &["query"],
    )
}

fn edit_schema() -> Schema {
    (
        "既存ファイルの中の old と完全に一致する部分を new に置き換える。old はファイル中で一度だけ一致する必要がある（前後の行を含めて一意にする）。replace_all が true の場合は一致するすべての箇所を置き換える。",
        json!({
            "path": {"type":"string","description":"workspace ルートからの相対パス"},
            "old": {"type":"string","description":"置き換える元の文字列。read の結果のとおり、空白やインデントも含めて正確に指定する"},
            "new": {"type":"string","description":"置き換え後の文字列"},
            "replace_all": {"type":"boolean","description":"true の場合、old に一致するすべての箇所を置き換える。既定は false"},
        }),
        &["path", "old", "new"],
    )
}

fn write_schema() -> Schema {
    (
        "ファイルを新規作成するか、既存ファイルの内容全体を置き換える。親ディレクトリがなければ作成する。既存ファイルを上書きする場合は、同じ実行の中で先に read で内容を確認している必要がある。一部だけ変更する場合は edit を使う。",
        json!({
            "path": {"type":"string","description":"workspace ルートからの相対パス"},
            "content": {"type":"string","description":"ファイルの新しい内容全体"},
        }),
        &["path", "content"],
    )
}

fn exec_schema() -> Schema {
    (
        "workspace のルートで `sh -c` によりシェルコマンドを実行し、終了コードと標準出力・標準エラーを返す。標準入力は使えないため、対話的なコマンドは実行できない。出力が長い場合は先頭と末尾を残して省略する。",
        json!({
            "command": {"type":"string","description":"実行するシェルコマンド"},
            "timeout_seconds": {"type":"integer","minimum":1,"maximum":600,"description":"制限時間（秒）。既定は 120。時間のかかるビルドやテストでは長めに指定する"},
        }),
        &["command"],
    )
}

/// workspace root を canonicalize し、ディレクトリであることを確認する。
pub(crate) fn canonical_workspace(root: &Path) -> io::Result<PathBuf> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::other("workspace がディレクトリではありません"));
    }
    Ok(root)
}

/// workspace 内の相対パスを canonicalize し、root 配下の通常ファイルに限定する。
pub(crate) fn resolve_file(root: &Path, raw: &str) -> io::Result<PathBuf> {
    let candidate = root.join(raw);
    let path = candidate.canonicalize()?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(io::Error::other(
            "workspace 外、または通常ファイルではありません",
        ));
    }
    Ok(path)
}

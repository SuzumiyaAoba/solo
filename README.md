# Solo

Rust + [GPUI Kit](https://gpui-kit.com/) の Slack 型 agent workspace。プロジェクトでチャンネルをまとめ、各チャンネルをセッションとして扱います。[調査の Phase 0](docs/research/analysis/roadmap.md) に沿った、
会話・差分・ログ・セッション・日本語入力とイベント契約の試作です。

GUI では OpenAI Subscription、ACP agent、ローカルの疑似シナリオを選べます。疑似シナリオの
実行結果と差分は fixture です。
GPUI 非依存の [ハーネスコア](src/harness.rs)は、モデルと tool のアダプターを渡して
単一 agent のループを実行します。GUI の既定の実行先は ChatGPT Codex モデルです。
会話・差分・ログ・実行スレッド・下書き・順番待ちは `~/.solo/sessions/` に保存し、
アプリ再起動後に復元します。実行中に終了した turn は「結果未確認」として戻り、
Subscription の継続実行には保存した会話履歴を再利用します。

## Nix で環境を用意する

Nix の `nix-command` / `flakes` を有効にした環境で、リポジトリのルートから実行します。

```sh
nix develop
cargo run --locked --bin solo
```

シェルに入らず直接起動することもできます。

```sh
nix develop --command cargo run --locked --bin solo
nix develop --command cargo run --locked --bin solo-design
```

`flake.nix` は `rust-toolchain.toml` に合わせた Rust / Cargo / Clippy / rustfmt と、
Git、ripgrep、pkg-config、nixfmt を提供します。macOS では C/C++ compiler、
libclang、Apple SDK、libiconv も Nix から用意します。SDK と bindgen の設定も環境に含めるため、
Nix 経由のビルドにホストの Rust や Xcode Command Line Tools の設定は不要です。
環境に入るだけでログインやモデル実行は開始しません。

初回は Nix の依存と Cargo の crate の取得が必要です。Nix の依存は `flake.lock`、
crate は `Cargo.lock`、Rust の版と component は `rust-toolchain.toml` で固定しています。
macOS は Apple Silicon / Intel、Linux は aarch64 / x86_64 の定義を用意しています。
GUI の実機検証対象は macOS / Apple Silicon、Linux は GUI を無効にした core 用です。

```sh
# CI・疑似シナリオの検証用
nix develop .#minimal
cargo test --locked --no-default-features --all-targets

# Subscription 用（通常の環境と同じ）
nix develop .#subscription
```

direnv を使う場合は付属の `.envrc` を一度 `direnv allow` で許可すると、
ディレクトリに入った際に同じ環境を読み込みます。`.direnv/` は Git の対象外です。
`.envrc` の読み込みには、利用するシェル側の direnv hook 設定が必要です。

依存の更新と Nix 定義の整形は次のコマンドで行います。

```sh
nix flake update
nix fmt flake.nix
nix flake check --all-systems --no-build
```

## OpenAI Subscription での実行

実モデル実行には Solo の `harness::run` と `WorkspaceTools` を使います。モデルへの接続と
ChatGPT OAuth は公開 Rust ライブラリ `genai-agentprism` に委ねます。Codex App Server は起動しません。
認証情報は `~/.solo/auth.json` に保存します。Codex CLI のログインとは独立です。
初回はデバイスコードと URL を表示します。ChatGPT Subscription を通常の OpenAI API キーとして流用しません。

```sh
cargo run --locked --no-default-features --bin solo-subscription -- --login
cargo run --locked --no-default-features --bin solo-subscription -- --cwd . "このリポジトリを要約して"
cargo run --locked --no-default-features --bin solo-subscription -- --model gpt-5.6-sol "このリポジトリを要約して"
```

`read`・`list`・`search` は自動実行します。`edit`・`write`・`exec` は承認モードに従います。
既定の Manual では `edit`・`write` は都度確認し、`exec` は allow / deny で決まらない場合に確認します。
CLI では `y` で今回だけ許可、`a` で Allow に登録して実行、`b` で Deny に登録して拒否できます。
`exec` は workspace で起動しますが、OS の sandbox は適用しません。
既定モデルは `gpt-5.6-sol` で、`--model` または `SOLO_MODEL` で変更できます。
利用可能なモデルはアカウントの権限に依存します。
GUI では新しいセッションの既定の実行先が **OpenAI Subscription** です。
先にログインする場合は入力欄右上のログインアイコンを押してください。認証ページがブラウザーで開き、
コードがクリップボードにコピーされます。ブラウザーに貼り付けて認証すると GUI が完了を表示します。
未ログインでメッセージを送信した場合も同じ画面へ案内し、認証後に実行を続けます。
`edit` と `exec` の承認要求は
チャンネルの **概要** に表示されます。会話やログからは通知の右矢印で移動できます。
コマンドは実行内容・作業ディレクトリ・実行元を確認して、
今回だけ実行、拒否、許可・拒否リストへの登録を選べます。四角の中止アイコンはモデル通信と実行中の command を止めます。
セッション内の続きの入力には保存された会話履歴を使い、再起動後も同じチャンネルで
継続できます。疑似シナリオへ切り替える場合は新しいセッションを作ってください。

CLI は GUI と独立した入口です。
モデル通信の実装は [genai-agentprism](https://docs.rs/genai-agentprism/) を参照してください。

## ACP agent の登録

workspace に `.solo/agents.json` を作ると、登録した ACP v1 対応 agent が GUI の
実行先に追加されます。コマンドは shell を介さずに起動します。たとえば以下のように
各 agent の実際の起動コマンドと引数を指定してください。以下は
[Gemini CLI](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/acp-mode.md) と
[OpenCode](https://opencode.ai/v2/docs/cli/acp/) の公式手順に沿った例です。

```json
{
  "agents": [
    { "id": "gemini", "name": "Gemini CLI", "command": "gemini", "args": ["--acp"] },
    { "id": "opencode", "name": "OpenCode", "command": "opencode", "args": ["acp"] }
  ]
}
```

CLI がアプリの `PATH` で見つからない場合は `command` を絶対パスにしてください。
設定は最大16件・ファイル全体で1 MiBまでです。読み直すには Solo を再起動します。
実行先で選んで送信すると、その UI セッション専用の agent プロセスを起動します。
続きの入力には同じ ACP セッションとプロセスを使い、別の UI セッションでは独立して動きます。
ACP の接続終了後に依頼を続ける場合は、新しいチャンネルを作成してください。送信前の依頼文は入力欄に保持します。
同じ workspace への実 agent の依頼は順番待ちに追加され、先の turn が完了してから
順に開始します。失敗・切断・中止時には自動実行を一時停止します。
承認要求は概要画面に表示し、`allow_once` が提示された場合だけ「今回だけ許可」を選べます。
実行中の中止は `session/cancel` を送り、UI の返答がなくても承認待ちを解除します。
接続準備中の中止や通知を送れない場合は接続を切断します。
agent 主導のログインに対応し、terminal ログインが必要な
agent は先にその CLI でログインしてください。
認証要求と実行中止は ACP のエラーコードで判定するため、エラー文面の言語に依存しません。

ACP agent は Solo と同じ OS 権限で別プロセスとして動きます。承認表示は agent が ACP で
要求した操作を対象とし、agent 自体を隔離しません。信頼できるコマンドを登録してください。
実装は [ACP v1 仕様](https://agentclientprotocol.com/protocol/v1/overview) に基づきます。

## 承認モードと設定ファイル

設定ファイルは `~/.config/solo/config.yml` です。GUI 右上の **Manual / Bypass / Auto** から
モードと Auto の判定モデルを変更・保存できます。承認モードは全プロジェクトと CLI で共通です。
ファイルがない場合は Manual で動作します。[設定例](config.example.yml)も参照してください。

| モード | 承認要求の扱い |
| --- | --- |
| `manual` | Deny は拒否、Allow は許可、それ以外は手動確認 |
| `bypass` | 確認と Allow / Deny の判定を省略して許可 |
| `auto` | Deny、Allow の順に優先し、それ以外を指定モデルで判定 |

```yaml
version: 1
approval:
  mode: auto
  auto:
    model: gpt-5.6-sol
    timeout_seconds: 30
workspaces: {}
```

`approval.auto.model` は**承認判定用**です。作業を実行するモデルの `--model` / `SOLO_MODEL` とは独立して指定します。
Auto ではモデル名が必須です。Solo の ChatGPT ログインを使用し、アカウントで利用できるモデルを指定します。
判定モデルには依頼文・ツールの要求内容・作業ディレクトリを渡し、ツールの実行機能は渡しません。
判定不能・未ログイン・通信エラー・不正な応答・タイムアウトの場合は手動確認に戻ります。
制限時間は `timeout_seconds` で1〜300秒を指定でき、既定は30秒です。

GUI は判定中のモデル名と判定結果・理由を表示します。途中で手動回答や中止を行うと判定を取り消します。
設定変更・セッション終了後に届いた古い判定は適用せず、判定完了時にも最新のルールを確認します。
GUI で保存した設定は待機中の要求にも反映されます。外部で編集したファイルは次の承認要求・結果適用時に読み直します。
ACP では agent が `allow_once` を提示する要求だけを許可でき、コマンド内容が不足する要求は Auto でも手動確認します。

## コマンドの allow / deny

右上のスライダーアイコン **コマンド実行ルール** から一覧・追加・編集・削除ができます。
承認画面の **登録して許可** / **登録して拒否** でも登録できます。
ルールは `~/.config/solo/config.yml` の `workspaces` にワークスペース別で保存し、GUI と CLI で共有します。
リポジトリ内の設定ファイルから許可ルールを自動で取り込むことはありません。

- **ワイルドカード**: `*` は0文字以上の任意の文字列に一致します。`cargo test *` や `git *` のように登録できます。
  パターンはコマンド全体に適用し、`?` / `[]` は通常の文字として扱います。文字そのものの `*` は `\*`、`\` は `\\` と記述します。
- **Allow**: パターンに一致すると確認を省略して実行。作業ディレクトリ・実行元は完全一致です。
- **Deny**: パターンに一致すると実行を拒否。完全一致ルールとワイルドカードルールが重なっても Deny を優先します。
- **完全一致**: 管理画面のスイッチをオフにすると、空白・改行・`*` も含めて入力内容そのものと照合します。
  既存のルールと承認画面・CLI の「登録して許可／拒否」はこの方式を維持します。
  登録済みのルールは一覧のスライダーアイコンで編集し、ワイルドカードへ変更できます。
- **未登録**: Manual では都度確認、Auto では指定モデルが判定します。Allow の `*` は `;` / `&&` / 改行 / リダイレクトなどのシェル演算子を跨ぎません。
  `cargo test *` は `cargo test --release` を許可しますが、`cargo test --release; other-command` には一致しません。
  `cargo test * && cargo clippy *` のように演算子を明記したパターンには対応します。

  変数展開・コマンド置換・ヒアドキュメント・グループ・コメントを含む操作は、ワイルドカードの Allow では自動許可しません。
  Deny はこれらも含めたコマンド文字列に一致させます。
- **保存・読込失敗**: 自動許可せず、保存が成功するまで「登録して実行」も実行しません。

### 基本ツール

同じ Sheet の **ツール** タブで、組込みの `read` / `list` / `search` / `edit` / `write` / `exec`
それぞれに既定 / Allow / Deny / Ask を設定できます。既定は read・list・search が許可、
edit・write・exec が確認です。
Allow は確認を省略します（コマンドの Deny ルールに一致する exec は Deny を優先します）。
Deny は常に拒否、Ask は read・list・search も含めて承認確認に回します。
保存先はコマンドルールと同じ `workspaces` エントリの `tool_decisions` です。
  管理画面でエラーを確認し、設定を修正して再読み込みできます。

手入力での登録は Solo の `sh -c` 実行が対象です。ACP では `kind: execute` の承認要求のうち、
`rawInput` からコマンドと作業ディレクトリを特定できる場合に登録できます。
ACP のルールもコマンドのパターンに対応します。agent の ID・起動コマンド・起動引数・tool 名、
`rawInput` のコマンド以外の値（環境変数など）と入力形式は完全一致です。
シェル文字列形式と argv 形式のルールは共有しません。
作業ディレクトリや入力が未提供の場合は手動確認に残し、`allow_once` を提示しない agent の要求は
Allow でも自動許可しません。ルールの一致条件は管理画面の情報ボタンで確認できます。
ACP の適用範囲は agent が [承認要求](https://agentclientprotocol.com/protocol/v1/tool-calls#requesting-permission) として送る操作です。

YAML の設定形式は `version: 1` です。YAML がまだ存在しない場合だけ、旧ファイル
`~/.solo/command-rules.json`（version 1 / 2）を読み込み、次の保存時に YAML へ移行します。
旧ファイルは残します。既存の完全一致ルールは保持し、`*` を含むコマンドも自動的にはワイルドカードへ変えません。


## ハーネスコア

`harness::run` は会話履歴、モデルの tool call、tool 結果を循環させ、完了・中止・
モデルエラー・呼出し上限を明示して終了します。`Model`、`ToolExecutor`、`Policy`、
`Update` が接続点です。実行権限は呼出し側の `Policy` が毎回判定します。
組み込みエージェントは `Cancellation` の中止要求を通信と承認待ちに伝えます。
`WorkspaceTools` は `read`、`list`、`search`、`edit`、`write`、`exec` を提供します。
`read` は offset/limit で行範囲を読み、`list` は直下の一覧または glob での再帰検索を返します。
`search` は path・glob・regex で対象を絞り込めます。
`edit` は既存ファイルの一意な文字列を置換し（replace_all で全箇所も可）、読み取り後の変更を検出します。
`write` は新規作成と全体の上書きを行い、既存ファイルの上書きには先に read した内容が必要です。
`exec` は終了コードが非ゼロなら tool エラーを返し、標準出力と標準エラーの本文を
出力上限内に収めます（長い出力は先頭と末尾を残して省略）。timeout の既定は 120 秒で、
timeout_seconds で最大 600 秒まで延ばせます。中止・timeout・I/O エラーでも起動した shell を回収します。
システムプロンプトには workspace の `AGENTS.md` があればその内容を含めます。
各ツールの `ToolResult` には 1 行の `summary`(「3 件の一致」「12 バイト」など)を付け、
実行スレッドの結果表示と worker が発行する `tool_finished` イベントに使います。
モデル呼出し・ツール呼出しの上限やエラーで止まった turn も会話履歴に残し、
次の依頼で続きを再開できます。
`read`・`list`・`search`・`edit`・`write` は workspace 外のパスを拒否します。
`exec` は shell を起動するため、許可する場合は呼出し側で適切な隔離環境を用意してください。
timeout は直接起動した shell の終了を制御しますが、その子プロセスまで停止する保証はありません。

**変更** タブの差分は `edit` の置換だけでなく、`exec` が実行前後で書き換えた
ファイル(workspace スナップショットの比較)からも生成します。ベースラインは承認の後、
実行の直前に取るため、確認中の自分の編集は差分に出ません。コマンドが失敗しても
実行済みの変更を表示し、1回の exec では最大8ファイルまで出します。
`.git` / `target` / `node_modules` / `.next` / `dist` / `build` / `.venv` / `__pycache__`
は検索と差分追跡の両方で除外し、256 KiB を超えるファイルは追跡対象外です。
`search` は走査エントリの上限に達すると、部分結果と注記を返します。

## デザイン確認アプリ

メインとは別に、基本コンポーネントを確認する **Solo Design** を起動できます。
ボタン・入力・選択・ナビゲーション・通知・ダイアログを、ライト／ダーク両テーマで操作できます。

```sh
cargo run --locked --bin solo-design
cargo run --locked --bin solo-design -- --page buttons --light
```

テーマ切替は **⌘ Shift L**。仕様・使い方・検証方法は [デザインシステム](docs/design-system.md) を参照してください。
メインと確認アプリは GPUI Kit 0.6.6 の標準部品を共有します。
`src/design/` が Kit とアプリのイベントを接続し、`src/design_tokens.rs` の配色を
Kit の Theme に反映します。ボタン、入力、検索付き選択、通知、ダイアログ、Lucide アイコンに加え、
メイン画面では SidebarMenu / TabBar / Message / MessageScroller / Resizable / StatusBar を使います。

## 起動

初期 GUI 検証対象は macOS / Apple Silicon。上記の `nix develop` で必要な環境を用意できます。
Nix を使わない場合は Rust 1.98.0 と Xcode Command Line Tools を用意してください。
GPUI Kit が固定する GPUI (`gpui-pre` 0.3.6) の `runtime_shaders` により、Metal shader は起動時にコンパイルします。
開発用 shader の事前コンパイルに full Xcode を要求しません。

```sh
cargo run --locked --bin solo
cargo run --locked --bin solo -- --light
cargo run --locked --bin solo -- --compact
```

このリポジトリの作業ルールで RTK を利用する場合は、各コマンドに `rtk` を付けます。
初回は依存の取得・ビルドに時間がかかります。GUI の起動にはログイン済み desktop session
へのアクセスが必要です。画面を閉じるとアプリと実行中の worker が終了します。

## プロジェクトとチャンネル

Slack 型の構成で、左側の **プロジェクト** がチャンネルをまとめるグループ、
**# チャンネル** が1つのセッションです。プロジェクト見出しで折りたたみ、チャンネル名で会話を切り替えます。
見出しの **＋** でそのプロジェクトにチャンネルを作成します。

左下の **プロジェクトを追加** / **⌘ Shift O** から作業フォルダを追加します。
左上のスライダーアイコン、または **⌘ Shift P** で管理用のプロジェクト一覧を開きます。
鉛筆アイコンで表示名を変更、× で登録を解除できます。登録解除はフォルダを削除しません。
実行中・順番待ちのあるプロジェクトは、停止・取り消しを終えてから解除できます。

各プロジェクトは独立したセッション一覧を持ち、下書き、選択セッション、タブ、スクロール、
実行先、順番待ち、承認ルールを保持します。切り替えてもバックグラウンドの実行は続きます。
別プロジェクトに要対応がある場合はベルで知らせ、一覧から件数を確認できます。
実行の作業ディレクトリと `.solo/agents.json` は、そのプロジェクトのフォルダを使います。

プロジェクトのフォルダ・表示名・選択状態は `~/.solo/projects.json` に保存します。
初回は起動ディレクトリを登録し、次回以降は前回の一覧と選択を復元します。
同じフォルダやシンボリックリンク経由の重複登録は、既存のプロジェクトへの切り替えになります。
別ウィンドウで設定が更新された場合は上書きせず、一覧の再読み込みアイコンで同期します。
プロジェクトは最大16件、セッションは各プロジェクト最大8件です。
セッションの会話・下書き・実行先・差分レビュー・順番待ちは `~/.solo/sessions/` に
ワークスペース別で保存し、再起動後に復元します。順番待ちは一時停止状態で戻り、
再生アイコンで明示的に再開します。実行中にアプリが終了したセッションは
「結果未確認」として表示し、自動実行は再開しません。

## 実行スレッド

会話の依頼にある **実行スレッドを開く** から、その依頼のツール呼び出しとサブエージェントの
実行履歴を確認できます。右上の吹き出し / **⌘ Shift T** では最新のスレッドを開閉します。
広い画面では会話の右に並べ、幅1120px未満では本文領域をスレッドへ切り替えます。
× で閉じると会話へ戻ります。チャンネルを切り替えても選択スレッドと下書きを保持します。

各実行にはツール名のバッジ・コマンド・作業場所・結果の 1 行要約・所要時間を表示し、
承認を経た実行には「承認待ち」「許可 · 出どころ」「拒否」のバッジを添えます。
ツール呼び出しが起こした差分は変更ファイルのチップとして一覧し、押すと **変更** タブの
該当ファイルへ移動します。名前を押すと実行内容の詳細を展開できます。
スレッドの見出しには実行件数・全体の所要時間・変更ファイル数を併記します。
依頼を続けても以前のスレッドは保持されます。
中止・失敗・切断で未完了の実行を成功扱いにせず、「中止」または「結果未確認」と表示します。
履歴はセッションごとに最新128依頼・各依頼256実行まで保持し、上限による省略を表示します。
表示上限を超えた分もイベント自体は `~/.solo/sessions/` に残り、再起動後に復元されます。

実ツールは Subscription / ACP の受信イベントから表示します。サブエージェントの表示は
`agent_started` / `agent_finished` と、ツールの任意の `agent_id` に対応します。
サブエージェントを起動するランタイムは未実装です。入力欄の **ツールとサブエージェント**
シナリオでは、実行を伴わない疑似イベントで表示と操作を試せます。

## 操作

通常の操作はアイコンで表示し、意味はホバー時のツールチップと読み上げ用ラベルで確認できます。
許可・拒否など判断が必要な操作には短いラベルを残します。

- プロジェクト見出しの **＋** でチャンネルを作成。最初の依頼から名前を付け、下書きと scroll 位置をチャンネルごとに保持します。
- **概要** に実行状態・最近の操作・変更ファイル・次のアクションを集約。会話の初期画面の **調査する / 変更をレビュー** は下書きに追加するだけで、自動送信しません。
- **要対応** フィルターで承認待ち・ログイン待ち・未読の実行結果・未確認の変更を絞り込み。**⌘ Shift A** で現在のプロジェクトの次の要対応へ移動できます。
- 入力欄の上で **OpenAI Subscription** または登録した **ACP agent** を選んで送信。疑似シナリオは **会話と差分 / ツールとサブエージェント / 1万イベント / 10万イベント / 100 MiB ログ / 未知イベント・切断** で再生。
- 上矢印で送信。別の実 agent が作業中ならリスト追加アイコンで **順番待ちに追加**。実行先と依頼を保持し、先の作業が終わると追加順に開始します。鉛筆アイコンで順番待ちを取り消し、元の依頼を編集できます。
- 実 agent の失敗・切断・中止・実行中のチャンネルを閉じた場合は自動実行を一時停止。結果を確認後、再生アイコンで再開します。再開するまで待機中の依頼は実行しません。失敗・中止した依頼は 戻すアイコンで下書きに復元し、編集して送信できます。
- **変更** で追加・削除行数を確認し、ファイルごとに チェックアイコンで確認済みにし、右矢印で次の未確認ファイルへ移動。差分を起こした実行が特定できる場合は吹き出しアイコンでその実行スレッドへ戻り、差分アイコンで unified パッチをコピーできます。ファイル一覧では最新の実行で変更されたファイルに波形の印を付けます。新しい差分の受信で未確認に戻ります。実行中または省略された差分は確認済みにできません。確認はレビューの記録で、コミット操作ではありません。
- 実行先の選択欄は名前で検索可能。**⌘ Enter** または送信ボタンで送信、**Enter** で改行。IME 変換中は送信せず、Enter は変換確定に利用。
- 中止アイコンは要求中と停止確認を分けて表示。デモの切断アイコンは結果未確認として表示。
- 会話のコピーボタン、ログ行・差分行のクリックで表示内容をコピー。
- ログのコピーアイコンでセッション内のログ一時ファイル一覧を取得。ログは最新1,000件を表示。
- ログを上へ scroll すると追従を止め、ピンアイコンまたは下矢印で再開。
- **⌘ N** 新しいチャンネル、**⌘ W** window を閉じる、**⌘ Q** 終了。会話・下書き・実行を含むチャンネルを右上の × で閉じる際は確認を表示します。
- **⌘ 0 / 1 / 2 / 3** で概要・会話・変更・ログを切り替え。
- **⌘ Shift L** または右上の太陽・月アイコンでライト／ダークを切り替え。
- コピー完了は共通の通知で表示。表示更新回数などはタブ横の波形アイコンから確認できます。

メイン画面全体がデザインシステムの色・文字・余白を参照します。テーマを切り替えても
各セッションの下書き・会話の scroll 位置を保持します。`--compact` は820×620で起動します。

入力欄は Kit の Textarea です。2〜5行で高さが変わり、貼付け時の改行を保持します。
会話を上へスクロールすると末尾追従を止め、「最新のメッセージへ」で再開できます。
会話は最大1,024 byte の表示 block に分け、block 単位でコピーできます。
セッションは各プロジェクト最大8件。順番待ちとレビュー記録も `~/.solo/sessions/` に保存します。
全文ログはセッションの `logs/` に書き出され、チャンネルを閉じると履歴ごと削除します。
アプリを終了しても消えず、次回起動時に復元します。閉じられたセッションと
イベントの無い残骸は起動時に回収します。state.json を読めないセッションは復元せず
エラーとして報告し、ディスク上は削除しません。他のウィンドウが開いているセッションも
回収しません。下書きは1 MiB まで保存します。
再起動中に終了した実行は成功扱いにせず「結果未確認」とします。
ヘッダーの外部リンクアイコンで、会話とイベントを `~/.solo/exports/` に書き出せます。

## 境界と契約

```text
GPUI window / Composer
    ← 16 ms ごとに最大128件の表示更新
    ← bounded channel (256件)
Solo ハーネス + Codex モデル / ACP agent / 疑似 worker / 一時ログ I/O

event.rs       共通 envelope・採番(Sequencer/Emitter)と version 付き event の decode
backend.rs     実行先(Subscription/ACP/Mock)の安定識別子。表示 index とは分離
projection.rs  session ごとの順序・重複・turn 検査、表示状態(SessionProjection)
projection/diff.rs 差分の行番号・追加削除数・表示上限
projection/thread.rs 依頼ごとのツール・サブエージェント履歴と保持上限
orchestration.rs  workspace の順番待ち(BackendKind で保持)、停止・再開
projects.rs    フォルダ単位のプロジェクト登録と設定の保存・競合検出
storage.rs     ファイル読み取りの上限と設定ストアの排他更新・アトミック保存
session_store.rs  ~/.solo/sessions への追記型イベント保存・復元・掃除・書き出し
session_store/store.rs  イベント・meta・履歴ファイルの読み書きとストア操作
session_store/model.rs 保存メタ・順番待ち・復元セッションのデータ型
session_store/transcript.rs セッションの Markdown 書き出し
mock.rs        疑似イベント、backpressure、中止、障害注入、ログ退避
harness.rs     モデル・tool・承認を接続する実行ループと中止・呼出し上限
harness/workspace.rs tool 名・spec・引数の解釈と dispatch
harness/workspace/files.rs read/write/edit とパス解決・上書きガード
harness/workspace/search.rs search/list の走査と glob 絞り込み
harness/workspace/prompt.rs システムプロンプトと AGENTS.md の埋め込み
harness/workspace/command.rs コマンドの起動・出力制限・中止・終了処理
codex.rs       ChatGPT OAuth と Codex モデルを使う Model アダプター
config.rs      ~/.config/solo/config.yml の読み込み・検証・保存
command_rules.rs 承認ルール(完全一致/ワイルドカード)と workspace 別保存
auto_approval.rs  指定モデルによるツール承認の判定と手動確認への復帰(AutoReview)
codex_worker.rs  Solo ハーネスの更新から Solo event への変換
codex_worker/workspace_diff.rs ワークスペースのスナップショットと差分検出
diffgen.rs     before/after テキストから unified diff を生成(全 backend 共通)
acp.rs         ACP v1 の設定・JSON-RPC 契約
acp_worker.rs  ACP の要求・応答から Solo event への変換
acp_worker/connection.rs ACP 接続の状態とプロセス・ワーカーの寿命
acp_worker/stdio.rs 中止可能な非同期の標準入出力
acp_worker/reader.rs 受信サイズを制限した同期 client への橋渡し
acp_worker/writer.rs ACP の送信順序を保ち、制御通知で UI を待たせない書込み
approval/mod.rs 承認要求の正規化と worker↔UI の往路(ApprovalReply)
approval/acp.rs ACP permission 要求から ApprovalRequest への変換
text/          表示向けユーティリティ(preview/task_title)と IME 用 TextBuffer(buffer.rs)
ui/mod.rs      GPUI の起動、ワークスペース・セッションの表示状態と寿命
ui/execution.rs 実行先の選択、ワーカー起動、入力復元とストリーム接続
ui/stream.rs   イベントの一括受信、表示の更新、承認要求と順番待ちへの反映
ui/views.rs    ワークスペースの共通レイアウトとナビゲーション
ui/projects/navigation.rs プロジェクト別のチャンネル一覧と選択
ui/views/      会話・実行スレッド・差分・ログ・入力欄それぞれの描画
design/        GPUI Kit の部品・入力・テーマと Solo のイベント契約の接続
gallery/       デザイン確認用アプリのページと操作例
```

`schema_version / event_id / session_id / sequence / timestamp_ms / turn_id / payload`
が共通 envelope です。既知 event の不正 payload は拒否し、未知 type・新 schema は
生 payload を保持して decode し、画面には長さを制限した診断を表示します。
同一 event ID は再適用せず、逆順・別 session・別 turn は拒否します。
承認の問い合わせと決定は `approval_requested` / `approval_decided` イベントとして
events.jsonl に残り、復元時に追跡できます(決定の出どころは user/auto/rule/bypass/read-only 等の source)。
`tool_started` の `tool`、`tool_finished` の `summary`、`diff_updated` と両承認イベントの
`invocation_id` はいずれも省略可能な追加フィールドです。`schema_version` は 1 のままで、
これらを含まない保存済みイベントは従来どおり読み込めます。
`diff_updated` の `invocation_id` と envelope の `turn_id` は、差分を起こした
実行スレッドへの逆参照(origin)として投影に残ります。
イベント欠落や未完了 tool がある完了通知を成功表示にしません。
usage/cost は未取得なら `null` /「不明」で、0 に補完しません。

会話は Kit の `MessageScroller`、ログ・diff は `uniform_list` で可視範囲だけ描画します。
会話は16,384 block、ログは1,000 preview に制限し、100 MiB の巨大一行は worker で
4 KiB ごとに退避します。差分は20,000行、各行は2,048 byteまで表示します。
省略したログは一時ファイルから参照できます。会話の省略分はこの試作では保存しません。
重複検査用 ID 集合は session 内のイベント数に比例します。
巨大 diff の artifact 化とページ読み込みは後続実装です。`events.jsonl` は 512 MiB、`history.json` は 8 MiB を上限に保存し、超過分は復元対象外として表示します。

## 検証

CI も `nix develop .#minimal` で同じ環境を使用します。以下は Nix shell 内で実行します。

```sh
nixfmt --check flake.nix
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-default-features --all-targets
cargo build --locked --bins
cargo run --locked --bin solo-design -- --smoke
cargo run --locked --bin solo -- --smoke
```

GUI を無効にした契約・Unicode・worker テストは macOS 以外でも実行できます。
`--smoke` は一時フォルダと設定だけで、プロジェクトの追加・改名・切り替え・登録解除、
セッション・実行先・承認ルール・順番待ちの分離を検査します。続けて実ウィンドウで両テーマ・全タブ・空状態、10万イベント、100 MiB ログ、
入力 handler の composition、中止・切断、セッション切替時の下書きと scroll の保持、
受信中のセッションクローズ、順番待ち・取り消し・失敗後の停止、裏での実行先保持、
レビュー・要対応への移動、縮小表示とショートカットを検査して自動終了します。
OS の IME 候補ウィンドウや実キー操作を自動検証したことにはなりません。

```sh
cargo run --locked --no-default-features --example phase0_bench -- 10k
cargo run --locked --no-default-features --example phase0_bench -- 100k
cargo run --locked --no-default-features --example phase0_bench -- 100mib
cargo run --locked --no-default-features --example phase0_bench -- faults
```

上記は同じ疑似イベントを使用し、GUI を介さない投影処理・入力 buffer 操作の
標本数、p50/p95/最大、ログ退避 byte 数を JSON で出力します。描画 frame や
入力→表示の時間とは別の測定です。debug / release も分けて比較してください。

GUI の手動受入では次を確認します。

1. 10万イベント再生中に日本語変換、候補選択、確定、再変換、絵文字・結合文字の削除ができる。
2. 受信中に会話を上へ scroll し、新着が来ても表示位置が飛ばない。session 切替後も draft が残る。
3. 100 MiB ログを受信中に入力と scroll ができ、全文 byte 数と一時ファイルのサイズが一致する。
4. 未知イベント・切断のシナリオが「完了」にならず、未対応2件・拒否2件・重複1件を確認できる。
5. 中止要求から確認まで状態を区別し、window を閉じると worker が残らない。
6. window の拡大縮小、IME 候補位置、長文入力の横スクロールを確認する。

[調査上の性能目標](docs/research/analysis/performance.md)（入力 p95 ≤ 50 ms、frame p95 ≤ 16.7 ms 等）は
実機の入力→present 計測を行ってから判定します。この試作で達成済みとは扱いません。

依存は `Cargo.lock` と完全一致の crate version で固定しています。
出典・ライセンスは [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) を参照してください。

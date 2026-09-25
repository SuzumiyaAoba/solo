# Solo

Rust + [GPUI Kit](https://gpui-kit.com/) の agent workspace。[調査の Phase 0](docs/research/analysis/roadmap.md) に沿った、
会話・差分・ログ・セッション・日本語入力とイベント契約の試作です。

GUI では OpenAI Subscription、ACP agent、ローカルの疑似シナリオを選べます。疑似シナリオの
実行結果と差分は fixture です。
GPUI 非依存の [ハーネスコア](src/harness.rs)は、モデルと tool のアダプターを渡して
単一 agent のループを実行します。GUI の既定の実行先は ChatGPT Codex モデルです。
アプリ再起動後の会話再開は未実装です。

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

CLI の承認要求は毎回 `y` の明示入力が必要です。`read` と `search` は自動実行し、
`edit` と `exec` は都度承認します。`exec` は workspace で起動しますが、OS の sandbox は適用しません。
既定モデルは `gpt-5.6-sol` で、`--model` または `SOLO_MODEL` で変更できます。
利用可能なモデルはアカウントの権限に依存します。
GUI では新しいセッションの既定の実行先が **OpenAI Subscription** です。
先にログインする場合は **ChatGPT ログイン** を押してください。認証ページがブラウザーで開き、
コードがクリップボードにコピーされます。ブラウザーに貼り付けて認証すると GUI が完了を表示します。
未ログインでメッセージを送信した場合も同じ画面へ案内し、認証後に実行を続けます。
`edit` と `exec` の承認要求は
会話画面下部に表示され、今回だけ許可または拒否できます。**中止**はモデル通信と実行中の command を止めます。
セッション内の続きの入力にはメモリ上の会話履歴を使います。疑似シナリオへ切り替える場合は
新しいセッションを作ってください。アプリ再起動後の自動再開は未実装です。

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
設定を読み直すには Solo を再起動します。
実行先で選んで送信すると、その UI セッション専用の agent プロセスを起動します。
続きの入力には同じ ACP セッションとプロセスを使い、別の UI セッションでは独立して動きます。
同じ workspace への実 agent の同時実行は開始時に止め、先の turn が完了してから別の
セッションを実行できます。
承認要求は会話画面に表示し、`allow_once` が提示された場合だけ「今回だけ許可」を選べます。
中止は `session/cancel` を送ります。agent 主導のログインに対応し、terminal ログインが必要な
agent は先にその CLI でログインしてください。

ACP agent は Solo と同じ OS 権限で別プロセスとして動きます。承認表示は agent が ACP で
要求した操作を対象とし、agent 自体を隔離しません。信頼できるコマンドを登録してください。
実装は [ACP v1 仕様](https://agentclientprotocol.com/protocol/v1/overview) に基づきます。

## ハーネスコア

`harness::run` は会話履歴、モデルの tool call、tool 結果を循環させ、完了・中止・
モデルエラー・呼出し上限を明示して終了します。`Model`、`ToolExecutor`、`Policy`、
`Update` が接続点です。実行権限は呼出し側の `Policy` が毎回判定します。
`WorkspaceTools` は `read`、`search`、`edit`、`exec` を提供します。
`edit` は既存ファイルの一意な文字列だけを置換し、読み取り後の変更を検出します。
`read` と `edit` は workspace 外のパスを拒否します。`exec` は shell を起動するため、
許可する場合は呼出し側で適切な隔離環境を用意してください。timeout は直接起動した
shell の終了を制御しますが、その子プロセスまで停止する保証はありません。

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
メイン画面では Sidebar / TabBar / Message / MessageScroller / Resizable / StatusBar を使います。

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

## 操作

- サイドバーでセッションを作成・切替。境界をドラッグして幅を調整。入力途中の文章と scroll 位置はセッションごとに保持。
- 上部で **OpenAI Subscription** または登録した **ACP agent** を選んで送信。疑似シナリオは **会話と差分 / 1万イベント / 10万イベント / 100 MiB ログ / 未知イベント・切断** で再生。
- 実行先の選択欄は名前で検索可能。**⌘ Enter** または送信ボタンで送信、**Enter** で改行。IME 変換中は送信せず、Enter は変換確定に利用。
- **中止** は要求中と停止確認を分けて表示。**切断を試す** は結果未確認として表示。
- 会話のコピーボタン、ログ行・差分行のクリックで表示内容をコピー。
- **全文のパスをコピー** でセッション内のログ一時ファイル一覧を取得。ログは最新1,000件を表示。
- ログを上へ scroll すると追従を止め、**自動追従**スイッチまたは**最新のログへ**ボタンで再開。
- **⌘ N** 新しいセッション、**⌘ W** window を閉じる、**⌘ Q** 終了。
- **⌘ 1 / 2 / 3** で会話・差分・ログを切り替え。
- **⌘ Shift L**、左下のテーマ選択、右上のアイコンでライト／ダークを切り替え。
- コピー完了は共通の通知で表示。表示更新回数などは下部の**計測**から確認できます。

メイン画面全体がデザインシステムの色・文字・余白を参照します。テーマを切り替えても
各セッションの下書き・会話の scroll 位置を保持します。`--compact` は820×620で起動します。

入力欄は Kit の Textarea です。2〜5行で高さが変わり、貼付け時の改行を保持します。
会話を上へスクロールすると末尾追従を止め、「最新のメッセージへ」で再開できます。
会話は最大1,024 byte の表示 block に分け、block 単位でコピーできます。
セッションは最大8件。ログ一時ファイルはセッションを閉じるかアプリが正常終了すると削除します。
crash 後の回収・永続的な履歴は Phase 2 で扱います。

## 境界と契約

```text
GPUI window / Composer
    ← 16 ms ごとに最大128件の表示更新
    ← bounded channel (256件)
Solo ハーネス + Codex モデル / ACP agent / 疑似 worker / 一時ログ I/O

event.rs       version付き envelope と event の decode
projection.rs  session ごとの順序・重複・turn 検査、表示状態
mock.rs        疑似イベント、backpressure、中止、障害注入、ログ退避
codex_subscription.rs  ChatGPT OAuth と Codex モデルを使う Model アダプター
subscription_worker.rs Solo ハーネスの更新から Solo event への変換
acp.rs         ACP v1 の設定・JSON-RPC 契約
acp_worker.rs  ACP プロセスと Solo event への変換
text.rs        UTF-16 / UTF-8、grapheme、IME composition のコア契約（GPUI 非依存）
ui/            GPUI の Entity・focus・リスト・入力・window寿命
design/        GPUI Kit の部品・入力・テーマと Solo のイベント契約の接続
gallery/       デザイン確認用アプリのページと操作例
```

`schema_version / event_id / session_id / sequence / timestamp_ms / turn_id / payload`
が共通 envelope です。既知 event の不正 payload は拒否し、未知 type・新 schema は
生 payload を保持して decode し、画面には長さを制限した診断を表示します。
同一 event ID は再適用せず、逆順・別 session・別 turn は拒否します。
イベント欠落や未完了 tool がある完了通知を成功表示にしません。
usage/cost は未取得なら `null` /「不明」で、0 に補完しません。

会話は Kit の `MessageScroller`、ログ・diff は `uniform_list` で可視範囲だけ描画します。
会話は16,384 block、ログは1,000 preview に制限し、100 MiB の巨大一行は worker で
4 KiB ごとに退避します。差分は20,000行、各行は2,048 byteまで表示します。
省略したログは一時ファイルから参照できます。会話の省略分はこの試作では保存しません。
重複検査用 ID 集合は session 内のイベント数に比例します。
永続 event store、巨大 diff の artifact 化とページ読み込みは後続実装です。

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
`--smoke` は実ウィンドウで両テーマ・全タブ・空状態、10万イベント、100 MiB ログ、
入力 handler の composition、中止・切断、セッション切替時の下書きと scroll の保持、
受信中のセッションクローズ、縮小表示とショートカットを検査して自動終了します。
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

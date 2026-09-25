# Solo

Rust + GPUI の agent workspace。[調査の Phase 0](docs/research/analysis/roadmap.md) に沿った、
会話・差分・ログ・セッション・日本語入力とイベント契約の試作です。

現在はローカルの疑似プロバイダーを使用します。モデルへの接続、shell 実行、workspace
の編集、永続化・再開は後続 Phase の対象です。画面に出る実行結果と差分は fixture です。

## デザイン確認アプリ

メインとは別に、基本コンポーネントを確認する **Solo Design** を起動できます。
ボタン・入力・選択・ナビゲーション・通知・ダイアログを、ライト／ダーク両テーマで操作できます。

```sh
cargo run --locked --bin solo-design
cargo run --locked --bin solo-design -- --page buttons --light
```

テーマ切替は **⌘ Shift L**。仕様・使い方・検証方法は [デザインシステム](docs/design-system.md) を参照してください。
メインと確認アプリは、同じ `src/design/` の部品と `src/design_tokens.rs` の定義を使います。

## 起動

初期 GUI 検証対象は macOS / Apple Silicon。Rust 1.98.0、Xcode Command Line Tools が必要です。
GPUI 0.2.2 の `runtime_shaders` を使うため、Metal shader は起動時にコンパイルします。
開発用 shader の事前コンパイルに full Xcode を要求しません。

```sh
cargo run --locked --bin solo
```

このリポジトリの作業ルールで RTK を利用する場合は、各コマンドに `rtk` を付けます。
初回は依存の取得・ビルドに時間がかかります。GUI の起動にはログイン済み desktop session
へのアクセスが必要です。画面を閉じるとアプリと疑似 worker が終了します。

## 操作

- サイドバーでセッションを作成・切替。入力途中の文章と scroll 位置はセッションごとに保持。
- **会話と差分 / 1万イベント / 10万イベント / 100 MiB ログ / 未知イベント・切断** で再生。
- **⌘ Enter** または送信ボタンで入力を送信。IME 変換中は送信せず、Enter は変換確定に利用。
- **中止** は要求中と停止確認を分けて表示。**切断を試す** は結果未確認として表示。
- 会話のコピーボタン、ログ行・差分行のクリックで表示内容をコピー。
- **全文の場所をコピー** でセッション内のログ一時ファイル一覧を取得。ログは最新1,000件を表示。
- ログを上へ scroll すると追従を止め、**最新へ** で再開。
- **⌘ N** 新しいセッション、**⌘ W** window を閉じる、**⌘ Q** 終了。

入力欄は横スクロールする一行入力です。貼付け時の改行は空白になります。
会話は最大1,024 byte の表示 block に分け、block 単位でコピーできます。
セッションは最大8件。ログ一時ファイルはセッションを閉じるかアプリが正常終了すると削除します。
crash 後の回収・永続的な履歴は Phase 2 で扱います。

## 境界と契約

```text
GPUI window / Composer
    ← 16 ms ごとに最大128件の表示更新
    ← bounded channel (256件)
疑似 worker / 一時ログ I/O

event.rs       version付き envelope と event の decode
projection.rs  session ごとの順序・重複・turn 検査、表示状態
mock.rs        疑似イベント、backpressure、中止、障害注入、ログ退避
text.rs        UTF-16 / UTF-8、grapheme、IME composition（GPUI 非依存）
ui/            GPUI の Entity・focus・リスト・入力・window寿命
design/        両アプリで共有する基本コンポーネントとテーマ
gallery/       デザイン確認用アプリのページと操作例
```

`schema_version / event_id / session_id / sequence / timestamp_ms / turn_id / payload`
が共通 envelope です。既知 event の不正 payload は拒否し、未知 type・新 schema は
生 payload を保持して decode し、画面には長さを制限した診断を表示します。
同一 event ID は再適用せず、逆順・別 session・別 turn は拒否します。
イベント欠落や未完了 tool がある完了通知を成功表示にしません。
usage/cost は未取得なら `null` /「不明」で、0 に補完しません。

会話は可変高さ `ListState`、ログ・diff は `uniform_list` で可視範囲だけ描画します。
会話は16,384 block、ログは1,000 preview に制限し、100 MiB の巨大一行は worker で
4 KiB ごとに退避します。差分は20,000行、各行は2,048 byteまで表示します。
省略したログは一時ファイルから参照できます。会話の省略分はこの試作では保存しません。
重複検査用 ID 集合は session 内のイベント数に比例します。
永続 event store、巨大 diff の artifact 化とページ読み込みは後続実装です。

## 検証

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-default-features --all-targets
cargo build --locked --bins
cargo run --locked --bin solo-design -- --smoke
cargo run --locked --bin solo -- --smoke
```

GUI を無効にした契約・Unicode・worker テストは macOS 以外でも実行できます。
`--smoke` は実ウィンドウで全タブ、10万イベント、100 MiB ログ、入力 handler の composition、
切断表示を検査して自動終了します。
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

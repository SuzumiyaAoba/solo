# Solo Design

メインアプリとデザイン確認アプリが共有する、GPUI の基本コンポーネントです。
低彩度の面、控えめな区切り、明確な文字の階層を使い、主操作を紫で示します。
参考にしたのは [Linear の2026年のデザイン解説](https://linear.app/now/behind-the-latest-design-refresh) にある、
補助的なナビゲーションを静かにし、コンテンツを主役にする考え方です。
色・アイコン・実装は Solo 独自のものです。

## 独立したアプリで確認する

```sh
rtk cargo run --locked --bin solo-design
```

`solo-design` はメインの `solo` とは別の実行ファイルです。同時に起動でき、
セッションや疑似 worker を起動しません。操作例の状態は確認アプリ内だけで保持します。

```sh
rtk cargo run --locked --bin solo-design -- --light
rtk cargo run --locked --bin solo-design -- --page buttons
rtk cargo run --locked --bin solo-design -- --page inputs --light --compact
rtk cargo run --locked --bin solo-design -- --smoke
```

`--page` は `overview / foundations / buttons / inputs / selection / navigation / feedback / overlays`。
`--compact` は980×720のウィンドウで起動します。左の検索欄でページを絞り込めます。
テーマは左下、右上のアイコン、または **⌘ Shift L** で切り替えます。
ページを移動しても入力・選択・各ページの scroll 位置を保持します。

## 基本仕様

| 項目 | 基準 |
| --- | --- |
| 色 | Canvas / Sidebar / Surface / Elevated と、用途に対応した文字・状態色 |
| テーマ | Dark / Light。メインアプリは Dark を採用 |
| フォント | macOS のシステムフォント。コードと色値は Menlo |
| 文字サイズ | 11 / 12 / 13 / 15 / 20 / 30px |
| 余白 | 4 / 8 / 12 / 16 / 24 / 32 / 40px |
| 角丸 | 小要素4 / control 6 / card 10 / dialog 12px |
| control の高さ | Small 28 / Medium 34 / Large 40px |
| アイコン | 独自の26種。16px グリッド、1.4px stroke、バイナリ内に保持 |
| キーボード | Tab / Shift Tab で移動、Enter / Space で操作、Esc で閉じる |

基本文字色と状態ラベル、ボタンの文字色は4.5:1以上、focus と入力枠は3:1以上を
テストで確認します。基準の参考は W3C の [文字コントラスト](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) と
[非テキストコントラスト](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html) です。
色の検査は全アプリのアクセシビリティ適合を保証するものではありません。

## コンポーネントと動作

| コンポーネント | 仕様・確認できる状態 |
| --- | --- |
| Button | Primary / Secondary / Ghost / Danger、3サイズ、hover / pressed / focus / disabled / loading |
| Icon button | 同じ Button の icon-only variant。tooltip に操作名を表示 |
| TextInput | placeholder、アイコン、3サイズ、focus、invalid、disabled、read-only、IME、選択・コピー・貼付け |
| Checkbox | 選択 / 未選択 / mixed / disabled。ラベルを含めてクリック可能 |
| Radio | 選択 / 未選択 / disabled。確認アプリでは矢印キーでグループを移動 |
| Switch | on / off / disabled。ラベルを含めてクリック可能 |
| Select | 選択 / disabled / 空の選択肢。上下キー、Enter、Esc、外側クリック |
| Tab / Nav item | 選択状態、キーボード focus。内容の切替は呼出し側が管理 |
| Badge / Avatar / Keycap | 状態とラベル、識別用イニシャル、ショートカットの表示 |
| Card / Divider | 情報のまとまりと区切り |
| Progress / Skeleton | 0–100%の進捗、静的な読み込み placeholder |
| Empty state / Alert | 空の状態、情報 / 成功 / 注意 / エラー |
| Tooltip | GPUI の hover に応じて操作名を表示 |
| ToastHost | 4秒後に閉じる通知。閉じるボタンあり。新しい通知で古い timer を破棄 |
| Dialog | 確認 / 取消、背景クリック / Esc、内部の Tab 循環、閉じた後の focus 復帰 |

disabled / loading の Button はクリック handler を登録せず、Tab 移動からも外します。
disabled の入力は選択・編集の対象にせず、read-only の入力は選択とコピーを許可します。
TextInput は一行入力です。貼付け時の改行は空白に変換します。
IME 変換中の送信は抑止し、UTF-16 の選択範囲と UTF-8 の buffer を変換します。

`Toggle::radio` は単体の部品です。グループの矢印操作は `gallery/pages.rs` の利用例を参照してください。
Select の popup は後段描画し、window 内に収めます。候補は Tab 順から外し、上下キーで移動します。
Dialog は背景の操作を止めるため、root の最後の子として置きます。

## 実装の使い方

Render 内でのボタンの使用例です。

```rust
Button::new("save", "保存")
    .variant(ButtonVariant::Primary)
    .disabled(is_saving)
    .on_click(cx.listener(|this, _, _, cx| this.save(cx)))
```

- `Application::new().with_assets(DesignAssets)` と `ds::init(cx)` をアプリで一度登録します。
- `ds::root(cx)` は基本書体・色と Tab 操作を提供します。
- `ds::set_theme(ColorScheme::Light, cx)` で全 window を更新します。
- `Button` の `Styled` API で幅や配置を調整できます。色と高さは variant / size を優先します。
- `TextInput` は `InputChanged` と `Submitted` を発行します。`InputChanged.composing` が true の間は検証を確定しません。
- 入力値は送信後も保持するのが既定です。会話入力には `.clear_on_submit(true)` を使います。
- 選択系部品は呼出し側が値を保持する controlled component、入力・Select・Dialog・ToastHost は Entity です。
- `PreviewState` は gallery で hover / pressed / focus を並べるための指定です。通常の画面では指定しません。
- `cx.subscribe(...)` が返す Subscription は、利用する Entity とともに保持してください。

## ファイル構成

```text
src/design_tokens.rs     色・寸法・コントラスト計算（GPUI 非依存）
src/design/mod.rs        テーマ、初期化、共通 root
src/design/components.rs 基本表示部品と操作部品
src/design/input.rs      共通の入力欄
src/design/select.rs     ドロップダウン
src/design/overlays.rs   ダイアログと通知
src/design/icons.rs      アイコンと組込み assets
src/gallery/             確認アプリのページ、操作例、smoke test
src/bin/solo-design.rs   独立したエントリーポイント
```

## 検証

```sh
rtk cargo fmt --all -- --check
rtk cargo clippy --locked --all-targets -- -D warnings
rtk cargo test --locked --no-default-features --all-targets
rtk cargo build --locked --bins
rtk cargo run --locked --bin solo-design -- --smoke
rtk cargo run --locked --bin solo -- --smoke
```

gallery の smoke test は実ウィンドウで8ページ×両テーマ、縮小表示、IME handler、
入力の編集禁止、disabled ボタンの Tab 除外、Select の矢印操作、Dialog の Tab 循環・Esc・focus 復帰を検査します。
Button の Enter / Space は GPUI の標準 key-up 処理を使います。
クリック・Enter / Space の操作感、OS の IME 候補、VoiceOver などの支援技術は実機での確認も必要です。
支援技術への semantic role の公開は、この版では追加していません。

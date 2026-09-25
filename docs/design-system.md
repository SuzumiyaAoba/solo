# Solo Design

メインアプリとデザイン確認アプリは [GPUI Kit 0.6.6](https://gpui-kit.com/) の
標準コンポーネントを使います。独自の描画・入力エンジンを持たず、`src/design/` が
Solo のイベント契約と Kit を接続します。紫の主操作色、低彩度の背景、日本語の
システムフォントは `src/design_tokens.rs` から Kit の Theme / semantic tokens に反映します。

## 起動

```sh
rtk proxy nix develop --command cargo run --locked --bin solo-design
rtk proxy nix develop --command cargo run --locked --bin solo-design -- --page inputs --light --compact
```

`--page` は `overview / foundations / buttons / inputs / selection / navigation / feedback / overlays`。
`--compact` は980×720。検索欄でページを絞り込み、左下・右上または **⌘ Shift L** で
テーマを切り替えます。ページ移動後も入力・選択・scroll 位置を保持します。
確認アプリはモデル接続や worker を起動しません。

## Kit の利用箇所

| 画面・用途 | GPUI Kit のコンポーネント |
| --- | --- |
| ウィンドウとテーマ | `Root`, `Theme`、フォーカス管理、overlay layers |
| 操作 | `Button`, `Tooltip`、loading spinner |
| 入力 | `Input` / `InputState`、`Textarea` / `TextareaState` |
| 選択 | `Checkbox`, `Switch`, `RadioGroup`, `Select` / `SearchableVec` |
| メインのセッション一覧 | `Sidebar`, `SidebarGroup`, `SidebarMenu`, `SidebarMenuItem` |
| 会話・差分・ログの切替、テーマ選択 | `TabBar`, `Tab` |
| 会話 | `Message`, `MessageHeader`, `MessageContent`, `MessageScroller` |
| レイアウト | `h_resizable`, `resizable_panel`, `StatusBar` |
| 状態表示 | `Tag`, `Avatar`, `Kbd`, `Progress`, `Skeleton`, `Empty`, `Alert` |
| 通知・確認 | `Notification`, `Dialog`, `DialogButtonProps`, `WindowExt` |
| アイコン | Kit の組込み Lucide assets |

会話は `MessageScrollerState` をセッションごとに保持し、上へスクロールすると追従を停止、
「最新のメッセージへ」で再開します。可変高さの会話も仮想化されます。差分・ログは
既存の行上限と `uniform_list` による仮想描画を保持します。会話の本文は投影された
表示 block のプレーンテキストです。

サイドバーの境界をドラッグして190〜340pxの範囲で幅を変えられます。
実行先は名前で検索でき、同名の選択肢も index で区別します。
セッション変更時は選択値を同期し、開いたメニューと検索語を破棄します。

## 入力とキーボード

検索・設定には単一行の `Input`、会話と Inputs ページの送信例には2〜5行で伸縮する
`Textarea` を使用します。**Enter** で改行、**⌘ Enter** またはボタンで送信します。
貼付けた改行は Textarea に保持されます。編集、Undo / Redo、文字選択、コピー、
IME の UTF-16 範囲処理は Kit の入力エンジンが担当します。

Solo のアダプターが、空入力・IME 変換中・実行中の送信を止めます。
実行中も次の下書きを編集でき、セッションを切り替えても内容を保持します。
送信成功時は入力欄を空にし、実行開始に失敗した場合は下書きを復元します。
disabled は編集できず、read-only は選択・コピーができます。

Tab / Shift Tab は Kit の `Root` に任せます。独自のグローバル Tab binding は
登録せず、ダイアログの focus trap と干渉しないようにしています。
通知は Kit の表示キューと自動クローズを利用します。

## テーマ

| 項目 | Solo の設定 |
| --- | --- |
| テーマ | Dark / Light |
| フォント | macOS システムフォント、コードと色値は Menlo |
| 文字サイズ | 11 / 12 / 13 / 15 / 20 / 30px |
| 余白 | 4 / 8 / 12 / 16 / 24 / 32 / 40px |
| control の高さ | Small 28 / Medium 34 / Large 40px |
| 角丸 | control 6 / card 10px |

`ds::set_theme` は Solo と Kit の色・フォント・semantic tokens を一緒に更新します。
コントラストテストは Solo のパレットを検査し、Kit の全状態やアプリ全体の
アクセシビリティ適合を保証するものではありません。

## 実装の使い方

```rust
// 起動時に一度だけ初期化。Kit の assets と Root が必要。
gpui_kit::application().with_assets(DesignAssets).run(|cx| {
    ds::init(cx);
    // open_window 内:
    // let view = cx.new(|cx| Workspace::new(...));
    // cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
});
```

`ds::root(cx)` は Solo の基本色・書体を適用する Div です。ウィンドウの最上位には
別途 Kit の `Root` を置き、画面の最後で `Root::render_dialog_layer` と
`Root::render_notification_layer` を描画します。

```rust
Button::new("save", "保存")
    .variant(ButtonVariant::Primary)
    .disabled(is_saving)
    .on_click(cx.listener(|this, _, _, cx| this.save(cx)))
```

`TextInput` は `InputChanged` / `Submitted`、`Select` は `SelectionChanged` を発行します。
入力値は `value(cx)` で取得し、worker からの復元には `set_value(value, cx)` を使います。
復元値は次の描画または入力操作の前に Kit の state に適用されます。
`ToastHost` は Window を持たないコールバックの通知を描画側へ渡すキューです。
購読の `Subscription` は Entity とともに保持してください。

## 検証

```sh
rtk proxy nix develop --command cargo fmt --all -- --check
rtk proxy nix develop --command cargo clippy --locked --all-targets -- -D warnings
rtk proxy nix develop --command cargo test --locked --no-default-features --all-targets
rtk proxy nix develop --command cargo build --locked --bins
rtk proxy nix develop --command cargo run --locked --bin solo-design -- --smoke
rtk proxy nix develop --command cargo run --locked --bin solo -- --smoke
```

デザイン確認アプリの smoke は8ページ×両テーマ、縮小表示、Kit の native IME handler、
編集禁止、改行と送信、選択欄のキー操作、通知、ダイアログの Tab 循環・Esc・focus 復帰を検査します。
メインの smoke は両テーマ・全タブ、10万イベント、100 MiB ログ、入力中のセッション切替、
会話の追従状態、中止・切断・受信中クローズとショートカットを検査します。
OS の IME 候補ウィンドウや実際の VoiceOver 操作は手動確認の対象です。

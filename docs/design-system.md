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
| 概要・会話・変更・ログの切替、テーマ選択 | `TabBar`, `Tab` |
| 会話 | `Message`, `MessageHeader`, `MessageContent`, `MessageScroller` |
| レイアウト | `h_resizable`, `resizable_panel`, `StatusBar` |
| 状態表示 | `Tag`, `Avatar`, `Kbd`, `Progress`, `Skeleton`, `Empty`, `Alert` |
| 通知・確認 | `Notification`, `Dialog`, `DialogButtonProps`, `WindowExt` |
| アイコン | Kit の組込み Lucide assets と Lobe Icons のブランド SVG |

ブランドアイコンには [Lobe Icons](https://github.com/lobehub/lobe-icons) の
`@lobehub/icons-static-svg` 1.95.1 を使用します。必要な SVG を `assets/icons/lobe/` に
同梱し、`DesignAssets` から Kit の assets と合わせて提供します。実行時の取得は不要です。
`Icon::OpenAi` は通常のアイコンと同じ API で使え、`icon_avatar` でアバターにもできます。
新しいセッションでは、既定の OpenAI Subscription が選ばれた初期状態から、
サイドバーと入力欄の実行元に OpenAI アイコンを表示します。実行前は実行先の選択に追従し、
実行後は会話のアシスタントを含め、実際に使った実行先のアイコンを保持します。
実行先の選択だけでは過去の応答元を変えません。

会話は `MessageScrollerState` をセッションごとに保持し、上へスクロールすると追従を停止、
「最新のメッセージへ」で再開します。可変高さの会話も仮想化されます。差分・ログは
既存の行上限と `uniform_list` による仮想描画を保持します。会話の本文は投影された
表示 block のプレーンテキストです。

サイドバーの境界をドラッグして190〜340pxの範囲で幅を変えられます。
実行先は名前で検索でき、同名の選択肢も index で区別します。
セッション変更時は選択値を同期し、開いたメニューと検索語を破棄します。

## アイコンとラベル

通常の操作は `Button::icon` で表示し、ツールチップと読み上げ用ラベルを付けます。
案内文や重複する説明は常設せず、タブ・状態名・許可範囲など判断に必要な短いラベルを残します。
`indicator` は状態アイコンと数値を表示し、その意味をツールチップと `aria_label` で提供します。
切替操作には `toggled` を使い、見た目の選択状態とアクセシビリティの pressed 状態を同期します。

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

## プロジェクトとセッション

左上のフォルダ名がプロジェクト切替の入口です。一覧は Sheet にまとめ、追加・改名・
登録解除はアイコンで操作します。プロジェクト名、フォルダ、セッション数、実行中・待機・
要対応の数を表示します。アプリ名や固定のロゴは表示しません。

プロジェクトごとに独立した `Workspace` Entity を保持し、切り替えでは破棄しません。
worker、入力、タブ、スクロール、エージェント設定、ルール、実行キューはその Entity に所属します。
通知だけを共有し、バックグラウンドの通知にはプロジェクト名を付けます。
実行・順番待ちがあるプロジェクトの登録解除は UI と処理の双方で拒否します。
保存完了後に登録情報を置き換え、壊れた設定や古いウィンドウからの上書きを拒否します。

**⌘ Shift P** で一覧、**⌘ Shift O** でフォルダを追加します。
`--smoke` のプロジェクト検証はユーザー設定を変更せず、一時フォルダを使用します。
`SOLO_PROJECT_PREVIEW_MS` で、一覧のライト／ダーク表示を最大30秒ずつ保持できます。

## タスクの実行とレビュー

概要画面では「次にできる操作」を実行の集計より先に置きます。依頼テンプレートは
下書きに追加し、既存の入力を残します。最初の依頼からタスク名を生成し、
サイドバーの要対応フィルターは承認・ログイン・未読の結果・未確認の変更を対象にします。
状態は色だけでなく文言でも示します。概要への移動は ⌘ 0、次の要対応は ⌘ Shift A です。

同じ workspace の実 agent は順番に実行します。待機中は依頼と実行先を固定し、
取り消すと下書きを復元します。失敗・切断・中止時には自動実行を一時停止します。
エージェントの操作履歴は実イベントから表示し、架空の進捗率を使いません。
変更の確認記録は差分更新で解除し、実行中・表示省略時の確認済み操作を禁止します。
タスクの削除前には失われる情報を示します。順番待ちと確認記録はメモリ上に保持します。

概要・レビュー・順番待ちの GUI テストは `--smoke` に含まれます。
`SOLO_UX_PREVIEW_MS` を指定すると、概要と変更レビューの検証画面で最大30秒ずつ停止します。

## コマンドの承認とルール

コマンドは Terminal アイコンと「コマンド実行の確認」の見出し、等幅のコマンド欄、
作業ディレクトリ、実行元をまとめて表示します。コマンド全文と要求の詳細をコピーでき、
今回だけ実行、拒否、Whitelist / Blacklist への登録を選択できます。

ヘッダーの「コマンド実行ルール」は Kit の Sheet を開きます。TabBar で許可・拒否リストを
切り替え、Textarea から登録、一覧から編集・削除、一致条件の Dialog 表示ができます。
新規登録は `*` のワイルドカードを既定とし、スイッチで完全一致へ切り替えます。
編集時は元の一致方法を保持し、保存まで判定を変更しません。
既存の完全一致ルールは自動でワイルドカードへ変更しません。
ルールはワークスペース別にユーザー設定へ保存し、承認を待っている間に別画面で
Blacklist が更新された場合も、実行確定時に再検査します。

同じ Sheet の「承認モード」タブで Manual / Bypass / Auto を Kit の Select から選べます。
Auto の判定モデルは Input で指定し、保存時に `~/.config/solo/config.yml` を更新します。
モードは全プロジェクト共通で、ヘッダーにも現在のモードを表示します。
Bypass は承認とリスト判定を省略し、Auto はリストで決まらない要求だけをモデルへ渡します。
判定中はモデル名を表示し、結果と理由は概要に残します。エラーや不確実な判定は手動確認へ戻ります。
手動回答・中止・画面クローズで判定を取り消し、設定変更後の古い結果を実行に使いません。

GUI の smoke は一時ディレクトリのルールと疑似承認チャンネルを使い、実コマンドの実行や
ユーザー設定の更新なしで承認・登録・拒否・保存失敗・画面クローズを検査します。
モード検証では疑似判定モデルを使い、選択モデルの反映・許可・拒否・手動確認への復帰・設定変更・
遅延した判定の破棄を検査します。実モデルへの通信は行いません。
`SOLO_SMOKE_APPROVAL_PAUSE_MS` を指定すると承認画面・ルール画面・モード設定で最大30秒ずつ停止し、
目視確認できます。通常の起動には影響しません。

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

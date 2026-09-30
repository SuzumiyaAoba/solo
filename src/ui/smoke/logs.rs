//! ログタブと入力欄の検証: フィルタ・レベル・選択/詳細・一括コピーと、
//! composer の ↑ 呼び出し・変換中バッジを実画面で確認する。
use super::super::{Tab, Workspace, views};
use super::key_down;
use gpui_kit::{AsyncWindowContext, Focusable, WeakEntity};
use solo::event::{Event, Sequencer};
use std::time::Duration;

const PROMPT: &str = "前の依頼を思い出す🙂\n複数行の依頼";

pub(super) async fn run(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    // 複数レベルのログと User メッセージを持つセッションを立てる。
    this.update_in(cx, |this, window, cx| {
        this.new_session(window, cx);
        let session = this.session_mut();
        let mut seq = Sequencer::new(
            session.model.id.clone(),
            session.model.last_sequence(),
            "smoke-turn".to_owned(),
        );
        session.model.apply(seq.next(Event::TurnStarted {
            prompt: PROMPT.into(),
        }));
        for (index, (level, text)) in [
            ("event", "worker に接続しました"),
            ("tool", "cargo test を実行"),
            ("error", "ビルドに失敗"),
            ("plan", "手順を整理"),
            ("info", "完了を確認"),
        ]
        .into_iter()
        .enumerate()
        {
            session.model.apply(seq.next(Event::Log {
                level: level.into(),
                preview: text.into(),
                offset: index as u64,
                bytes: text.len() as u64,
            }));
        }
        session.model.apply(seq.next(Event::TurnCompleted {
            reason: "smoke".into(),
            usage: Default::default(),
        }));
        this.show_tab(Tab::Logs, cx);
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;

    // 純粋関数: フィルタ写像・一括コピー書式・直前の依頼。
    this.update_in(cx, |this, _, _| {
        let session = this.session();
        let logs = session.model.logs();
        let all = logs.len();
        assert_eq!(views::logs::filtered_indices(logs, "", None).len(), all);
        assert_eq!(
            views::logs::filtered_indices(logs, "", Some("error")).len(),
            1
        );
        // 大小区別しない部分一致: text・level・sequence のどれにもマッチする。
        assert_eq!(views::logs::filtered_indices(logs, "CARGO", None).len(), 1);
        assert_eq!(views::logs::filtered_indices(logs, "tool", None).len(), 1);
        let sequence = logs.front().unwrap().sequence.to_string();
        assert!(!views::logs::filtered_indices(logs, &sequence, None).is_empty());
        // 検索語とレベルは AND で絞る。
        assert_eq!(
            views::logs::filtered_indices(logs, "実行", Some("tool")).len(),
            1
        );
        assert_eq!(
            views::logs::filtered_indices(logs, "実行", Some("error")).len(),
            0
        );
        assert!(views::logs::filtered_indices(logs, "存在しない", None).is_empty());
        // 「表示分をコピー」の書式は seq\tlevel\ttext の連結。
        let text = views::logs::filtered_log_text(logs, "", Some("tool"));
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("\ttool\tcargo test を実行"));
        // 直前の依頼は末尾の User ランの全文。
        assert_eq!(
            views::chat::last_user_prompt(session.model.chat()).as_deref(),
            Some(PROMPT)
        );
    })
    .unwrap();

    // 検索ボックスの入力が InputChanged 経由で view.log_filter へ届く。
    this.update_in(cx, |this, window, cx| {
        this.session().log_search.update(cx, |input, cx| {
            input.replace_text_in_range(None, "ビルド", window, cx);
        });
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    this.update_in(cx, |this, _, cx| {
        let session = this.session_mut();
        assert_eq!(session.view.log_filter, "ビルド");
        // レベルチップと選択状態を入れて詳細パネル付きで描画する。
        session.view.log_level = Some("error".into());
        let filtered = views::logs::filtered_indices(
            session.model.logs(),
            &session.view.log_filter,
            session.view.log_level.as_deref(),
        );
        assert_eq!(filtered.len(), 1);
        session.view.log_selected = Some(session.model.logs_discarded() + filtered[0]);
        cx.notify();
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;

    // フィルタ済みの全行をクリップボードへ一括コピーする。
    this.update_in(cx, |this, _, cx| {
        let session = this.session();
        let text = views::logs::filtered_log_text(
            session.model.logs(),
            &session.view.log_filter,
            session.view.log_level.as_deref(),
        );
        assert!(text.contains("\terror\tビルドに失敗"));
        this.copy(text.clone(), "smoke copy", cx);
        let clipboard = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(clipboard.as_deref(), Some(text.as_str()));
    })
    .unwrap();

    // composer が空でフォーカスされているとき ↑ で直前の依頼を呼び出す。
    this.update_in(cx, |this, window, cx| {
        this.show_tab(Tab::Chat, cx);
        let session = this.session();
        session
            .composer
            .update(cx, |input, cx| input.set_value("", cx));
        window.focus(&session.composer.focus_handle(cx), cx);
        cx.notify();
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    key_down(cx, "up");
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    this.update_in(cx, |this, _, cx| {
        assert_eq!(this.session().composer.read(cx).value(cx), PROMPT);
        // 呼び出した内容があれば ↑ は通常のカーソル移動へ流れ、値を維持する。
        this.session().composer.update(cx, |input, cx| {
            input.set_value("編集中の下書き", cx);
        });
        cx.notify();
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    key_down(cx, "up");
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    this.update_in(cx, |this, _, cx| {
        assert_eq!(
            this.session().composer.read(cx).value(cx),
            "編集中の下書き",
            "入力済みの下書きを ↑ で上書きしないこと"
        );
        // IME 変換中バッジの描画経路を通す。
        this.session_mut().input_composing = true;
        cx.notify();
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;

    // 後片付け: フィルタ・選択・変換中を戻してからセッションを閉じる。
    this.update_in(cx, |this, window, cx| {
        let session = this.session_mut();
        session.view.log_filter.clear();
        session.view.log_level = None;
        session.view.log_selected = None;
        session.input_composing = false;
        session.log_search.update(cx, |input, cx| input.set_value("", cx));
        session.composer.update(cx, |input, cx| input.set_value("", cx));
        this.close_session(window, cx);
        println!(
            "Logs smoke OK: filter/level chips, search box, detail panel, copy visible, composer recall, composing badge"
        );
    })
    .unwrap();
}

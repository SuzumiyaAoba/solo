//! 会話タブの表示検証: 分割ブロックの連結・Markdown・折りたたみを実画面で確認する。
use super::smoke::{env_pause_ms, finish_smoke_turn, preview_cycle, start_smoke_turn};
use super::*;
use solo::event::{Event, Sequencer};
use std::time::Duration;

/// 1,024 byte・60 行を超える Markdown 応答。ブロック分割と折りたたみの対象にする。
fn markdown_fixture() -> String {
    let mut text = String::from(
        "## 調査結果\n\n\
         - 手順と確認項目をまとめました\n\
         - 詳細は [ドキュメント](https://example.com/docs) を参照してください\n\n\
         ```rust\n",
    );
    for index in 0..70 {
        text.push_str(&format!("let item_{index:02} = check({index});\n"));
    }
    text.push_str("```\n\n`cargo test` と `cargo clippy` で確認します。\n");
    text
}

pub(super) async fn run(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    // ブロック分割・連結・折りたたみの対象となるセッションを立てる。
    let head = this
        .update_in(cx, |this, window, cx| {
            this.new_session(window, cx);
            let session = this.session_mut();
            start_smoke_turn(session);
            let mut seq = Sequencer::new(
                session.model.id.clone(),
                session.model.last_sequence(),
                "smoke-turn".to_owned(),
            );
            session.model.apply(seq.next(Event::MessageDelta {
                message_id: "smoke-md".into(),
                text: markdown_fixture(),
            }));
            finish_smoke_turn(session);
            let chat = session.model.chat();
            // 前提: 1 メッセージが 1,024 byte 上限の複数ブロックに割れている。
            let head = chat
                .iter()
                .position(|block| block.message_id == "smoke-md")
                .expect("markdown メッセージのブロックがあること");
            let blocks = chat
                .iter()
                .filter(|block| block.message_id == "smoke-md")
                .count();
            assert!(blocks > 1, "長い応答が複数ブロックに分割されていること");
            // ラン先頭から連結すると全文に戻る。区切り文字は挟まない。
            let (text, run_len) = views::chat::message_text(chat, head);
            assert_eq!(text, markdown_fixture());
            assert_eq!(run_len, blocks);
            // ラン先頭はランごとに 1 行。user 依頼・assistant 応答・終了 notice の 3 ラン。
            let heads = (0..chat.len())
                .filter(|&row| views::chat::is_run_head(chat, row))
                .collect::<Vec<_>>();
            assert_eq!(
                heads.len(),
                3,
                "user / assistant / notice の 3 ランだけが先頭行"
            );
            assert_eq!(heads[1], head);
            assert_eq!(
                views::chat::run_bounds(chat, head),
                Some(head..head + blocks)
            );
            // scroller の行数はブロック数のまま維持する(継続行は高さ 0 で隠す)。
            let count = chat.len();
            session
                .chat_list
                .update(cx, |list, cx| list.reset(count, cx));
            this.show_tab(Tab::Chat, cx);
            head
        })
        .unwrap();

    // ブロック境界をまたぐ Markdown を両テーマで描画する。
    this.update_in(cx, |this, _, cx| {
        this.session_mut().chat_list.update(cx, |list, cx| {
            assert!(list.scroll_to_item(head, cx));
        });
    })
    .unwrap();
    preview_cycle(
        this,
        cx,
        [
            (ColorScheme::Dark, 1240., 840.),
            (ColorScheme::Light, 820., 620.),
        ],
        |_, window, cx, &(scheme, width, height)| {
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            cx.notify();
        },
        |&(scheme, width, _)| {
            Some(format!(
                "Chat markdown ready: {} / {}",
                scheme.label(),
                width
            ))
        },
        Duration::from_millis(env_pause_ms("SOLO_CHAT_PREVIEW_MS", 180)),
    )
    .await;

    // 「すべて表示」/「折りたたむ」の状態遷移とランの再測定を確認する。
    for expanded in [true, false] {
        this.update_in(cx, |this, _, cx| {
            let session = this.session_mut();
            let id = "smoke-md".to_owned();
            if expanded {
                assert!(session.view.expanded_messages.insert(id.clone()));
            } else {
                assert!(session.view.expanded_messages.remove(&id));
            }
            let range = views::chat::run_bounds(session.model.chat(), head).expect("ランの範囲");
            session
                .chat_list
                .update(cx, |list, cx| assert!(list.remeasure_items(range, cx)));
            cx.notify();
        })
        .unwrap();
        cx.background_executor()
            .timer(Duration::from_millis(120))
            .await;
    }

    this.update_in(cx, |this, window, cx| {
        let session = this.session();
        assert_eq!(
            session.chat_list.read(cx).item_count(),
            session.model.chat().len(),
            "連結表示でも行数はブロック数のまま"
        );
        assert!(!session.view.expanded_messages.contains("smoke-md"));
        this.close_session(window, cx);
        ds::set_theme(ColorScheme::Light, cx);
        println!(
            "Chat smoke OK: grouped blocks, markdown fence across blocks, expand/collapse, both themes"
        );
    })
    .unwrap();
}

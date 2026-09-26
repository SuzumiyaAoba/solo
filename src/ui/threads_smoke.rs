//! 実モデルを呼ばず、実画面で依頼・チャンネル・スレッドの対応を確認する。
use super::*;
use gpui_kit::component::WindowExt;
use std::time::Duration;

pub(super) async fn run(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    let channel = this
        .update_in(cx, |this, window, cx| {
            this.new_session(window, cx);
            assert!(this.sessions[this.selected].tab == Tab::Chat);
            this.start_mock(
                this.selected,
                Scenario::Threads,
                "チャンネルと実行スレッドの UI を設計する".into(),
                cx,
            );
            this.sessions[this.selected].model.id.clone()
        })
        .unwrap();
    wait_for_turn(this, cx).await;
    let first = this
        .update_in(cx, |this, window, cx| {
            let session = &this.sessions[this.selected];
            assert_eq!(session.model.threads.len(), 1);
            assert_eq!(session.model.threads[0].activities.len(), 5);
            let id = session.model.threads[0].id;
            this.open_thread(id, window, cx);
            this.show_metrics = false;
            this.sessions[this.selected]
                .chat_list
                .update(cx, |list, cx| {
                    list.scroll_to_item(0, cx);
                });
            window.clear_notifications(cx);
            id
        })
        .unwrap();
    for (scheme, width, height) in [
        (ColorScheme::Dark, 1240., 840.),
        (ColorScheme::Light, 1240., 840.),
        (ColorScheme::Light, 820., 620.),
        (ColorScheme::Dark, 820., 620.),
    ] {
        this.update_in(cx, |_, window, cx| {
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            cx.notify();
        })
        .unwrap();
        println!("Thread UI ready: {} / {}", scheme.label(), width);
        let pause = std::env::var("SOLO_THREAD_PREVIEW_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(180)
            .min(30_000);
        cx.background_executor()
            .timer(Duration::from_millis(pause))
            .await;
    }
    this.update_in(cx, |this, window, cx| {
        this.sessions[this.selected]
            .composer
            .update(cx, |input, cx| {
                input.set_value("スレッドを見ながら保持する下書き", cx)
            });
        this.new_session(window, cx);
        assert!(this.sessions[this.selected].selected_thread.is_none());
        this.select_session(&channel, window, cx);
        assert_eq!(this.sessions[this.selected].selected_thread, Some(first));
        assert_eq!(
            this.sessions[this.selected].composer.read(cx).value(cx),
            "スレッドを見ながら保持する下書き"
        );
        this.start_mock(
            this.selected,
            Scenario::Threads,
            "前回の調査をもとに検証を続ける".into(),
            cx,
        );
    })
    .unwrap();
    wait_for_turn(this, cx).await;
    this.update_in(cx, |this, _, cx| {
        let session = &this.sessions[this.selected];
        assert_eq!(session.model.threads.len(), 2);
        assert_eq!(
            session.selected_thread,
            Some(first),
            "new run stole the selected historical thread"
        );
        assert_ne!(session.model.threads[1].id, first);
        assert_eq!(session.model.threads[0].activities.len(), 5);
        this.show_tab(Tab::Chat, cx);
    })
    .unwrap();
    for expected_open in [false, true] {
        cx.update(|window, cx| {
            window.dispatch_keystroke(Keystroke::parse("cmd-shift-t").unwrap(), cx);
        })
        .unwrap();
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
        this.update_in(cx, |this, _, _| {
            let session = &this.sessions[this.selected];
            assert_eq!(session.selected_thread.is_some(), expected_open);
            if expected_open {
                assert_eq!(
                    session.selected_thread,
                    session.model.threads.back().map(|t| t.id)
                );
            }
        })
        .unwrap();
    }
    this.update_in(cx, |this, window, cx| {
        this.close_session(window, cx);
        let empty = this.sessions.last().unwrap().model.id.clone();
        this.select_session(&empty, window, cx);
        this.close_session(window, cx);
        assert_eq!(this.sessions.len(), 1);
        window.resize(size(px(820.), px(620.)));
        ds::set_theme(ColorScheme::Light, cx);
        println!("Thread smoke OK: channel default, tool/agent activity, historical threads, independent drafts and selection, compact and split layouts, both themes, keyboard toggle");
    }).unwrap();
}

async fn wait_for_turn(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    let started = Instant::now();
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(60))
            .await;
        let done = this
            .update_in(cx, |this, _, _| {
                let s = &this.sessions[this.selected];
                if s.model.status.is_active() {
                    return false;
                }
                assert_eq!(s.model.status, Status::Completed);
                assert_eq!(s.model.rejected, 0);
                true
            })
            .unwrap();
        if done {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "thread demo timed out"
        );
    }
}

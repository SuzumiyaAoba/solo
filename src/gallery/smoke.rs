//! 実ウィンドウで両テーマの全ページとキーボード操作を検査する。
use super::*;

pub(super) fn start(window: &Window, cx: &mut Context<Gallery>) {
    cx.spawn_in(window, async move |this, cx| {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for page in Page::ALL {
                this.update_in(cx, |this, window, cx| {
                    ds::set_theme(scheme, cx);
                    this.focus.focus(window);
                    this.navigate(page, cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(160)).await;
            }
        }
        this.update_in(cx, |this, window, cx| {
            this.navigate(Page::Inputs, cx);
            for index in [3, 4] {
                this.fields[index].update(cx, |input, cx| {
                    let before = input.buffer.content.clone();
                    input.replace_text_in_range(None, "変更されない", window, cx);
                    input.replace_and_mark_text_in_range(None, "変換", None, window, cx);
                    assert_eq!(input.buffer.content, before, "disabled/read-only input mutated");
                });
            }
            this.fields[0].update(cx, |input, cx| {
                input.replace_text_in_range(Some(0..usize::MAX), "前🙂", window, cx);
                input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
                assert_eq!(input.marked_text_range(window, cx), Some(3..7));
                input.submit(cx);
                assert!(input.buffer.marked.is_some());
                input.replace_text_in_range(None, "日本語", window, cx);
                assert_eq!(input.buffer.content, "前🙂日本語");
            });
        }).unwrap();

        this.update_in(cx, |this, window, cx| { this.navigate(Page::Buttons, cx); this.probe_focus.focus(window); }).unwrap();
        cx.background_executor().timer(Duration::from_millis(200)).await;
        for _ in 0..40 {
            key_down(cx, "tab");
            cx.background_executor().timer(Duration::from_millis(30)).await;
            this.update_in(cx, |this, window, _| assert!(!this.disabled_focus.is_focused(window), "disabled button became a tab stop")).unwrap();
        }
        this.update_in(cx, |this, window, cx| {
            assert_eq!(this.clicks, 0);
            this.navigate(Page::Selection, cx); window.focus(&this.select.focus_handle(cx));
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        key_down(cx, "down");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        key_down(cx, "down");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| this.select.update(cx, |select, cx| select.accept(window, cx))).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| {
            assert_eq!(this.select.read(cx).selected, 1, "Select keyboard selection failed");
            assert!(!this.select.read(cx).open);
            this.navigate(Page::Overlays, cx); this.probe_focus.focus(window);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| this.dialog.update(cx, |dialog, cx| dialog.show(window, cx))).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        for _ in 0..5 {
            key_down(cx, "tab");
            cx.background_executor().timer(Duration::from_millis(80)).await;
            this.update_in(cx, |this, window, cx| {
                assert!(this.dialog.read(cx).open);
                assert!(this.dialog.read(cx).has_focus(window), "focus escaped dialog");
            }).unwrap();
        }
        key_down(cx, "escape");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| {
            assert!(!this.dialog.read(cx).open);
            assert!(this.probe_focus.is_focused(window), "dialog did not restore focus");
            window.resize(size(px(980.), px(720.)));
            ds::set_theme(ColorScheme::Dark, cx);
        }).unwrap();
        for page in Page::ALL {
            this.update_in(cx, |this, _, cx| this.navigate(page, cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(120)).await;
        }
        this.update_in(cx, |this, _, cx| {
            assert!(this.rendered >= 24);
            println!("Design smoke OK: 8 pages, 2 themes, compact window, IME, disabled/read-only, tab navigation, select arrows, dialog focus trap/restore. renders={}", this.rendered);
            cx.quit();
        }).unwrap();
    }).detach();
}

fn key_down(cx: &mut AsyncWindowContext, key: &str) {
    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
    })
    .unwrap();
}

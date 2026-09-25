//! 実ウィンドウで Kit の入力・選択・overlay と全ページを検査する。
use super::*;
use gpui_kit::component::WindowExt;

pub(super) fn start(window: &Window, cx: &mut Context<Gallery>) {
    cx.spawn_in(window, async move |this, cx| {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for page in Page::ALL {
                this.update_in(cx, |this, window, cx| {
                    ds::set_theme(scheme, cx);
                    this.focus.focus(window, cx);
                    this.navigate(page, cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(160)).await;
            }
        }
        this.update_in(cx, |this, window, cx| {
            this.navigate(Page::Inputs, cx);
            for index in [3, 4] {
                this.fields[index].update(cx, |input, cx| {
                    let before = input.value(cx);
                    input.replace_text_in_range(None, "変更されない", window, cx);
                    input.replace_and_mark_text_in_range(None, "変換", None, window, cx);
                    assert_eq!(input.value(cx), before, "disabled/read-only input mutated");
                });
            }
            this.fields[0].update(cx, |input, cx| {
                input.replace_text_in_range(Some(0..usize::MAX), "前🙂", window, cx);
                input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
                assert_eq!(input.marked_text_range(window, cx), Some(3..7));
                input.submit(window, cx);
                assert!(input.marked_text_range(window, cx).is_some());
                input.replace_text_in_range(None, "日本語", window, cx);
                assert_eq!(input.value(cx), "前🙂日本語");
            });
            window.focus(&this.composer.focus_handle(cx), cx);
            this.composer.update(cx, |input, cx| input.replace_text_in_range(Some(0..usize::MAX), "first", window, cx));
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        key_down(cx, "enter");
        cx.background_executor().timer(Duration::from_millis(120)).await;
        this.update_in(cx, |this, window, cx| {
            assert!(this.submissions.is_empty(), "Enter must insert a newline");
            assert_eq!(this.composer.read(cx).value(cx), "first\n");
            this.composer.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
            });
        }).unwrap();
        key_down(cx, "cmd-enter");
        cx.background_executor().timer(Duration::from_millis(120)).await;
        this.update_in(cx, |this, window, cx| {
            assert!(this.submissions.is_empty(), "composition must not submit");
            this.composer.update(cx, |input, cx| input.replace_text_in_range(None, "日本語", window, cx));
        }).unwrap();
        key_down(cx, "cmd-enter");
        cx.background_executor().timer(Duration::from_millis(120)).await;
        this.update_in(cx, |this, _, cx| {
            assert_eq!(this.submissions, ["first\n日本語"], "Cmd Enter must submit exactly once without inserting a newline");
            assert!(this.composer.read(cx).value(cx).is_empty());
            this.navigate(Page::Selection, cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| window.focus(&this.select.focus_handle(cx), cx)).unwrap();
        key_down(cx, "down");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        key_down(cx, "down");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        key_down(cx, "enter");
        cx.background_executor().timer(Duration::from_millis(200)).await;
        this.update_in(cx, |this, window, cx| {
            assert_eq!(this.select.read(cx).selected, 1, "Kit Select keyboard selection failed");
            assert!(!window.notifications(cx).is_empty(), "selection notification missing");
        }).unwrap();
        // 検索結果の行番号ではなく、元の選択肢の index を返すことを確認。
        this.update_in(cx, |this, window, cx| window.focus(&this.select.focus_handle(cx), cx)).unwrap();
        key_down(cx, "down");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        cx.update(|window, cx| {
            let input = window.focused_input(cx).expect("Select search input");
            let input = input.as_input().expect("single-line search").clone();
            input.update(cx, |state, cx| state.replace_text_in_range(None, "アーカイブ", window, cx));
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(300)).await;
        key_down(cx, "enter");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| {
            assert_eq!(this.select.read(cx).selected, 3, "filtered choice lost its original index");
            this.navigate(Page::Overlays, cx);
            window.focus(&this.search.focus_handle(cx), cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| this.dialog.update(cx, |dialog, cx| dialog.show(window, cx))).unwrap();
        cx.background_executor().timer(Duration::from_millis(160)).await;
        for _ in 0..5 {
            key_down(cx, "tab");
            cx.background_executor().timer(Duration::from_millis(80)).await;
            this.update_in(cx, |_, window, cx| {
                assert!(window.has_active_dialog(cx));
                assert!(gpui_kit::base::active_focus_trap(window, cx).expect("dialog trap").contains_focused(window, cx), "focus escaped dialog");
            }).unwrap();
        }
        key_down(cx, "escape");
        cx.background_executor().timer(Duration::from_millis(160)).await;
        this.update_in(cx, |this, window, cx| {
            assert!(!this.dialog.read(cx).open);
            assert!(!window.has_active_dialog(cx));
            assert!(this.search.focus_handle(cx).is_focused(window), "dialog did not restore focus");
            window.resize(size(px(980.), px(720.)));
            ds::set_theme(ColorScheme::Dark, cx);
        }).unwrap();
        for page in Page::ALL {
            this.update_in(cx, |this, _, cx| this.navigate(page, cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(120)).await;
        }
        this.update_in(cx, |this, _, cx| {
            assert!(this.rendered >= 24);
            println!("Design smoke OK: GPUI Kit, 8 pages, 2 themes, compact window, IME, disabled/read-only, select keyboard, notifications, dialog focus/restore. renders={}", this.rendered);
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

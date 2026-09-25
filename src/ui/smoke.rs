use super::*;
use std::time::Duration;

pub(super) fn start(window: &Window, cx: &mut Context<Workspace>) {
    cx.spawn_in(window, async move |this, cx| {
        this.update_in(cx, |this, _, cx| {
            assert_eq!(this.scenario_picker.read(cx).options.len(), 1 + this.acp_agents.len() + SCENARIOS.len());
        }).unwrap();
        for (index, scenario) in [Scenario::Demo, Scenario::Events100k, Scenario::Log100MiB, Scenario::Faults].into_iter().enumerate() {
            this.update_in(cx, |this, _, cx| {
                ds::set_theme(if index % 2 == 0 { ColorScheme::Dark } else { ColorScheme::Light }, cx);
                if scenario != Scenario::Demo { this.scenario(scenario, cx); }
            }).expect("smoke window remains open");
            let started = Instant::now();
            let mut tick = 0;
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let done = this.update_in(cx, |this, window, cx| {
                    let s = &mut this.sessions[this.selected];
                    s.tab = [Tab::Chat, Tab::Logs, Tab::Diff][tick % 3];
                    s.composer.update(cx, |input, cx| {
                        input.buffer = Default::default();
                        input.replace_text_in_range(None, "前🙂", window, cx);
                        input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
                        assert_eq!(input.marked_text_range(window, cx), Some(3..7));
                        assert!(input.buffer.take_committed().is_none());
                        input.replace_text_in_range(None, "日本語", window, cx);
                        assert_eq!(input.buffer.content, "前🙂日本語");
                    });
                    let done = !s.model.status.is_active();
                    assert_eq!(this.scenario_picker.read(cx).disabled, !done);
                    if done {
                        assert_eq!(s.model.status, if scenario == Scenario::Faults { Status::Disconnected } else { Status::Completed });
                        assert!(!s.model.diffs.is_empty());
                        println!("Solo smoke: {} / {} / events={} / renders={} / max batch={:.2}ms", scenario.label(), ds::scheme(cx).label(), s.model.accepted, this.rendered, s.max_batch_ms);
                    }
                    cx.notify();
                    done
                }).expect("smoke window remains open");
                if done { break; }
                assert!(started.elapsed() < Duration::from_secs(45), "smoke stream timed out");
                tick += 1;
            }
        }

        let original_id = this.update_in(cx, |this, window, cx| {
            let original_id = this.sessions[0].model.id.clone();
            this.sessions[0].composer.update(cx, |input, cx| {
                input.replace_text_in_range(Some(0..usize::MAX), "保持する下書き🙂", window, cx);
            });
            this.sessions[0].chat_list.scroll_to(ListOffset { item_ix: 3, offset_in_item: px(0.) });
            this.new_session(cx);
            original_id
        }).unwrap();
        // 空状態も両テーマで描画し、セッションを切り替えても下書き・scroll を失わない。
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            for tab in [Tab::Chat, Tab::Diff, Tab::Logs] {
                this.update_in(cx, |this, _, cx| { ds::set_theme(scheme, cx); this.show_tab(tab, cx); }).unwrap();
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
        }
        this.update_in(cx, |this, window, cx| {
            let empty_id = this.sessions[this.selected].model.id.clone();
            this.select_session(&original_id, window, cx);
            assert_eq!(this.sessions[this.selected].composer.read(cx).buffer.content, "保持する下書き🙂");
            assert_eq!(this.sessions[this.selected].chat_list.logical_scroll_top().item_ix, 3);
            this.select_session(&empty_id, window, cx);
            this.close_session(window, cx);
            assert_eq!(this.sessions.len(), 1);
            assert!(this.sessions[0].composer.focus_handle(cx).is_focused(window));
        }).unwrap();

        // UI の停止要求と停止確認を区別し、開始前の cancel も通す。
        for expected in [Status::Cancelled, Status::Disconnected] {
            this.update_in(cx, |this, _, cx| {
                this.new_session(cx);
                this.scenario(Scenario::Events100k, cx);
                if expected == Status::Cancelled {
                    this.cancel(cx);
                    assert_eq!(this.sessions[this.selected].model.status, Status::Cancelling);
                } else {
                    this.sessions[this.selected].controller.as_ref().unwrap().disconnect();
                }
                assert!(this.scenario_picker.read(cx).disabled);
            }).unwrap();
            let started = Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let done = this.update_in(cx, |this, _, cx| {
                    let s = &this.sessions[this.selected];
                    if s.model.status.is_active() { return false; }
                    assert_eq!(s.model.status, expected);
                    assert_eq!(s.model.rejected, 0);
                    assert!(!this.scenario_picker.read(cx).disabled);
                    true
                }).unwrap();
                if done { break; }
                assert!(started.elapsed() < Duration::from_secs(5), "stop confirmation timed out");
            }
            this.update_in(cx, |this, window, cx| this.close_session(window, cx)).unwrap();
        }

        this.update_in(cx, |this, window, cx| {
            this.new_session(cx);
            this.scenario(Scenario::Events100k, cx);
            this.close_session(window, cx); // 受信中に閉じても、生き残った session を更新しない。
            assert_eq!(this.sessions[0].model.id, original_id);
            this.show_metrics = true;
            this.sessions[0].model.title = "長いセッション名でもレイアウトと入力欄が崩れないことを確認しています".repeat(3);
            window.resize(size(px(820.), px(620.)));
            cx.notify();
        }).unwrap();
        // キーボードのタブ切替とテーマ切替を、実際の action dispatch 経路で確認する。
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            this.update_in(cx, |_, _, cx| ds::set_theme(scheme, cx)).unwrap();
            for (key, tab) in [("cmd-1", Tab::Chat), ("cmd-2", Tab::Diff), ("cmd-3", Tab::Logs)] {
                cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx); }).unwrap();
                cx.background_executor().timer(Duration::from_millis(120)).await;
                this.update_in(cx, |this, _, _| assert!(this.sessions[0].tab == tab)).unwrap();
            }
        }
        cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse("cmd-shift-l").unwrap(), cx); }).unwrap();
        cx.background_executor().timer(Duration::from_millis(120)).await;
        this.update_in(cx, |this, _, cx| {
            assert_eq!(ds::scheme(cx), ColorScheme::Light);
            assert!(this.rendered > 30);
            assert_eq!(this.sessions[0].model.unknown, 2);
            assert_eq!(this.sessions[0].model.rejected, 2);
            assert_eq!(this.sessions[0].composer.read(cx).buffer.content, "保持する下書き🙂");
            println!("Solo smoke OK: both themes, all panes, 100k events, 100MiB logs, IME, session drafts/scroll, cancel/disconnect, close while running, compact layout, keyboard shortcuts");
            cx.quit();
        }).unwrap();
    }).detach();
}

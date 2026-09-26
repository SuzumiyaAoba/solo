use super::*;
use solo::subscription_worker::Delivery as SubscriptionDelivery;
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
                    s.tab = [Tab::Overview, Tab::Chat, Tab::Logs, Tab::Diff][tick % 4];
                    s.composer.update(cx, |input, cx| {
                        input.set_value("", cx);
                        input.replace_text_in_range(None, "前🙂", window, cx);
                        input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
                        assert_eq!(input.marked_text_range(window, cx), Some(3..7));
                        input.submit(window, cx);
                        assert!(input.marked_text_range(window, cx).is_some());
                        input.replace_text_in_range(None, "日本語", window, cx);
                        assert_eq!(input.value(cx), "前🙂日本語");
                    });
                    let done = !s.model.status.is_active();
                    assert_eq!(s.chat_list.read(cx).item_count(), s.model.chat.len(), "visible conversation rows must follow the projection");
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
            this.sessions[0].chat_list.update(cx, |list, cx| { assert!(list.scroll_to_item(3, cx)); });
            this.new_session(window, cx);
            original_id
        }).unwrap();
        // 空状態も両テーマで描画し、セッションを切り替えても下書き・scroll を失わない。
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            for tab in [Tab::Overview, Tab::Chat, Tab::Diff, Tab::Logs] {
                this.update_in(cx, |this, _, cx| { ds::set_theme(scheme, cx); this.show_tab(tab, cx); }).unwrap();
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
        }
        this.update_in(cx, |this, window, cx| {
            let empty_id = this.sessions[this.selected].model.id.clone();
            this.select_session(&original_id, window, cx);
            assert_eq!(this.sessions[this.selected].composer.read(cx).value(cx), "保持する下書き🙂");
            assert!(!this.sessions[this.selected].chat_list.read(cx).is_following_tail());
            this.select_session(&empty_id, window, cx);
            this.close_session(window, cx);
            assert_eq!(this.sessions.len(), 1);
            assert_eq!(this.scenario_picker.read(cx).selected, this.sessions[0].selected_backend, "closing a session must restore its neighbor backend");
            assert!(this.sessions[0].composer.focus_handle(cx).is_focused(window));
        }).unwrap();

        // UI の停止要求と停止確認を区別し、開始前の cancel も通す。
        for expected in [Status::Cancelled, Status::Disconnected] {
            this.update_in(cx, |this, window, cx| {
                this.new_session(window, cx);
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
            this.new_session(window, cx);
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
            for (key, tab) in [("cmd-0", Tab::Overview), ("cmd-1", Tab::Chat), ("cmd-2", Tab::Diff), ("cmd-3", Tab::Logs)] {
                cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx); }).unwrap();
                cx.background_executor().timer(Duration::from_millis(120)).await;
                this.update_in(cx, |this, _, _| assert!(this.sessions[0].tab == tab)).unwrap();
            }
        }
        cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse("cmd-shift-l").unwrap(), cx); }).unwrap();
        cx.background_executor().timer(Duration::from_millis(120)).await;
        approval_checks(&this, cx).await;
        approval_modes_smoke::check(&this, cx).await;
        workflow_smoke::run(&this, cx).await;
        threads_smoke::run(&this, cx).await;
        this.update_in(cx, |this, _, cx| {
            assert_eq!(ds::scheme(cx), ColorScheme::Light);
            assert!(this.rendered > 30);
            assert_eq!(this.sessions[0].model.unknown, 2);
            assert_eq!(this.sessions[0].model.rejected, 2);
            assert_eq!(this.sessions[0].composer.read(cx).value(cx), "保持する下書き🙂");
            println!("Solo smoke OK: both themes, all panes, 100k events, 100MiB logs, IME, session drafts/scroll, cancel/disconnect, close while running, compact layout, keyboard shortcuts");
            cx.quit();
        }).unwrap();
    }).detach();
}

async fn approval_checks(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    use gpui_kit::component::WindowExt;
    use solo::harness::ToolCall;
    let request = this.update_in(cx, |this, window, cx| {
        this.new_session(window, cx);
        this.sessions[this.selected].model.status = Status::Running;
        this.show_tab(Tab::Overview, cx);
        ApprovalRequest::tool(&ToolCall { id: "approval-smoke".into(), name: "exec".into(),
            arguments: serde_json::json!({"command":"cargo test --locked\ncargo clippy --locked --all-targets -- -D warnings"}) },
            std::path::Path::new(&this.workspace_path))
    }).unwrap();
    let store = this
        .update_in(cx, |this, _, _| {
            this.command_rules.as_ref().unwrap().clone()
        })
        .unwrap();
    let command = request.command.clone().unwrap();
    let inject = |request: ApprovalRequest, cx: &mut AsyncWindowContext| {
        this.update_in(cx, |this, _, cx| {
            let (reply, answer) = async_channel::bounded(1);
            let session = &this.sessions[this.selected];
            let id = session.model.id.clone();
            let generation = session.stream_generation;
            this.consume(
                &id,
                generation,
                vec![UiDelivery::Subscription(SubscriptionDelivery::Approval {
                    request: Box::new(request),
                    reply,
                })],
                false,
                cx,
            );
            answer
        })
        .unwrap()
    };
    let answer = inject(request.clone(), cx);
    for (scheme, width, height) in [
        (ColorScheme::Light, 1240., 840.),
        (ColorScheme::Dark, 820., 620.),
    ] {
        this.update_in(cx, |this, window, cx| {
            assert!(this.sessions[this.selected].approval.is_some());
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            cx.notify();
        })
        .unwrap();
        cx.background_executor()
            .timer(Duration::from_millis(200))
            .await;
    }
    assert!(matches!(
        answer.try_recv(),
        Err(async_channel::TryRecvError::Empty)
    ));
    println!("Approval UI ready: command / cwd / rule actions, no commands executed");
    approval_preview_pause(cx).await;
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(answer.try_recv().expect("approval should be answered"));
    assert!(
        store.load().unwrap().whitelist.is_empty(),
        "allow once persisted a rule"
    );

    let waiting = inject(request.clone(), cx);
    store.add(RuleList::Blacklist, command.clone()).unwrap();
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(
        matches!(waiting.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "new blacklist was ignored while confirmation was open"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!waiting.try_recv().unwrap());
    store.remove(RuleList::Blacklist, &command).unwrap();

    let answer = inject(request.clone(), cx);
    this.update_in(cx, |this, _, cx| {
        this.remember_approval(RuleList::Whitelist, cx)
    })
    .unwrap();
    assert!(answer.try_recv().expect("approval should be answered"));
    let answer = inject(request.clone(), cx);
    assert!(
        answer.try_recv().expect("approval should be answered"),
        "whitelist did not auto-allow"
    );
    this.update_in(cx, |this, _, _| {
        assert!(this.sessions[this.selected].approval.is_none())
    })
    .unwrap();
    store.add(RuleList::Blacklist, command.clone()).unwrap();
    let answer = inject(request.clone(), cx);
    assert!(
        !answer.try_recv().expect("approval should be answered"),
        "blacklist did not override whitelist"
    );

    let mut unlisted = request.clone();
    unlisted
        .command
        .as_mut()
        .unwrap()
        .command
        .push_str(" --extra");
    unlisted.display_command = Some(unlisted.command.as_ref().unwrap().command.clone());
    let answer = inject(unlisted, cx);
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!answer.try_recv().expect("approval should be answered"));

    store.remove(RuleList::Blacklist, &command).unwrap();
    let mut no_allow_once = request.clone();
    no_allow_once.can_allow = false;
    let answer = inject(no_allow_once, cx);
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(
        matches!(answer.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "ACP without allow_once was accepted"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!answer.try_recv().expect("approval should be answered"));

    let valid = std::fs::read(store.path()).unwrap();
    std::fs::write(store.path(), "{").unwrap();
    let answer = inject(request.clone(), cx);
    this.update_in(cx, |this, _, cx| {
        this.remember_approval(RuleList::Whitelist, cx);
        assert!(
            this.sessions[this.selected]
                .approval
                .as_ref()
                .unwrap()
                .policy_error
                .is_some()
        );
        this.answer_approval(true, cx);
    })
    .unwrap();
    assert!(
        matches!(answer.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "failed save approved execution"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!answer.try_recv().expect("approval should be answered"));
    assert_eq!(std::fs::read_to_string(store.path()).unwrap(), "{");
    std::fs::write(store.path(), valid).unwrap();

    this.update_in(cx, |this, _, cx| {
        command_rules::smoke(&this.rule_editor, cx)
    })
    .unwrap();
    this.update_in(cx, |this, window, cx| this.open_command_rules(window, cx))
        .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(250))
        .await;
    println!("Command rules UI ready: persisted whitelist");
    approval_preview_pause(cx).await;
    this.update_in(cx, |_, window, cx| {
        assert!(window.has_active_sheet(cx));
        window.close_sheet(cx);
    })
    .unwrap();
    store.remove(RuleList::Whitelist, &command).unwrap();
    let answer = inject(request, cx);
    this.update_in(cx, |this, window, cx| {
        this.close_session(window, cx);
        ds::set_theme(ColorScheme::Light, cx);
    })
    .unwrap();
    assert!(
        matches!(answer.try_recv(), Err(async_channel::TryRecvError::Closed)),
        "closing a session left approval waiting"
    );
    println!(
        "Command approval smoke OK: allow once, saved whitelist, blacklist precedence, unknown commands, unsupported permission, failed save, rules sheet, close while waiting"
    );
}

pub(super) async fn approval_preview_pause(cx: &mut AsyncWindowContext) {
    if let Some(ms) = std::env::var("SOLO_SMOKE_APPROVAL_PAUSE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        cx.background_executor()
            .timer(Duration::from_millis(ms.min(30_000)))
            .await;
    }
}

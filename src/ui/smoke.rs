use super::*;
use solo::codex_worker::Delivery as SubscriptionDelivery;
use solo::event::{Event, Sequencer};
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
            let mut tick = 0;
            until(cx, Duration::from_secs(45), Duration::from_millis(100), "smoke stream timed out", |cx| {
                let done = this.update_in(cx, |this, window, cx| {
                    // scenario_picker/rendered を読むため sessions の借用を分割する。
                    let sessions = &mut this.sessions;
                    let s = &mut sessions[this.selected];
                    s.view.tab = [Tab::Overview, Tab::Chat, Tab::Logs, Tab::Diff][tick % 4];
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
                    let done = !s.display_status().is_active();
                    assert_eq!(s.chat_list.read(cx).item_count(), s.model.chat().len(), "visible conversation rows must follow the projection");
                    assert_eq!(this.scenario_picker.read(cx).disabled, !done);
                    if done {
                        assert_eq!(s.model.status(), if scenario == Scenario::Faults { Status::Disconnected } else { Status::Completed });
                        assert!(!s.model.diffs().is_empty());
                        println!("Solo smoke: {} / {} / events={} / renders={} / max batch={:.2}ms", scenario.label(), ds::scheme(cx).label(), s.model.accepted(), this.rendered, s.metrics.max_batch_ms);
                    }
                    cx.notify();
                    done
                }).expect("smoke window remains open");
                if done {
                    Some(())
                } else {
                    tick += 1;
                    None
                }
            }).await;
        }

        let original_id = this.update_in(cx, |this, window, cx| {
            let original_id = this.session_at(0).model.id.clone();
            this.session_at_mut(0).composer.update(cx, |input, cx| {
                input.replace_text_in_range(Some(0..usize::MAX), "保持する下書き🙂", window, cx);
            });
            this.session_at_mut(0).chat_list.update(cx, |list, cx| { assert!(list.scroll_to_item(3, cx)); });
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
            let empty_id = this.session().model.id.clone();
            this.select_session(&original_id, window, cx);
            assert_eq!(this.session().composer.read(cx).value(cx), "保持する下書き🙂");
            assert!(!this.session().chat_list.read(cx).is_following_tail());
            this.select_session(&empty_id, window, cx);
            this.close_session(window, cx);
            assert_eq!(this.sessions.len(), 1);
            assert_eq!(this.scenario_picker.read(cx).selected, this.session_at(0).selected_backend, "closing a session must restore its neighbor backend");
            assert!(this.session_at(0).composer.focus_handle(cx).is_focused(window));
        }).unwrap();

        // UI の停止要求と停止確認を区別し、開始前の cancel も通す。
        for expected in [Status::Cancelled, Status::Disconnected] {
            this.update_in(cx, |this, window, cx| {
                this.new_session(window, cx);
                this.scenario(Scenario::Events100k, cx);
                if expected == Status::Cancelled {
                    this.cancel(cx);
                    assert_eq!(this.session().model.status(), Status::Cancelling);
                } else {
                    this.session().exec.controller.as_ref().unwrap().disconnect();
                }
                assert!(this.scenario_picker.read(cx).disabled);
            }).unwrap();
            until(cx, Duration::from_secs(5), Duration::from_millis(100), "stop confirmation timed out", |cx| {
                this.update_in(cx, |this, _, cx| {
                    let s = this.session();
                    if s.display_status().is_active() { return None; }
                    assert_eq!(s.model.status(), expected);
                    assert_eq!(s.model.rejected(), 0);
                    assert!(!this.scenario_picker.read(cx).disabled);
                    Some(())
                }).unwrap()
            }).await;
            this.update_in(cx, |this, window, cx| this.close_session(window, cx)).unwrap();
        }

        this.update_in(cx, |this, window, cx| {
            this.new_session(window, cx);
            this.scenario(Scenario::Events100k, cx);
            this.close_session(window, cx); // 受信中に閉じても、生き残った session を更新しない。
            assert_eq!(this.session_at(0).model.id, original_id);
            this.show_metrics = true;
            // タイトルは SessionCreated から投影されるため、長いタイトル専用のセッションを立てる。
            this.new_session(window, cx);
            let workspace = this.workspace_path.clone();
            {
                let session = this.session_mut();
                let mut seq = Sequencer::new(
                    session.model.id.clone(),
                    session.model.last_sequence(),
                    "smoke-title".to_owned(),
                );
                session.model.apply(seq.next(Event::SessionCreated {
                    title: "長いセッション名でもレイアウトと入力欄が崩れないことを確認しています".repeat(3),
                    workspace_id: workspace,
                    settings: serde_json::Value::Null,
                }));
            }
            window.resize(size(px(820.), px(620.)));
            cx.notify();
        }).unwrap();
        // 長いタイトルの表示を確認したら畳み、元のセッションで以降の操作検証を続ける。
        cx.background_executor().timer(Duration::from_millis(120)).await;
        this.update_in(cx, |this, window, cx| this.close_session(window, cx)).unwrap();
        // キーボードのタブ切替とテーマ切替を、実際の action dispatch 経路で確認する。
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            this.update_in(cx, |_, _, cx| ds::set_theme(scheme, cx)).unwrap();
            for (key, tab) in [("cmd-0", Tab::Overview), ("cmd-1", Tab::Chat), ("cmd-2", Tab::Diff), ("cmd-3", Tab::Logs)] {
                key_down(cx, key);
                cx.background_executor().timer(Duration::from_millis(120)).await;
                this.update_in(cx, |this, _, _| assert!(this.session_at(0).view.tab == tab)).unwrap();
            }
        }
        key_down(cx, "cmd-shift-l");
        cx.background_executor().timer(Duration::from_millis(120)).await;
        approval_checks(&this, cx).await;
        approval_modes_smoke::check(&this, cx).await;
        workflow_smoke::run(&this, cx).await;
        threads_smoke::run(&this, cx).await;
        chat_smoke::run(&this, cx).await;
        // 永続化: 同じストアから読み直し、会話・状態・下書き・全文ログが復元されることを確認する。
        this.update_in(cx, |this, _, cx| {
            let store = this.store.clone().expect("smoke store must exist");
            let model = &this.session_at(0).model;
            let restored = store.load(&model.id).expect("session restore must succeed");
            assert_eq!(restored.session.id, model.id);
            assert_eq!(restored.session.accepted(), model.accepted());
            assert_eq!(restored.session.status(), model.status());
            assert_eq!(restored.session.chat().len(), model.chat().len());
            assert_eq!(restored.session.diffs().len(), model.diffs().len());
            assert!(!restored.log_paths.is_empty(), "全文ログがストアに残ること");
            assert_eq!(
                restored.meta.as_ref().map(|meta| meta.draft.as_str()),
                Some("保持する下書き🙂"),
            );
            println!("Solo smoke: restore events={} logs={} draft_ok", restored.session.accepted(), restored.log_paths.len());
            cx.notify();
        }).unwrap();
        this.update_in(cx, |this, _, cx| {
            assert_eq!(ds::scheme(cx), ColorScheme::Light);
            assert!(this.rendered > 30);
            assert_eq!(this.session_at(0).model.unknown(), 2);
            assert_eq!(this.session_at(0).model.rejected(), 2);
            assert_eq!(this.session_at(0).composer.read(cx).value(cx), "保持する下書き🙂");
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
        start_smoke_turn(this.session_mut());
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
    let answer = inject_approval(this, request.clone(), cx);
    preview_cycle(
        this,
        cx,
        [
            (ColorScheme::Light, 1240., 840.),
            (ColorScheme::Dark, 820., 620.),
        ],
        |this, window, cx, &(scheme, width, height)| {
            assert!(this.session().approval.is_some());
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            cx.notify();
        },
        |_| None,
        Duration::from_millis(200),
    )
    .await;
    assert!(matches!(
        answer.try_recv(),
        Err(async_channel::TryRecvError::Empty)
    ));
    println!("Approval UI ready: command / cwd / rule actions, no commands executed");
    approval_preview_pause(cx).await;
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(
        answer
            .try_recv()
            .expect("approval should be answered")
            .accepted
    );
    assert!(
        store.load().unwrap().allow.is_empty(),
        "allow once persisted a rule"
    );

    let waiting = inject_approval(this, request.clone(), cx);
    store.add(RuleList::Deny, command.clone()).unwrap();
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(
        matches!(waiting.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "new deny rule was ignored while confirmation was open"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!waiting.try_recv().unwrap().accepted);
    store.remove(RuleList::Deny, &command).unwrap();

    let answer = inject_approval(this, request.clone(), cx);
    this.update_in(cx, |this, _, cx| {
        this.remember_approval(RuleList::Allow, cx)
    })
    .unwrap();
    assert!(
        answer
            .try_recv()
            .expect("approval should be answered")
            .accepted
    );
    let answer = inject_approval(this, request.clone(), cx);
    assert!(
        answer
            .try_recv()
            .expect("approval should be answered")
            .accepted,
        "allow rule did not auto-allow"
    );
    this.update_in(cx, |this, _, _| assert!(this.session().approval.is_none()))
        .unwrap();
    store.add(RuleList::Deny, command.clone()).unwrap();
    let answer = inject_approval(this, request.clone(), cx);
    assert!(
        !answer
            .try_recv()
            .expect("approval should be answered")
            .accepted,
        "deny rule did not override the allow rule"
    );

    let mut unlisted = request.clone();
    unlisted
        .command
        .as_mut()
        .unwrap()
        .command
        .push_str(" --extra");
    unlisted.display_command = Some(unlisted.command.as_ref().unwrap().command.clone());
    let answer = inject_approval(this, unlisted, cx);
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(
        !answer
            .try_recv()
            .expect("approval should be answered")
            .accepted
    );

    store.remove(RuleList::Deny, &command).unwrap();
    let mut no_allow_once = request.clone();
    no_allow_once.can_allow = false;
    let answer = inject_approval(this, no_allow_once, cx);
    this.update_in(cx, |this, _, cx| this.answer_approval(true, cx))
        .unwrap();
    assert!(
        matches!(answer.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "ACP without allow_once was accepted"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(
        !answer
            .try_recv()
            .expect("approval should be answered")
            .accepted
    );

    let valid = std::fs::read(store.path()).unwrap();
    std::fs::write(store.path(), "{").unwrap();
    let answer = inject_approval(this, request.clone(), cx);
    this.update_in(cx, |this, _, cx| {
        this.remember_approval(RuleList::Allow, cx);
        assert!(
            this.session()
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
    assert!(
        !answer
            .try_recv()
            .expect("approval should be answered")
            .accepted
    );
    assert_eq!(std::fs::read_to_string(store.path()).unwrap(), "{");
    std::fs::write(store.path(), valid).unwrap();

    this.update_in(cx, |this, _, cx| {
        command_rules_smoke::smoke(&this.rule_editor, cx)
    })
    .unwrap();
    this.update_in(cx, |this, window, cx| this.open_command_rules(window, cx))
        .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(250))
        .await;
    println!("Command rules UI ready: persisted allow rules");
    approval_preview_pause(cx).await;
    this.update_in(cx, |_, window, cx| {
        assert!(window.has_active_sheet(cx));
        window.close_sheet(cx);
    })
    .unwrap();
    store.remove(RuleList::Allow, &command).unwrap();
    let answer = inject_approval(this, request, cx);
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
        "Command approval smoke OK: allow once, saved allow rule, deny precedence, unknown commands, unsupported permission, failed save, rules sheet, close while waiting"
    );
}

/// smoke 用のターン開始注入。status を Running へ進めるため TurnStarted を投影へ流す。
pub(super) fn start_smoke_turn(session: &mut SessionView) {
    let mut seq = Sequencer::new(
        session.model.id.clone(),
        session.model.last_sequence(),
        "smoke-turn".to_owned(),
    );
    session.model.apply(seq.next(Event::TurnStarted {
        prompt: "smoke".into(),
    }));
}

/// `start_smoke_turn` で開いたターンを完了へ戻す。turn_id は固定値で対応させる。
pub(super) fn finish_smoke_turn(session: &mut SessionView) {
    let mut seq = Sequencer::new(
        session.model.id.clone(),
        session.model.last_sequence(),
        "smoke-turn".to_owned(),
    );
    session.model.apply(seq.next(Event::TurnCompleted {
        reason: "smoke".into(),
        usage: Default::default(),
    }));
}

pub(super) async fn approval_preview_pause(cx: &mut AsyncWindowContext) {
    let pause = env_pause_ms("SOLO_SMOKE_APPROVAL_PAUSE_MS", 0);
    if pause > 0 {
        cx.background_executor()
            .timer(Duration::from_millis(pause))
            .await;
    }
}

/// 承認要求を selected session に注入し、応答受信用の channel を返す。
pub(super) fn inject_approval(
    this: &WeakEntity<Workspace>,
    request: ApprovalRequest,
    cx: &mut AsyncWindowContext,
) -> async_channel::Receiver<ApprovalReply> {
    this.update_in(cx, |this, _, cx| {
        let (reply, answer) = async_channel::bounded(1);
        let session = this.session();
        let id = session.model.id.clone();
        let generation = session.exec.stream_generation;
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
}

/// `step` が `Some` を返すまで `interval` おきに再試行し、`timeout` を超えたら `what` で失敗する。
pub(super) async fn until<R>(
    cx: &mut AsyncWindowContext,
    timeout: Duration,
    interval: Duration,
    what: &'static str,
    mut step: impl FnMut(&mut AsyncWindowContext) -> Option<R>,
) -> R {
    let started = Instant::now();
    loop {
        cx.background_executor().timer(interval).await;
        if let Some(done) = step(cx) {
            return done;
        }
        assert!(started.elapsed() < timeout, "{what}");
    }
}

/// `var` に指定された環境変数でプレビュー滞留ミリ秒を上書きし、未設定・不正値なら `default` を返す。
/// 長すぎる待機で smoke が止まらないよう 30 秒で打ち切る。
pub(super) fn env_pause_ms(var: &str, default: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
        .min(30_000)
}

/// action dispatch 経路でキー入力を送る。実際のキーバインドを通すため window への dispatch を使う。
pub(super) fn key_down(cx: &mut AsyncWindowContext, key: &str) {
    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
    })
    .unwrap();
}

/// テーマ・サイズなどの `steps` を順に適用し、各ステップを `pause` だけ描画させる。
/// `label` が `Some` を返したときだけ各ステップの "ready" 行を出力する。
pub(super) async fn preview_cycle<E: 'static, S>(
    this: &WeakEntity<E>,
    cx: &mut AsyncWindowContext,
    steps: impl IntoIterator<Item = S>,
    mut prepare: impl FnMut(&mut E, &mut Window, &mut Context<E>, &S),
    mut label: impl FnMut(&S) -> Option<String>,
    pause: Duration,
) {
    for step in steps {
        this.update_in(cx, |this, window, cx| prepare(this, window, cx, &step))
            .unwrap();
        if let Some(label) = label(&step) {
            println!("{label}");
        }
        cx.background_executor().timer(pause).await;
    }
}

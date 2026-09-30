use super::super::{Tab, Workspace, execution::Backend, stream::UiDelivery};
use super::{env_pause_ms, preview_cycle, start_smoke_turn, until};
use gpui_kit::component::WindowExt;
use gpui_kit::{AsyncWindowContext, Context, WeakEntity, Window, px, size};
use solo::codex_worker::Delivery as SubscriptionDelivery;
use solo::event::{Event, Sequencer};
use solo::{
    acp::AgentProfile,
    codex::DeviceLogin,
    design::{self as ds, ColorScheme},
    mock::Scenario,
    projection::Status,
};
use std::time::Duration;

pub(super) async fn run(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    this.update_in(cx, login_lifecycle).unwrap();
    this.update_in(cx, closed_acp_conversation).unwrap();
    // 実モデルに接続せず、workspace の占有と待機解除を検証する。
    let (blocker, queued) = this
        .update_in(cx, |this, window, cx| {
            this.new_session(window, cx);
            let blocker = this.sessions.selected;
            start_smoke_turn(this.session_at_mut(blocker));
            this.session_at_mut(blocker).backend.active = Some(Backend::Subscription);
            this.new_session(window, cx);
            let queued = this.sessions.selected;
            // 表示用 picker と異なっても、依頼元セッションの実行先を使う。
            this.backends.picker.update(cx, |picker, _| {
                picker.selected = 1 + this.backends.acp_agents.len()
            });
            this.start_selected(
                queued,
                "順番待ちの日本語の依頼🙂\n完了条件を保持".into(),
                cx,
            );
            assert_eq!(
                this.queue.position(&this.session_at(queued).model.id),
                Some(1)
            );
            assert!(this.session_at(queued).composer.read(cx).read_only);
            assert!(!this.session_at(queued).composer.read(cx).can_submit);
            assert!(this.backends.picker.read(cx).disabled);
            this.cancel_queued(cx);
            assert!(this.queue.is_empty());
            assert_eq!(
                this.session_at(queued).composer.read(cx).value(cx),
                "順番待ちの日本語の依頼🙂\n完了条件を保持"
            );
            assert!(!this.session_at(queued).composer.read(cx).read_only);
            this.fill_prompt("追加の完了条件", window, cx);
            assert!(
                this.session_at(queued)
                    .composer
                    .read(cx)
                    .value(cx)
                    .contains("保持\n\n追加の完了条件")
            );
            this.session_at_mut(queued).backend.selected = 1 + this.backends.acp_agents.len();
            this.enqueue(queued, "順番待ちから開始するデモ".into(), cx);
            let id = this.session_at(blocker).model.id.clone();
            let generation = this.session_at(blocker).exec.stream_generation;
            this.consume(
                &id,
                generation,
                vec![UiDelivery::Subscription(SubscriptionDelivery::Error(
                    "smoke: execution failed".into(),
                ))],
                false,
                cx,
            );
            assert!(this.queue.paused(), "a failed agent must pause queued work");
            this.dispatch_queue(cx);
            assert!(!this.session_at(queued).display_status().is_active());
            // Error で Failed になっているため、再開にはキューの停止解除だけ要る。
            this.session_at_mut(blocker).model.reset_idle();
            this.session_at_mut(blocker).backend.active = None;
            this.session_at_mut(blocker).unread_result = false;
            this.queue.set_paused(false);
            this.select_session(&this.session_at(blocker).model.id.clone(), window, cx);
            this.dispatch_queue(cx);
            assert!(this.session_at(queued).display_status().is_active());
            assert!(matches!(
                this.session_at(queued).backend.active,
                Some(Backend::Mock(_))
            ));
            assert_eq!(
                this.backends.picker.read(cx).selected,
                0,
                "background dispatch changed the foreground picker"
            );
            (blocker, queued)
        })
        .unwrap();
    until(
        cx,
        Duration::from_secs(10),
        Duration::from_millis(100),
        "queued mock timed out",
        |cx| {
            this.update_in(cx, |this, _, _| {
                !this.session_at(queued).display_status().is_active()
            })
            .unwrap()
            .then_some(())
        },
    )
    .await;
    this.update_in(cx, |this, window, cx| {
        assert!(this.session_at(queued).needs_attention());
        assert_eq!(
            this.session_at(queued).last_prompt,
            "順番待ちから開始するデモ"
        );
        this.select_session(&this.session_at(blocker).model.id.clone(), window, cx);
        this.next_attention(window, cx);
        assert_eq!(this.sessions.selected, queued);
        assert!(this.session_at(queued).view.tab == Tab::Overview);
        this.review_next(cx);
        assert!(this.session_at(queued).view.tab == Tab::Diff);
        let count = this.session_at(queued).model.unreviewed_count();
        assert!(count > 0);
        this.toggle_review(cx);
        assert_eq!(this.session_at(queued).model.unreviewed_count(), count - 1);
        this.toggle_review(cx);
        assert_eq!(this.session_at(queued).model.unreviewed_count(), count);
        let count = this.sessions.len();
        this.request_close_session(window, cx);
        assert_eq!(
            this.sessions.len(),
            count,
            "confirmation must precede data loss"
        );
        window.close_dialog(cx);
        this.show_tab(Tab::Overview, cx);
    })
    .unwrap();
    preview_cycle(
        this,
        cx,
        [
            (ColorScheme::Dark, 1240., 840., Tab::Overview),
            (ColorScheme::Light, 820., 620., Tab::Overview),
            (ColorScheme::Light, 820., 620., Tab::Diff),
        ],
        |this, window, cx, &(scheme, width, height, tab)| {
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            this.show_tab(tab, cx);
        },
        |&(scheme, width, _, tab)| {
            Some(format!(
                "UX preview: {} / {} / {}",
                scheme.label(),
                width,
                if tab == Tab::Overview {
                    "overview"
                } else {
                    "review"
                }
            ))
        },
        Duration::from_millis(env_pause_ms("SOLO_UX_PREVIEW_MS", 150)),
    )
    .await;
    this.update_in(cx, |this, window, cx| {
        this.close_session(window, cx);
        assert_eq!(this.sessions.selected, blocker);
        this.close_session(window, cx);
        assert_eq!(this.sessions.len(), 1);
        assert!(this.queue.is_empty());
        ds::set_theme(ColorScheme::Light, cx);
        println!("Workflow smoke OK: queue/cancel/resume, background backend, drafts, attention navigation, review, close confirmation, themes and compact layout");
    }).unwrap();
}

fn closed_acp_conversation(this: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let missing_agent = tempfile::tempdir().unwrap();
    let id = "closed-acp-regression";
    this.backends.acp_agents.push(AgentProfile {
        id: id.into(),
        name: "Closed ACP".into(),
        command: missing_agent
            .path()
            .join("missing-agent")
            .display()
            .to_string(),
        args: vec![],
    });
    this.new_session(window, cx);
    let index = this.sessions.selected;
    // acp_agents を読むため sessions の借用をフィールド分割する。
    let sessions = &mut this.sessions;
    let session = &mut sessions[index];
    session.backend.selected = this.backends.acp_agents.len();
    session.backend.active = Some(Backend::Acp(id.into()));
    // 終了済みターンの履歴(last_sequence/turn_id)をイベント注入で再現する。
    // 新規セッションなので sequence は 1 から始める(先頭欠落は incomplete 扱いになる)。
    let mut seq = Sequencer::new(session.model.id.clone(), 0, "previous-turn".to_owned());
    session.model.apply(seq.next(Event::TurnStarted {
        prompt: "smoke".into(),
    }));
    session.model.apply(seq.next(Event::TurnCompleted {
        reason: "smoke".into(),
        usage: Default::default(),
    }));
    this.start_selected(index, "続きの依頼を保持".into(), cx);
    assert!(this.message.contains("新しいチャンネル"));
    assert!(this.session_at(index).exec.controller.is_none());
    assert_eq!(this.session_at(index).model.status(), Status::Completed);
    assert_eq!(
        this.session_at(index).composer.read(cx).value(cx),
        "続きの依頼を保持"
    );
    this.backends.acp_agents.pop();
    this.close_session(window, cx);
    println!(
        "ACP conversation smoke OK: disconnected conversations preserve their drafts and require a new task"
    );
}

fn login_lifecycle(this: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    this.new_session(window, cx);
    // ツールメニュー経由のログインモーダル。実 worker は起こさず、開始条件と開閉だけ確認する。
    assert!(this.can_start_login());
    this.show_login_dialog(window, cx);
    assert!(window.has_active_dialog(cx));
    window.close_dialog(cx);
    assert!(!window.has_active_dialog(cx));
    {
        let session = this.session_mut();
        session.exec.connecting = true;
        session.login.only = true;
    }
    assert!(!this.can_start_login(), "実行中はログインを開始できない");
    {
        let session = this.session_mut();
        session.exec.connecting = false;
        session.login.only = false;
    }
    assert!(this.can_start_login());
    for (delivery, expected) in [
        (SubscriptionDelivery::Authenticated, Status::Idle),
        (SubscriptionDelivery::LoginCancelled, Status::Idle),
        (
            SubscriptionDelivery::Error("login failed".into()),
            Status::Failed,
        ),
    ] {
        let session = this.session_mut();
        // 「接続中」はドメイン status ではなく UI の表示状態なので exec 側で立てる。
        session.exec.connecting = true;
        session.login.only = true;
        session.exec.stream_generation += 1;
        let id = session.model.id.clone();
        let generation = session.exec.stream_generation;
        this.sync_controls(cx);
        assert!(this.workspace_busy());
        assert!(this.backends.picker.read(cx).disabled);
        this.consume(
            &id,
            generation - 1,
            vec![UiDelivery::Subscription(SubscriptionDelivery::Error(
                "stale stream".into(),
            ))],
            true,
            cx,
        );
        assert!(
            this.session().exec.connecting,
            "stale stream touched the live run"
        );
        this.consume(
            &id,
            generation,
            vec![UiDelivery::Subscription(delivery)],
            true,
            cx,
        );
        let session = this.session();
        assert_eq!(session.model.status(), expected);
        assert_eq!(
            session.backend.active, None,
            "logging in must not bind a new conversation"
        );
        assert!(!session.login.only);
        assert!(!this.workspace_busy());
        assert!(session.composer.read(cx).can_submit);
        assert!(!this.backends.picker.read(cx).disabled);
    }

    for expected in [Status::Cancelling, Status::Completed] {
        {
            let session = this.session_mut();
            let mut seq = Sequencer::new(
                session.model.id.clone(),
                session.model.last_sequence(),
                format!("late-login-{}", session.model.last_sequence() + 1),
            );
            session.model.apply(seq.next(Event::TurnStarted {
                prompt: "smoke".into(),
            }));
            session.model.apply(seq.next(Event::ModelRequestStarted {
                provider: "previous".into(),
                model: "provider".into(),
                request_id: "smoke".into(),
            }));
            if expected == Status::Cancelling {
                session.model.request_cancel();
                // turn を閉じて次のイテレーションが TurnStarted を受け付けるようにする。
                session.model.apply(seq.next(Event::TurnCancelled {
                    reason: "smoke".into(),
                }));
            } else {
                session.model.apply(seq.next(Event::TurnCompleted {
                    reason: "smoke".into(),
                    usage: Default::default(),
                }));
            }
        }
        let id = this.session().model.id.clone();
        let generation = this.session().exec.stream_generation;
        this.consume(
            &id,
            generation,
            vec![UiDelivery::Subscription(SubscriptionDelivery::Login(
                DeviceLogin {
                    verification_url: "about:blank".into(),
                    user_code: "late-code".into(),
                },
            ))],
            false,
            cx,
        );
        let session = this.session();
        assert!(
            session.login.pending.is_none(),
            "late login must not reopen a cancelled or completed task"
        );
        assert_eq!(session.model.provider(), Some("previous / provider"));
    }

    let session = this.session_mut();
    session.backend.active = Some(Backend::Subscription);
    session.exec.connecting = true;
    session.login.only = true;
    let id = session.model.id.clone();
    let generation = session.exec.stream_generation;
    this.consume(
        &id,
        generation,
        vec![UiDelivery::Subscription(
            SubscriptionDelivery::Authenticated,
        )],
        true,
        cx,
    );
    assert_eq!(this.session().backend.active, Some(Backend::Subscription));
    assert!(
        this.backends.picker.read(cx).disabled,
        "login must preserve an existing conversation backend"
    );
    this.scenario(Scenario::Demo, cx);
    assert_eq!(this.session().model.status(), Status::Idle);
    assert_eq!(this.session().backend.active, Some(Backend::Subscription));
    this.close_session(window, cx);
    println!(
        "Login smoke OK: completion, cancellation, failure, stale stream, late login and conversation backend"
    );
}

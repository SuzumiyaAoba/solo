use super::*;
use gpui_kit::component::WindowExt;
use solo::subscription_worker::Delivery as SubscriptionDelivery;
use std::time::Duration;

pub(super) async fn run(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    this.update_in(cx, login_lifecycle).unwrap();
    this.update_in(cx, closed_acp_conversation).unwrap();
    // 実モデルに接続せず、workspace の占有と待機解除を検証する。
    let (blocker, queued) = this
        .update_in(cx, |this, window, cx| {
            this.new_session(window, cx);
            let blocker = this.selected;
            this.sessions[blocker].model.status = Status::Running;
            this.sessions[blocker].backend = Some(Backend::Subscription);
            this.new_session(window, cx);
            let queued = this.selected;
            // 表示用 picker と異なっても、依頼元セッションの実行先を使う。
            this.scenario_picker
                .update(cx, |picker, _| picker.selected = 1 + this.acp_agents.len());
            this.start_selected(
                queued,
                "順番待ちの日本語の依頼🙂\n完了条件を保持".into(),
                cx,
            );
            assert_eq!(
                this.queue.position(&this.sessions[queued].model.id),
                Some(1)
            );
            assert!(this.sessions[queued].composer.read(cx).read_only);
            assert!(!this.sessions[queued].composer.read(cx).can_submit);
            assert!(this.scenario_picker.read(cx).disabled);
            this.cancel_queued(cx);
            assert!(this.queue.is_empty());
            assert_eq!(
                this.sessions[queued].composer.read(cx).value(cx),
                "順番待ちの日本語の依頼🙂\n完了条件を保持"
            );
            assert!(!this.sessions[queued].composer.read(cx).read_only);
            this.fill_prompt("追加の完了条件", window, cx);
            assert!(
                this.sessions[queued]
                    .composer
                    .read(cx)
                    .value(cx)
                    .contains("保持\n\n追加の完了条件")
            );
            this.sessions[queued].selected_backend = 1 + this.acp_agents.len();
            this.enqueue(queued, "順番待ちから開始するデモ".into(), cx);
            let id = this.sessions[blocker].model.id.clone();
            let generation = this.sessions[blocker].stream_generation;
            this.consume(
                &id,
                generation,
                vec![UiDelivery::Subscription(SubscriptionDelivery::Error(
                    "smoke: execution failed".into(),
                ))],
                false,
                cx,
            );
            assert!(this.queue.paused, "a failed agent must pause queued work");
            this.dispatch_queue(cx);
            assert!(!this.sessions[queued].model.status.is_active());
            this.sessions[blocker].model.status = Status::Idle;
            this.sessions[blocker].backend = None;
            this.sessions[blocker].unread_result = false;
            this.queue.paused = false;
            this.select_session(&this.sessions[blocker].model.id.clone(), window, cx);
            this.dispatch_queue(cx);
            assert!(this.sessions[queued].model.status.is_active());
            assert_eq!(this.sessions[queued].backend, Some(Backend::Mock));
            assert_eq!(
                this.scenario_picker.read(cx).selected,
                0,
                "background dispatch changed the foreground picker"
            );
            (blocker, queued)
        })
        .unwrap();
    let started = Instant::now();
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
        let done = this
            .update_in(cx, |this, _, _| {
                !this.sessions[queued].model.status.is_active()
            })
            .unwrap();
        if done {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "queued mock timed out"
        );
    }
    this.update_in(cx, |this, window, cx| {
        assert!(this.sessions[queued].needs_attention());
        assert_eq!(
            this.sessions[queued].last_prompt,
            "順番待ちから開始するデモ"
        );
        this.select_session(&this.sessions[blocker].model.id.clone(), window, cx);
        this.next_attention(window, cx);
        assert_eq!(this.selected, queued);
        assert!(this.sessions[queued].tab == Tab::Overview);
        this.review_next(cx);
        assert!(this.sessions[queued].tab == Tab::Diff);
        let count = this.sessions[queued].model.unreviewed_count();
        assert!(count > 0);
        this.toggle_review(cx);
        assert_eq!(this.sessions[queued].model.unreviewed_count(), count - 1);
        this.toggle_review(cx);
        assert_eq!(this.sessions[queued].model.unreviewed_count(), count);
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
    for (scheme, width, height, tab) in [
        (ColorScheme::Dark, 1240., 840., Tab::Overview),
        (ColorScheme::Light, 820., 620., Tab::Overview),
        (ColorScheme::Light, 820., 620., Tab::Diff),
    ] {
        this.update_in(cx, |this, window, cx| {
            ds::set_theme(scheme, cx);
            window.resize(size(px(width), px(height)));
            this.show_tab(tab, cx);
        })
        .unwrap();
        println!(
            "UX preview: {} / {} / {}",
            scheme.label(),
            width,
            if tab == Tab::Overview {
                "overview"
            } else {
                "review"
            }
        );
        let pause = std::env::var("SOLO_UX_PREVIEW_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(150)
            .min(30_000);
        cx.background_executor()
            .timer(Duration::from_millis(pause))
            .await;
    }
    this.update_in(cx, |this, window, cx| {
        this.close_session(window, cx);
        assert_eq!(this.selected, blocker);
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
    this.acp_agents.push(AgentProfile {
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
    let index = this.selected;
    let session = &mut this.sessions[index];
    session.selected_backend = this.acp_agents.len();
    session.backend = Some(Backend::Acp(id.into()));
    session.model.last_sequence = 4;
    session.model.turn_id = Some("previous-turn".into());
    session.model.status = Status::Completed;
    this.start_selected(index, "続きの依頼を保持".into(), cx);
    assert!(this.message.contains("新しいチャンネル"));
    assert!(this.sessions[index].controller.is_none());
    assert_eq!(this.sessions[index].model.status, Status::Completed);
    assert_eq!(
        this.sessions[index].composer.read(cx).value(cx),
        "続きの依頼を保持"
    );
    this.acp_agents.pop();
    this.close_session(window, cx);
    println!(
        "ACP conversation smoke OK: disconnected conversations preserve their drafts and require a new task"
    );
}

fn login_lifecycle(this: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    this.new_session(window, cx);
    for (delivery, expected) in [
        (SubscriptionDelivery::Authenticated, Status::Idle),
        (SubscriptionDelivery::LoginCancelled, Status::Idle),
        (
            SubscriptionDelivery::Error("login failed".into()),
            Status::Failed,
        ),
    ] {
        let session = &mut this.sessions[this.selected];
        session.model.status = Status::Connecting;
        session.login_only = true;
        session.stream_generation += 1;
        let id = session.model.id.clone();
        let generation = session.stream_generation;
        this.sync_controls(cx);
        assert!(this.workspace_busy());
        assert!(this.scenario_picker.read(cx).disabled);
        this.consume(
            &id,
            generation - 1,
            vec![UiDelivery::Subscription(SubscriptionDelivery::Error(
                "stale stream".into(),
            ))],
            true,
            cx,
        );
        assert_eq!(
            this.sessions[this.selected].model.status,
            Status::Connecting
        );
        this.consume(
            &id,
            generation,
            vec![UiDelivery::Subscription(delivery)],
            true,
            cx,
        );
        let session = &this.sessions[this.selected];
        assert_eq!(session.model.status, expected);
        assert_eq!(
            session.backend, None,
            "logging in must not bind a new conversation"
        );
        assert!(!session.login_only);
        assert!(!this.workspace_busy());
        assert!(session.composer.read(cx).can_submit);
        assert!(!this.scenario_picker.read(cx).disabled);
    }

    for status in [Status::Cancelling, Status::Completed] {
        let session = &mut this.sessions[this.selected];
        session.model.status = status;
        session.model.provider = "previous provider".into();
        let id = session.model.id.clone();
        let generation = session.stream_generation;
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
        let session = &this.sessions[this.selected];
        assert!(
            session.login.is_none(),
            "late login must not reopen a cancelled or completed task"
        );
        assert_eq!(session.model.provider, "previous provider");
    }

    let session = &mut this.sessions[this.selected];
    session.backend = Some(Backend::Subscription);
    session.model.status = Status::Connecting;
    session.login_only = true;
    let id = session.model.id.clone();
    let generation = session.stream_generation;
    this.consume(
        &id,
        generation,
        vec![UiDelivery::Subscription(
            SubscriptionDelivery::Authenticated,
        )],
        true,
        cx,
    );
    assert_eq!(
        this.sessions[this.selected].backend,
        Some(Backend::Subscription)
    );
    assert!(
        this.scenario_picker.read(cx).disabled,
        "login must preserve an existing conversation backend"
    );
    this.scenario(Scenario::Demo, cx);
    assert_eq!(this.sessions[this.selected].model.status, Status::Idle);
    assert_eq!(
        this.sessions[this.selected].backend,
        Some(Backend::Subscription)
    );
    this.close_session(window, cx);
    println!(
        "Login smoke OK: completion, cancellation, failure, stale stream, late login and conversation backend"
    );
}

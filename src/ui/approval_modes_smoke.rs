use super::*;
use solo::auto_approval::{ReviewInput, Verdict};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct FakeReviewer {
    models: Mutex<Vec<String>>,
    release: AtomicBool,
    saw_cancel: AtomicBool,
}
impl Reviewer for FakeReviewer {
    fn review(
        &self,
        settings: &AutoSettings,
        _: &ReviewInput,
        cancel: &CancellationToken,
    ) -> Result<Assessment, String> {
        self.models.lock().unwrap().push(settings.model.clone());
        if settings.model == "slow-model" {
            let start = Instant::now();
            while !self.release.load(Ordering::Acquire) {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "fake review was not released"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            self.saw_cancel
                .store(cancel.is_cancelled(), Ordering::Release);
            // Deliberately return a stale allow even after cancellation.
        }
        if settings.model == "error-model" {
            return Err("模擬タイムアウト".into());
        }
        Ok(Assessment {
            decision: match settings.model.as_str() {
                "deny-model" => Verdict::Deny,
                "ask-model" => Verdict::Ask,
                _ => Verdict::Allow,
            },
            reason: "検証用の判定です".into(),
        })
    }
}
fn config(mode: ApprovalMode, model: &str) -> ApprovalSettings {
    ApprovalSettings {
        mode,
        auto: AutoSettings {
            model: model.into(),
            timeout_seconds: 10,
        },
    }
}
fn inject(
    this: &WeakEntity<Workspace>,
    request: ApprovalRequest,
    cx: &mut AsyncWindowContext,
) -> async_channel::Receiver<bool> {
    this.update_in(cx, |this, _, cx| {
        let (reply, receiver) = async_channel::bounded(1);
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
        receiver
    })
    .unwrap()
}
async fn wait_answer(answer: &async_channel::Receiver<bool>, cx: &mut AsyncWindowContext) -> bool {
    let start = Instant::now();
    loop {
        match answer.try_recv() {
            Ok(answer) => return answer,
            Err(async_channel::TryRecvError::Closed) => {
                panic!("approval reply closed unexpectedly")
            }
            Err(_) => {}
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "approval did not complete"
        );
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
}
async fn wait_manual(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    let start = Instant::now();
    loop {
        if this
            .update_in(cx, |this, _, _| {
                this.sessions[this.selected]
                    .approval
                    .as_ref()
                    .is_some_and(|request| request.auto_settings.is_none())
            })
            .unwrap()
        {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "manual fallback did not appear"
        );
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
}

pub(super) async fn check(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    let fake = Arc::new(FakeReviewer::default());
    let (store, request, original_reviewer) = this
        .update_in(cx, |this, window, cx| {
            let old = this.approval_reviewer.clone();
            this.approval_reviewer = fake.clone();
            this.new_session(window, cx);
            this.sessions[this.selected].model.status = Status::Running;
            this.sessions[this.selected].last_prompt = "テストを実行してください".into();
            let request = ApprovalRequest::tool(
                &solo::harness::ToolCall {
                    id: "mode-smoke".into(),
                    name: "exec".into(),
                    arguments: serde_json::json!({"command":"cargo test --locked"}),
                },
                std::path::Path::new(&this.workspace_path),
            );
            (this.command_rules.as_ref().unwrap().clone(), request, old)
        })
        .unwrap();
    let cfg = store.config_store().unwrap().clone();
    let command = request.command.clone().unwrap();
    store.add(RuleList::Blacklist, command.clone()).unwrap();
    cfg.set_approval(config(ApprovalMode::Bypass, "")).unwrap();
    assert!(wait_answer(&inject(this, request.clone(), cx), cx).await);
    assert!(
        fake.models.lock().unwrap().is_empty(),
        "bypass called the model"
    );
    cfg.set_approval(config(ApprovalMode::Auto, "allow-model"))
        .unwrap();
    assert!(!wait_answer(&inject(this, request.clone(), cx), cx).await);
    assert!(
        fake.models.lock().unwrap().is_empty(),
        "blacklist called the model"
    );
    store.remove(RuleList::Blacklist, &command).unwrap();
    for (name, accepted) in [("allow-model", true), ("deny-model", false)] {
        cfg.set_approval(config(ApprovalMode::Auto, name)).unwrap();
        assert_eq!(
            wait_answer(&inject(this, request.clone(), cx), cx).await,
            accepted
        );
        assert_eq!(fake.models.lock().unwrap().last().unwrap(), name);
    }
    for name in ["ask-model", "error-model"] {
        cfg.set_approval(config(ApprovalMode::Auto, name)).unwrap();
        let answer = inject(this, request.clone(), cx);
        wait_manual(this, cx).await;
        assert!(matches!(
            answer.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        ));
        this.update_in(cx, |this, _, cx| {
            assert!(
                this.sessions[this.selected]
                    .approval
                    .as_ref()
                    .unwrap()
                    .review_note
                    .is_some()
            );
            this.answer_approval(false, cx);
        })
        .unwrap();
        assert!(!wait_answer(&answer, cx).await);
    }

    // Changing settings while a slow review is in progress cancels it and invalidates its result.
    cfg.set_approval(config(ApprovalMode::Auto, "slow-model"))
        .unwrap();
    let answer = inject(this, request.clone(), cx);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    cfg.set_approval(config(ApprovalMode::Manual, "slow-model"))
        .unwrap();
    this.update_in(cx, |this, _, cx| this.reconsider_approvals(cx))
        .unwrap();
    fake.release.store(true, Ordering::Release);
    cx.background_executor()
        .timer(Duration::from_millis(120))
        .await;
    assert!(fake.saw_cancel.load(Ordering::Acquire));
    assert!(
        matches!(answer.try_recv(), Err(async_channel::TryRecvError::Empty)),
        "stale model allowed execution"
    );
    this.update_in(cx, |this, _, cx| this.answer_approval(false, cx))
        .unwrap();
    assert!(!wait_answer(&answer, cx).await);

    // A blacklist written without a UI notification still wins when the model finishes.
    fake.release.store(false, Ordering::Release);
    cfg.set_approval(config(ApprovalMode::Auto, "slow-model"))
        .unwrap();
    let answer = inject(this, request.clone(), cx);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    store.add(RuleList::Blacklist, command.clone()).unwrap();
    fake.release.store(true, Ordering::Release);
    assert!(!wait_answer(&answer, cx).await);
    store.remove(RuleList::Blacklist, &command).unwrap();

    // Closing the session cancels review and closes the backend approval channel.
    fake.release.store(false, Ordering::Release);
    let answer = inject(this, request, cx);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    this.update_in(cx, |this, window, cx| this.close_session(window, cx))
        .unwrap();
    assert!(matches!(
        answer.try_recv(),
        Err(async_channel::TryRecvError::Closed)
    ));
    fake.release.store(true, Ordering::Release);
    this.update_in(cx, |this, _, cx| {
        command_rules::settings_smoke(&this.rule_editor, cx)
    })
    .unwrap();
    cfg.set_approval(config(ApprovalMode::Auto, "allow-model"))
        .unwrap();
    this.update_in(cx, |this, window, cx| {
        this.rule_editor.update(cx, |editor, cx| {
            editor.show_settings = true;
            cx.notify();
        });
        this.open_command_rules(window, cx);
    })
    .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(200))
        .await;
    println!("Approval modes UI ready: selected model settings");
    super::smoke::approval_preview_pause(cx).await;
    this.update_in(cx, |this, window, cx| {
        use gpui_kit::component::WindowExt;
        window.close_sheet(cx);
        this.rule_editor.update(cx, |editor, cx| {
            editor.show_settings = false;
            cx.notify();
        });
    })
    .unwrap();
    cfg.set_approval(ApprovalSettings::default()).unwrap();
    this.update_in(cx, |this, _, cx| {
        this.approval_reviewer = original_reviewer;
        this.reconsider_approvals(cx);
    })
    .unwrap();
    println!(
        "Approval modes smoke OK: bypass, blacklist priority, configured model, allow/deny/ask/error, changed settings, stale results, close/cancel; no real model requests"
    );
}

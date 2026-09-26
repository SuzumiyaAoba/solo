//! ワーカーからの受信をフレーム単位でまとめ、表示と実行待ちキューへ反映する。
use super::*;
use solo::{
    acp_worker::Delivery as AcpDelivery,
    mock::{Delivery as MockDelivery, FRAME_BATCH, FRAME_INTERVAL},
    subscription_worker::Delivery as SubscriptionDelivery,
};

pub(super) enum UiDelivery {
    Mock(MockDelivery),
    Subscription(SubscriptionDelivery),
    Acp(AcpDelivery),
}

impl SessionView {
    fn finish_login(&mut self) {
        self.login = None;
        self.login_only = false;
        self.controller = None;
    }

    fn apply_delivery(
        &mut self,
        delivery: UiDelivery,
        cx: &mut App,
    ) -> Option<(ApprovalRequest, async_channel::Sender<bool>)> {
        match delivery {
            UiDelivery::Mock(MockDelivery::Event(event))
            | UiDelivery::Subscription(SubscriptionDelivery::Event(event))
            | UiDelivery::Acp(AcpDelivery::Event(event)) => {
                self.model.apply(event);
            }
            UiDelivery::Mock(MockDelivery::LogOpened(path)) => self.artifacts.push(path),
            UiDelivery::Subscription(SubscriptionDelivery::History(messages)) => {
                self.history = messages;
                self.login = None;
            }
            UiDelivery::Subscription(SubscriptionDelivery::Login(login)) => {
                if self.model.status.is_active() && self.model.status != Status::Cancelling {
                    cx.write_to_clipboard(ClipboardItem::new_string(login.user_code.clone()));
                    cx.open_url(&login.verification_url);
                    self.login = Some(login);
                    self.model.provider = "Codex / ChatGPT ログイン待ち".into();
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::Authenticated) => {
                if self.login_only {
                    self.model.status = Status::Idle;
                    self.model.reason.clear();
                    self.model.provider = "ChatGPT ログイン済み".into();
                    self.finish_login();
                } else {
                    self.login = None;
                    self.model.provider = "OpenAI Codex".into();
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::LoginCancelled) => {
                if self.login_only {
                    self.model.status = Status::Idle;
                    self.model.provider = "OpenAI Codex".into();
                    self.finish_login();
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::Approval { request, reply })
            | UiDelivery::Acp(AcpDelivery::Approval { request, reply }) => {
                return Some((*request, reply));
            }

            UiDelivery::Acp(AcpDelivery::SessionId(_)) => {}
            UiDelivery::Mock(MockDelivery::Error(error))
            | UiDelivery::Subscription(SubscriptionDelivery::Error(error)) => {
                self.model.transport_failed(error);
                if self.login_only {
                    self.finish_login();
                }
            }
            UiDelivery::Acp(AcpDelivery::Error(error)) => {
                self.model.transport_failed(error);
                self.controller = None;
            }
        }
        None
    }
}

impl Workspace {
    pub(super) fn listen<D: Send + 'static>(
        receiver: async_channel::Receiver<D>,
        id: String,
        generation: u64,
        wrap: fn(D) -> UiDelivery,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let first = match receiver.recv().await {
                    Ok(event) => event,
                    Err(_) => {
                        let _ = this.update(cx, |this, cx| {
                            this.consume(&id, generation, Vec::new(), true, cx)
                        });
                        break;
                    }
                };
                // 空のときは recv で待つ。入力や描画を阻害せず、delta をフレーム単位でまとめる。
                cx.background_executor().timer(FRAME_INTERVAL).await;
                let mut batch = Vec::with_capacity(FRAME_BATCH);
                batch.push(wrap(first));
                while batch.len() < FRAME_BATCH {
                    match receiver.try_recv() {
                        Ok(event) => batch.push(wrap(event)),
                        Err(_) => break,
                    }
                }
                if this
                    .update(cx, |this, cx| {
                        this.consume(&id, generation, batch, false, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    pub(super) fn consume(
        &mut self,
        id: &str,
        generation: u64,
        batch: Vec<UiDelivery>,
        closed: bool,
        cx: &mut Context<Self>,
    ) {
        let mut approval_requests = Vec::new();
        let background = !self.is_visible || self.sessions[self.selected].model.id != id;
        let project_name = self.workspace_name.clone();
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.model.id == id && session.stream_generation == generation)
        else {
            return;
        };
        let start = Instant::now();
        // 会話は末尾への追記と先頭の破棄で変わるため、行数と末尾の長さで更新を判断する。
        let old_len = session.model.chat.len();
        let old_discarded = session.model.chat_discarded;
        let old_tail_bytes = session.model.chat.back().map(|block| block.text.len());
        let old_logs = (session.model.logs.len(), session.model.logs_discarded);
        let old_thread_revision = session.model.thread_revision;
        let was_active = session.model.status.is_active();
        for delivery in batch {
            if let Some(request) = session.apply_delivery(delivery, cx) {
                approval_requests.push(request);
            }
        }
        if closed {
            session.model.transport_closed();
            if matches!(session.backend, Some(Backend::Acp(_))) {
                session.controller = None;
            }
        }
        if !session.model.status.is_active() {
            session.login = None;
            session.approval = None;
            if was_active && session.model.status != Status::Idle {
                session.elapsed = session.started_at.map(|at| at.elapsed());
                session.unread_result = true;
                if session.uses_workspace()
                    && matches!(
                        session.model.status,
                        Status::Failed | Status::Disconnected | Status::Cancelled
                    )
                {
                    self.queue.paused = true;
                }
                if background {
                    self.toast.update(cx, |toast, cx| {
                        toast.push(
                            format!(
                                "{project_name} · {} · {}",
                                session.model.title,
                                session.model.status.label()
                            ),
                            if session.model.status == Status::Completed {
                                Tone::Success
                            } else {
                                Tone::Warning
                            },
                            cx,
                        )
                    });
                }
            }
        }
        let discarded = session.model.chat_discarded - old_discarded;
        if discarded > 0
            || old_len != session.model.chat.len()
            || old_tail_bytes != session.model.chat.back().map(|block| block.text.len())
        {
            if discarded > 0 {
                session
                    .chat_list
                    .update(cx, |list, cx| list.splice(0..discarded.min(old_len), 0, cx));
            }
            let old_len = old_len.saturating_sub(discarded);
            let from = old_len.saturating_sub(1);
            session.chat_list.update(cx, |list, cx| {
                list.splice(from..old_len, session.model.chat.len() - from, cx)
            });
        }
        if old_thread_revision != session.model.thread_revision
            && let Some(thread) = session.model.threads.back()
            && let Some(row) = session.model.chat.iter().position(|block| {
                block.speaker == Speaker::User && block.thread_id == Some(thread.id)
            })
        {
            session
                .chat_list
                .update(cx, |list, cx| list.splice(row..row + 1, 1, cx));
        }
        if session.follow_logs
            && !session.model.logs.is_empty()
            && old_logs != (session.model.logs.len(), session.model.logs_discarded)
        {
            session
                .log_scroll
                .scroll_to_item(session.model.logs.len() - 1, ScrollStrategy::Bottom);
        }
        let can_submit =
            !session.model.status.is_active() && self.queue.position(&session.model.id).is_none();
        if session.composer.read(cx).can_submit != can_submit {
            session.composer.update(cx, |input, cx| {
                input.can_submit = can_submit;
                cx.notify();
            });
        }
        session.batches += 1;
        session.max_batch_ms = session
            .max_batch_ms
            .max(start.elapsed().as_secs_f64() * 1000.);
        self.sync_controls(cx);
        self.dispatch_queue(cx);
        for (request, reply) in approval_requests {
            self.receive_approval(id, generation, request, reply, cx);
        }
        cx.notify();
    }
}

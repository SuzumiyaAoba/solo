//! ワーカーからの受信をフレーム単位でまとめ、表示と実行待ちキューへ反映する。
use super::*;
use solo::{
    acp_worker::Delivery as AcpDelivery, codex_worker::Delivery as SubscriptionDelivery,
    mock::Delivery as MockDelivery,
};

/// UI スレッドでのバッチ取り込み間隔と1バッチ上限。
const FRAME_BATCH: usize = 128;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub(super) enum UiDelivery {
    Mock(MockDelivery),
    Subscription(SubscriptionDelivery),
    Acp(AcpDelivery),
}

impl SessionView {
    fn finish_login(&mut self) {
        self.login = None;
        self.login_only = false;
        self.exec.controller = None;
        self.exec.connecting = false;
    }

    fn apply_delivery(
        &mut self,
        delivery: UiDelivery,
        store: Option<&WorkspaceStore>,
        cx: &mut App,
    ) -> Option<(ApprovalRequest, async_channel::Sender<ApprovalReply>)> {
        match delivery {
            UiDelivery::Mock(MockDelivery::Event(event))
            | UiDelivery::Subscription(SubscriptionDelivery::Event(event))
            | UiDelivery::Acp(AcpDelivery::Event(event)) => {
                // 最初のイベントで接続中表示を終える(受理・拒否にかかわらず届いた事実で降ろす)。
                self.exec.connecting = false;
                if let Some(store) = store {
                    self.persist_event(store, &event);
                }
                self.model.apply(event);
                self.sync_meta();
            }
            UiDelivery::Mock(MockDelivery::LogOpened(path)) => self.artifacts.push(path),
            UiDelivery::Subscription(SubscriptionDelivery::History(messages)) => {
                self.history = messages;
                self.login = None;
                if let Some(store) = store {
                    self.save_history(store);
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::Login(login)) => {
                if self.display_status().is_active() && self.model.status() != Status::Cancelling {
                    cx.write_to_clipboard(ClipboardItem::new_string(login.user_code.clone()));
                    cx.open_url(&login.verification_url);
                    self.login = Some(login);
                    self.exec.provider_hint = Some("Codex / ChatGPT ログイン待ち".into());
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::Authenticated) => {
                if self.login_only {
                    self.exec.provider_hint = Some("ChatGPT ログイン済み".into());
                    self.model.reset_idle();
                    self.finish_login();
                } else {
                    self.login = None;
                    self.exec.provider_hint = Some("OpenAI Codex".into());
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::LoginCancelled) => {
                if self.login_only {
                    self.exec.provider_hint = Some("OpenAI Codex".into());
                    self.model.reset_idle();
                    self.finish_login();
                }
            }
            UiDelivery::Subscription(SubscriptionDelivery::Approval { request, reply })
            | UiDelivery::Acp(AcpDelivery::Approval { request, reply }) => {
                return Some((*request, reply));
            }

            UiDelivery::Mock(MockDelivery::Error(error))
            | UiDelivery::Subscription(SubscriptionDelivery::Error(error)) => {
                self.model.transport_failed(error);
                self.exec.connecting = false;
                if self.login_only {
                    self.finish_login();
                }
            }
            UiDelivery::Acp(AcpDelivery::Error(error)) => {
                self.model.transport_failed(error);
                self.exec.connecting = false;
                self.exec.controller = None;
            }
        }
        None
    }

    /// 会話差分を行 scroller へ反映し、ログの follow 位置を進める。
    /// consume から抜き出した差分スクロール計算。
    fn update_scrollers(
        &mut self,
        old_len: usize,
        old_discarded: usize,
        old_tail_bytes: Option<usize>,
        old_logs: (usize, usize),
        old_thread_revision: u64,
        cx: &mut App,
    ) {
        let discarded = self.model.chat_discarded() - old_discarded;
        if discarded > 0
            || old_len != self.model.chat().len()
            || old_tail_bytes != self.model.chat().back().map(|block| block.text.len())
        {
            if discarded > 0 {
                self.chat_list
                    .update(cx, |list, cx| list.splice(0..discarded.min(old_len), 0, cx));
            }
            let old_len = old_len.saturating_sub(discarded);
            let mut from = old_len.saturating_sub(1);
            // 追記で伸びたブロックがランの途中なら、全文を描くラン先頭の行から測り直す。
            // 数十万行の 1 ランでは走査も再測定も嵩むため、先頭の探索は直近の行に限る。
            // 遠い先頭行のキャッシュ高は、次に描画されるタイミングで直る。
            for _ in 0..16 {
                if from == 0 || views::chat::is_run_head(self.model.chat(), from) {
                    break;
                }
                from -= 1;
            }
            self.chat_list.update(cx, |list, cx| {
                list.splice(from..old_len, self.model.chat().len() - from, cx)
            });
        }
        if old_thread_revision != self.model.thread_revision()
            && let Some(thread) = self.model.threads().back()
            && let Some(row) = self.model.chat().iter().position(|block| {
                block.speaker == Speaker::User && block.thread_id == Some(thread.id)
            })
        {
            self.chat_list
                .update(cx, |list, cx| list.splice(row..row + 1, 1, cx));
        }
        if old_logs != (self.model.logs().len(), self.model.logs_discarded()) {
            // 先頭の破棄で選択中の行が範囲外へ出たら詳細パネルを畳む。
            if let Some(global) = self.view.log_selected {
                let index = global.checked_sub(self.model.logs_discarded());
                if index.is_none_or(|index| index >= self.model.logs().len()) {
                    self.view.log_selected = None;
                }
            }
            // 追従中はフィルタ適用後の末尾へ進める(フィルタ外の追記では見た目上動かない)。
            if self.view.follow_logs && !self.model.logs().is_empty() {
                let tail = views::logs::filtered_indices(
                    self.model.logs(),
                    &self.view.log_filter,
                    self.view.log_level.as_deref(),
                )
                .len();
                if tail > 0 {
                    self.view
                        .log_scroll
                        .scroll_to_item(tail - 1, ScrollStrategy::Bottom);
                }
            }
        }
    }
}

/// consume の前半で確定する共有値。スクロール差分と完了遷移の判定材料。
struct IngestOutcome {
    approval_requests: Vec<(ApprovalRequest, async_channel::Sender<ApprovalReply>)>,
    was_active: bool,
    queue_changed: bool,
    /// バッチ処理の開始時刻。max_batch_ms の計測範囲は従来通り後半の反映まで含む。
    start: Instant,
    old_len: usize,
    old_discarded: usize,
    old_tail_bytes: Option<usize>,
    old_logs: (usize, usize),
    old_thread_revision: u64,
}

impl Workspace {
    pub(super) fn listen<D: Send + 'static>(
        receiver: async_channel::Receiver<D>,
        id: SessionId,
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
        id: &SessionId,
        generation: u64,
        batch: Vec<UiDelivery>,
        closed: bool,
        cx: &mut Context<Self>,
    ) {
        let background = !self.is_visible || self.session().model.id != *id;
        let Some(session_index) = self
            .session_index(id)
            .filter(|&index| self.session_at(index).exec.stream_generation == generation)
        else {
            return;
        };
        let outcome = self.ingest_batch(session_index, batch, closed, background, cx);
        // session 借用中も self.queue を読むため、キュー位置は先に取る
        // (update_scrollers は queue に触れない)。
        let queued = self
            .queue
            .position(&self.session_at(session_index).model.id)
            .is_some();
        let session = self.session_at_mut(session_index);
        session.update_scrollers(
            outcome.old_len,
            outcome.old_discarded,
            outcome.old_tail_bytes,
            outcome.old_logs,
            outcome.old_thread_revision,
            cx,
        );
        let can_submit = !session.display_status().is_active() && !queued;
        if session.composer.read(cx).can_submit != can_submit {
            session.composer.update(cx, |input, cx| {
                input.can_submit = can_submit;
                cx.notify();
            });
        }
        session.metrics.batches += 1;
        session.metrics.max_batch_ms = session
            .metrics
            .max_batch_ms
            .max(outcome.start.elapsed().as_secs_f64() * 1000.);
        // 状態遷移(unread_result・store_capped など)を反映した meta を確定する。
        if outcome.was_active != session.model.status().is_active() || outcome.queue_changed {
            self.save_session_meta(session_index, cx);
        }
        self.drain_persist_error(session_index);
        if outcome.queue_changed {
            self.save_workspace_state(cx);
        }
        self.sync_controls(cx);
        self.dispatch_queue(cx);
        for (request, reply) in outcome.approval_requests {
            self.receive_approval(id, generation, request, reply, cx);
        }
        cx.notify();
    }

    /// 受信バッチを model へ畳み込み、ストアの flush・transport 終了・実行完了遷移まで行う。
    fn ingest_batch(
        &mut self,
        session_index: usize,
        batch: Vec<UiDelivery>,
        closed: bool,
        background: bool,
        cx: &mut Context<Self>,
    ) -> IngestOutcome {
        let mut approval_requests = Vec::new();
        let store = self.store.clone();
        let session = self.session_at_mut(session_index);
        let start = Instant::now();
        // 会話は末尾への追記と先頭の破棄で変わるため、行数と末尾の長さで更新を判断する。
        let old_len = session.model.chat().len();
        let old_discarded = session.model.chat_discarded();
        let old_tail_bytes = session.model.chat().back().map(|block| block.text.len());
        let old_logs = (session.model.logs().len(), session.model.logs_discarded());
        let old_thread_revision = session.model.thread_revision();
        let was_active = session.model.status().is_active();
        for delivery in batch {
            if let Some(request) = session.apply_delivery(delivery, store.as_ref(), cx) {
                approval_requests.push(request);
            }
        }
        // バッチの末尾で1回だけ fsync。書き込みが無い場合は何もしない。
        // capped の通知は meta.store_capped が sync_meta で上書きされるため初回のみ発火する。
        // file の借用を畳んでから session 全体の可変借用(record_persist_error)に入る。
        let (flush_error, capped) = match session.persist.file.as_mut() {
            Some(file) => (file.flush(true).err(), file.capped()),
            None => (None, false),
        };
        if let Some(error) = flush_error {
            session.record_persist_error("セッション", error);
        }
        if capped && !session.persist.meta.store_capped {
            session
                .model
                .flag_incomplete("イベント保存が上限に達しました。以降のイベントは復元対象外です");
        }
        if closed {
            session.model.transport_closed();
            // TurnStarted 前に死んだ worker(Connecting 中)も「結果未確認」に落とす。
            if session.exec.connecting {
                session
                    .model
                    .disconnect_pending("完了イベントを受信する前に接続が閉じました");
            }
            session.exec.connecting = false;
            if matches!(session.backend, Some(Backend::Acp(_))) {
                session.exec.controller = None;
            }
        }
        let queue_changed = self.finish_run(session_index, was_active, background, cx);
        IngestOutcome {
            approval_requests,
            was_active,
            queue_changed,
            start,
            old_len,
            old_discarded,
            old_tail_bytes,
            old_logs,
            old_thread_revision,
        }
    }

    /// 実行が非 active へ遷移したときの後処理: 経過時間・未読・meta・キュー停止・トースト。
    /// queue.paused を触った場合だけ true を返し、呼出側の workspace 保存へ繋げる。
    fn finish_run(
        &mut self,
        session_index: usize,
        was_active: bool,
        background: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        // toast のクロージャが session を借りると self.toast/self.queue との借用が
        // 衝突するため、表示と分岐に使う値を先に取り出して借用を終える。
        let session = self.session_at_mut(session_index);
        if session.display_status().is_active() {
            return false;
        }
        session.login = None;
        session.approval = None;
        session.exec.connecting = false;
        if !was_active || session.model.status() == Status::Idle {
            return false;
        }
        session.metrics.elapsed = session.metrics.started_at.map(|at| at.elapsed());
        session.unread_result = true;
        session.sync_meta();
        let title = session.model.title().to_owned();
        let status = session.model.status();
        let mut queue_changed = false;
        if session.uses_workspace()
            && matches!(
                status,
                Status::Failed | Status::Disconnected | Status::Cancelled
            )
        {
            queue_changed = !self.queue.paused();
            self.queue.set_paused(true);
        }
        if background {
            let project_name = self.workspace_name.clone();
            self.toast.update(cx, |toast, cx| {
                toast.push(
                    format!("{project_name} · {title} · {}", status.label()),
                    if status == Status::Completed {
                        Tone::Success
                    } else {
                        Tone::Warning
                    },
                    cx,
                )
            });
        }
        queue_changed
    }
}

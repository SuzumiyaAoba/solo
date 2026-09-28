use super::*;
use gpui_kit::component::{WindowExt, dialog::DialogButtonProps};

impl SessionView {
    pub(super) fn needs_attention(&self) -> bool {
        self.approval.is_some()
            || self.login.is_some()
            || (!self.display_status().is_active()
                && (self.unread_result || self.model.unreviewed_count() > 0))
    }

    /// 「接続中」はドメインの Status ではなく UI の表示状態のため、表示用に合成する。
    /// connecting は worker 起動から最初の受理イベントまでを示し、apply_delivery が
    /// 降ろす(旧 Status::Connecting 相当)。切断経路は stream.rs の closed 分岐が閉じる。
    /// 暫定で接続中は Running を返す(表示色は views::status_tone 側)。
    pub(super) fn display_status(&self) -> Status {
        if self.exec.connecting {
            Status::Running
        } else {
            self.model.status()
        }
    }

    /// model.provider() を優先し、無ければ実行前に立てたヒントを返す。
    pub(super) fn provider_display(&self) -> Option<&str> {
        self.model.provider().or(self.exec.provider_hint.as_deref())
    }

    /// 順番待ち中の composer を依頼文で固定する(送信不可・変更不可)。
    pub(super) fn lock_composer(&mut self, prompt: impl Into<SharedString>, cx: &mut App) {
        self.composer.update(cx, |input, cx| {
            input.set_value(prompt, cx);
            input.can_submit = false;
            input.read_only = true;
        });
    }

    /// lock_composer の解除。下書きを残したまま編集可能に戻す。
    pub(super) fn unlock_composer(&mut self, prompt: impl Into<SharedString>, cx: &mut App) {
        self.composer.update(cx, |input, cx| {
            input.read_only = false;
            input.can_submit = true;
            input.set_value(prompt, cx);
        });
    }

    pub(super) fn state_label(&self) -> &'static str {
        let status = self.display_status();
        if self.approval.is_some() {
            "承認待ち"
        } else if self.login.is_some() {
            "ログイン待ち"
        } else if self.exec.connecting {
            "接続中"
        } else if status == Status::Completed && self.model.unreviewed_count() > 0 {
            "レビュー待ち"
        } else {
            status.label()
        }
    }

    pub(super) fn state_tone(&self) -> Tone {
        if self.approval.is_some() || self.login.is_some() {
            Tone::Warning
        } else {
            super::views::status_tone(self.display_status())
        }
    }
}

impl Workspace {
    pub(super) fn workspace_busy(&self) -> bool {
        self.sessions.iter().any(|session| {
            (session.model.status().is_active() || session.exec.connecting)
                && session.uses_workspace()
        })
    }

    pub(super) fn enqueue(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        // キューに載せる前にストアを確定させ、復元時に同じ ID で紐づくようにする。
        if let Some(store) = self.store.clone()
            && let Err(error) = self.session_at_mut(index).open_or_create(&store)
        {
            self.report(format!("セッションを保存できません: {error}"));
        }
        // queue.push は &mut self.queue を取るため、選択 index→BackendKind は先に確定させる。
        let backend = self
            .backend_kind_at(self.sessions[index].selected_backend)
            .unwrap_or(BackendKind::Subscription);
        let sessions = &mut self.sessions;
        let session = &mut sessions[index];
        if self.queue.push(QueuedRun {
            session_id: session.model.id.clone(),
            backend,
            prompt: prompt.clone(),
        }) {
            session.lock_composer(prompt, cx);
            session.view.tab = Tab::Overview;
            self.message.clear();
            self.save_workspace_state(cx);
            self.sync_controls(cx);
            self.dispatch_queue(cx);
            cx.notify();
        }
    }

    pub(super) fn cancel_queued(&mut self, cx: &mut Context<Self>) {
        let id = self.session().model.id.clone();
        if let Some(run) = self.queue.cancel(&id) {
            self.session_mut().unlock_composer(run.prompt, cx);
            self.save_workspace_state(cx);
            self.sync_controls(cx);
            cx.notify();
        }
    }

    pub(super) fn dispatch_queue(&mut self, cx: &mut Context<Self>) {
        while let Some(run) = self.queue.next(self.workspace_busy()) {
            let Some(index) = self.session_index(&run.session_id) else {
                continue;
            };
            self.session_at_mut(index).unlock_composer("", cx);
            self.run_backend(index, &run.backend, run.prompt, cx);
            if !self.session_at(index).display_status().is_active() {
                self.queue.set_paused(true);
            }
            // 取り出した実行と一時停止を保存する(連続 dispatch でも最終状態を残す)。
            self.save_workspace_state(cx);
            self.sync_controls(cx);
            break;
        }
    }

    pub(super) fn next_attention(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = (1..=self.sessions.len())
            .map(|offset| (self.selected + offset) % self.sessions.len())
            .find(|&index| self.session_at(index).needs_attention());
        if let Some(index) = next {
            let id = self.session_at(index).model.id.clone();
            self.select_session(&id, window, cx);
            self.show_tab(Tab::Overview, cx);
        }
    }

    pub(super) fn review_next(&mut self, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if let Some(index) = session.model.diffs().iter().position(|diff| !diff.reviewed) {
            session.select_diff(index);
        }
        self.show_tab(Tab::Diff, cx);
    }

    pub(super) fn toggle_review(&mut self, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if session.model.toggle_reviewed(session.view.diff_index) {
            if session.model.unreviewed_count() == 0 {
                session.unread_result = false;
            }
            session.persist.meta.reviewed = session
                .model
                .diffs()
                .iter()
                .filter(|diff| diff.reviewed)
                .map(|diff| diff.path.clone())
                .collect();
            self.save_session_meta(self.selected, cx);
            cx.notify();
        }
    }

    pub(super) fn fill_prompt(
        &mut self,
        prompt: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session();
        if self.queue.position(&session.model.id).is_some() {
            return;
        }
        let draft = session.composer.read(cx).value(cx);
        let value = if draft.trim().is_empty() {
            prompt.to_owned()
        } else {
            format!("{draft}\n\n{prompt}")
        };
        session
            .composer
            .update(cx, |input, cx| input.set_value(value, cx));
        window.focus(&session.composer.focus_handle(cx), cx);
        cx.notify();
    }

    pub(super) fn request_close_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session();
        if session.model.chat().is_empty()
            && !session.display_status().is_active()
            && session.composer.read(cx).value(cx).is_empty()
            && self.queue.position(&session.model.id).is_none()
        {
            self.close_session(window, cx);
            return;
        }
        let id = session.model.id.clone();
        let title = session.model.title().to_owned();
        let p = ds::theme(cx);
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let id = id.clone();
            dialog
                .title("チャンネルを閉じる")
                .w(px(440.))
                .child(div().font_weight(FontWeight::MEDIUM).child(title.clone()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Trash.view(p.danger))
                        .child("会話・下書き・保存済みの履歴と全文ログを削除"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Square.view(p.warning))
                        .child("実行・順番待ちを中止"),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("閉じる")
                        .cancel_text("戻る")
                        .show_cancel(true)
                        .ok_variant(gpui_kit::component::button::ButtonVariant::Danger),
                )
                .on_ok(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if let Some(index) = this.session_index(&id) {
                            this.selected = index;
                            this.close_session(window, cx);
                        }
                    });
                    true
                })
        });
    }
}

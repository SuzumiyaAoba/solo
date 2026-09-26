use super::*;
use gpui_kit::component::{WindowExt, dialog::DialogButtonProps};

impl SessionView {
    pub(super) fn needs_attention(&self) -> bool {
        self.approval.is_some()
            || self.login.is_some()
            || (!self.model.status.is_active()
                && (self.unread_result || self.model.unreviewed_count() > 0))
    }

    pub(super) fn state_label(&self) -> &'static str {
        if self.approval.is_some() {
            "承認待ち"
        } else if self.login.is_some() {
            "ログイン待ち"
        } else if self.model.status == Status::Completed && self.model.unreviewed_count() > 0 {
            "レビュー待ち"
        } else {
            self.model.status.label()
        }
    }

    pub(super) fn state_tone(&self) -> Tone {
        if self.approval.is_some() || self.login.is_some() {
            Tone::Warning
        } else {
            super::views::status_tone(self.model.status)
        }
    }
}

impl Workspace {
    pub(super) fn workspace_busy(&self) -> bool {
        self.sessions
            .iter()
            .any(|session| session.model.status.is_active() && session.uses_workspace())
    }

    pub(super) fn enqueue(&mut self, index: usize, prompt: String, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        if self.queue.push(QueuedRun {
            session_id: session.model.id.clone(),
            backend: session.selected_backend,
            prompt: prompt.clone(),
        }) {
            session.composer.update(cx, |input, cx| {
                input.set_value(prompt, cx);
                input.can_submit = false;
                input.read_only = true;
            });
            session.tab = Tab::Overview;
            self.message.clear();
            self.sync_controls(cx);
            self.dispatch_queue(cx);
            cx.notify();
        }
    }

    pub(super) fn cancel_queued(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if let Some(run) = self.queue.cancel(&session.model.id) {
            session.composer.update(cx, |input, cx| {
                input.read_only = false;
                input.can_submit = true;
                input.set_value(run.prompt, cx);
            });
            self.sync_controls(cx);
            cx.notify();
        }
    }

    pub(super) fn dispatch_queue(&mut self, cx: &mut Context<Self>) {
        while let Some(run) = self.queue.next(self.workspace_busy()) {
            let Some(index) = self
                .sessions
                .iter()
                .position(|s| s.model.id == run.session_id)
            else {
                continue;
            };
            self.sessions[index].composer.update(cx, |input, cx| {
                input.read_only = false;
                input.can_submit = true;
                input.set_value("", cx);
            });
            self.run_backend(index, run.backend, run.prompt, cx);
            if !self.sessions[index].model.status.is_active() {
                self.queue.paused = true;
            }
            self.sync_controls(cx);
            break;
        }
    }

    pub(super) fn next_attention(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = (1..=self.sessions.len())
            .map(|offset| (self.selected + offset) % self.sessions.len())
            .find(|&index| self.sessions[index].needs_attention());
        if let Some(index) = next {
            let id = self.sessions[index].model.id.clone();
            self.select_session(&id, window, cx);
            self.show_tab(Tab::Overview, cx);
        }
    }

    pub(super) fn review_next(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if let Some(index) = session.model.diffs.iter().position(|diff| !diff.reviewed) {
            session.diff_index = index;
            session.diff_scroll = UniformListScrollHandle::new();
        }
        self.show_tab(Tab::Diff, cx);
    }

    pub(super) fn toggle_review(&mut self, cx: &mut Context<Self>) {
        let session = &mut self.sessions[self.selected];
        if session.model.toggle_reviewed(session.diff_index) {
            if session.model.unreviewed_count() == 0 {
                session.unread_result = false;
            }
            cx.notify();
        }
    }

    pub(super) fn fill_prompt(
        &mut self,
        prompt: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = &self.sessions[self.selected];
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
        let session = &self.sessions[self.selected];
        if session.model.chat.is_empty()
            && !session.model.status.is_active()
            && session.composer.read(cx).value(cx).is_empty()
            && self.queue.position(&session.model.id).is_none()
        {
            self.close_session(window, cx);
            return;
        }
        let id = session.model.id.clone();
        let title = session.model.title.clone();
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
                        .child("会話・下書き・一時ログを削除"),
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
                        if let Some(index) = this.sessions.iter().position(|s| s.model.id == id) {
                            this.selected = index;
                            this.close_session(window, cx);
                        }
                    });
                    true
                })
        });
    }
}

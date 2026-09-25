//! Overlay の描画・フォーカス復帰・通知の寿命は Kit の Root が管理する。
use super::Tone;
use gpui_kit::component::{
    WindowExt, button::ButtonVariant, dialog::DialogButtonProps, notification::Notification,
};
use gpui_kit::{prelude::*, *};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogEvent {
    Confirmed,
    Cancelled,
}
pub struct Dialog {
    pub open: bool,
    pub title: SharedString,
    pub description: SharedString,
    pub confirm_label: SharedString,
    pub destructive: bool,
}
impl EventEmitter<DialogEvent> for Dialog {}
impl Dialog {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            open: false,
            title: "変更を保存しますか？".into(),
            description: "変更を確認してから保存できます。".into(),
            confirm_label: "保存する".into(),
            destructive: false,
        }
    }
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        let title = self.title.clone();
        let description = self.description.clone();
        let confirm = self.confirm_label.clone();
        let destructive = self.destructive;
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let confirmed = weak.clone();
            let closed = weak.clone();
            dialog
                .title(title.clone())
                .child(description.clone())
                .w(px(440.))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(confirm.clone())
                        .cancel_text("キャンセル")
                        .show_cancel(true)
                        .ok_variant(if destructive {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        }),
                )
                .on_ok(move |_, _, cx| {
                    let _ = confirmed.update(cx, |this, cx| {
                        this.open = false;
                        cx.emit(DialogEvent::Confirmed);
                        cx.notify();
                    });
                    true
                })
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| {
                        if this.open {
                            this.open = false;
                            cx.emit(DialogEvent::Cancelled);
                            cx.notify();
                        }
                    });
                })
        });
        cx.notify();
    }
}
impl Render for Dialog {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Window を持たない worker コールバックの通知を描画側に受け渡す。
pub struct ToastHost {
    pending: Vec<(SharedString, Tone)>,
}
impl ToastHost {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            pending: Vec::new(),
        }
    }
    pub fn push(&mut self, message: impl Into<SharedString>, tone: Tone, cx: &mut Context<Self>) {
        self.pending.push((message.into(), tone));
        cx.notify();
    }
}
impl Render for ToastHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for (message, tone) in self.pending.drain(..) {
            let notification = match tone {
                Tone::Success => Notification::success(message),
                Tone::Warning => Notification::warning(message),
                Tone::Danger => Notification::error(message),
                _ => Notification::info(message),
            };
            window.push_notification(notification, cx);
        }
        div()
    }
}

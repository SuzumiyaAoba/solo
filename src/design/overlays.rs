use super::{Button, ButtonVariant, Icon, NextFocus, PreviousFocus, Tone, radius, theme};
use gpui::{prelude::*, *};
use std::time::Duration;

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
    scope: FocusHandle,
    buttons: [FocusHandle; 3],
    previous_focus: Option<FocusHandle>,
}
impl EventEmitter<DialogEvent> for Dialog {}
impl Dialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            title: "変更を保存しますか？".into(),
            description: "変更を確認してから保存できます。".into(),
            confirm_label: "保存する".into(),
            destructive: false,
            scope: cx.focus_handle(),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            previous_focus: None,
        }
    }
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.previous_focus = window.focused(cx);
        self.open = true;
        self.buttons[1].focus(window);
        cx.notify();
    }
    pub fn dismiss(&mut self, result: DialogEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(focus) = self.previous_focus.take() {
            focus.focus(window);
        }
        cx.emit(result);
        cx.notify();
    }
    fn cycle_focus(&self, reverse: bool, window: &mut Window) {
        let current = self
            .buttons
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap_or(1);
        self.buttons[(current + if reverse { 2 } else { 1 }) % 3].focus(window);
    }
    pub fn has_focus(&self, window: &Window) -> bool {
        self.buttons.iter().any(|focus| focus.is_focused(window))
    }
}
impl Render for Dialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme(cx);
        div().when(self.open, |v| {
            v.absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x00000070))
                .occlude()
                .track_focus(&self.scope)
                .on_action(cx.listener(|this, _: &NextFocus, window, cx| {
                    this.cycle_focus(false, window);
                    cx.stop_propagation();
                }))
                .on_action(cx.listener(|this, _: &PreviousFocus, window, cx| {
                    this.cycle_focus(true, window);
                    cx.stop_propagation();
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.dismiss(DialogEvent::Cancelled, window, cx);
                        cx.stop_propagation();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.dismiss(DialogEvent::Cancelled, window, cx)
                    }),
                )
                .child(
                    div()
                        .w(px(440.))
                        .flex()
                        .flex_col()
                        .rounded(px(radius::DIALOG))
                        .bg(rgb(p.elevated))
                        .border_1()
                        .border_color(rgb(p.border))
                        .shadow_lg()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .p_6()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_size(px(18.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(p.text))
                                                .child(self.title.clone()),
                                        )
                                        .child(
                                            Button::icon("dialog-close", Icon::Close, "閉じる")
                                                .focus_handle(&self.buttons[0])
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.dismiss(DialogEvent::Cancelled, window, cx)
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .line_height(px(21.))
                                        .text_color(rgb(p.secondary))
                                        .child(self.description.clone()),
                                ),
                        )
                        .child(
                            div()
                                .px_6()
                                .py_4()
                                .border_t_1()
                                .border_color(rgb(p.border))
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("dialog-cancel", "キャンセル")
                                        .focus_handle(&self.buttons[1])
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.dismiss(DialogEvent::Cancelled, window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("dialog-confirm", self.confirm_label.clone())
                                        .focus_handle(&self.buttons[2])
                                        .variant(if self.destructive {
                                            ButtonVariant::Danger
                                        } else {
                                            ButtonVariant::Primary
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.dismiss(DialogEvent::Confirmed, window, cx)
                                        })),
                                ),
                        ),
                )
        })
    }
}

pub struct ToastHost {
    pub message: Option<(SharedString, Tone)>,
    timer: Option<Task<()>>,
}
impl ToastHost {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            message: None,
            timer: None,
        }
    }
    pub fn push(&mut self, message: impl Into<SharedString>, tone: Tone, cx: &mut Context<Self>) {
        self.message = Some((message.into(), tone));
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                this.message = None;
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        self.message = None;
        cx.notify();
    }
}
impl Render for ToastHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme(cx);
        div().absolute().bottom_6().right_6().when_some(
            self.message.clone(),
            |v, (message, tone)| {
                let (color, _) = p.tone(tone);
                v.w(px(350.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded(px(radius::CARD))
                    .bg(rgb(p.elevated))
                    .border_1()
                    .border_color(rgb(p.border))
                    .shadow_lg()
                    .occlude()
                    .child(
                        if tone == Tone::Success {
                            Icon::Check
                        } else {
                            Icon::Info
                        }
                        .view(color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(rgb(p.text))
                            .child(message),
                    )
                    .child(
                        Button::icon("dismiss-toast", Icon::Close, "通知を閉じる")
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
                    )
            },
        )
    }
}

use super::{theme, Button, ButtonVariant, ControlSize, Icon, NextFocus, PreviousFocus, radius};
use gpui::{prelude::*, *};

pub struct SelectionChanged { pub index: usize, pub label: SharedString }
pub struct Select {
    pub options: Vec<SharedString>,
    pub selected: usize,
    pub disabled: bool,
    pub open: bool,
    highlighted: usize,
    focus: FocusHandle,
}
impl EventEmitter<SelectionChanged> for Select {}
impl Focusable for Select { fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() } }
impl Select {
    pub fn new(options: impl IntoIterator<Item = impl Into<SharedString>>, selected: usize, cx: &mut Context<Self>) -> Self {
        let options: Vec<_> = options.into_iter().map(Into::into).collect();
        let selected = selected.min(options.len().saturating_sub(1));
        Self { options, selected, highlighted: selected, open: false, disabled: false, focus: cx.focus_handle().tab_stop(true) }
    }
    pub fn disabled(mut self, disabled: bool) -> Self { self.disabled = disabled; self }
    pub fn close(&mut self, cx: &mut Context<Self>) { self.open = false; cx.notify(); }
    pub fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || index >= self.options.len() { return; }
        self.selected = index; self.highlighted = index; self.open = false;
        cx.emit(SelectionChanged { index, label: self.options[index].clone() });
        self.focus.focus(window); cx.notify();
    }
}
impl Render for Select {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme(cx);
        let disabled = self.disabled || self.options.is_empty();
        let was_open = self.open;
        div().relative().w_full()
            .child(div().id("select-trigger").track_focus(&self.focus).tab_stop(!disabled)
                .flex().items_center().justify_between().gap_3().h(px(ControlSize::Medium.height())).px_3()
                .rounded(px(radius::CONTROL)).bg(rgb(p.canvas)).border_1().border_color(rgb(if self.open { p.focus } else { p.control_border }))
                .text_size(px(13.)).text_color(rgb(p.text))
                .when(disabled, |v| v.opacity(0.4))
                .when(!disabled, |v| v.cursor_pointer().focus(move |s| s.border_color(rgb(p.focus)))
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if was_open && matches!(event, ClickEvent::Keyboard(_)) { this.choose(this.highlighted, window, cx); }
                        else { this.open = !was_open; this.highlighted = this.selected; this.focus.focus(window); cx.notify(); }
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "down" => { if this.open { this.highlighted = (this.highlighted + 1) % this.options.len(); } this.open = true; }
                            "up" => { if this.open { this.highlighted = (this.highlighted + this.options.len() - 1) % this.options.len(); } this.open = true; }
                            "home" if this.open => this.highlighted = 0,
                            "end" if this.open => this.highlighted = this.options.len() - 1,
                            "escape" => this.open = false,
                            _ => return,
                        }
                        cx.stop_propagation(); cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &NextFocus, window, cx| { this.close(cx); window.focus_next(); }))
                    .on_action(cx.listener(|this, _: &PreviousFocus, window, cx| { this.close(cx); window.focus_prev(); })))
                .child(self.options.get(self.selected).cloned().unwrap_or_else(|| "選択肢がありません".into()))
                .child(Icon::ChevronDown.view(p.muted)))
            .when(self.open && !disabled, |v| v.child(deferred(
                anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(40.))).snap_to_window()
                    .child(div().id("select-menu").w(px(260.)).max_h(px(280.)).overflow_y_scroll().p_1().rounded(px(radius::CONTROL))
                        .bg(rgb(p.elevated)).border_1().border_color(rgb(p.border)).shadow_lg().occlude()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close(cx)))
                        .children(self.options.iter().enumerate().map(|(index, label)| {
                            Button::new(("option", index), label.clone()).variant(ButtonVariant::Ghost).w_full().justify_start()
                                .bg(rgb(if self.highlighted == index { p.hover } else { p.elevated }))
                                .when(self.selected == index, |b| b.trailing_icon(Icon::Check))
                                .on_click(cx.listener(move |this, _, window, cx| this.choose(index, window, cx)))
                        })))
            )))
    }
}

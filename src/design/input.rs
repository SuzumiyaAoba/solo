//! Input / Textarea の編集、選択、IME、Undo は GPUI Kit に委ねる。
//! Solo 固有の送信条件と worker からの下書き復元だけをここで扱う。
use super::{ControlSize, Icon, theme};
use gpui_kit::component::{
    Sizable,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
};
use gpui_kit::{prelude::*, *};
use std::ops::Range;

actions!(solo_input, [Submit]);
pub fn bind_keys(cx: &mut App) {
    #[cfg(target_os = "macos")]
    use gpui_kit::component::input as kit_input;
    cx.bind_keys([
        KeyBinding::new("cmd-enter", Submit, Some("SoloInput > Input")),
        // Cocoa テキストフィールドと同じ Emacs 系編集キー。
        // 矢印・⌘/⌥ 系は Kit の "Input" コンテキストがすでに束縛済み。
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-b", kit_input::MoveLeft, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-f", kit_input::MoveRight, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-n", kit_input::MoveDown, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-p", kit_input::MoveUp, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-d", kit_input::Delete, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-h", kit_input::Backspace, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-k", kit_input::DeleteToEndOfLine, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-v", kit_input::MovePageDown, Some("Input")),
    ]);
}

pub struct Submitted(pub String);
pub struct InputChanged {
    pub text: String,
    pub composing: bool,
}

enum State {
    Single(Entity<InputState>),
    Multi(Entity<TextareaState>),
}
macro_rules! with_state {
    ($this:expr, $state:ident, $body:expr) => {
        match &$this.state {
            State::Single($state) => $body,
            State::Multi($state) => $body,
        }
    };
}

pub struct TextInput {
    state: State,
    pub can_submit: bool,
    pub disabled: bool,
    pub read_only: bool,
    pub invalid: bool,
    appearance: bool,
    placeholder: SharedString,
    size: ControlSize,
    leading: Option<Icon>,
    clear_on_submit: bool,
    pending_value: Option<SharedString>,
    _subscription: Subscription,
}
impl EventEmitter<Submitted> for TextInput {}
impl EventEmitter<InputChanged> for TextInput {}
impl Focusable for TextInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        with_state!(self, state, state.focus_handle(cx))
    }
}
impl TextInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = State::Single(cx.new(|cx| InputState::new(window, cx)));
        Self::with_state(state, window, cx)
    }
    pub fn multiline(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = State::Multi(cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 5)));
        Self::with_state(state, window, cx)
    }
    fn with_state(state: State, window: &mut Window, cx: &mut Context<Self>) -> Self {
        macro_rules! subscribe {
            ($entity:expr) => {
                cx.subscribe_in(
                    $entity,
                    window,
                    |this, _, event: &InputEvent, window, cx| match event {
                        InputEvent::Change => {
                            let composing = this.marked_text_range(window, cx).is_some();
                            cx.emit(InputChanged {
                                text: this.value(cx).to_string(),
                                composing,
                            });
                            cx.notify();
                        }
                        InputEvent::PressEnter {
                            secondary: true, ..
                        } => this.submit(window, cx),
                        _ => {}
                    },
                )
            };
        }
        let subscription = match &state {
            State::Single(entity) => subscribe!(entity),
            State::Multi(entity) => subscribe!(entity),
        };
        Self {
            state,
            can_submit: true,
            disabled: false,
            read_only: false,
            invalid: false,
            appearance: true,
            placeholder: "メッセージを入力…".into(),
            size: ControlSize::Medium,
            leading: None,
            clear_on_submit: false,
            pending_value: None,
            _subscription: subscription,
        }
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }
    pub fn default_value(mut self, text: &str) -> Self {
        self.pending_value = Some(text.to_owned().into());
        self
    }
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.leading = Some(icon);
        self
    }
    /// 枠・背景を外し、外側のコンテナが見た目を提供するときに使う。
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn clear_on_submit(mut self, clear: bool) -> Self {
        self.clear_on_submit = clear;
        self
    }
    pub fn value(&self, cx: &App) -> SharedString {
        self.pending_value
            .clone()
            .unwrap_or_else(|| with_state!(self, state, state.read(cx).value()))
    }
    /// Worker のコールバックからも復元できる。次の描画または入力処理前に適用する。
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.pending_value = Some(value.into());
        cx.notify();
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.pending_value.take();
        with_state!(
            self,
            entity,
            entity.update(cx, |state, cx| {
                if let Some(value) = value {
                    state.set_value(value, window, cx);
                }
                state.set_disabled(self.disabled, cx);
                state.set_readonly(self.read_only, cx);
                if state.presentation().placeholder() != &self.placeholder {
                    state.set_placeholder(self.placeholder.clone(), window, cx);
                }
            })
        );
    }
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync(window, cx);
        if !self.can_submit
            || self.disabled
            || self.read_only
            || self.marked_text_range(window, cx).is_some()
        {
            return;
        }
        let text = self.value(cx).to_string();
        if text.trim().is_empty() {
            return;
        }
        if self.clear_on_submit {
            self.set_value("", cx);
            self.sync(window, cx);
            cx.emit(InputChanged {
                text: String::new(),
                composing: false,
            });
        }
        cx.emit(Submitted(text));
        cx.notify();
    }
    pub fn marked_text_range(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        with_state!(
            self,
            entity,
            entity.update(cx, |state, cx| state.marked_text_range(window, cx))
        )
    }
    /// 実際の Kit の native input handler を smoke 検証からも使う。
    pub fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync(window, cx);
        let len = self.value(cx).encode_utf16().count();
        let range = range.map(|r| r.start.min(len)..r.end.min(len));
        with_state!(
            self,
            entity,
            entity.update(cx, |state, cx| state
                .replace_text_in_range(range, text, window, cx))
        );
    }
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync(window, cx);
        with_state!(
            self,
            entity,
            entity.update(cx, |state, cx| state
                .replace_and_mark_text_in_range(range, text, selected, window, cx))
        );
    }
}
impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(window, cx);
        let p = theme(cx);
        let input = match &self.state {
            State::Single(state) => Input::new(state)
                .with_size(match self.size {
                    ControlSize::Small => gpui_kit::component::Size::Small,
                    ControlSize::Medium => gpui_kit::component::Size::Medium,
                    ControlSize::Large => gpui_kit::component::Size::Large,
                })
                .h(px(self.size.height()))
                .w_full()
                .aria_label(self.placeholder.clone())
                .appearance(self.appearance)
                .disabled(self.disabled)
                .readonly(self.read_only)
                .when_some(self.leading, |input, icon| input.prefix(icon.view(p.muted)))
                .when(self.invalid, |input| input.border_color(rgb(p.danger)))
                .into_any_element(),
            State::Multi(state) => Textarea::new(state)
                .aria_label(self.placeholder.clone())
                .appearance(self.appearance)
                .disabled(self.disabled)
                .readonly(self.read_only)
                .w_full()
                .into_any_element(),
        };
        div()
            .key_context("SoloInput")
            .w_full()
            .on_action(cx.listener(|this, _: &Submit, window, cx| this.submit(window, cx)))
            .child(input)
    }
}

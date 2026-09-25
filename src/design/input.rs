// TextElement's layout/painting follows GPUI 0.2.2 examples/input.rs.
// Copyright 2022–2025 Zed Industries, Inc. Apache-2.0; see THIRD_PARTY_NOTICES.md.
// Modified: independent Unicode buffer, composition ranges, focus and horizontal scrolling.
use gpui::{prelude::*, *};
use crate::text::TextBuffer;
use super::{theme, ControlSize, Icon, radius};
use std::ops::Range;

actions!(
    composer,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Paste,
        Cut,
        Copy,
        Submit,
        CharacterPalette
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-left", Home, Some("TextInput")),
        KeyBinding::new("cmd-right", End, Some("TextInput")),
        KeyBinding::new("cmd-shift-left", SelectHome, Some("TextInput")),
        KeyBinding::new("cmd-shift-right", SelectEnd, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("cmd-enter", Submit, Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space", CharacterPalette, Some("TextInput")),
    ]);
}

pub struct Submitted(pub String);
pub struct InputEvent { pub text: String, pub composing: bool }
pub struct TextInput {
    pub buffer: TextBuffer,
    pub can_submit: bool,
    pub disabled: bool,
    pub read_only: bool,
    pub invalid: bool,
    pub placeholder: SharedString,
    pub size: ControlSize,
    pub leading: Option<Icon>,
    pub clear_on_submit: bool,
    focus: FocusHandle,
    layout: Option<ShapedLine>,
    bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    selecting: bool,
}

impl EventEmitter<Submitted> for TextInput {}
impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            buffer: TextBuffer::default(),
            can_submit: true,
            disabled: false,
            read_only: false,
            invalid: false,
            placeholder: "メッセージを入力…".into(),
            size: ControlSize::Medium,
            leading: None,
            clear_on_submit: false,
            focus: cx.focus_handle().tab_stop(true),
            layout: None,
            bounds: None,
            scroll_x: px(0.),
            selecting: false,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self { self.placeholder = text.into(); self }
    pub fn default_value(mut self, text: &str) -> Self { self.buffer.replace(None, text); self }
    pub fn control_size(mut self, size: ControlSize) -> Self { self.size = size; self }
    pub fn leading_icon(mut self, icon: Icon) -> Self { self.leading = Some(icon); self }
    pub fn invalid(mut self, invalid: bool) -> Self { self.invalid = invalid; self }
    pub fn read_only(mut self, read_only: bool) -> Self { self.read_only = read_only; self }
    pub fn disabled(mut self, disabled: bool) -> Self { self.disabled = disabled; self.focus = self.focus.tab_stop(!disabled); self }
    pub fn clear_on_submit(mut self, clear: bool) -> Self { self.clear_on_submit = clear; self }
    pub fn editable(&self) -> bool { !self.disabled && !self.read_only }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(InputEvent { text: self.buffer.content.clone(), composing: self.buffer.marked.is_some() });
        cx.notify();
    }

    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.can_submit || !self.editable() || self.buffer.marked.is_some() || self.buffer.content.trim().is_empty() { return; }
        let text = if self.clear_on_submit { self.buffer.take_committed().expect("committed text") } else { self.buffer.content.clone() };
        if self.clear_on_submit { self.scroll_x = px(0.); self.changed(cx); }
        cx.emit(Submitted(text));
        cx.notify();
    }

    fn index_at(&self, position: Point<Pixels>) -> usize {
        match (&self.layout, self.bounds) {
            (Some(line), Some(bounds)) => line
                .closest_index_for_x(position.x - bounds.left() + self.scroll_x)
                .min(self.buffer.content.len()),
            _ => 0,
        }
    }

    fn copy(&self, cx: &mut App) {
        if !self.buffer.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.buffer.content[self.buffer.selection.clone()].into(),
            ));
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(range);
        *actual = Some(self.buffer.range_to_utf16(range.clone()));
        Some(self.buffer.content[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(self.buffer.selection.clone()),
            reversed: self.buffer.reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked
            .clone()
            .map(|range| self.buffer.range_to_utf16(range))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.marked = None;
        self.changed(cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() { return; }
        self.buffer.replace(range, &text.replace(['\n', '\r'], " "));
        self.changed(cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() { return; }
        self.buffer.replace_and_mark(range, text, selected);
        self.changed(cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        let line = self.layout.as_ref()?;
        let range = self.buffer.range_from_utf16(range);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start) - self.scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end) - self.scroll_x + px(1.),
                bounds.bottom(),
            ),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.bounds?;
        Some(crate::text::to_utf16(
            &self.buffer.content,
            self.index_at(point),
        ))
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme(cx);
        let invalid = self.invalid;
        div()
            .id("composer")
            .key_context("TextInput")
            .track_focus(&self.focus)
            .tab_stop(!self.disabled)
            .cursor(CursorStyle::IBeam)
            .flex().items_center().gap_2()
            .w_full()
            .h(px(self.size.height()))
            .px(px(self.size.padding()))
            .py(px((self.size.height() - 22.) / 2.))
            .overflow_hidden()
            .rounded(px(radius::CONTROL))
            .bg(rgb(p.canvas))
            .border_1()
            .border_color(rgb(if self.invalid { p.danger } else { p.control_border }))
            .text_color(rgb(p.text))
            .text_size(px(self.size.font_size()))
            .line_height(px(22.))
            .when(self.disabled, |v| v.opacity(0.4).cursor_default())
            .when(!self.disabled, |v| v.focus(move |s| s.border_color(rgb(if invalid { p.danger } else { p.focus }))))
            .when(!self.disabled, |view| view
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                if !this.editable() { return; }
                this.buffer.backspace();
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &Delete, _, cx| {
                if !this.editable() { return; }
                this.buffer.delete();
                this.changed(cx);
            }))
            .on_action(cx.listener(|this, _: &Left, _, cx| {
                let offset = if this.buffer.selection.is_empty() {
                    this.buffer.previous()
                } else {
                    this.buffer.selection.start
                };
                this.buffer.move_to(offset, false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Right, _, cx| {
                let offset = if this.buffer.selection.is_empty() {
                    this.buffer.next()
                } else {
                    this.buffer.selection.end
                };
                this.buffer.move_to(offset, false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| {
                this.buffer.move_to(this.buffer.previous(), true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| {
                this.buffer.move_to(this.buffer.next(), true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Home, _, cx| {
                this.buffer.move_to(0, false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &End, _, cx| {
                this.buffer.move_to(this.buffer.content.len(), false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| {
                this.buffer.move_to(0, true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| {
                this.buffer.move_to(this.buffer.content.len(), true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.buffer.move_to(0, false);
                this.buffer.move_to(this.buffer.content.len(), true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                if this.editable() && !this.buffer.selection.is_empty() {
                    this.copy(cx);
                    this.buffer.replace(None, "");
                    this.changed(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.replace_text_in_range(None, &text, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Submit, _, cx| this.submit(cx)))
            .on_action(
                cx.listener(|_, _: &CharacterPalette, window, _| window.show_character_palette()),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.selecting = true;
                    this.buffer
                        .move_to(this.index_at(event.position), event.modifiers.shift);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.selecting {
                    this.buffer.move_to(this.index_at(event.position), true);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            ))
            .when_some(self.leading, |v, icon| v.child(icon.view(p.muted)))
            .child(div().flex_1().min_w_0().overflow_hidden().child(TextElement { input: cx.entity() }))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}
struct TextPaint {
    line: Option<ShapedLine>,
    cursor: PaintQuad,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}
impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = TextPaint;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> TextPaint {
        let input = self.input.read(cx);
        let style = window.text_style();
        let empty = input.buffer.content.is_empty();
        let text: SharedString = if empty {
            input.placeholder.clone()
        } else {
            input.buffer.content.clone().into()
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: if empty {
                rgb(theme(cx).muted).into()
            } else {
                style.color
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = &input.buffer.marked {
            vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.len(),
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };
        let line = window.text_system().shape_line(
            text,
            style.font_size.to_pixels(window.rem_size()),
            &runs,
            None,
        );
        let cursor_x = line.x_for_index(input.buffer.cursor());
        let scroll_x = input
            .scroll_x
            .min(cursor_x)
            .max(cursor_x - bounds.size.width + px(4.))
            .max(px(0.));
        let selection = (!input.buffer.selection.is_empty()).then(|| {
            fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(input.buffer.selection.start) - scroll_x,
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(input.buffer.selection.end) - scroll_x,
                        bounds.bottom(),
                    ),
                ),
                rgba((theme(cx).accent << 8) | 0x4d),
            )
        });
        TextPaint {
            line: Some(line),
            scroll_x,
            selection,
            cursor: fill(
                Bounds::new(
                    point(bounds.left() + cursor_x - scroll_x, bounds.top()),
                    size(px(2.), bounds.size.height),
                ),
                rgb(theme(cx).focus),
            ),
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut TextPaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        if !self.input.read(cx).disabled { window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        ); }
        if let Some(selection) = state.selection.take() {
            window.paint_quad(selection);
        }
        let line = state.line.take().expect("prepaint shapes a line");
        let _ = line.paint(
            point(bounds.left() - state.scroll_x, bounds.top()),
            window.line_height(),
            window,
            cx,
        );
        if !self.input.read(cx).disabled && focus.is_focused(window) && self.input.read(cx).buffer.selection.is_empty() {
            window.paint_quad(state.cursor.clone());
        }
        self.input.update(cx, |input, _| {
            input.layout = Some(line);
            input.bounds = Some(bounds);
            input.scroll_x = state.scroll_x;
        });
    }
}

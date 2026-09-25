use super::{ControlSize, Icon, Tone, radius, theme, typography};
use gpui::{prelude::*, *};
use std::rc::Rc;

pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Ghost,
    Danger,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewState {
    #[default]
    Rest,
    Hover,
    Pressed,
    Focus,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    tab_stop: bool,
    loading: bool,
    icon: Option<Icon>,
    icon_only: bool,
    trailing: Option<Icon>,
    tooltip: Option<SharedString>,
    focus: Option<FocusHandle>,
    preview: PreviewState,
    on_click: Option<ClickHandler>,
    style: StyleRefinement,
}
impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::Secondary,
            size: ControlSize::Medium,
            disabled: false,
            tab_stop: true,
            loading: false,
            icon: None,
            icon_only: false,
            trailing: None,
            tooltip: None,
            focus: None,
            preview: PreviewState::Rest,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }
    pub fn icon(id: impl Into<ElementId>, icon: Icon, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        let mut button = Self::new(id, label.clone())
            .with_icon(icon)
            .variant(ButtonVariant::Ghost)
            .tooltip(label);
        button.icon_only = true;
        button
    }
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn trailing_icon(mut self, icon: Icon) -> Self {
        self.trailing = Some(icon);
        self
    }
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
    pub fn focus_handle(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    /// Gallery の比較用。イベントによる実際の hover/focus も同じスタイルを使う。
    pub fn preview(mut self, state: PreviewState) -> Self {
        self.preview = state;
        self
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}
impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme(cx);
        let (bg, hover, pressed, fg, border) = match self.variant {
            ButtonVariant::Primary => (
                p.accent,
                p.accent_hover,
                p.accent_pressed,
                p.on_accent,
                p.accent,
            ),
            ButtonVariant::Secondary => (p.elevated, p.hover, p.pressed, p.text, p.border),
            ButtonVariant::Ghost => (p.surface, p.hover, p.pressed, p.secondary, p.surface),
            ButtonVariant::Danger => (
                p.danger_solid,
                0xa42b42,
                0x8e2036,
                p.on_accent,
                p.danger_solid,
            ),
        };
        let blocked = self.disabled || self.loading;
        let fill = match self.preview {
            PreviewState::Hover => hover,
            PreviewState::Pressed => pressed,
            _ => bg,
        };
        let ghost = self.variant == ButtonVariant::Ghost;
        let mut view = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .flex_shrink_0()
            .h(px(self.size.height()))
            .px(px(self.size.padding()))
            .rounded(px(radius::CONTROL))
            .text_size(px(self.size.font_size()))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .border_1()
            .border_color(rgb(if self.preview == PreviewState::Focus {
                p.focus
            } else {
                border
            }))
            .bg(rgb(fill))
            .when(ghost && self.preview != PreviewState::Focus, |v| {
                v.border_color(rgba(0))
            })
            .when(
                ghost && matches!(self.preview, PreviewState::Rest | PreviewState::Focus),
                |v| v.bg(rgba(0)),
            )
            .text_color(rgb(fg))
            .when(self.icon_only, |v| v.w(px(self.size.height())).px_0())
            .when(self.disabled, |v| v.opacity(0.4).cursor_default())
            .when(!blocked, |v| {
                v.focusable()
                    .tab_stop(self.tab_stop)
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(hover)))
                    .active(move |s| s.bg(rgb(pressed)))
                    .focus(move |s| s.border_color(rgb(p.focus)))
            })
            .when_some(self.focus, |v, focus| {
                v.track_focus(&focus).tab_stop(!blocked && self.tab_stop)
            })
            .when_some(self.tooltip, |v, text| {
                v.tooltip(move |_, cx| cx.new(|_| Tooltip(text.clone())).into())
            })
            .when(self.loading, |v| v.child(Icon::Spinner.view(fg)))
            .when(!self.loading, |v| {
                v.when_some(self.icon, |v, icon| v.child(icon.view(fg)))
            })
            .when(!self.icon_only, |v| v.child(self.label))
            .when_some(self.trailing, |v, icon| v.child(icon.view(fg)));
        if !blocked && let Some(handler) = self.on_click {
            view = view.on_click(move |event, window, cx| handler(event, window, cx));
        }
        view.style().refine(&self.style);
        view
    }
}

pub struct Tooltip(pub SharedString);
impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme(cx);
        div()
            .px_3()
            .py_2()
            .rounded(px(radius::CONTROL))
            .bg(rgb(p.elevated))
            .border_1()
            .border_color(rgb(p.border))
            .shadow_md()
            .text_size(px(typography::LABEL))
            .text_color(rgb(p.text))
            .child(self.0.clone())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToggleKind {
    Checkbox,
    Radio,
    Switch,
}
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    label: SharedString,
    checked: bool,
    mixed: bool,
    disabled: bool,
    kind: ToggleKind,
    on_change: Option<ChangeHandler>,
    focus: Option<FocusHandle>,
}
impl Toggle {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        kind: ToggleKind,
        checked: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            checked,
            mixed: false,
            disabled: false,
            on_change: None,
            focus: None,
        }
    }
    pub fn checkbox(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self::new(id, label, ToggleKind::Checkbox, checked)
    }
    pub fn radio(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        Self::new(id, label, ToggleKind::Radio, checked)
    }
    pub fn switch(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        Self::new(id, label, ToggleKind::Switch, checked)
    }
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn focus_handle(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}
impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme(cx);
        let selected = self.checked || self.mixed;
        let mark = match self.kind {
            ToggleKind::Checkbox => div()
                .size(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(if selected { p.accent } else { p.control_border }))
                .bg(rgb(if selected { p.accent } else { p.surface }))
                .when(selected, |v| {
                    v.child(
                        if self.mixed { Icon::Minus } else { Icon::Check }
                            .view(p.on_accent)
                            .size(px(12.)),
                    )
                })
                .into_any_element(),
            ToggleKind::Radio => div()
                .size(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(rgb(if selected {
                    p.accent_text
                } else {
                    p.control_border
                }))
                .when(selected, |v| {
                    v.child(div().size(px(8.)).rounded_full().bg(rgb(p.accent_text)))
                })
                .into_any_element(),
            ToggleKind::Switch => div()
                .w(px(30.))
                .h(px(18.))
                .flex()
                .items_center()
                .px(px(2.))
                .rounded_full()
                .bg(rgb(if selected { p.accent } else { p.control_border }))
                .when(selected, |v| v.justify_end())
                .child(div().size(px(14.)).rounded_full().bg(rgb(0xffffff)))
                .into_any_element(),
        };
        let mut view = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(30.))
            .px(px(3.))
            .rounded(px(radius::SMALL))
            .border_1()
            .border_color(rgba(0))
            .text_size(px(typography::BODY))
            .text_color(rgb(p.text))
            .when(self.disabled, |v| v.opacity(0.4))
            .when(!self.disabled, |v| {
                v.focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .focus(move |s| s.border_color(rgb(p.focus)))
            })
            .when_some(self.focus, |v, focus| {
                v.track_focus(&focus).tab_stop(!self.disabled)
            })
            .child(mark)
            .child(self.label);
        if !self.disabled
            && let Some(handler) = self.on_change
        {
            let next = self.kind == ToggleKind::Radio || !self.checked;
            view = view.on_click(move |_, window, cx| handler(&next, window, cx));
        }
        view
    }
}

pub fn badge(label: impl Into<SharedString>, tone: Tone, cx: &App) -> Div {
    let (fg, bg) = theme(cx).tone(tone);
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .px_2()
        .h(px(22.))
        .rounded(px(radius::SMALL))
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .text_size(px(typography::CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .child(div().size(px(5.)).rounded_full().bg(rgb(fg)))
        .child(label.into())
}
pub fn avatar(initials: impl Into<SharedString>, tone: Tone, cx: &App) -> Div {
    let (fg, bg) = theme(cx).tone(tone);
    div()
        .size(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .child(initials.into())
}
pub fn keycap(label: impl Into<SharedString>, cx: &App) -> Div {
    let p = theme(cx);
    div()
        .min_w(px(20.))
        .h(px(21.))
        .px(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(p.border))
        .bg(rgb(p.canvas))
        .text_color(rgb(p.muted))
        .text_size(px(11.))
        .font_family(typography::MONO)
        .child(label.into())
}
pub fn card(cx: &App) -> Div {
    let p = theme(cx);
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::CARD))
        .border_1()
        .border_color(rgb(p.border))
        .bg(rgb(p.surface))
}
pub fn divider(cx: &App) -> Div {
    div()
        .h(px(1.))
        .w_full()
        .bg(rgb(theme(cx).border))
        .flex_shrink_0()
}
pub fn progress(value: f32, tone: Tone, cx: &App) -> Div {
    let p = theme(cx);
    let (color, _) = p.tone(tone);
    let value = if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    };
    div()
        .w_full()
        .h(px(4.))
        .rounded_full()
        .bg(rgb(p.hover))
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(value))
                .rounded_full()
                .bg(rgb(color)),
        )
}
pub fn skeleton(width: f32, cx: &App) -> Div {
    div()
        .w(px(width))
        .h(px(10.))
        .rounded(px(3.))
        .bg(rgb(theme(cx).hover))
}
pub fn empty_state(
    icon: Icon,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    cx: &App,
) -> Div {
    let p = theme(cx);
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .py_8()
        .px_6()
        .gap_3()
        .child(
            div()
                .size(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .border_1()
                .border_color(rgb(p.border))
                .child(icon.view(p.muted).size(px(20.))),
        )
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(p.text))
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(p.muted))
                .child(description.into()),
        )
}
pub fn alert(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    tone: Tone,
    cx: &App,
) -> Div {
    let (fg, bg) = theme(cx).tone(tone);
    div()
        .flex()
        .gap_3()
        .p_4()
        .rounded(px(radius::CONTROL))
        .bg(rgb(bg))
        .child(
            if matches!(tone, Tone::Warning | Tone::Danger) {
                Icon::Warning
            } else {
                Icon::Info
            }
            .view(fg),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .text_color(rgb(fg))
                .child(div().font_weight(FontWeight::MEDIUM).child(title.into()))
                .child(div().text_size(px(12.)).child(description.into())),
        )
}

/// 選択状態とキーボード操作を Button と共通にする。
pub fn tab(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Button {
    let p = theme(cx);
    Button::new(id, label)
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .bg(rgb(if selected { p.hover } else { p.surface }))
        .text_color(rgb(if selected { p.text } else { p.secondary }))
}
pub fn nav_item(
    id: impl Into<ElementId>,
    icon: Icon,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Button {
    let p = theme(cx);
    Button::new(id, label)
        .variant(ButtonVariant::Ghost)
        .with_icon(icon)
        .w_full()
        .justify_start()
        .bg(rgb(if selected { p.hover } else { p.sidebar }))
        .text_color(rgb(if selected { p.text } else { p.secondary }))
}

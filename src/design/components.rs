use super::{ControlSize, Icon, Tone, glass, radius, theme, typography};
use gpui_kit::component::{
    self as kit, Disableable, Selectable, Sizable,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenu},
};
use gpui_kit::{prelude::*, *};
use std::rc::Rc;

pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type MenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

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
    loading: bool,
    toggled: Option<bool>,
    icon: Option<Icon>,
    icon_only: bool,
    align_start: bool,
    trailing: Option<Icon>,
    tooltip: Option<SharedString>,
    preview: PreviewState,
    on_click: Option<ClickHandler>,
    menu: Option<(Anchor, MenuBuilder)>,
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
            loading: false,
            toggled: None,
            icon: None,
            icon_only: false,
            align_start: false,
            trailing: None,
            tooltip: None,
            preview: PreviewState::Rest,
            on_click: None,
            menu: None,
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
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    /// Kit の内容行を伸ばして、アイコンとラベルを左端にそろえる。
    pub fn align_start(mut self) -> Self {
        self.align_start = true;
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
    /// Gallery の静的な状態見本。通常の操作状態は Kit が管理する。
    pub fn preview(mut self, state: PreviewState) -> Self {
        self.preview = state;
        self
    }
    /// Kit の DropdownMenu をトリガーに付ける。メニューは開くたびに作り直される。
    pub fn dropdown_menu(
        self,
        build: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.dropdown_menu_with_anchor(Anchor::TopLeft, build)
    }
    /// メニューのアンカー角を指定する版。ヘッダー右端のトリガーは TopRight で揃える。
    pub fn dropdown_menu_with_anchor(
        mut self,
        anchor: impl Into<Anchor>,
        build: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.menu = Some((anchor.into(), Rc::new(build)));
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
        let mut button = kit::button::Button::new(self.id)
            .with_variant(match self.variant {
                ButtonVariant::Primary => kit::button::ButtonVariant::Primary,
                ButtonVariant::Secondary => kit::button::ButtonVariant::Default,
                ButtonVariant::Ghost => kit::button::ButtonVariant::Ghost,
                ButtonVariant::Danger => kit::button::ButtonVariant::Danger,
            })
            .with_size(match self.size {
                ControlSize::Small => kit::Size::Small,
                ControlSize::Medium => kit::Size::Medium,
                ControlSize::Large => kit::Size::Large,
            })
            .h(px(self.size.height()))
            .text_size(px(self.size.font_size()))
            .when(self.icon_only, |b| b.w(px(self.size.height())))
            .disabled(self.disabled)
            .loading(self.loading)
            .when_some(self.toggled, |b, value| b.selected(value).toggled(value))
            .tab_stop(!self.loading)
            .accessibility_label(self.label.clone())
            .when(!self.icon_only && !self.align_start, |b| {
                b.label(self.label.clone())
            })
            .when(!self.icon_only && self.align_start, |b| {
                b.child(div().flex_1().min_w_0().truncate().child(self.label))
            })
            .when_some(self.icon, |b, icon| b.icon(icon.kit()))
            .when_some(self.trailing, |b, icon| b.child(icon.kit().size(px(14.))))
            .when_some(self.tooltip, |b, text| b.tooltip(text))
            .when(self.preview == PreviewState::Hover, |b| b.bg(rgb(p.hover)))
            .when(self.preview == PreviewState::Pressed, |b| b.selected(true))
            .when(self.preview == PreviewState::Focus, |b| {
                b.border_color(rgb(p.focus))
            })
            .when_some(self.on_click, |b, handler| {
                b.on_click(move |e, w, cx| handler(e, w, cx))
            });
        button.style().refine(&self.style);
        match self.menu {
            Some((anchor, build)) => button
                .dropdown_menu_with_anchor(anchor, move |menu, window, cx| build(menu, window, cx))
                .into_any_element(),
            None => button.into_any_element(),
        }
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
    disabled: bool,
    kind: ToggleKind,
    on_change: Option<ChangeHandler>,
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
            disabled: false,
            on_change: None,
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
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}
impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let handler = self.on_change;
        let on_change = move |value: &bool, window: &mut Window, cx: &mut App| {
            if let Some(handler) = &handler {
                handler(value, window, cx);
            }
        };
        match self.kind {
            ToggleKind::Checkbox => kit::checkbox::Checkbox::new(self.id)
                .label(self.label)
                .checked(self.checked)
                .disabled(self.disabled)
                .on_change(on_change)
                .into_any_element(),
            ToggleKind::Radio => kit::radio::Radio::new(self.id)
                .label(self.label)
                .checked(self.checked)
                .disabled(self.disabled)
                .on_change(on_change)
                .into_any_element(),
            ToggleKind::Switch => kit::switch::Switch::new(self.id)
                .label(self.label)
                .checked(self.checked)
                .disabled(self.disabled)
                .on_change(on_change)
                .into_any_element(),
        }
    }
}

pub fn badge(label: impl Into<SharedString>, tone: Tone, cx: &App) -> kit::tag::Tag {
    let (fg, bg) = theme(cx).tone(tone);
    kit::tag::Tag::custom(rgb(bg).into(), rgb(fg).into(), rgb(bg).into())
        .small()
        .child(label.into())
}

/// 文言を常設しない状態・数値表示。意味は tooltip と読み上げラベルで提供する。
pub fn indicator(
    id: impl Into<ElementId>,
    icon: Icon,
    value: impl Into<SharedString>,
    label: impl Into<SharedString>,
    tone: Tone,
    cx: &App,
) -> Stateful<Div> {
    let label = label.into();
    let (fg, _) = theme(cx).tone(tone);
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(typography::LABEL))
        .text_color(rgb(fg))
        .aria_label(label.clone())
        .child(icon.view(fg).size(px(14.)))
        .child(value.into())
        .tooltip(move |window, cx| kit::tooltip::Tooltip::new(label.clone()).build(window, cx))
}
pub fn avatar(initials: impl Into<SharedString>, tone: Tone, cx: &App) -> kit::avatar::Avatar {
    let (fg, bg) = theme(cx).tone(tone);
    kit::avatar::Avatar::new()
        .name(initials)
        .with_size(px(28.))
        .bg(rgb(bg))
        .text_color(rgb(fg))
}
pub fn icon_avatar(icon: Icon, tone: Tone, cx: &App) -> kit::avatar::Avatar {
    let (fg, bg) = theme(cx).tone(tone);
    kit::avatar::Avatar::new()
        .placeholder(icon.view(fg).size(px(18.)))
        .with_size(px(28.))
        .bg(rgb(bg))
        .text_color(rgb(fg))
}
pub fn keycap(label: impl Into<SharedString>, _: &App) -> kit::kbd::Kbd {
    let label = label.into();
    let key = match label.as_ref() {
        "⌘ ⇧ L" => "cmd-shift-l",
        "⌘ ↵" => "cmd-enter",
        "⌘ K" => "cmd-k",
        _ => label.as_ref(),
    };
    kit::kbd::Kbd::new(Keystroke::parse(key).expect("valid shortcut"))
}
pub fn card(cx: &App) -> Div {
    let p = theme(cx);
    div()
        .flex_col()
        .rounded(px(radius::CARD))
        .border_1()
        .border_color(glass(p.border, 0.7))
        .bg(glass(p.surface, super::GLASS_SURFACE))
}
pub fn divider(cx: &App) -> Div {
    div()
        .h(px(1.))
        .w_full()
        .bg(rgb(theme(cx).border))
        .flex_shrink_0()
}
pub fn progress(value: f32, tone: Tone, cx: &App) -> kit::progress::Progress {
    let value = if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    };
    kit::progress::Progress::new("progress")
        .value(value * 100.)
        .color(rgb(theme(cx).tone(tone).0))
        .w_full()
}
pub fn skeleton(width: f32, _: &App) -> kit::skeleton::Skeleton {
    kit::skeleton::Skeleton::new()
        .w(px(width))
        .h(px(typography::LABEL))
}
pub fn empty_state(
    icon: Icon,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    _: &App,
) -> kit::empty::Empty {
    use kit::empty::*;
    Empty::new().flex_none().header(
        EmptyHeader::new()
            .media(EmptyMedia::new().child(icon.kit()))
            .title(EmptyTitle::new().child(title.into()))
            .description(EmptyDescription::new().child(description.into())),
    )
}
pub fn alert(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    tone: Tone,
    _: &App,
) -> Div {
    let title = title.into();
    let description: SharedString = description.into();
    let variant = match tone {
        Tone::Success => kit::alert::AlertVariant::Success,
        Tone::Warning => kit::alert::AlertVariant::Warning,
        Tone::Danger => kit::alert::AlertVariant::Error,
        _ => kit::alert::AlertVariant::Info,
    };
    div().flex().items_start().gap_2().child(
        kit::alert::Alert::new(title.clone(), description)
            .title(title)
            .with_variant(variant)
            .flex_1(),
    )
}

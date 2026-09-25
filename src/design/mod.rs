//! GPUI Kit を Solo の画面とイベント契約に接続する共通 UI。
mod components;
mod icons;
pub mod input;
mod overlays;
mod select;

pub use crate::design_tokens::*;
pub use components::*;
use gpui_kit::{prelude::*, *};
pub use icons::{DesignAssets, Icon};
pub use input::{InputChanged, Submitted, TextInput};
pub use overlays::{Dialog, DialogEvent, ToastHost};
pub use select::{Select, SelectionChanged};

#[derive(Clone, Copy, Default)]
pub struct Theme(pub ColorScheme);
impl Global for Theme {}
pub fn theme(cx: &App) -> Palette {
    cx.try_global::<Theme>()
        .copied()
        .unwrap_or_default()
        .0
        .palette()
}
pub fn scheme(cx: &App) -> ColorScheme {
    cx.try_global::<Theme>().copied().unwrap_or_default().0
}
pub fn set_theme(scheme: ColorScheme, cx: &mut App) {
    cx.set_global(Theme(scheme));
    use gpui_kit::component::{Theme as KitTheme, ThemeMode};
    KitTheme::change(
        if scheme == ColorScheme::Dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let p = scheme.palette();
    let t = KitTheme::global_mut(cx);
    t.font_family = typography::FONT.into();
    t.font_size = px(typography::BODY);
    t.mono_font_family = typography::MONO.into();
    t.radius = px(radius::CONTROL);
    t.radius_lg = px(radius::CARD);
    t.colors.background = rgb(p.canvas).into();
    t.colors.foreground = rgb(p.text).into();
    t.colors.border = rgb(p.border).into();
    t.colors.input = rgb(p.control_border).into();
    t.colors.ring = rgb(p.focus).into();
    t.colors.primary = rgb(p.accent).into();
    t.colors.primary_hover = rgb(p.accent_hover).into();
    t.colors.primary_active = rgb(p.accent_pressed).into();
    t.colors.primary_foreground = rgb(p.on_accent).into();
    t.colors.button_primary = rgb(p.accent).into();
    t.colors.button_primary_hover = rgb(p.accent_hover).into();
    t.colors.button_primary_active = rgb(p.accent_pressed).into();
    t.colors.button_primary_foreground = rgb(p.on_accent).into();
    t.colors.button = rgb(p.elevated).into();
    t.colors.button_hover = rgb(p.hover).into();
    t.colors.button_active = rgb(p.pressed).into();
    t.colors.button_foreground = rgb(p.text).into();
    t.colors.button_secondary = rgb(p.elevated).into();
    t.colors.button_secondary_hover = rgb(p.hover).into();
    t.colors.button_secondary_active = rgb(p.pressed).into();
    t.colors.button_secondary_foreground = rgb(p.text).into();
    t.colors.secondary = rgb(p.elevated).into();
    t.colors.secondary_hover = rgb(p.hover).into();
    t.colors.secondary_active = rgb(p.pressed).into();
    t.colors.secondary_foreground = rgb(p.text).into();
    t.colors.accent = rgb(p.hover).into();
    t.colors.accent_foreground = rgb(p.text).into();
    t.colors.muted = rgb(p.hover).into();
    t.colors.muted_foreground = rgb(p.muted).into();
    t.colors.popover = rgb(p.elevated).into();
    t.colors.popover_foreground = rgb(p.text).into();
    t.colors.sidebar = rgb(p.sidebar).into();
    t.colors.sidebar_foreground = rgb(p.secondary).into();
    t.colors.sidebar_accent = rgb(p.hover).into();
    t.colors.sidebar_accent_foreground = rgb(p.text).into();
    t.colors.sidebar_border = rgb(p.border).into();
    t.colors.list = rgb(p.surface).into();
    t.colors.list_hover = rgb(p.hover).into();
    t.colors.list_active = rgb(p.accent_soft).into();
    t.colors.list_active_border = rgb(p.accent).into();
    t.colors.caret = rgb(p.accent_text).into();
    t.colors.selection = rgb(p.accent_soft).into();
    t.colors.progress_bar = rgb(p.accent).into();
    t.colors.danger = rgb(p.danger_solid).into();
    t.colors.danger_foreground = rgb(p.on_accent).into();
    t.colors.button_danger = rgb(p.danger_solid).into();
    t.colors.button_danger_foreground = rgb(p.on_accent).into();
    t.tokens = t.colors.into();
    KitTheme::sync_base(cx);
    cx.refresh_windows();
}

pub fn init(cx: &mut App) {
    gpui_kit::init(cx);
    input::bind_keys(cx);
    set_theme(ColorScheme::Dark, cx);
}
pub fn root(cx: &App) -> Div {
    let p = theme(cx);
    div()
        .size_full()
        .bg(rgb(p.canvas))
        .text_color(rgb(p.text))
        .font_family(typography::FONT)
        .text_size(px(typography::BODY))
}

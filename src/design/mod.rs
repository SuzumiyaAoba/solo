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
use std::rc::Rc;

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
    t.colors.background = glass(p.canvas, GLASS_STRONG).into();
    t.colors.foreground = rgb(p.text).into();
    t.colors.border = glass(p.border, 0.72).into();
    t.colors.input = glass(p.control_border, 0.8).into();
    t.colors.ring = rgb(p.focus).into();
    t.colors.primary = rgb(p.accent).into();
    t.colors.primary_hover = rgb(p.accent_hover).into();
    t.colors.primary_active = rgb(p.accent_pressed).into();
    t.colors.primary_foreground = rgb(p.on_accent).into();
    t.colors.button_primary = rgb(p.accent).into();
    t.colors.button_primary_hover = rgb(p.accent_hover).into();
    t.colors.button_primary_active = rgb(p.accent_pressed).into();
    t.colors.button_primary_foreground = rgb(p.on_accent).into();
    t.colors.button = glass(p.elevated, GLASS_SURFACE).into();
    t.colors.button_hover = glass(p.hover, GLASS_HOVER).into();
    t.colors.button_active = glass(p.pressed, GLASS_HOVER).into();
    t.colors.button_foreground = rgb(p.text).into();
    t.colors.button_secondary = glass(p.elevated, GLASS_SURFACE).into();
    t.colors.button_secondary_hover = glass(p.hover, GLASS_HOVER).into();
    t.colors.button_secondary_active = glass(p.pressed, GLASS_HOVER).into();
    t.colors.button_secondary_foreground = rgb(p.text).into();
    t.colors.secondary = glass(p.elevated, GLASS_SURFACE).into();
    t.colors.secondary_hover = glass(p.hover, GLASS_HOVER).into();
    t.colors.secondary_active = glass(p.pressed, GLASS_HOVER).into();
    t.colors.secondary_foreground = rgb(p.text).into();
    t.colors.accent = glass(p.hover, GLASS_HOVER).into();
    t.colors.accent_foreground = rgb(p.text).into();
    t.colors.muted = glass(p.hover, GLASS_SURFACE).into();
    t.colors.muted_foreground = rgb(p.muted).into();
    t.colors.popover = glass(p.elevated, GLASS_ELEVATED).into();
    t.colors.popover_foreground = rgb(p.text).into();
    t.colors.sidebar = glass(p.sidebar, GLASS_SIDEBAR).into();
    t.colors.sidebar_foreground = rgb(p.secondary).into();
    t.colors.sidebar_accent = glass(p.hover, GLASS_HOVER).into();
    t.colors.sidebar_accent_foreground = rgb(p.text).into();
    t.colors.sidebar_border = glass(p.border, 0.6).into();
    t.colors.list = glass(p.surface, GLASS_SURFACE).into();
    t.colors.list_hover = glass(p.hover, GLASS_HOVER).into();
    t.colors.list_active = glass(p.accent_soft, 0.5).into();
    t.colors.list_active_border = rgb(p.accent).into();
    t.colors.caret = rgb(p.accent_text).into();
    t.colors.selection = glass(p.accent_soft, 0.55).into();
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

/// Liquid Glass 用の不透明度。範囲外はクランプする。
pub const GLASS_SURFACE: f32 = 0.62;
pub const GLASS_ELEVATED: f32 = 0.72;
pub const GLASS_SIDEBAR: f32 = 0.55;
pub const GLASS_HOVER: f32 = 0.45;
pub const GLASS_STRONG: f32 = 0.85;

/// トークン色を半透明化して `Rgba` を返す。Liquid Glass の層ごとの透け感。
pub fn glass(color: u32, alpha: f32) -> gpui_kit::Rgba {
    let a = (alpha.clamp(0., 1.) * 255.).round() as u32;
    rgba((color << 8) | a)
}

pub fn root(cx: &App) -> Div {
    let p = theme(cx);
    div()
        .size_full()
        .text_color(rgb(p.text))
        .font_family(typography::FONT)
        .text_size(px(typography::BODY))
        .bg(glass(p.canvas, GLASS_STRONG))
}

/// パネル境界は線を引かず背景色の差で表す。ドラッグ中だけアクセント色を出す。
/// ヒット領域とカーソルはハンドル側が持つため、ここでは見た目だけを返す。
pub fn split_handle(cx: &App) -> gpui_kit::base::ResizeHandleRenderer {
    let p = theme(cx);
    Rc::new(move |handle, _, _| {
        let color = if handle.is_active() {
            glass(p.focus, 0.9)
        } else {
            rgba(0)
        };
        Some(
            div()
                .flex_none()
                .bg(color)
                .map(|v| match handle.axis() {
                    Axis::Horizontal => v.h_full().w(px(1.)),
                    Axis::Vertical => v.w_full().h(px(1.)),
                })
                .into_any_element(),
        )
    })
}

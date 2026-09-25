//! Solo 共通の GPUI コンポーネント。メイン画面と gallery の唯一の実装元。
mod components;
mod icons;
pub mod input;
mod overlays;
mod select;

pub use crate::design_tokens::*;
pub use components::*;
use gpui::{prelude::*, *};
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
    cx.refresh_windows();
}

actions!(design_system, [NextFocus, PreviousFocus]);
pub fn init(cx: &mut App) {
    cx.set_global(Theme::default());
    input::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("tab", NextFocus, None),
        KeyBinding::new("shift-tab", PreviousFocus, None),
    ]);
}
pub fn root(cx: &App) -> Div {
    let p = theme(cx);
    div()
        .size_full()
        .bg(rgb(p.canvas))
        .text_color(rgb(p.text))
        .font_family(typography::FONT)
        .text_size(px(typography::BODY))
        .on_action(|_: &NextFocus, window, _| window.focus_next())
        .on_action(|_: &PreviousFocus, window, _| window.focus_prev())
}

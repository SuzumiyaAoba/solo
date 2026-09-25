use gpui::{prelude::*, *};
use std::borrow::Cow;

/// 同一の 16px グリッド、1.4px stroke で描く独自のアイコン。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon { Plus, Check, Minus, Close, ChevronDown, ChevronRight, ArrowRight, Search, Grid, Layers, Cursor, Text, Sliders, Layout, Bell, Window, Sun, Moon, Copy, Trash, Settings, Info, Warning, Spinner, Folder, Command }
impl Icon {
    pub const ALL: [Self; 26] = [Self::Plus, Self::Check, Self::Minus, Self::Close, Self::ChevronDown, Self::ChevronRight, Self::ArrowRight, Self::Search, Self::Grid, Self::Layers, Self::Cursor, Self::Text, Self::Sliders, Self::Layout, Self::Bell, Self::Window, Self::Sun, Self::Moon, Self::Copy, Self::Trash, Self::Settings, Self::Info, Self::Warning, Self::Spinner, Self::Folder, Self::Command];
    pub fn name(self) -> &'static str {
        match self {
            Self::Plus => "plus", Self::Check => "check", Self::Minus => "minus", Self::Close => "close",
            Self::ChevronDown => "chevron-down", Self::ChevronRight => "chevron-right", Self::ArrowRight => "arrow-right",
            Self::Search => "search", Self::Grid => "grid", Self::Layers => "layers", Self::Cursor => "cursor", Self::Text => "text",
            Self::Sliders => "sliders", Self::Layout => "layout", Self::Bell => "bell", Self::Window => "window", Self::Sun => "sun",
            Self::Moon => "moon", Self::Copy => "copy", Self::Trash => "trash", Self::Settings => "settings", Self::Info => "info",
            Self::Warning => "warning", Self::Spinner => "spinner", Self::Folder => "folder", Self::Command => "command",
        }
    }
    pub fn view(self, color: u32) -> Svg { svg().path(format!("icons/{}", self.name())).size(px(16.)).flex_shrink_0().text_color(rgb(color)) }
    fn body(self) -> &'static str {
        match self {
            Self::Plus => "<path d='M8 3v10M3 8h10'/>",
            Self::Check => "<path d='m3 8 3.2 3.2L13 4.5'/>",
            Self::Minus => "<path d='M3 8h10'/>",
            Self::Close => "<path d='m4 4 8 8M12 4l-8 8'/>",
            Self::ChevronDown => "<path d='m4 6 4 4 4-4'/>",
            Self::ChevronRight => "<path d='m6 4 4 4-4 4'/>",
            Self::ArrowRight => "<path d='M3 8h10m-4-4 4 4-4 4'/>",
            Self::Search => "<circle cx='7' cy='7' r='4.2'/><path d='m10.2 10.2 3.5 3.5'/>",
            Self::Grid => "<rect x='2' y='2' width='4.5' height='4.5' rx='1'/><rect x='9.5' y='2' width='4.5' height='4.5' rx='1'/><rect x='2' y='9.5' width='4.5' height='4.5' rx='1'/><rect x='9.5' y='9.5' width='4.5' height='4.5' rx='1'/>",
            Self::Layers => "<path d='m8 2 6 3.5L8 9 2 5.5 8 2Zm-6 7L8 12l6-3M2 12l6 3 6-3'/>",
            Self::Cursor => "<path d='m3 2 10 7-5 .8L6 14 3 2Z'/>",
            Self::Text => "<path d='M3 3h10M8 3v10M5 13h6M3 3v2m10-2v2'/>",
            Self::Sliders => "<path d='M2 5h5m3 0h4M2 11h8m3 0h1'/><circle cx='8.5' cy='5' r='1.5'/><circle cx='11.5' cy='11' r='1.5'/>",
            Self::Layout => "<rect x='2' y='2.5' width='12' height='11' rx='2'/><path d='M6 3v10M6 6h7.5'/>",
            Self::Bell => "<path d='M6.5 13.5a1.5 1.5 0 0 0 3 0M8 2a3.5 3.5 0 0 1 3.5 3.5v3L13 11H3l1.5-2.5v-3A3.5 3.5 0 0 1 8 2Z'/>",
            Self::Window => "<rect x='2' y='3' width='12' height='10' rx='2'/><path d='M2 6h12'/>",
            Self::Sun => "<circle cx='8' cy='8' r='3'/><path d='M8 1v1M8 14v1M1 8h1m12 0h1M3 3l.7.7M12.3 12.3l.7.7M13 3l-.7.7M3.7 12.3 3 13'/>",
            Self::Moon => "<path d='M13.5 9.3A5.8 5.8 0 0 1 6.7 2a5.8 5.8 0 1 0 6.8 7.3Z'/>",
            Self::Copy => "<rect x='5.5' y='5.5' width='8' height='8' rx='1.5'/><path d='M10.5 5.5V3A1.5 1.5 0 0 0 9 1.5H3A1.5 1.5 0 0 0 1.5 3v6A1.5 1.5 0 0 0 3 10.5h2.5'/>",
            Self::Trash => "<path d='M2 4h12M6 4V2h4v2M3.5 4l.7 10h7.6l.7-10M6.5 7v4m3-4v4'/>",
            Self::Settings => "<circle cx='8' cy='8' r='2.5'/><path d='m7 1-.5 2-1 .6-2-.5L2 5l1.5 1.5v3L2 11l1.5 1.9 2-.5 1 .6.5 2h2l.5-2 1-.6 2 .5L14 11l-1.5-1.5v-3L14 5l-1.5-1.9-2 .5-1-.6L9 1H7Z'/>",
            Self::Info => "<circle cx='8' cy='8' r='6'/><path d='M8 7v4m0-6v.1'/>",
            Self::Warning => "<path d='m8 2 6 11H2L8 2Zm0 4v3m0 2v.1'/>",
            Self::Spinner => "<path d='M8 2a6 6 0 1 1-6 6'/>",
            Self::Folder => "<path d='M2 4V3h4l2 2h6v8H2V4Z'/>",
            Self::Command => "<path d='M5.5 5.5h5v5h-5zM5.5 5.5H4A1.5 1.5 0 1 1 5.5 4v1.5Zm5 0V4A1.5 1.5 0 1 1 12 5.5h-1.5Zm0 5H12a1.5 1.5 0 1 1-1.5 1.5v-1.5Zm-5 0V12A1.5 1.5 0 1 1 4 10.5h1.5Z'/>",
        }
    }
}

pub struct DesignAssets;
impl AssetSource for DesignAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL.into_iter().find(|icon| path == format!("icons/{}", icon.name())).map(|icon| {
            Cow::Owned(format!("<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 16 16' fill='none' stroke='white' stroke-width='1.4' stroke-linecap='round' stroke-linejoin='round'>{}</svg>", icon.body()).into_bytes())
        }))
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Icon::ALL.into_iter().map(|icon| format!("icons/{}", icon.name())).filter(|name| name.starts_with(path)).map(Into::into).collect())
    }
}

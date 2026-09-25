use gpui_kit::{prelude::*, *};

/// GPUI Kit に同梱された Lucide アイコンのアプリ内での名前。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Plus,
    Check,
    Minus,
    Close,
    ChevronDown,
    ChevronRight,
    ArrowRight,
    Search,
    Grid,
    Layers,
    Cursor,
    Text,
    Sliders,
    Layout,
    Bell,
    Window,
    Sun,
    Moon,
    Copy,
    Trash,
    Settings,
    Info,
    Warning,
    Spinner,
    Folder,
    Command,
}
impl Icon {
    pub const ALL: [Self; 26] = [
        Self::Plus,
        Self::Check,
        Self::Minus,
        Self::Close,
        Self::ChevronDown,
        Self::ChevronRight,
        Self::ArrowRight,
        Self::Search,
        Self::Grid,
        Self::Layers,
        Self::Cursor,
        Self::Text,
        Self::Sliders,
        Self::Layout,
        Self::Bell,
        Self::Window,
        Self::Sun,
        Self::Moon,
        Self::Copy,
        Self::Trash,
        Self::Settings,
        Self::Info,
        Self::Warning,
        Self::Spinner,
        Self::Folder,
        Self::Command,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Check => "check",
            Self::Minus => "minus",
            Self::Close => "close",
            Self::ChevronDown => "chevron-down",
            Self::ChevronRight => "chevron-right",
            Self::ArrowRight => "arrow-right",
            Self::Search => "search",
            Self::Grid => "grid",
            Self::Layers => "layers",
            Self::Cursor => "cursor",
            Self::Text => "text",
            Self::Sliders => "sliders",
            Self::Layout => "layout",
            Self::Bell => "bell",
            Self::Window => "window",
            Self::Sun => "sun",
            Self::Moon => "moon",
            Self::Copy => "copy",
            Self::Trash => "trash",
            Self::Settings => "settings",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Spinner => "spinner",
            Self::Folder => "folder",
            Self::Command => "command",
        }
    }
    pub fn kit(self) -> gpui_kit::component::Icon {
        use gpui_kit::assets::IconName as N;
        let name = match self {
            Self::Plus => N::Plus,
            Self::Check => N::Check,
            Self::Minus => N::Minus,
            Self::Close => N::X,
            Self::ChevronDown => N::ChevronDown,
            Self::ChevronRight => N::ChevronRight,
            Self::ArrowRight => N::ArrowRight,
            Self::Search => N::Search,
            Self::Grid => N::LayoutGrid,
            Self::Layers => N::Layers,
            Self::Cursor => N::MousePointer2,
            Self::Text => N::Type,
            Self::Sliders => N::SlidersHorizontal,
            Self::Layout => N::PanelsTopLeft,
            Self::Bell => N::Bell,
            Self::Window => N::AppWindow,
            Self::Sun => N::Sun,
            Self::Moon => N::Moon,
            Self::Copy => N::Copy,
            Self::Trash => N::Trash,
            Self::Settings => N::Settings,
            Self::Info => N::Info,
            Self::Warning => N::TriangleAlert,
            Self::Spinner => N::LoaderCircle,
            Self::Folder => N::Folder,
            Self::Command => N::Command,
        };
        gpui_kit::component::Icon::new(name)
    }
    pub fn view(self, color: u32) -> gpui_kit::component::Icon {
        self.kit()
            .size(px(16.))
            .flex_shrink_0()
            .text_color(rgb(color))
    }
}

pub use gpui_kit::assets::AllAssets as DesignAssets;

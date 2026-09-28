use gpui_kit::{prelude::*, *};
use std::borrow::Cow;

const OPENAI_PATH: &str = "icons/lobe/openai.svg";

/// Lucide の UI アイコンと Lobe Icons のブランドアイコン。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Plus,
    Check,
    Minus,
    Close,
    ChevronDown,
    ChevronRight,
    ArrowRight,
    ArrowLeft,
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
    Terminal,
    ArrowUp,
    Play,
    Square,
    ListPlus,
    FileDiff,
    MessageSquare,
    RotateCcw,
    ExternalLink,
    LogIn,
    Clock,
    Pause,
    CircleCheck,
    Activity,
    Unplug,
    Pin,
    Pencil,
    OpenAi,
}
impl Icon {
    pub const ALL: [Self; 45] = [
        Self::Plus,
        Self::Check,
        Self::Minus,
        Self::Close,
        Self::ChevronDown,
        Self::ChevronRight,
        Self::ArrowRight,
        Self::ArrowLeft,
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
        Self::Terminal,
        Self::ArrowUp,
        Self::Play,
        Self::Square,
        Self::ListPlus,
        Self::FileDiff,
        Self::MessageSquare,
        Self::RotateCcw,
        Self::ExternalLink,
        Self::LogIn,
        Self::Clock,
        Self::Pause,
        Self::CircleCheck,
        Self::Activity,
        Self::Unplug,
        Self::Pin,
        Self::Pencil,
        Self::OpenAi,
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
            Self::ArrowLeft => "arrow-left",
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
            Self::Terminal => "terminal",
            Self::ArrowUp => "arrow-up",
            Self::Play => "play",
            Self::Square => "square",
            Self::ListPlus => "list-plus",
            Self::FileDiff => "file-diff",
            Self::MessageSquare => "message-square",
            Self::RotateCcw => "rotate-ccw",
            Self::ExternalLink => "external-link",
            Self::LogIn => "log-in",
            Self::Clock => "clock",
            Self::Pause => "pause",
            Self::CircleCheck => "circle-check",
            Self::Activity => "activity",
            Self::Unplug => "unplug",
            Self::Pin => "pin",
            Self::Pencil => "pencil",
            Self::OpenAi => "openai",
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
            Self::ArrowLeft => N::ArrowLeft,
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
            Self::Terminal => N::Terminal,
            Self::ArrowUp => N::ArrowUp,
            Self::Play => N::Play,
            Self::Square => N::Square,
            Self::ListPlus => N::ListPlus,
            Self::FileDiff => N::FileDiff,
            Self::MessageSquare => N::MessageSquare,
            Self::RotateCcw => N::RotateCcw,
            Self::ExternalLink => N::ExternalLink,
            Self::LogIn => N::LogIn,
            Self::Clock => N::Clock,
            Self::Pause => N::Pause,
            Self::CircleCheck => N::CircleCheck,
            Self::Activity => N::Activity,
            Self::Unplug => N::Unplug,
            Self::Pin => N::Pin,
            Self::Pencil => N::Pencil,
            Self::OpenAi => return gpui_kit::component::Icon::default().path(OPENAI_PATH),
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

/// Kit の組込みアセットと、バイナリに埋め込んだブランドアイコンを提供する。
pub struct DesignAssets;

impl AssetSource for DesignAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == OPENAI_PATH {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/lobe/openai.svg"
            ))));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_kit::assets::AllAssets.list(path)?;
        if OPENAI_PATH.starts_with(path) {
            assets.push(OPENAI_PATH.into());
        }
        Ok(assets)
    }
}

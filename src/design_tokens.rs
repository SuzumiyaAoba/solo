//! UI の意味に対応した色と寸法。GPUI に依存しない共通仕様。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub canvas: u32,
    pub sidebar: u32,
    pub surface: u32,
    pub elevated: u32,
    pub hover: u32,
    pub pressed: u32,
    pub border: u32,
    pub control_border: u32,
    pub text: u32,
    pub secondary: u32,
    pub muted: u32,
    pub disabled: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub accent_pressed: u32,
    pub accent_text: u32,
    pub accent_soft: u32,
    pub on_accent: u32,
    pub focus: u32,
    pub success: u32,
    pub success_soft: u32,
    pub warning: u32,
    pub warning_soft: u32,
    pub danger: u32,
    pub danger_soft: u32,
    pub danger_solid: u32,
}

pub const DARK: Palette = Palette {
    canvas: 0x151517,
    sidebar: 0x111113,
    surface: 0x1b1b1e,
    elevated: 0x232326,
    hover: 0x2a2a2e,
    pressed: 0x35353a,
    border: 0x303035,
    control_border: 0x70707a,
    text: 0xededf0,
    secondary: 0xb0b0b9,
    muted: 0x95959f,
    disabled: 0x65656f,
    accent: 0x6259ce,
    accent_hover: 0x6e65d9,
    accent_pressed: 0x5148b8,
    accent_text: 0xb5aeff,
    accent_soft: 0x29263e,
    on_accent: 0xffffff,
    focus: 0xa39bf4,
    success: 0x89cfad,
    success_soft: 0x1c3027,
    warning: 0xe2bd83,
    warning_soft: 0x332b20,
    danger: 0xf09aa5,
    danger_soft: 0x38242a,
    danger_solid: 0xb9344b,
};
pub const LIGHT: Palette = Palette {
    canvas: 0xffffff,
    sidebar: 0xf7f7f8,
    surface: 0xfafafb,
    elevated: 0xffffff,
    hover: 0xefeff2,
    pressed: 0xe5e5ea,
    border: 0xe4e4e8,
    control_border: 0x85858f,
    text: 0x242428,
    secondary: 0x5b5b65,
    muted: 0x6b6b76,
    disabled: 0xa0a0a9,
    accent: 0x6259ce,
    accent_hover: 0x554bbf,
    accent_pressed: 0x473daa,
    accent_text: 0x5b50c1,
    accent_soft: 0xeeecfb,
    on_accent: 0xffffff,
    focus: 0x7165df,
    success: 0x26704d,
    success_soft: 0xeaf5ef,
    warning: 0x855610,
    warning_soft: 0xfbf2df,
    danger: 0xb03047,
    danger_soft: 0xfceef0,
    danger_solid: 0xb9344b,
};
impl ColorScheme {
    pub const fn palette(self) -> Palette {
        match self {
            Self::Dark => DARK,
            Self::Light => LIGHT,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
    pub const fn opposite(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}
impl ControlSize {
    pub const fn height(self) -> f32 {
        match self {
            Self::Small => 28.,
            Self::Medium => 34.,
            Self::Large => 40.,
        }
    }
    pub const fn padding(self) -> f32 {
        match self {
            Self::Small => 10.,
            Self::Medium => 12.,
            Self::Large => 16.,
        }
    }
    pub const fn font_size(self) -> f32 {
        match self {
            Self::Small => 12.,
            Self::Medium => 13.,
            Self::Large => 14.,
        }
    }
}

pub mod space {
    pub const XS: f32 = 4.;
    pub const SM: f32 = 8.;
    pub const MD: f32 = 12.;
    pub const LG: f32 = 16.;
    pub const XL: f32 = 24.;
    pub const XXL: f32 = 32.;
    pub const SECTION: f32 = 40.;
    pub const SCALE: [f32; 7] = [XS, SM, MD, LG, XL, XXL, SECTION];
}
pub mod radius {
    pub const SMALL: f32 = 4.;
    pub const CONTROL: f32 = 6.;
    pub const CARD: f32 = 10.;
    pub const DIALOG: f32 = 12.;
}
pub mod typography {
    pub const FONT: &str = ".AppleSystemUIFont";
    pub const MONO: &str = "Menlo";
    pub const CAPTION: f32 = 11.;
    pub const LABEL: f32 = 12.;
    pub const BODY: f32 = 13.;
    pub const LEAD: f32 = 15.;
    pub const HEADING: f32 = 20.;
    pub const TITLE: f32 = 30.;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}
impl Palette {
    pub fn tone(self, tone: Tone) -> (u32, u32) {
        match tone {
            Tone::Neutral => (self.secondary, self.hover),
            Tone::Accent => (self.accent_text, self.accent_soft),
            Tone::Success => (self.success, self.success_soft),
            Tone::Warning => (self.warning, self.warning_soft),
            Tone::Danger => (self.danger, self.danger_soft),
        }
    }
}

/// sRGB の相対輝度によるコントラスト比。ギャラリーと検証で同じ値を使う。
pub fn contrast(foreground: u32, background: u32) -> f64 {
    fn luminance(color: u32) -> f64 {
        let channel = |shift| {
            let value = ((color >> shift) & 255_u32) as f64 / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }
    let a = luminance(foreground);
    let b = luminance(background);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

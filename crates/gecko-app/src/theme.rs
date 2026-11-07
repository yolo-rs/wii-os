use iced::theme::Mode;
use iced::{Color, Theme};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub bg_2: Color,
    pub surface: Color,
    pub surface_2: Color,
    pub border: Color,
    pub border_2: Color,
    pub text: Color,
    pub text_dim: Color,
    pub text_mute: Color,
    pub accent: Color,
    pub purple: Color,
    pub is_dark: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

impl Palette {
    pub const LIGHT: Self = Self {
        bg: self::rgb(0xff, 0xff, 0xff),
        bg_2: self::rgb(0xfa, 0xfa, 0xfa),
        surface: self::rgb(0xf4, 0xf4, 0xf5),
        surface_2: self::rgb(0xe4, 0xe4, 0xe7),
        border: self::rgb(0xe4, 0xe4, 0xe7),
        border_2: self::rgb(0xd4, 0xd4, 0xd8),
        text: self::rgb(0x18, 0x18, 0x1b),
        text_dim: self::rgb(0x52, 0x52, 0x5b),
        text_mute: self::rgb(0xa1, 0xa1, 0xaa),
        accent: self::rgb(0x22, 0xc5, 0x5e),
        purple: self::rgb(0xa8, 0x55, 0xf7),
        is_dark: false,
    };

    pub const DARK: Self = Self {
        bg: self::rgb(0x0a, 0x0a, 0x0a),
        bg_2: self::rgb(0x14, 0x14, 0x14),
        surface: self::rgb(0x1c, 0x1c, 0x1c),
        surface_2: self::rgb(0x26, 0x26, 0x26),
        border: self::rgb(0x26, 0x26, 0x26),
        border_2: self::rgb(0x3f, 0x3f, 0x46),
        text: self::rgb(0xfa, 0xfa, 0xfa),
        text_dim: self::rgb(0xa1, 0xa1, 0xaa),
        text_mute: self::rgb(0x71, 0x71, 0x7a),
        accent: self::rgb(0x22, 0xc5, 0x5e),
        purple: self::rgb(0xb9, 0x6d, 0xff),
        is_dark: true,
    };

    pub fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Dark => Self::DARK,
            Mode::None | Mode::Light => Self::LIGHT,
        }
    }
}

pub fn theme(palette: &Palette) -> Theme {
    let name = if palette.is_dark { "Gecko Dark" } else { "Gecko Light" };
    Theme::custom(
        name.to_owned(),
        iced::theme::palette::Seed {
            background: palette.bg,
            text: palette.text,
            primary: palette.accent,
            success: palette.accent,
            warning: palette.purple,
            danger: palette.purple,
        },
    )
}

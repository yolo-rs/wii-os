use iced::widget::{container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Padding};

use crate::game::{Format, Platform, Region};
use crate::theme::Palette;

pub fn region_badge<'a, Message: 'a>(palette: &Palette, region: Region) -> Element<'a, Message> {
    let color = palette.accent;
    row![
        self::dot_marker(color),
        text(region.short().to_owned()).size(11).color(color)
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

pub fn platform_badge<'a, Message: 'a>(palette: &Palette, platform: Platform) -> Element<'a, Message> {
    let (text_color, bg_color) = match platform {
        Platform::Gcn => {
            let tinted = Color {
                a: 0.20,
                ..palette.purple
            };
            (palette.purple, tinted)
        }
        Platform::Wii => (Color::from_rgb(0.10, 0.10, 0.12), Color::WHITE),
    };
    self::pill(platform.short(), text_color, bg_color)
}

pub fn format_badge<'a, Message: 'a>(palette: &Palette, format: Format) -> Element<'a, Message> {
    text(format.short().to_owned()).size(11).color(palette.text_mute).into()
}

fn pill<'a, Message: 'a>(label: &str, text_color: Color, bg_color: Color) -> Element<'a, Message> {
    container(text(label.to_owned()).size(10).color(text_color))
        .padding(Padding::from([2, 7]))
        .style(move |_| container::Style {
            background: Some(Background::Color(bg_color)),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn dot_marker<'a, Message: 'a>(color: Color) -> Element<'a, Message> {
    container(text(""))
        .width(6)
        .height(6)
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::app::Message;
use crate::game::Game;
use crate::theme::Palette;
use crate::widgets::{badge, cover};

const REGION_COL_WIDTH: f32 = 80.0;
const PLATFORM_COL_WIDTH: f32 = 48.0;
const FORMAT_COL_WIDTH: f32 = 40.0;

pub fn game_row<'a>(palette: &Palette, index: usize, game: &'a Game) -> Element<'a, Message> {
    let disc_marker = if game.disc_id > 0 {
        format!("  ·  disc {}", game.disc_id + 1)
    } else {
        String::new()
    };
    let meta = format!(
        "{} / {}{}  ·  {}",
        game.game_id, game.maker_code, disc_marker, game.file_name
    );

    let info = column![
        text(&game.title).size(16).color(palette.text),
        text(meta).size(11).color(palette.text_mute),
    ]
    .spacing(3);

    let body = row![
        cover::cover(palette, game),
        info.width(Length::Fill),
        container(badge::region_badge(palette, game.region))
            .width(REGION_COL_WIDTH)
            .align_x(Horizontal::Center),
        container(badge::platform_badge(palette, game.platform))
            .width(PLATFORM_COL_WIDTH)
            .align_x(Horizontal::Center),
        container(badge::format_badge(palette, game.format))
            .width(FORMAT_COL_WIDTH)
            .align_x(Horizontal::Center),
    ]
    .spacing(16)
    .align_y(Alignment::Center);

    let hover_bg = palette.surface;
    let text_color = palette.text;
    button(body)
        .on_press(Message::GameClicked(index))
        .padding(Padding::from([10, 12]))
        .width(Length::Fill)
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => hover_bg,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    radius: 0.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..button::Style::default()
            }
        })
        .into()
}

pub fn empty_state<'a>(palette: &Palette, msg: &'a str, hint: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(msg).size(20).color(palette.text_dim),
            text(hint).size(12).color(palette.text_mute),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .padding(48)
    .into()
}

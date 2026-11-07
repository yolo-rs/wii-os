use iced::alignment::{Horizontal, Vertical};
use iced::widget::{container, image, text};
use iced::{Background, Border, ContentFit, Element, Length};

use crate::game::Game;
use crate::theme::Palette;

const COVER_WIDTH: f32 = 96.0;
const COVER_HEIGHT: f32 = 32.0;

pub fn cover<'a, Message: 'a>(palette: &Palette, game: &Game) -> Element<'a, Message> {
    if let Some(handle) = game.banner_handle() {
        return image::Image::new(handle)
            .content_fit(ContentFit::Contain)
            .width(Length::Fixed(COVER_WIDTH))
            .height(Length::Fixed(COVER_HEIGHT))
            .into();
    }

    let letter = game
        .title
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?')
        .to_string();
    let bg = palette.surface;
    let border = palette.border;

    container(text(letter).size(18).color(palette.text_dim))
        .width(Length::Fixed(COVER_WIDTH))
        .height(Length::Fixed(COVER_HEIGHT))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: border,
            },
            ..container::Style::default()
        })
        .into()
}

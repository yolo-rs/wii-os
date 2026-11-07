use iced::widget::{container, text_input};
use iced::{Background, Border, Element, Length, Padding};

use crate::app::Message;
use crate::theme::Palette;

pub fn search_bar<'a>(palette: &Palette, value: &'a str) -> Element<'a, Message> {
    let bg = palette.surface;
    let border = palette.border;
    let icon = palette.text_mute;
    let placeholder = palette.text_mute;
    let value_color = palette.text;
    let selection = palette.accent;

    let input = text_input("Search games", value)
        .on_input(Message::SearchChanged)
        .padding(Padding::from([8, 12]))
        .size(14)
        .style(move |_, _status| text_input::Style {
            background: Background::Color(bg),
            border: Border {
                color: border,
                width: 1.0,
                radius: 8.0.into(),
            },
            icon,
            placeholder,
            value: value_color,
            selection,
        });

    container(input)
        .width(Length::Fill)
        .padding(Padding {
            top: 10.0,
            right: 10.0,
            bottom: 10.0,
            left: 10.0,
        })
        .into()
}

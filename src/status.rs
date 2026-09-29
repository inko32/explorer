//! Status bar: item counts left, view-mode selector right.

use iced::widget::{button, container, row, text, Space};
use iced::{Color, Element, Length};

use crate::model::{self, Explorer, Message, ViewMode};
use crate::ui_common::{bg, dbg, skin, CHROME_BTN};

pub(crate) fn statusbar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let mut left = format!("{} items", state.entries.len());
    if !state.selected.is_empty() {
        let bytes: u64 = state.selected.iter().filter_map(|&i| state.entries.get(i)).map(|e| e.size).sum();
        left += &format!(
            "    {} selected    {}",
            state.selected.len(),
            model::format_size(bytes)
        );
    }
    if !state.status.is_empty() {
        left += &format!("    {}", state.status);
    }

    let view_btn = |icon_name: &'static str, active: bool, msg: Message| {
        button(crate::icon::view(icon_name, 18.0, &skin))
            .width(Length::Fixed(CHROME_BTN))
            .height(Length::Fixed(CHROME_BTN))
            .style(move |_t, status| button::Style {
                background: if active {
                    Some(skin.pressed.into())
                } else {
                    match status {
                        button::Status::Hovered => Some(skin.hover.into()),
                        _ => None,
                    }
                },
                text_color: skin.text,
                border: iced::Border {
                    color: if active { skin.accent } else { Color::TRANSPARENT },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Default::default(),
                snap: true,
            })
            .on_press(msg)
    };

    container(
        row![
            text(left).size(11.0).color(skin.subtle),
            Space::new().width(Length::Fill),
            view_btn("view_details", state.view == ViewMode::Details, Message::SetView(ViewMode::Details)),
            view_btn("view_large", state.view == ViewMode::Large, Message::SetView(ViewMode::Large)),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 15.0 }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(24.0))
    .style(bg(skin.status_bg))
    .into()
}


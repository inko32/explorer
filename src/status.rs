//! Status bar: item counts left, view-mode selector right.

use iced::Widget as _;
use iced::widget::{button, container, row, text, Space};
use iced::{Color, Element, Length};

use crate::model::{self, Explorer, Message};
use crate::ui_common::{bg, skin, CHROME_BTN};

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
        // icon = button / bar_button_multiplier; tight padding (nav_circle)
        let icon_sz = (CHROME_BTN / state.ribbon.bar_button_multiplier()).round().max(8.0);
        button(crate::icon::view(icon_name, icon_sz, &skin))
            .width(Length::Fixed(CHROME_BTN))
            .height(Length::Fixed(CHROME_BTN))
            .padding(((CHROME_BTN - icon_sz) / 2.0).max(0.0))
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
            text(left).size(state.fonts.statusbar).color(skin.subtle),
            Space::new().width(Length::Fill),
            // SPEC/FOLDER-SPEC.MD §5: a view button is active only when
            // the current step index is exactly its preset
            view_btn("view_details", state.view_index == state.view_cfg.presets.details, Message::SetView(state.view_cfg.presets.details)),
            view_btn("view_large", state.view_index == state.view_cfg.presets.large, Message::SetView(state.view_cfg.presets.large)),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 15.0 }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(24.0))
    .style(bg(skin.status_bg))
    .boxed()
}


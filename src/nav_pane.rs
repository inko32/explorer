//! Navigation pane: Favorites / This PC / Network sections.

use std::path::{Path, PathBuf};

use iced::widget::{column, container, mouse_area, row, scrollable, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message};

/// Full navigation pane width (shrinks proportionally below the yaml
/// `sacrifice.nav_resize_below` content width).
pub(crate) const NAV_W: f32 = 210.0;
use crate::ui_common::{bg, dbg, icon, skin};

fn nav_item(
    label: String,
    icon_name: &'static str,
    selected: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let item: Element<Message> = container(
        row![
            crate::icon::view(icon_name, 16.0, &skin),
            text(label).size(12.0).color(skin.text),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([2.0, 0.0]),
    )
    .padding([3.0, 8.0])
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into();
    match msg {
        Some(m) => mouse_area(item).on_press(m).into(),
        None => item,
    }
}

pub(crate) fn nav_pane(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut list = column![].spacing(0).padding(4);

    list = list.push(
        row![
            icon("star", 14.0, skin),
            text("Favorites").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([2.0, 6.0]),
    );
    let mut favs: Vec<(&str, &'static str, Option<PathBuf>)> = vec![
        ("Home", "home", home.clone()),
        ("Desktop", "desktop", home.as_ref().map(|h| h.join("Desktop"))),
        ("Documents", "documents", home.as_ref().map(|h| h.join("Documents"))),
        ("Downloads", "downloads", home.as_ref().map(|h| h.join("Downloads"))),
        ("Recent places", "recent", None),
    ];
    for (label, handle, target) in favs.drain(..) {
        let selected = target.as_deref() == Some(state.cwd.as_path());
        let msg = target.map(Message::Navigate);
        list = list.push(nav_item(label.to_string(), handle, selected, msg, skin));
    }

    list = list.push(
        row![
            icon("computer", 14.0, skin),
            text("This PC").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item(
        "Computer".to_string(),
        "computer",
        state.cwd == Path::new("/"),
        Some(Message::Navigate(PathBuf::from("/"))),
        skin,
    ));
    for v in &state.volumes {
        let selected = state.cwd == v.mount;
        let mount = v.mount.clone();
        list = list.push(nav_item(
            v.name.clone(),
            "drive",
            selected,
            Some(Message::Navigate(mount)),
            skin,
        ));
    }

    list = list.push(
        row![
            icon("network", 14.0, skin),
            text("Network").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item("Network".to_string(), "network", false, None, skin));

    container(list)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .style(bg(skin.nav_bg))
        .into()
}

// ---------------- file view ----------------


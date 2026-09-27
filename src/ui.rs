//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.

pub mod skin {
    use iced::color;

    #[derive(Clone, Copy)]
    pub struct Skin {
        pub accent: iced::Color,
        pub accent_dark: iced::Color,
        pub titlebar_inactive: iced::Color,
        pub window_bg: iced::Color,
        pub nav_bg: iced::Color,
        pub toolbar_bg: iced::Color,
        pub status_bg: iced::Color,
        pub col_header_bg: iced::Color,
        pub hover: iced::Color,
        pub sel: iced::Color,
        pub sel_border: iced::Color,
        pub sep: iced::Color,
        pub text: iced::Color,
        pub subtle: iced::Color,
        pub disabled: iced::Color,
        pub folder: iced::Color,
        pub close_hover: iced::Color,
        pub address_border: iced::Color,
    }

    pub const LIGHT: Skin = Skin {
        accent: color!(0x0078D7),
        accent_dark: color!(0x00559B),
        titlebar_inactive: color!(0xC8C8C8),
        window_bg: color!(0xFFFFFF),
        nav_bg: color!(0xFFFFFF),
        toolbar_bg: color!(0xFFFFFF),
        status_bg: color!(0xFFFFFF),
        col_header_bg: color!(0xF7F7F7),
        hover: color!(0xE5F1FB),
        sel: color!(0xCDE6F7),
        sel_border: color!(0x0078D7),
        sep: color!(0xDADADA),
        text: color!(0x262626),
        subtle: color!(0x666666),
        disabled: color!(0xA0A0A0),
        folder: color!(0xF6C356),
        close_hover: color!(0xE81123),
        address_border: color!(0xA9A9A9),
    };

    pub const DARK: Skin = Skin {
        accent: color!(0x0078D7),
        accent_dark: color!(0x004A86),
        titlebar_inactive: color!(0x3A3A3A),
        window_bg: color!(0x1E1E1E),
        nav_bg: color!(0x252526),
        toolbar_bg: color!(0x2D2D2D),
        status_bg: color!(0x252526),
        col_header_bg: color!(0x2B2B2C),
        hover: color!(0x3A4046),
        sel: color!(0x2F5480),
        sel_border: color!(0x0078D7),
        sep: color!(0x454545),
        text: color!(0xE8E8E8),
        subtle: color!(0xA0A0A0),
        disabled: color!(0x6E6E6E),
        folder: color!(0xC89B3C),
        close_hover: color!(0xE81123),
        address_border: color!(0x555555),
    };

    impl Skin {
        pub fn palette(self) -> iced::theme::Palette {
            iced::theme::Palette {
                background: self.window_bg,
                text: self.text,
                primary: self.accent,
                success: color!(0x1F9E3E),
                warning: color!(0xFFB900),
                danger: self.close_hover,
            }
        }
    }
}

use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, Space};
use iced::{Color, Element, Length};

use crate::model::{self, Entry, EntryKind, Explorer, Message};

pub fn view(state: &Explorer) -> Element<'_, Message> {
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    column![
        titlebar(state, skin),
        address_bar(state, skin),
        row![nav_pane(state, skin), file_view(state, skin)].height(Length::Fill),
        statusbar(state, skin),
    ]
    .into()
}

fn bg(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: Some(color.into()),
        ..Default::default()
    }
}

fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let title = state.title();
    let title_text = text(title)
        .size(12.0)
        .color(Color::WHITE)
        .width(Length::Shrink);
    let drag_zone = mouse_area(
        container(title_text)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(Message::TitlebarPress)
    .on_double_click(Message::TitlebarDoubleClick);

    let caption = |label: &'static str, msg: Message, hover_bg: Color| {
        button(text(label).size(11.0).color(Color::WHITE))
            .width(Length::Fixed(46.0))
            .height(Length::Fill)
            .style(move |_t, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(hover_bg.into()),
                    button::Status::Pressed => Some(hover_bg.scale_alpha(0.7).into()),
                    _ => None,
                },
                text_color: Color::WHITE,
                border: Default::default(),
                shadow: Default::default(),
                snap: true,
            })
            .on_press(msg)
    };

    container(
        row![
            container(Space::new().width(Length::Fixed(8.0)).height(Length::Fixed(30.0)))
                .style(bg(skin.folder)),
            drag_zone,
            caption("—", Message::Minimize, Color::from_rgba(1.0, 1.0, 1.0, 0.18)),
            caption("□", Message::Maximize, Color::from_rgba(1.0, 1.0, 1.0, 0.18)),
            caption("✕", Message::Close, skin.close_hover),
        ]
        .height(Length::Fixed(30.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(30.0))
    .style(bg(skin.accent))
    .into()
}

fn nav_pane(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut items: Vec<(&str, Option<PathBuf>)> = vec![
        ("Desktop", home.as_ref().map(|h| h.join("Desktop"))),
        ("Documents", home.as_ref().map(|h| h.join("Documents"))),
        ("Downloads", home.as_ref().map(|h| h.join("Downloads"))),
    ];
    items.push(("Computer", Some(PathBuf::from("/"))));

    let mut list = column![].spacing(0).padding(4);
    for (label, target) in items {
        let selected = target.as_deref() == Some(state.cwd.as_path());
        let bgc = if selected {
            skin.sel
        } else {
            Color::TRANSPARENT
        };
        let t = target.clone();
        let item: Element<Message> = container(
            text(label.to_string())
                .size(12.0)
                .color(if selected { skin.text } else { skin.text }),
        )
        .padding([3.0, 8.0])
        .width(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: if selected { skin.sel_border } else { Color::TRANSPARENT },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into();
        let el: Element<Message> = match t {
            Some(path) => mouse_area(item).on_press(Message::Navigate(path)).into(),
            None => item,
        };
        list = list.push(el);
    }

    container(list)
        .width(Length::Fixed(210.0))
        .height(Length::Fill)
        .style(bg(skin.nav_bg))
        .into()
}

fn address_bar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let nav_btn = |label: &'static str, msg: Message, enabled: bool| {
        button(text(label).size(12.0).color(if enabled { skin.accent } else { skin.disabled }))
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .style(move |_t, _status| button::Style {
                background: None,
                text_color: if enabled { skin.accent } else { skin.disabled },
                border: Default::default(),
                shadow: Default::default(),
                snap: true,
            })
            .on_press_maybe(enabled.then_some(msg))
    };

    let mut crumbs = row![].spacing(2).align_y(iced::Alignment::Center);
    let segs: Vec<String> = {
        let mut v = vec!["Computer".to_string()];
        for c in state.cwd.components() {
            if let std::path::Component::Normal(c) = c {
                v.push(c.to_string_lossy().into_owned());
            }
        }
        v
    };
    for (i, seg) in segs.iter().enumerate() {
        let is_last = i == segs.len() - 1;
        let seg_text = text(seg.clone()).size(12.0).color(skin.text);
        let cell: Element<Message> = container(seg_text)
            .padding([2.0, 6.0])
            .style(move |_t: &iced::Theme| container::Style {
                background: Some(
                    if is_last { Color::TRANSPARENT } else { Color::TRANSPARENT }.into(),
                ),
                ..Default::default()
            })
            .into();
        let target = prefix_of(&state.cwd, i);
        let cell_el: Element<Message> = if is_last {
            cell
        } else {
            mouse_area(cell).on_press(Message::Navigate(target)).into()
        };
        crumbs = crumbs.push(cell_el);
        if !is_last {
            crumbs = crumbs.push(
                text("❯").size(9.0).color(skin.subtle),
            );
        }
    }

    let crumb_bar = container(crumbs)
        .width(Length::Fill)
        .height(Length::Fixed(26.0))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.window_bg.into()),
            border: iced::Border {
                color: skin.address_border,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        });

    let search = container(
        text(format!("Search {}", state.title()))
            .size(12.0)
            .color(skin.disabled),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(26.0))
    .padding([2.0, 8.0])
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: skin.address_border,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    });

    container(
        row![
            nav_btn("◀", Message::Back, !state.back_is_empty()),
            nav_btn("▶", Message::Forward, !state.forward_is_empty()),
            nav_btn("▲", Message::Up, state.cwd.parent().is_some()),
            crumb_bar,
            search,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .padding(6),
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .style(bg(skin.toolbar_bg))
    .into()
}

fn file_view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let header = container(
        row![
            container(text("Name").size(11.0).color(skin.subtle))
                .width(Length::Fill)
                .padding([4.0, 8.0]),
            container(text("Date modified").size(11.0).color(skin.subtle))
                .width(Length::Fixed(170.0))
                .padding([4.0, 8.0]),
            container(text("Type").size(11.0).color(skin.subtle))
                .width(Length::Fixed(140.0))
                .padding([4.0, 8.0]),
            container(text("Size").size(11.0).color(skin.subtle))
                .width(Length::Fixed(90.0))
                .padding([4.0, 8.0]),
        ]
        .height(Length::Fixed(24.0))
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.col_header_bg.into()),
        border: iced::Border {
            color: skin.sep,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    let mut rows = column![].spacing(0);
    for (i, e) in state.entries.iter().enumerate() {
        rows = rows.push(file_row(i, e, state, skin));
    }

    container(column![header, scrollable(rows).height(Length::Fill)])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into()
}

fn file_row<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected == Some(i);
    let hovered = state.hovered == Some(i);
    let bgc = if selected {
        skin.sel
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };

    let icon_color = if e.kind == EntryKind::Dir { skin.folder } else { skin.text };
    let name = text(e.name.clone()).size(12.0).color(skin.text);
    let date = text(e.modified.map(model::format_date).unwrap_or_default())
        .size(12.0)
        .color(skin.text);
    let typ = text(model::type_label(e)).size(12.0).color(skin.text);
    let size = text(if e.kind == EntryKind::Dir {
        String::new()
    } else {
        model::format_size(e.size)
    })
    .size(12.0)
    .color(skin.text);

    let content = row![
        container(Space::new().width(Length::Fixed(6.0)).height(Length::Fixed(12.0)))
            .style(bg(icon_color)),
        container(name).width(Length::Fill).padding([2.0, 4.0]),
        container(date).width(Length::Fixed(170.0)).padding([2.0, 4.0]),
        container(typ).width(Length::Fixed(140.0)).padding([2.0, 4.0]),
        container(size).width(Length::Fixed(90.0)).padding([2.0, 4.0]),
    ]
    .height(Length::Fixed(22.0))
    .align_y(iced::Alignment::Center);

    let styled = container(content)
        .width(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::Hover(None))
        .into()
}

fn statusbar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let mut left = format!("{} items", state.entries.len());
    if let Some(i) = state.selected {
        if let Some(e) = state.entries.get(i) {
            left += &format!("    1 selected    {}", model::format_size(e.size));
        }
    }
    let dark_btn = button(text(if state.dark { "Light" } else { "Dark" }).size(11.0).color(skin.text))
        .style(move |_t, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(skin.hover.into()),
                _ => None,
            },
            text_color: skin.text,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(Message::ToggleDark);

    container(
        row![
            text(left).size(11.0).color(skin.subtle),
            Space::new().width(Length::Fill),
            dark_btn,
        ]
        .align_y(iced::Alignment::Center)
        .padding([0.0, 8.0]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(23.0))
    .style(bg(skin.status_bg))
    .into()
}

fn prefix_of(path: &Path, seg_index: usize) -> PathBuf {
    if seg_index == 0 {
        return PathBuf::from("/");
    }
    let comps: Vec<_> = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .collect();
    let mut out = PathBuf::from("/");
    for c in comps.iter().take(seg_index) {
        out.push(c.as_os_str());
    }
    out
}

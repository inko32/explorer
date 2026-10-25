//! Navigation band: back/forward/up circles, breadcrumb address bar,
//! search box, and the (invisible) splitter between the two bars.

use std::path::{Path, PathBuf};

use iced::Widget as _;
use iced::widget::{button, container, mouse_area, row, text, Space};
use iced::{Element, Length};

use crate::border::frame_insets;
use crate::model::{Explorer, Message};
use crate::ui_common::{bg, dbg, icon, skin, CHROME_BTN};

fn nav_circle(icon_name: &'static str, enabled: bool, msg: Message, skin: skin::Skin, inner: f32, mult: f32) -> Element<'static, Message> {
    // icon = button / bar_button_multiplier (ribbon.yaml); tight padding
    // so iced's default button padding does not clamp the Fixed icon
    let icon_sz = (inner / mult).round().max(8.0);
    let pad = ((inner - icon_sz) / 2.0).max(0.0);
    let btn = button(crate::icon::view(icon_name, icon_sz, &skin))
        .width(Length::Fixed(inner))
        .height(Length::Fixed(inner))
        .padding(pad)
        .style(move |_t, status| button::Style {
            background: match (enabled, status) {
                (true, button::Status::Hovered) => Some(skin.nav_hover.into()),
                (true, button::Status::Pressed) => Some(skin.pressed.into()),
                _ => None,
            },
            text_color: if enabled { skin.accent } else { skin.disabled },
            border: iced::Border {
                color: match (enabled, status) {
                    (true, button::Status::Hovered) => skin.nav_hover_border,
                    (true, button::Status::Pressed) => skin.nav_hover_border,
                    (true, _) => skin.accent,
                    _ => skin.sep,
                },
                width: 1.0,
                radius: (inner / 2.0).into(),
            },
            shadow: Default::default(),
            snap: true,
        });
    if enabled {
        btn.on_press(msg).boxed()
    } else {
        btn.boxed()
    }
}

pub(crate) fn address_bar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fl - fr).max(320.0);
    let search_w = (content_w * state.search_share).clamp(120.0, (content_w - 280.0).max(120.0));
    // inner bar size derives from the band height (8px band padding)
    let inner = (state.ribbon.band_height() - 8.0).max(12.0);
    let text_sz = (inner * 0.55).round().max(8.0);
    let refresh_icon = (inner / state.ribbon.bar_button_multiplier()).round().max(8.0);
    let refresh = button(icon("refresh", refresh_icon, skin))
        .width(Length::Fixed(inner))
        .height(Length::Fixed(inner))
        .padding(((inner - refresh_icon) / 2.0).max(0.0))
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
        .on_press(Message::Refresh);

    let mult = state.dpi_multiplier();
    let btn_mult = state.ribbon.bar_button_multiplier();
    let show_search = content_w >= state.ribbon.sacrifice().3 * mult;
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
        let seg_text = text(seg.clone()).size(text_sz).color(skin.text);
        let cell: Element<Message> = container(seg_text)
            .padding([0.0, 4.0])
            .style(move |_t: &iced::Theme| container::Style {
                background: None,
                ..Default::default()
            })
            .boxed();
        let cell_el: Element<Message> = if is_last {
            cell
        } else {
            let target = prefix_of(&state.cwd, i);
            button(cell)
                .style(move |_t, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(skin.hover.into()),
                        button::Status::Pressed => Some(skin.pressed.into()),
                        _ => None,
                    },
                    text_color: skin.text,
                    border: Default::default(),
                    shadow: Default::default(),
                    snap: true,
                })
                .on_press(Message::Navigate(target))
                .boxed()
        };
        crumbs = crumbs.push(cell_el);
        if !is_last {
            crumbs = crumbs.push(icon("chevron_right", (inner * 0.5).round().max(5.0), skin));
        }
    }

    let crumb_bar = container(
        row![crumbs, Space::new().width(Length::Fill), refresh]
            .align_y(iced::Alignment::Center)
            .padding([0.0, 4.0]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(inner))
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: skin.nav_border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // search box: placeholder left, recessed magnifier button at the right
    // end (Win8: light-blue tint + thin divider)
    let search_btn = container(icon("search", (inner * 0.67).round().max(6.0), skin))
        .width(Length::Fixed(inner + 2.0))
        .height(Length::Fill)
        .center_x(Length::Fixed(inner + 2.0))
        .center_y(Length::Fixed(inner))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.search_btn_bg.into()),
            border: iced::Border {
                color: skin.search_btn_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    let search = container(
        row![
            text(format!("Search {}", state.title()))
                .size(text_sz)
                .color(skin.disabled),
            Space::new().width(Length::Fill),
            search_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(search_w))
    .height(Length::Fixed(inner))
    .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 4.0 })
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: skin.nav_border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // invisible splitter: the natural gap between the bars IS the drag
    // target (halved: 15 -> 8 px total); only the cursor betrays it
    let splitter = mouse_area(
        Space::new().width(Length::Fixed(4.0)).height(Length::Fixed(inner)),
    )
    .on_press(Message::BeginBarSplit)
    .on_enter(Message::SplitterHover(true))
    .on_exit(Message::SplitterHover(false))
    .interaction(iced::mouse::Interaction::ResizingHorizontally);
    let mut band: iced::widget::Row<Element<'_, Message>> = row![
        nav_circle("back", !state.back_is_empty(), Message::Back, skin, inner, btn_mult),
        nav_circle("forward", !state.forward_is_empty(), Message::Forward, skin, inner, btn_mult),
        nav_circle("up", state.cwd.parent().is_some(), Message::Up, skin, inner, btn_mult),
        crumb_bar,
    ];
    if show_search {
        band = band.push(splitter.boxed()).push(search.boxed());
    }
    container(band.spacing(2).align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(state.ribbon.band_height()))
        .center_y(Length::Fixed(state.ribbon.band_height()))
        .padding([0.0, 4.0])
        .style(bg(skin.toolbar_bg))
        .boxed()
}

// ---------------- nav pane ----------------


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


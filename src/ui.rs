//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.


use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message};
use crate::ribbon;
use crate::ui_common::{bg, dbg, skin, UI_DEBUG, MENU_ITEM_H, MENU_SECTION_H, MENU_W};

pub fn view(state: &Explorer) -> Element<'_, Message> {
    UI_DEBUG.store(state.ui_debug, std::sync::atomic::Ordering::Relaxed);
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    let mut root = column![];
    if state.self_border {
        root = root.push(dbg("titlebar", crate::border::titlebar(state, skin)));
    }
    root = root.push(dbg("ribbon", crate::ribbon::view(state, skin)));
    root = root.push(dbg("address_bar", crate::navigation::address_bar(state, skin)));
    let nav = dbg("nav_pane", crate::nav_pane::nav_pane(state, skin));
    let fv = dbg("file_view", crate::folder::file_view(state, skin));
    root = root.push(dbg("main_row", row![nav, fv].height(Length::Fill)));
    root = root.push(dbg("statusbar", crate::status::statusbar(state, skin)));
    // ribbon dropdown menu (above everything; backdrop click closes)
    let menu_layer: Option<Element<Message>> = state.open_menu.as_ref().map(|m| menu_overlay(state, m, skin));
    let (ft, fr, fb, fl) = crate::border::frame_insets(state);
    let framed: Element<Message> = if state.self_border {
        // Win8: two-tone frame — deep 1px outer edge (#368BE9), light fill
        // (#6BADF6); total insets unchanged (2/6/6/6). Win10: 1px dark.
        if state.frame_style == crate::model::FrameStyle::Win8 {
            container(
                container(root)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(iced::Padding {
                        top: ft - 1.0,
                        right: fr - 1.0,
                        bottom: fb - 1.0,
                        left: fl - 1.0,
                    })
                    .style(move |_: &iced::Theme| container::Style {
                        background: Some(skin.accent.into()),
                        ..Default::default()
                    }),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(1.0)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(skin.accent_dark.into()),
                ..Default::default()
            })
            .into()
        } else {
            container(root)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top: ft,
                    right: fr,
                    bottom: fb,
                    left: fl,
                })
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(crate::border::frame_color(state, skin).into()),
                    ..Default::default()
                })
                .into()
        }
    } else {
        root.into()
    };
    if state.self_border {
        let grab = if state.frame_style == crate::model::FrameStyle::Win8 { 8.0 } else { 5.0 };
        let mut s = stack![dbg("frame", framed), dbg("resize_ring", crate::border::resize_frame(grab)),
                           dbg("captions", crate::border::caption_overlay(state, skin))];
        // splitter drag: hold the resize cursor window-wide while dragging
        // (the pointer leaves the small splitter target mid-drag)
        if state.bar_split {
            s = s.push(
                mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally),
            );
        }
        if let Some(l) = menu_layer {
            s = s.push(dbg("menu", l));
        }
        s.into()
    } else {
        match menu_layer {
            Some(l) => stack![framed, l].into(),
            None => framed,
        }
    }
}

/// Dropdown menu overlay: transparent backdrop (click closes) + the menu
/// popup anchored under the ribbon at x. MS Paint metrics: menu bg FBFCFD,
/// border DCDDDE, fixed 140px width; items 22px (16px icon + 16px gap +
/// label, TMenuItem Insets(3,8,3,5)); section headers 28px bold on F5F6F7
/// with D9DADB lines (TMenuSection).
fn menu_overlay(state: &Explorer, m: &crate::model::OpenMenu, skin: skin::Skin) -> Element<'static, Message> {
    use crate::model::MenuEntry;
    let has_sel = !state.selected.is_empty();
    let has_clip = state.clipboard.is_some();
    let has_items = !state.entries.is_empty();
    // resolve labels/icons for item rows (sections carry their own label)
    let mut labels: Vec<(MenuEntry, String, Option<String>, bool)> = Vec::new();
    for entry in &m.items {
        match entry {
            MenuEntry::Section(name) => {
                labels.push((entry.clone(), name.clone(), None, false));
            }
            MenuEntry::Item(id) => {
                // labels: ribbon button label -> command-table label -> raw id
                let mut label = String::new();
                let mut icon_name: Option<String> = None;
                for tab in state.ribbon.tabs.iter().chain(state.ribbon.contextual_tabs.iter()) {
                    for g in &tab.groups {
                        for c in &g.commands {
                            if &c.id == id {
                                label = c.label.clone();
                                icon_name = c.icon.clone();
                            }
                        }
                    }
                }
                if label.is_empty() {
                    label = state
                        .ribbon
                        .commands
                        .get(id)
                        .and_then(|d| d.label.clone())
                        .unwrap_or_else(|| id.clone());
                }
                let enabled = ribbon::command_enabled(id, has_sel, has_clip, has_items);
                labels.push((entry.clone(), label, icon_name, enabled));
            }
        }
    }
    // TPopupMenu setWidth(140): 140 is the floor, labels can push it wider
    let w = labels
        .iter()
        .map(|(_, l, _, _)| l.chars().count() as f32 * 7.0 + 34.0)
        .fold(m.source_w.max(MENU_W), f32::max);
    let mut items_col = column![].spacing(0);
    let mut first_section = true;
    for (entry, label, icon_name, enabled) in labels {
        match entry {
            MenuEntry::Section(_) => {
                // TMenuSection: F5F6F7 fill, D9DADB lines (no top line on the
                // first section), bold 12px text at 10px from the left;
                // height = font(12) + 16 = 28
                let top_line: Element<Message> = if first_section {
                    Space::new().width(Length::Fill).height(Length::Fixed(0.0)).into()
                } else {
                    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                        .style(move |_| container::Style {
                            background: Some(skin.menu_section_border.into()),
                            ..Default::default()
                        })
                        .into()
                };
                first_section = false;
                let head = container(
                    text(label)
                        .size(12.0)
                        .color(skin.text)
                        .width(Length::Fill),
                )
                .width(Length::Fixed(w))
                .height(Length::Fixed(MENU_SECTION_H - 2.0))
                .align_y(iced::Alignment::Center)
                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 })
                .style(bg(skin.ribbon_bg));
                let bottom_line = container(
                    Space::new().width(Length::Fill).height(Length::Fixed(1.0)),
                )
                .style(move |_| container::Style {
                    background: Some(skin.menu_section_border.into()),
                    ..Default::default()
                });
                items_col = items_col.push(column![top_line, head, bottom_line].width(Length::Fixed(w)));
            }
            MenuEntry::Item(id) => {
                // TMenuItem: 16px icon + 16px iconTextGap + label, Insets(3,8,3,5)
                let label_row = row![
                    container(
                        crate::icon::view(icon_name.as_deref().unwrap_or("file"), 16.0, &skin),
                    )
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0))
                    .center_x(Length::Fixed(16.0))
                    .center_y(Length::Fixed(16.0)),
                    text(label)
                        .size(12.0)
                        .color(if enabled { skin.text } else { skin.disabled }),
                ]
                .spacing(16)
                .align_y(iced::Alignment::Center);
                let item = button(
                    container(label_row)
                        .width(Length::Fill)
                        .height(Length::Fixed(MENU_ITEM_H))
                        .center_y(Length::Fixed(MENU_ITEM_H))
                        .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 8.0 }),
                )
                .width(Length::Fixed(w))
                .height(Length::Fixed(MENU_ITEM_H))
                .padding(0)
                .style(move |_t: &iced::Theme, status: button::Status| button::Style {
                    background: match (enabled, status) {
                        (true, button::Status::Hovered) => Some(skin.hover_btn.into()),
                        _ => None,
                    },
                    text_color: if enabled { skin.text } else { skin.disabled },
                    border: Default::default(),
                    shadow: Default::default(),
                    snap: true,
                });
                let item_el: Element<Message> = if enabled {
                    item.on_press(Message::MenuPick(id)).into()
                } else {
                    item.into()
                };
                items_col = items_col.push(item_el);
            }
        }
    }
    let popup = container(items_col)
        .width(Length::Fixed(w))
        .style(move |_| container::Style {
            background: Some(skin.menu_bg.into()),
            border: iced::Border {
                color: skin.menu_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                offset: iced::Vector::new(2.0, 2.0),
                blur_radius: 6.0,
            },
            ..Default::default()
        });
    // anchors are content-relative; the content is inset by the frame
    let (ft, _, _, fl) = crate::border::frame_insets(state);
    let menu_y = ft + m.y;
    let backdrop = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)),
    )
    .on_press(Message::CloseMenu);
    let positioned = container(popup)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: menu_y,
            left: fl + m.x,
            ..Default::default()
        })
        .align_x(iced::Alignment::Start)
        .align_y(iced::Alignment::Start);
    stack![backdrop, positioned].into()
}


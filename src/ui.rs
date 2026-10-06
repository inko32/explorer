//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.


use iced::widget::{column, container, mouse_area, row, stack, Space};
use iced::{Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{dbg, skin, UI_DEBUG};

pub fn view(state: &Explorer) -> Element<'_, Message> {
    UI_DEBUG.store(state.ui_debug, std::sync::atomic::Ordering::Relaxed);
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    let (ft, fr, fb, fl) = crate::border::frame_insets(state);
    let content_w = state.win_w - fl - fr;
    // thresholds are in ribbon layout units; win_w is in compositor
    // units — the --dpi multiplier separates the two
    let mult = state.dpi_multiplier();
    let (min_ribbon_w, min_nav_w, nav_resize_below, _) = state.ribbon.sacrifice();
    let (min_ribbon_w, min_nav_w, nav_resize_below) = (
        min_ribbon_w * mult,
        min_nav_w * mult,
        nav_resize_below * mult,
    );
    let mut root = column![];
    if state.self_border {
        root = root.push(dbg("titlebar", crate::border::titlebar(state, skin)));
    }
    // sacrifice the ribbon entirely when the window is resized too small
    if content_w >= min_ribbon_w {
        root = root.push(dbg("ribbon", crate::ribbon::view(state)));
    }
    root = root.push(dbg("address_bar", crate::navigation::address_bar(state, skin)));
    // sacrifice the navigation pane when the window gets too narrow
    if content_w >= min_nav_w {
        // below nav_resize_below the nav pane shrinks proportionally,
        // keeping its share of the content width
        let nav_w = if content_w >= nav_resize_below {
            crate::nav_pane::NAV_W
        } else {
            (content_w * crate::nav_pane::NAV_W / nav_resize_below).max(110.0)
        };
        let nav = dbg("nav_pane", crate::nav_pane::nav_pane(state, skin, nav_w));
        let fv = dbg("file_view", crate::folder::file_view(state, skin, content_w - nav_w));
        root = root.push(dbg("main_row", row![nav, fv].height(Length::Fill)));
    } else {
        let fv = dbg("file_view", crate::folder::file_view(state, skin, content_w));
        root = root.push(dbg(
            "main_row",
            container(fv).width(Length::Fill).height(Length::Fill),
        ));
    }
    root = root.push(dbg("statusbar", crate::status::statusbar(state, skin)));
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
        s.into()
    } else {
        framed
    }
}

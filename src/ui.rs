//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.


use iced::Widget as _;
use iced::widget::{column, container, mouse_area, row, stack, Space};
use iced::{Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{bg, dbg, skin, UI_DEBUG};

/// A pane splitter: an invisible (colorless) 4px drag target.
/// A pane splitter: an invisible (colorless) 4px drag target. It needs
/// the window background explicitly — without it the accent frame fill
/// behind the main row would shine through the gap.
fn pane_splitter(msg: Message, skin: skin::Skin) -> Element<'static, Message> {
    mouse_area(
        container(Space::new().width(Length::Fixed(4.0)).height(Length::Fill))
            .style(bg(skin.window_bg)),
    )
    .on_press(msg)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .boxed()
}

/// A functionless spacer (window background; with `line` the grey
/// theme separator instead).
fn spacer(w: f32, skin: skin::Skin, line: bool) -> Element<'static, Message> {
    container(Space::new().width(Length::Fixed(w)).height(Length::Fill))
        .style(bg(if line { skin.sep } else { skin.window_bg }))
        .boxed()
}

pub fn view(state: &Explorer) -> Element<'_, Message> {
    UI_DEBUG.store(state.ui_debug, std::sync::atomic::Ordering::Relaxed);
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    let (ft, fr, fb, fl) = crate::border::frame_insets(state);
    let content_w = state.win_w - fl - fr;
    // thresholds are in ribbon layout units; win_w is in compositor
    // units — the --dpi multiplier separates the two
    let mult = state.dpi_multiplier();
    // nav/side widths and their hide thresholds live in the model
    // (layout_widths); only the ribbon sacrifice stays here
    let min_ribbon_w = state.ribbon.sacrifice().0 * mult;
    let mut root = column![];
    if state.self_border {
        root = root.push(dbg("titlebar", crate::border::titlebar(state, skin)));
    }
    // sacrifice the ribbon entirely when the window is resized too small
    if content_w >= min_ribbon_w {
        root = root.push(dbg("ribbon", crate::ribbon::view(state)));
    }
    root = root.push(dbg("address_bar", crate::navigation::address_bar(state, skin)));
    // sacrifice the navigation pane when the window gets too narrow;
    // the widths come from the model (yaml sacrifice rules + the
    // splitter-dragged widths)
    let (nav_w, side_w) = state.layout_widths();
    if nav_w > 0.0 {
        let nav = dbg("nav_pane", crate::nav_pane::nav_pane(state, skin, nav_w));
        let fv = dbg("file_view", crate::folder::file_view(state, skin, content_w - nav_w - side_w));
        let mut main: iced::widget::Row<Element<'_, Message>> = row![
            nav,
            dbg("nav_splitter", pane_splitter(Message::BeginNavSplit, skin)),
            // functionless spacer (yaml metrics.nav_gap)
            spacer(state.ribbon.nav_gap(), skin, false),
            fv,
        ]
        .height(Length::Fill);
        // side pane right of the folder view (Preview / Details),
        // resizable via the splitter on its left edge; it hides before
        // the folder view gets too narrow (yaml sacrifice.pane_below)
        if side_w > 0.0 {
            main = main.push(dbg("pane_splitter", pane_splitter(Message::BeginPaneSplit, skin)));
            // 1px grey theme separator hugging the pane's left edge
            main = main.push(spacer(1.0, skin, true));
            main = main.push(dbg("side_pane", crate::pane::view(state, skin, side_w)));
        }
        root = root.push(dbg("main_row", main.boxed()));
    } else {
        // the nav-gap spacer belongs to the folder view: it shows even
        // with the nav pane hidden
        let fv = dbg("file_view", crate::folder::file_view(state, skin, content_w - side_w));
        let mut main: iced::widget::Row<Element<'_, Message>> = row![
            spacer(state.ribbon.nav_gap(), skin, false),
            container(fv).width(Length::Fill).height(Length::Fill)
        ]
        .height(Length::Fill);
        if side_w > 0.0 {
            main = main.push(dbg("pane_splitter", pane_splitter(Message::BeginPaneSplit, skin)));
            // 1px grey theme separator hugging the pane's left edge
            main = main.push(spacer(1.0, skin, true));
            main = main.push(dbg("side_pane", crate::pane::view(state, skin, side_w)));
        }
        root = root.push(dbg("main_row", main.boxed()));
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
            .boxed()
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
                .boxed()
        }
    } else {
        root.boxed()
    };
    if state.self_border {
        let grab = if state.frame_style == crate::model::FrameStyle::Win8 { 8.0 } else { 5.0 };
        let mut s: iced::widget::Stack<Element<'_, Message>> = stack![dbg("frame", framed), dbg("resize_ring", crate::border::resize_frame(grab)),
                           dbg("captions", crate::border::caption_overlay(state, skin))];
        // splitter drag: hold the resize cursor window-wide while dragging
        // (the pointer leaves the small splitter target mid-drag)
        if state.bar_split || state.pane_split || state.nav_split {
            s = s.push(
                mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .boxed(),
            );
        }
        // context menu above everything (SPEC/MENU_SPEC.MD): in-window
        // by default, overlay layer with menu.yaml layout.popup
        if let Some(open) = &state.menu {
            let el = if state.menu_cfg.layout.popup {
                crate::menu::host(open, &state.menu_cfg, skin)
            } else {
                crate::menu::layer(open, &state.menu_cfg, skin)
            };
            s = s.push(dbg("menu", el));
        }
        s.boxed()
    } else {
        if let Some(open) = &state.menu {
            let el = if state.menu_cfg.layout.popup {
                crate::menu::host(open, &state.menu_cfg, skin)
            } else {
                crate::menu::layer(open, &state.menu_cfg, skin)
            };
            return stack![framed, dbg("menu", el)].boxed();
        }
        framed
    }
}

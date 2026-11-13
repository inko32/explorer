//! Navigation band: back/forward/history-chevron/up buttons, the
//! breadcrumb address bar, the search box, and the (invisible)
//! splitter between the two bars.
//!
//! Layout: SPEC/Address_bar.md — 6px top/bottom, 8px left, 12px right
//! band spaces; 24px buttons with the 22px address bar centered; the
//! 12px gap between the bars is the draggable splitter. All sizes are
//! logical (DPI-scaled with the UI); only border widths keep a constant
//! physical size (`theme::border_w`).

use std::path::{Path, PathBuf};

use iced::Widget as _;
use iced::widget::{
    button, container, mouse_area, row, svg, text, text_input, tooltip, Space,
};
use iced::{Element, Length};

use crate::border::frame_insets;
use crate::model::{Explorer, Message};
use crate::ui_common::{bg, icon, skin};

// logical sizes (SPEC/Address_bar.md)
const BTN: f32 = 24.0; // back / forward / up buttons
const BAR: f32 = 22.0; // address bar height, border included
const BACK_LIST_W: f32 = 13.0; // history chevron between forward and up
const ARROW_W: f32 = 15.0; // breadcrumb dropdown arrow
const RIGHT_LIST: f32 = 18.0; // address bar right-list button
const LEAD_ICON: f32 = 16.0; // leading location icon in the address bar
const CRUMB_TEXT: f32 = 12.0; // breadcrumb / search text size (spec px + 2)
const TRI_W: f32 = 7.0; // chrome triangle width (down-pointing)
const TRI_H: f32 = 4.0; // triangle height (back_list)
/// Breadcrumb arrow glyph: an isoceles right triangle (90° apex,
/// 45° base angles — height = 2 × width). Idle it points right,
/// open it points down (the same triangle rotated).
const ARR_R: (f32, f32) = (4.0, 8.0); // ► width, height
const ARR_D: (f32, f32) = (8.0, 4.0); // ▼ width, height
const CHEVRON_H: f32 = 6.0; // right-list chevron height
/// Glyph colors the design fixes (not theme tokens).
const TRI_IDLE: &str = "#808080";
const TRI_HOVER: &str = "#50ACFF";

/// Widget id of the address edit field (the model focuses/selects it
/// when edit mode starts).
pub(crate) const ADDRESS_INPUT_ID: &str = "address-input";

/// Widget id of the search edit field (the model focuses it when edit
/// mode starts).
pub(crate) const SEARCH_INPUT_ID: &str = "search-input";

/// 1px logical border at a constant physical width (SPEC/Border_DPI.md).
fn border1(color: iced::Color) -> iced::Border {
    iced::Border {
        color,
        width: crate::theme::border_w(1.0),
        radius: 0.0.into(),
    }
}

/// Triangle orientation of the chrome dropdown glyphs: the breadcrumb
/// arrows idle pointing right and snap to `Down` while their dropdown
/// is open; the right-list chevron always points down.
#[derive(Clone, Copy)]
enum Tri {
    Down,
    Right,
}

/// Idle breadcrumb arrows point right, open ones point down.
fn dir_of(open: bool) -> Tri {
    if open {
        Tri::Down
    } else {
        Tri::Right
    }
}

/// Small solid chrome triangle (the spec fixes these colors, so they
/// are literal SVGs rather than skin-colored pack icons).
fn triangle(dir: Tri, w: f32, h: f32, color: &str) -> Element<'static, Message> {
    let pts = match dir {
        Tri::Down => format!("0,0 {w},0 {},{h}", w / 2.0),
        // isoceles right triangle: the apex (right) is 90°, the base
        // corners 45° — i.e. the height is 2x the width
        Tri::Right => format!("0,0 0,{h} {w},{}", h / 2.0),
    };
    svg(svg::Handle::from_memory(
        format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w} {h}'>\
                 <polygon points='{pts}' fill='{color}'/></svg>")
        .into_bytes(),
    ))
    .width(Length::Fixed(w))
    .height(Length::Fixed(h))
    .boxed()
}

/// A breadcrumb member button: idle it is fully transparent (the
/// adjacent elements touch — connected borders); hovered it draws the
/// light-blue background + 1px border. `grey` is the special
/// arrow-hovered state: the text element then shows a plain grey fill
/// (grey border) while its arrow stays blue (SPEC/Address_bar.md).
fn crumb_btn(skin: skin::Skin, hovered: bool, grey: bool) -> button::Style {
    let (fill, border) = if hovered {
        (skin.nav_hover, skin.nav_hover_border)
    } else if grey {
        (skin.nav_grey, skin.address_border)
    } else {
        return button::Style {
            background: None,
            text_color: skin.text,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        };
    };
    button::Style {
        background: Some(fill.into()),
        text_color: skin.text,
        border: border1(border),
        shadow: Default::default(),
        snap: true,
    }
}

/// Standalone breadcrumb chrome (the independent first arrow, the
/// right-list button): hover/press show a background + border.
fn plate(skin: skin::Skin, status: button::Status) -> button::Style {
    button::Style {
        background: match status {
            button::Status::Hovered => Some(skin.nav_hover.into()),
            button::Status::Pressed => Some(skin.pressed.into()),
            _ => None,
        },
        text_color: skin.text,
        border: match status {
            button::Status::Hovered | button::Status::Pressed => border1(skin.nav_hover_border),
            _ => Default::default(),
        },
        shadow: Default::default(),
        snap: true,
    }
}

/// Back/forward: 24px logical button holding a 20px-diameter circle
/// with a 1px border (accent when enabled, grey when not), centered.
fn nav_circle(
    icon_name: &'static str,
    enabled: bool,
    msg: Message,
    skin: skin::Skin,
) -> Element<'static, Message> {
    const CIRCLE: f32 = 20.0;
    const ICON: f32 = 11.0;
    let btn = button(icon(icon_name, ICON, skin))
        .width(Length::Fixed(CIRCLE))
        .height(Length::Fixed(CIRCLE))
        .padding((CIRCLE - ICON) / 2.0)
        .style(move |_t, status| button::Style {
            background: match (enabled, status) {
                (true, button::Status::Hovered) => Some(skin.nav_hover.into()),
                (true, button::Status::Pressed) => Some(skin.pressed.into()),
                _ => None,
            },
            text_color: skin.text,
            border: iced::Border {
                color: match (enabled, status) {
                    (true, button::Status::Hovered) | (true, button::Status::Pressed) => {
                        skin.nav_hover_border
                    }
                    (true, _) => skin.accent,
                    _ => skin.sep,
                },
                width: crate::theme::border_w(1.0),
                radius: (CIRCLE / 2.0).into(),
            },
            shadow: Default::default(),
            snap: true,
        });
    let el = if enabled {
        btn.on_press(msg).boxed()
    } else {
        btn.boxed()
    };
    // the circle floats centered inside the 24px logical button
    container(el)
        .width(Length::Fixed(BTN))
        .height(Length::Fixed(BTN))
        .center_x(Length::Fixed(BTN))
        .center_y(Length::Fixed(BTN))
        .boxed()
}

/// History chevron (back/forward dropdown): 13x24 button, 7x4 down
/// triangle centered inside, #808080 -> #50ACFF on hover, no hovering
/// border. The history dropdown itself is a deferred feature; the
/// button is inert.
fn back_list_btn(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    // the glyph keeps its blue hover color while the dropdown is open
    let open = state
        .menu
        .as_ref()
        .and_then(|m| m.bar.as_ref())
        .is_some_and(|b| b.kind == crate::menu::BarMenuKind::History);
    let color = if state.back_list_hover || open {
        TRI_HOVER
    } else {
        TRI_IDLE
    };
    mouse_area(
        button(triangle(Tri::Down, TRI_W, TRI_H, color))
            .width(Length::Fixed(BACK_LIST_W))
            .height(Length::Fixed(BTN))
            .padding([(BTN - TRI_H) / 2.0, (BACK_LIST_W - TRI_W) / 2.0])
            .style(move |_t, _| crumb_btn(skin, false, false))
            .on_press(Message::HistoryMenu),
    )
    .on_enter(Message::BackListHover(true))
    .on_exit(Message::BackListHover(false))
    .boxed()
}

/// Up one level: 24px square with the 8x12 arrow glyph (triangle 8x6,
/// 2px stem, #7E7E7E with a 1px white skin line — the white copy sits
/// 1px right and clips at the viewBox edge), a hovering border, and a
/// hovering caption.
fn up_btn(enabled: bool, skin: skin::Skin) -> Element<'static, Message> {
    const UP_ARROW: &str = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 8 12'>\
        <polygon points='1,6 9,6 5,0' fill='#FFFFFF'/>\
        <rect x='4' y='6' width='2' height='6' fill='#FFFFFF'/>\
        <polygon points='0,6 8,6 4,0' fill='#7E7E7E'/>\
        <rect x='3' y='6' width='2' height='6' fill='#7E7E7E'/></svg>";
    // the 8x12 glyph floats centered inside the 24px button
    let btn = button(
        svg(svg::Handle::from_memory(UP_ARROW.as_bytes().to_vec()))
            .width(Length::Fixed(8.0))
            .height(Length::Fixed(12.0)),
    )
    .width(Length::Fixed(BTN))
    .height(Length::Fixed(BTN))
    .padding([(BTN - 12.0) / 2.0, (BTN - 8.0) / 2.0])
    .style(move |_t, status| plate(skin, status));
    let el = if enabled {
        btn.on_press(Message::Up).boxed()
    } else {
        btn.boxed()
    };
    tooltip(
        el,
        container(text("Up one level").size(12.0).color(skin.text))
            .padding([3.0, 7.0])
            .style(move |_t: &iced::Theme| container::Style {
                background: Some(skin.menu_bg.into()),
                border: border1(skin.menu_border),
                ..Default::default()
            }),
    )
    .position(tooltip::Position::Bottom)
    .boxed()
}

/// A breadcrumb text element plus its dropdown arrow. The two members
/// are separate hover targets with asymmetric feedback (SPEC/
/// Address_bar.md): hovering the text lights both members blue;
/// hovering the arrow keeps the arrow blue but greys the text. Each
/// member is a 20px-tall (bar height minus the bar border) button with
/// its own 1px border so the pair shares connected borders. The arrow
/// dropdown (subfolder menu) is a deferred feature; clicks on the
/// text still navigate and every segment stays a drop target.
#[allow(clippy::too_many_arguments)]
fn crumb_group(
    state: &Explorer,
    skin: skin::Skin,
    i: usize,
    seg: &str,
    prefix: &Path,
    targeted: bool,
    is_last: bool,
    show_arrow: bool,
) -> Element<'static, Message> {
    let text_hovered = state.crumb_hover == Some((i, false));
    let arrow_hovered = state.crumb_hover.is_some_and(|(g, _)| g == i);
    let grey = state.crumb_hover == Some((i, true));
    // the arrow snaps from the 45° idle glyph to "down" while its
    // dropdown is open
    let open = state
        .menu
        .as_ref()
        .and_then(|m| m.bar.as_ref())
        .is_some_and(|b| b.kind == crate::menu::BarMenuKind::Subdirs && b.prefix.as_path() == prefix);
    // text element: space + text + space (the spacer is part of the
    // label — " Computer " — per the spec's element box), vertically
    // centered. For the clickable segments the button draws the hover
    // effect; the last segment is a bare container and draws it itself.
    let text_cell = container(text(format!(" {seg} ")).size(CRUMB_TEXT).color(skin.text))
        .height(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: if targeted {
                Some(skin.sel.into())
            } else if is_last && text_hovered {
                Some(skin.nav_hover.into())
            } else if is_last && grey {
                Some(skin.nav_grey.into())
            } else {
                None
            },
            border: if is_last && (text_hovered || grey) {
                border1(if text_hovered {
                    skin.nav_hover_border
                } else {
                    skin.address_border
                })
            } else {
                Default::default()
            },
            ..Default::default()
        });
    let text_el: Element<Message> = if is_last {
        // the last segment shares the group hover but is not clickable
        mouse_area(text_cell)
            .on_enter(Message::CrumbHoverEnter {
                group: i,
                arrow: false,
            })
            .on_exit(Message::CrumbHoverExit {
                group: i,
                arrow: false,
            })
            .boxed()
    } else {
        let el = button(text_cell)
            .height(Length::Fixed(BAR - 2.0))
            .padding(0)
            .style(move |_t, _| crumb_btn(skin, text_hovered, grey))
            .on_press(Message::Navigate(prefix.to_path_buf()))
            .boxed();
        // every crumb segment is a drop target (§4.2)
        let el = crate::dnd::drop_zone(el, crate::dnd::DropTarget::Breadcrumb(prefix.to_path_buf()));
        mouse_area(el)
            .on_enter(Message::CrumbHoverEnter {
                group: i,
                arrow: false,
            })
            .on_exit(Message::CrumbHoverExit {
                group: i,
                arrow: false,
            })
            .boxed()
    };

    let mut group: iced::widget::Row<Element<'static, Message>> =
        row![text_el].height(Length::Fixed(BAR - 2.0));
    if show_arrow {
        let (tw, th) = if open { ARR_D } else { ARR_R };
        // while the dropdown is open the arrow keeps the clicked look
        let active = arrow_hovered || open;
        group = group.push(
            mouse_area(
                button(triangle(dir_of(open), tw, th, TRI_IDLE))
                    .width(Length::Fixed(ARROW_W))
                    .height(Length::Fixed(BAR - 2.0))
                    .padding([(BAR - 2.0 - th) / 2.0, (ARROW_W - tw) / 2.0])
                    .style(move |_t, _| crumb_btn(skin, active, false))
                    .on_press(Message::CrumbArrow(i)),
            )
            .on_enter(Message::CrumbHoverEnter {
                group: i,
                arrow: true,
            })
            .on_exit(Message::CrumbHoverExit {
                group: i,
                arrow: true,
            })
            .boxed(),
        );
    }
    group.boxed()
}

/// The leading location icon (computer at the root, folder elsewhere).
fn lead_icon(cwd: &Path, skin: skin::Skin) -> Element<'static, Message> {
    icon(
        if cwd.parent().is_none() {
            "computer"
        } else {
            "folder"
        },
        LEAD_ICON,
        skin,
    )
}

/// Window-logical rectangle of the address bar — the click-away hit
/// test for edit mode. Mirrors the band layout constants in
/// [`address_bar`] (left buttons + spec'd gaps + band paddings).
pub(crate) fn bar_rect(state: &Explorer) -> iced::Rectangle {
    let (ft, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fl - fr).max(320.0);
    let search_w =
        (content_w * state.search_share).clamp(120.0, (content_w - 280.0).max(120.0));
    let show_search = content_w >= state.ribbon.sacrifice().3 * state.dpi_multiplier();
    let band_x = fl + 8.0;
    // back 24, 5, forward 24, 5, back_list 13, 4, up 24, 1, then the bar
    let x = band_x + BTN + 5.0 + BTN + 5.0 + BACK_LIST_W + 4.0 + BTN + 1.0;
    let w = if show_search {
        (content_w - 12.0 - 12.0 - search_w - (x - band_x)).max(0.0)
    } else {
        (content_w - 12.0 - (x - band_x)).max(0.0)
    };
    let title_h = if state.self_border {
        crate::border::titlebar_h(state.frame_style, state.maximized)
    } else {
        0.0
    };
    let mult = state.dpi_multiplier();
    let (min_ribbon_w, _, _, _) = state.ribbon.sacrifice();
    let ribbon_h = if content_w >= min_ribbon_w * mult {
        crate::ribbon::widget::layout::ribbon_height(&state.ribbon.framework().view())
    } else {
        0.0
    };
    // the 24px button row sits 6px below the band top
    let y = ft + title_h + ribbon_h + 6.0;
    iced::Rectangle::new(iced::Point::new(x, y), iced::Size::new(w, BTN))
}

/// Window-logical rectangle of the search box — the click-away hit
/// test for search edit mode. Mirrors [`bar_rect`] (the search box
/// hugs the band's right padding).
pub(crate) fn search_rect(state: &Explorer) -> iced::Rectangle {
    let (ft, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fl - fr).max(320.0);
    let search_w =
        (content_w * state.search_share).clamp(120.0, (content_w - 280.0).max(120.0));
    let title_h = if state.self_border {
        crate::border::titlebar_h(state.frame_style, state.maximized)
    } else {
        0.0
    };
    let mult = state.dpi_multiplier();
    let (min_ribbon_w, _, _, _) = state.ribbon.sacrifice();
    let ribbon_h = if content_w >= min_ribbon_w * mult {
        crate::ribbon::widget::layout::ribbon_height(&state.ribbon.framework().view())
    } else {
        0.0
    };
    let x = fl + content_w - 12.0 - search_w;
    let y = ft + title_h + ribbon_h + 6.0;
    iced::Rectangle::new(iced::Point::new(x, y), iced::Size::new(search_w, BTN))
}

pub(crate) fn address_bar<'a>(
    state: &'a Explorer,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fl - fr).max(320.0);
    let search_w = (content_w * state.search_share).clamp(120.0, (content_w - 280.0).max(120.0));
    let show_search = content_w >= state.ribbon.sacrifice().3 * state.dpi_multiplier();

    // ---- left buttons: back / forward / history chevron / up ----
    let back = nav_circle("back", !state.back_is_empty(), Message::Back, skin);
    let forward = nav_circle("forward", !state.forward_is_empty(), Message::Forward, skin);
    let back_list = back_list_btn(state, skin);
    let up = up_btn(state.cwd.parent().is_some(), skin);

    // ---- breadcrumbs ----
    let segs: Vec<String> = {
        let mut v = vec!["Computer".to_string()];
        for c in state.cwd.components() {
            if let std::path::Component::Normal(c) = c {
                v.push(c.to_string_lossy().into_owned());
            }
        }
        v
    };
    let last = segs.len() - 1;
    // the last crumb shows its arrow only when the folder has subfolders
    let has_subdirs = state.entries.iter().any(|e| e.is_dir());

    // an open address-bar dropdown (marks its arrow/chevron "down");
    // the selector opened from edit mode also drops the border blue
    let bar_menu = state.menu.as_ref().and_then(|m| m.bar.as_ref());
    let selector_open =
        bar_menu.is_some_and(|b| b.kind == crate::menu::BarMenuKind::Selector);
    // edit mode paints the bar borders blue (everything but the
    // refresh button); the open selector puts the blue back
    let blue = state.address_edit.is_some() && !selector_open;
    let mut crumbs: iced::widget::Row<Element<'a, Message>> =
        row![lead_icon(&state.cwd, skin), Space::new().width(Length::Fixed(2.0))]
            .spacing(0.0)
            .align_y(iced::Alignment::Center)
            .height(Length::Fill)
            .width(Length::Fill);
    // the first arrow is independent: no text element before it, its
    // dropdown (root subfolders) is a deferred feature. The on_press
    // hook exists so iced reports real hover/press statuses (a button
    // without on_press is always Disabled) — the press itself is a
    // no-op until the dropdown lands.
    let first_open = bar_menu.is_some_and(|b| {
        b.kind == crate::menu::BarMenuKind::Subdirs && b.prefix.as_os_str() == "/"
    });
    let (fw, fh) = if first_open { ARR_D } else { ARR_R };
    crumbs = crumbs.push(
        button(triangle(dir_of(first_open), fw, fh, TRI_IDLE))
            .width(Length::Fixed(ARROW_W))
            .height(Length::Fixed(BAR - 2.0))
            .padding([(BAR - 2.0 - fh) / 2.0, (ARROW_W - fw) / 2.0])
            // clicked look kept while the dropdown is open
            .style(move |_t, status| {
                plate(
                    skin,
                    if first_open {
                        button::Status::Pressed
                    } else {
                        status
                    },
                )
            })
            .on_press(Message::CrumbArrow(0))
            .boxed(),
    );
    for (i, seg) in segs.iter().enumerate() {
        let is_last = i == last;
        let prefix = prefix_of(&state.cwd, i);
        // §5.1 drag & drop feedback: this crumb is the current target
        let targeted = state.drop_targeted(&prefix);
        // a proper ancestor always holds the next segment; only the
        // last crumb's arrow is conditional (subfolders of cwd)
        let show_arrow = !is_last || has_subdirs;
        crumbs = crumbs.push(crumb_group(
            state,
            skin,
            i,
            seg,
            &prefix,
            targeted,
            is_last,
            show_arrow,
        ));
    }

    // ---- bar left side: breadcrumbs, or the editable path in edit
    // mode (2px space, 16px icon, 2px space, full address)
    let bar_left: Element<'a, Message> = if let Some(buf) = &state.address_edit {
        row![
            lead_icon(&state.cwd, skin),
            Space::new().width(Length::Fixed(2.0)),
            text_input("Address", buf)
                .id(iced::widget::Id::new(ADDRESS_INPUT_ID))
                .on_input(Message::AddressEditInput)
                .on_submit(Message::AddressEditSubmit)
                .size(CRUMB_TEXT)
                .padding(0)
                .style(move |_t: &iced::Theme, _status: text_input::Status| text_input::Style {
                    background: iced::Color::TRANSPARENT.into(),
                    border: Default::default(),
                    placeholder: skin.disabled,
                    value: skin.text,
                    selection: skin.sel,
                })
                .width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fill)
        .boxed()
    } else {
        // clicking the free space (right of the last crumb) edits the
        // address; the target fills whatever the crumbs leave
        crumbs.push(
            mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(Message::AddressEditStart)
                .boxed(),
        )
        .boxed()
    };

    // ---- address bar right side ----
    // 1px divider left of the right-list button, visible only while it
    // is hovered (or pressed — the hover state covers the visual)
    let right_list_hover = state.right_list_hover;
    let right_div = container(
        Space::new()
            .width(Length::Fixed(crate::theme::border_w(1.0)))
            .height(Length::Fill),
    )
    .style(move |_t: &iced::Theme| container::Style {
        background: right_list_hover.then_some(skin.address_border.into()),
        ..Default::default()
    });
    // right-list: 18px button, 7x6 down chevron centered inside, whole
    // area clickable; its dropdown selector is a deferred feature. The
    // hover effect comes from the tracked hover state, not the button
    // status (the inert button would report Disabled forever).
    // the right-list chevron always points down; only the clicked look
    // follows the open state
    let right_list = mouse_area(
        button(triangle(Tri::Down, TRI_W, CHEVRON_H, TRI_IDLE))
            .width(Length::Fixed(RIGHT_LIST))
            .height(Length::Fixed(RIGHT_LIST))
            .padding([(RIGHT_LIST - CHEVRON_H) / 2.0, (RIGHT_LIST - TRI_W) / 2.0])
            .style(move |_t, _| crumb_btn(skin, right_list_hover || selector_open, false))
            .on_press(Message::SelectorMenu),
    )
    .on_enter(Message::RightListHover(true))
    .on_exit(Message::RightListHover(false))
    .boxed();
    // 1px divider, always visible (blue in edit mode)
    let div = container(
        Space::new()
            .width(Length::Fixed(crate::theme::border_w(1.0)))
            .height(Length::Fill),
    )
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(if blue { skin.accent } else { skin.address_border }.into()),
        ..Default::default()
    });
    // refresh: 24px wide, bar-minus-border tall, 12px icon centered
    let refresh = button(icon("refresh", 12.0, skin))
        .width(Length::Fixed(BTN))
        .height(Length::Fixed(BAR - 2.0))
        .padding([(BAR - 2.0 - 12.0) / 2.0, (BTN - 12.0) / 2.0])
        .style(move |_t, status| plate(skin, status))
        .on_press(Message::Refresh)
        .boxed();

    // ---- the address bar itself (22px incl border, 2px sides) ----
    let address = container(
        row![
            bar_left,
            right_div,
            right_list,
            div,
            refresh,
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BAR))
    .padding(iced::Padding {
        top: 0.0,
        right: 2.0,
        bottom: 0.0,
        left: 2.0,
    })
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: border1(if blue { skin.accent } else { skin.nav_border }),
        ..Default::default()
    });

    // ---- search box: same height as the address bar; 9px left, text,
    // 18px search button (12px magnifier), 5px right. Edit mode turns
    // the hint text into a text input and the border blue (like the
    // address bar); the buffer survives edit mode (like Win8) ----
    let search_btn = button(icon("search", 12.0, skin))
        .width(Length::Fixed(RIGHT_LIST))
        .height(Length::Fixed(RIGHT_LIST))
        .padding(0)
        .style(move |_t, _| button::Style {
            background: None,
            text_color: skin.text,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(if state.search_active {
            Message::SearchSubmit
        } else {
            Message::SearchEditStart
        });
    let search_field: Element<'a, Message> = if state.search_active {
        text_input(
            format!("Search {}", state.title()),
            &state.search_text,
        )
        .id(iced::widget::Id::new(SEARCH_INPUT_ID))
        .on_input(Message::SearchInput)
        .on_submit(Message::SearchSubmit)
        .size(CRUMB_TEXT)
        .padding(0)
        .style(move |_t: &iced::Theme, _status: text_input::Status| text_input::Style {
            background: iced::Color::TRANSPARENT.into(),
            border: Default::default(),
            placeholder: skin.disabled,
            value: skin.text,
            selection: skin.sel,
        })
        .width(Length::Fill)
        .boxed()
    } else {
        // clicking the hint text edits the search box; the buffer (if
        // any) is shown instead of the hint, like Win8
        let label = if state.search_text.is_empty() {
            format!("Search {}", state.title())
        } else {
            state.search_text.clone()
        };
        mouse_area(
            text(label)
                .size(CRUMB_TEXT)
                .color(skin.disabled),
        )
        .on_press(Message::SearchEditStart)
        .boxed()
    };
    let search = container(
        row![
            search_field,
            Space::new().width(Length::Fill),
            search_btn,
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fill),
    )
    .width(Length::Fixed(search_w))
    .height(Length::Fixed(BAR))
    .padding(iced::Padding {
        top: 0.0,
        right: 5.0,
        bottom: 0.0,
        left: 9.0,
    })
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: border1(if state.search_active {
            skin.accent
        } else {
            skin.nav_border
        }),
        ..Default::default()
    });

    // the 12px gap between the bars IS the drag target (only the
    // cursor betrays it)
    let splitter = mouse_area(Space::new().width(Length::Fixed(12.0)).height(Length::Fixed(BTN)))
        .on_press(Message::BeginBarSplit)
        .on_enter(Message::SplitterHover(true))
        .on_exit(Message::SplitterHover(false))
        .interaction(iced::mouse::Interaction::ResizingHorizontally);

    // band: 6px top/bottom, 8px left, 12px right; the fixed spacers are
    // the spec'd gaps (5/5/4/1px) between the left buttons and the bar
    let mut band: iced::widget::Row<Element<'a, Message>> = row![
        back,
        Space::new().width(Length::Fixed(5.0)).height(Length::Fixed(BTN)),
        forward,
        Space::new().width(Length::Fixed(5.0)).height(Length::Fixed(BTN)),
        back_list,
        Space::new().width(Length::Fixed(4.0)).height(Length::Fixed(BTN)),
        up,
        Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(BTN)),
        address.boxed(),
    ];
    if show_search {
        band = band.push(splitter.boxed()).push(search.boxed());
    }
    container(band.align_y(iced::Alignment::Center).height(Length::Fixed(BTN)))
        .width(Length::Fill)
        .height(Length::Fixed(state.ribbon.band_height()))
        .center_y(Length::Fixed(state.ribbon.band_height()))
        .padding(iced::Padding {
            top: 6.0,
            right: 12.0,
            bottom: 6.0,
            left: 8.0,
        })
        .style(bg(skin.toolbar_bg))
        .boxed()
}

// ---------------- nav pane ----------------


pub(crate) fn prefix_of(path: &Path, seg_index: usize) -> PathBuf {
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


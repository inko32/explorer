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

/// The "no drop here" badge: a red circle with a diagonal slash (no
/// caption box around it).
fn banned_icon() -> Element<'static, Message> {
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" fill="none" stroke="#E81123" stroke-width="2.2"/><line x1="5.6" y1="14.4" x2="14.4" y2="5.6" stroke="#E81123" stroke-width="2.2"/></svg>"##;
    iced::widget::svg(iced::widget::svg::Handle::from_memory(
        SVG.as_bytes().to_vec(),
    ))
    .width(Length::Fixed(20.0))
    .height(Length::Fixed(20.0))
    .boxed()
}

/// The caption's bordered text plate (menu colors — the same visual
/// family as the context menu).
fn boxed_caption(label: &str, skin: skin::Skin) -> Element<'_, Message> {
    container(iced::widget::text(label).size(13.0))
        .padding(iced::Padding::from([2.0, 7.0]))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.menu_bg.into()),
            border: iced::Border {
                color: skin.menu_border,
                width: crate::theme::border_w(1.0),
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .boxed()
}

/// The drag cursor caption (SPEC/DRAGGING.md §5.1): a small bordered
/// label floating right-lower of the cursor — "Move to …", "Copy to …",
/// "Link here", "Run with …", "…" while the action is undecided, the
/// banned glyph when nothing can happen here, or nothing at all while
/// hovering the dragged items themselves. The caption text is cached in
/// the drag session (rebuilt on target/modifier changes only); this
/// layer just places it at the cursor. Independent from the icon badge.
fn drag_caption(state: &Explorer, skin: skin::Skin) -> Option<Element<'_, Message>> {
    if !state.dnd.active() {
        return None;
    }
    let cap = state.dnd.caption()?;
    // near the cursor: 10px right-down, in window-logical pixels
    const DX: f32 = 16.0;
    const DY: f32 = 16.0;
    let badge: Element<Message> = match cap {
        crate::dnd::DragCaption::Hidden => return None,
        crate::dnd::DragCaption::Banned => banned_icon(),
        crate::dnd::DragCaption::Undecided => boxed_caption("…", skin),
        crate::dnd::DragCaption::Action { label, .. } => boxed_caption(label, skin),
    };
    // a full-window layer; the container's left/top alignment parks the
    // badge at the padded offset
    Some(
        container(badge)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: state.cursor.y + DY,
                left: state.cursor.x + DX,
                ..Default::default()
            })
            .boxed(),
    )
}

/// The drag-image badge, independent from the cursor caption: a fixed
/// 50px blue square (44px icon area + 3px padding on all sides) with
/// up to five 32px icons — each next one 3px right-down, covering the
/// previous, the last one on top — and the item count in the square's
/// bottom-left corner. Square, icons and count all draw at 0.6 opacity.
fn drag_icon_badge(state: &Explorer, skin: skin::Skin) -> Option<Element<'static, Message>> {
    if !state.dnd.active() {
        return None;
    }
    let items = match state.dnd.payload()? {
        crate::dnd::DragPayload::Files(items) if !items.is_empty() => items,
        _ => return None,
    };
    /// Everything in the badge fades together.
    const FADE: f32 = 0.6;
    const BADGE: f32 = 50.0; // outer square side
    const PAD: f32 = 3.0; // square padding, all sides
    const AREA: f32 = BADGE - 2.0 * PAD; // 44px icon area
    const STEP: f32 = 3.0; // cascade step, right-down per icon
    const MAX: usize = 5; // a full deck: 32px icons + 4 steps = 44
    const COUNT: f32 = 18.0; // multi-drag count plate side
    // the grabbed item first (front of the deck); draw deepest first so
    // each next covers it — the front icon ends bottom-right-most, on top
    let top = state.dnd.top();
    let mut shown: Vec<&crate::dnd::DraggedItem> = items
        .iter()
        .filter(|i| Some(i.uri.as_str()) != top)
        .take(MAX - 1)
        .collect();
    if let Some(t) = items.iter().find(|i| Some(i.uri.as_str()) == top) {
        shown.insert(0, t);
    } else {
        // no recorded top: payload order, first on top
        shown = items.iter().take(MAX).collect();
    }
    let n = shown.len();
    // fewer than five icons: scale them up so the deck still fills the
    // area (n = 5 lands on 32px exactly: 44 - 4*3)
    let icon = AREA - (n as f32 - 1.0) * STEP;
    let mut deck: iced::widget::Stack<Element<'static, Message>> = stack![];
    for (j, it) in shown.iter().enumerate().rev() {
        let off = (n - 1 - j) as f32 * STEP;
        let name = crate::dnd::uri_to_path(&it.uri)
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_else(|| it.uri.clone());
        deck = deck.push(
            container(crate::icon::for_name_opacity(
                it.kind == crate::dnd::ItemKind::Directory,
                &name,
                icon,
                &skin,
                FADE,
            ))
            .width(Length::Fixed(AREA))
            .height(Length::Fixed(AREA))
            // top+left only: [v, h] would pad all four sides and the
            // largest offsets would squeeze the 32px icon below 32
            .padding(iced::Padding {
                top: off,
                left: off,
                ..Default::default()
            })
            .boxed(),
        );
    }
    let mut badge: iced::widget::Stack<Element<'static, Message>> = stack![container(deck)
        .width(Length::Fixed(BADGE))
        .height(Length::Fixed(BADGE))
        .padding(PAD)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(iced::Color { a: FADE, ..skin.accent }.into()),
            ..Default::default()
        })
        .boxed()];
    if items.len() > 1 {
        // solid plate, bottom-left: the number of dragged files (solid
        // on the faded badge — readable)
        let total = items.len().to_string();
        let plate = container(
            iced::widget::text(total)
                .size(11.0)
                .color(iced::Color::WHITE)
                .boxed(),
        )
        .width(Length::Fixed(COUNT))
        .height(Length::Fixed(COUNT))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.accent.into()),
            ..Default::default()
        });
        badge = badge.push(
            container(plate)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Bottom)
                .boxed(),
        );
    }
    // left-higher of the caption: the badge's bottom-right corner sits
    // 19px above-left of the caption plate (the caption sits at DX/DY)
    const DX: f32 = 16.0;
    const DY: f32 = 20.0;
    const GAP: f32 = 19.0;
    Some(
        container(badge)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: (state.cursor.y + DY - BADGE - GAP).max(0.0),
                left: (state.cursor.x + DX - BADGE - GAP).max(0.0),
                ..Default::default()
            })
            .boxed(),
    )
}

pub fn view(state: &Explorer) -> Element<'_, Message> {
    UI_DEBUG.store(state.ui_debug, std::sync::atomic::Ordering::Relaxed);
    // border widths: constant physical size unless the theme says they
    // follow the DPI (published every view, like UI_DEBUG)
    crate::theme::set_border_scale(
        crate::theme::get().border_follow_dpi,
        state.scale_dpi(),
    );
    let skin = if state.dark { skin::dark() } else { skin::light() };
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
        // nav/folder boundary: 2px colorless – 1px separator – 1px
        // colorless (the separator keeps a constant physical width);
        // the whole strip is the splitter drag target
        let bw = crate::theme::border_w(1.0);
        let nav_splitter = dbg(
            "nav_splitter",
            mouse_area(
                row![
                    spacer(2.0 * bw, skin, false),
                    spacer(bw, skin, true),
                    spacer(bw, skin, false),
                ]
                .height(Length::Fill),
            )
            .on_press(Message::BeginNavSplit)
            .interaction(iced::mouse::Interaction::ResizingHorizontally)
            .boxed(),
        );
        let mut main: iced::widget::Row<Element<'_, Message>> = row![nav, nav_splitter, fv]
            .height(Length::Fill);
        // side pane right of the folder view (Preview / Details),
        // resizable via the splitter on its left edge; it hides before
        // the folder view gets too narrow (yaml sacrifice.pane_below)
        if side_w > 0.0 {
            main = main.push(dbg("pane_splitter", pane_splitter(Message::BeginPaneSplit, skin)));
            // 1px grey theme separator hugging the pane's left edge
            main = main.push(spacer(crate::theme::border_w(1.0), skin, true));
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
            main = main.push(spacer(crate::theme::border_w(1.0), skin, true));
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
        // drag icon badge + cursor caption above the content, below the menu
        if let Some(el) = drag_icon_badge(state, skin) {
            s = s.push(dbg("drag_icon_badge", el));
        }
        if let Some(el) = drag_caption(state, skin) {
            s = s.push(dbg("drag_caption", el));
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
        // drag cursor caption above the content, below the menu
        if let Some(open) = &state.menu {
            let el = if state.menu_cfg.layout.popup {
                crate::menu::host(open, &state.menu_cfg, skin)
            } else {
                crate::menu::layer(open, &state.menu_cfg, skin)
            };
            let mut s: iced::widget::Stack<Element<'_, Message>> = stack![framed];
            if let Some(b) = drag_icon_badge(state, skin) {
                s = s.push(b);
            }
            if let Some(c) = drag_caption(state, skin) {
                s = s.push(c);
            }
            return s.push(dbg("menu", el)).boxed();
        }
        let badge = drag_icon_badge(state, skin);
        match (drag_caption(state, skin), badge) {
            (Some(c), Some(b)) => {
                stack![framed, dbg("drag_icon_badge", b), dbg("drag_caption", c)].boxed()
            }
            (Some(c), None) => stack![framed, dbg("drag_caption", c)].boxed(),
            (None, Some(b)) => stack![framed, dbg("drag_icon_badge", b)].boxed(),
            (None, None) => framed,
        }
    }
}

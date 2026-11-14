//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.


use iced::Widget as _;
use iced::widget::{column, container, mouse_area, row, stack, Space};
use iced::{Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{bg, skin};

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
    if !state.dnd.active() || state.dnd_wl_drag {
        // Wayland drag: the compositor paints caption + badge on the
        // drag-image surface (see dnd_wayland)
        return None;
    }
    let cap = state.dnd.caption()?;
    // near the cursor, in window-logical pixels — the SAME offsets the
    // Wayland drag surface paints (dnd_wayland::paint_drag_visual):
    // badge at cursor+(2,2), this plate at cursor+(71,71), so the
    // handoff from in-window overlay to compositor icon is seamless
    // (the drag hotspot is fixed at the icon's top-left corner)
    const DX: f32 = 71.0;
    const DY: f32 = 71.0;
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
    if !state.dnd.active() || state.dnd_wl_drag {
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
    // same origin as the Wayland drag surface: badge top-left at
    // cursor+(2,2) — the handoff at the window edge must not jump
    const DX: f32 = 2.0;
    const DY: f32 = 2.0;
    Some(
        container(badge)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: (state.cursor.y + DY).max(0.0),
                left: (state.cursor.x + DX).max(0.0),
                ..Default::default()
            })
            .boxed(),
    )
}

pub fn view(state: &Explorer) -> Element<'_, Message> {
    // border widths: constant physical size unless the theme says they
    // follow the DPI (published every view)
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
    // Win8.1 (not maximized) draws the title bar INSIDE the frame layer
    // (its accent runs flush to the outer border, before the inner
    // outline) — the frame branch adds it there instead of here. The
    // maximized frame reports zero insets, so the title bar is plain
    // content there like every other style.
    let win81_framed = state.self_border
        && state.frame_style == crate::model::FrameStyle::Win81
        && !state.maximized;
    if state.self_border && !win81_framed {
        root = root.push(crate::border::titlebar(state, skin));
    }
    // sacrifice the ribbon entirely when the window is resized too small
    if content_w >= min_ribbon_w {
        root = root.push(crate::ribbon::view(state));
    }
    root = root.push(crate::navigation::address_bar(state, skin));
    // sacrifice the navigation pane when the window gets too narrow;
    // the widths come from the model (yaml sacrifice rules + the
    // splitter-dragged widths)
    let (nav_w, side_w) = state.layout_widths();
    if nav_w > 0.0 {
        let nav = crate::nav_pane::nav_pane(state, skin, nav_w);
        let fv = crate::folder::file_view(state, skin, content_w - nav_w - side_w);
        // nav/folder boundary: 2px colorless – 1px separator – 1px
        // colorless (the separator keeps a constant physical width);
        // the whole strip is the splitter drag target
        let bw = crate::theme::border_w(1.0);
        let nav_splitter = mouse_area(
                row![
                    spacer(2.0 * bw, skin, false),
                    spacer(bw, skin, true),
                    spacer(bw, skin, false),
                ]
                .height(Length::Fill),
            )
        .on_press(Message::BeginNavSplit)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .boxed();
        let mut main: iced::widget::Row<Element<'_, Message>> = row![nav, nav_splitter, fv]
            .height(Length::Fill);
        // side pane right of the folder view (Preview / Details),
        // resizable via the splitter on its left edge; it hides before
        // the folder view gets too narrow (yaml sacrifice.pane_below)
        if side_w > 0.0 {
            main = main.push(pane_splitter(Message::BeginPaneSplit, skin));
            // 1px grey theme separator hugging the pane's left edge
            main = main.push(spacer(crate::theme::border_w(1.0), skin, true));
            main = main.push(crate::pane::view(state, skin, side_w));
        }
        root = root.push(main.boxed());
    } else {
        // the nav-gap spacer belongs to the folder view: it shows even
        // with the nav pane hidden
        let fv = crate::folder::file_view(state, skin, content_w - side_w);
        let mut main: iced::widget::Row<Element<'_, Message>> = row![
            spacer(state.ribbon.nav_gap(), skin, false),
            container(fv).width(Length::Fill).height(Length::Fill)
        ]
        .height(Length::Fill);
        if side_w > 0.0 {
            main = main.push(pane_splitter(Message::BeginPaneSplit, skin));
            // 1px grey theme separator hugging the pane's left edge
            main = main.push(spacer(crate::theme::border_w(1.0), skin, true));
            main = main.push(crate::pane::view(state, skin, side_w));
        }
        root = root.push(main.boxed());
    }
    root = root.push(crate::status::statusbar(state, skin));
    let framed: Element<Message> = if state.self_border {
        match state.frame_style {
            // Win8: two-tone frame — deep 1px outer edge (#368BE9), light
            // fill (#6BADF6); total insets unchanged (2/6/6/6).
            crate::model::FrameStyle::Win8 => container(
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
            .boxed(),
            // Win10: 1px dark.
            crate::model::FrameStyle::Win10 => container(root)
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
                .boxed(),
            // Win8.1 (SPEC/win8_border.md): 1px outer outline + 6px border
            // + 1px inner outline per side; the top inner outline is the
            // 1px row directly below the 29px title bar. The 1px outlines
            // keep a constant physical width (never DPI-scaled).
            crate::model::FrameStyle::Win81 => {
                let outline_w = crate::theme::border_w(1.0);
                use crate::border::win81 as w81;
                // a solid strip positioned inside the window (frame-level
                // details: the top inner outline, the maximized
                // title-bar extensions)
                let strip = |y: f32,
                             h: f32,
                             l: f32,
                             r: f32,
                             c: iced::Color|
                -> Element<'static, Message> {
                    container(
                        container(Space::new().width(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fixed(h))
                            .style(move |_: &iced::Theme| container::Style {
                                background: Some(c.into()),
                                ..Default::default()
                            }),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(iced::Padding {
                        top: y,
                        right: r,
                        left: l,
                        ..Default::default()
                    })
                    .boxed()
                };
                // the vertical counterpart: a solid bar at x, inset
                // t/b, width w
                let vstrip = |x: f32,
                              w: f32,
                              t: f32,
                              b: f32,
                              c: iced::Color|
                -> Element<'static, Message> {
                    container(
                        container(Space::new().height(Length::Fill))
                            .width(Length::Fixed(w))
                            .height(Length::Fill)
                            .style(move |_: &iced::Theme| container::Style {
                                background: Some(c.into()),
                                ..Default::default()
                            }),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(iced::Padding {
                        left: x,
                        top: t,
                        bottom: b,
                        ..Default::default()
                    })
                    .boxed()
                };
                let framed: Element<Message> = if state.maximized {
                    // maximized: zero insets (see frame_insets_pub) — the
                    // frame is gone entirely (outer outline, 6px side/
                    // bottom border, side inner outlines all omitted) and
                    // the content extends to the window edges; only the
                    // 1px inner outline under the title bar remains,
                    // drawn as a full-width strip below
                    container(root).width(Length::Fill).height(Length::Fill).boxed()
                } else {
                    // Flattened frame — tiled strips, not nested fills.
                    // The frame colors are translucent (win81theme.yaml
                    // border_opacity) and nested container backgrounds
                    // would compound the alpha: the title bar (accent
                    // over the accent fill) blended darker than the
                    // border strips beside it. Here every frame pixel
                    // is exactly ONE translucent layer over the desktop
                    // or the opaque content; regions that share a color
                    // (the accent beside the title bar) are one draw.
                    let side = 6.0; // sizing border, like the Win8 frame
                    let o = outline_w;
                    let h = state.win_h;
                    // the title bar's bottom edge (= the strip start)
                    let bar_bottom = ft + crate::border::win81_bar_h(false);
                    // title bar at the CONTENT origin (inset by the
                    // frame insets): its contextual blocks position
                    // themselves above their ribbon tabs, so the bar
                    // must align with the ribbon. The accent strips
                    // beside it come from the frame (they start at the
                    // top, not below the bar).
                    let titled = container(crate::border::titlebar(state, skin))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(iced::Padding {
                            top: ft,
                            right: fr,
                            left: fl,
                            ..Default::default()
                        });
                    // content below the title bar + the top inner outline
                    let body = container(root)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(iced::Padding {
                            top: ft
                                + crate::border::titlebar_h(
                                    crate::model::FrameStyle::Win81,
                                    false,
                                ),
                            right: fr,
                            bottom: fb,
                            left: fl,
                        });
                    stack![
                        titled,
                        body,
                        // outer outline: 1px perimeter
                        vstrip(0.0, o, o, o, w81::outline()),
                        vstrip(state.win_w - o, o, o, o, w81::outline()),
                        strip(0.0, o, 0.0, 0.0, w81::outline()),
                        strip(h - o, o, 0.0, 0.0, w81::outline()),
                        // accent fill, beside the title bar: flush to
                        // the bar — it also covers the inner outline's
                        // column (y ft..bar_bottom)
                        vstrip(o, side + o, ft, h - bar_bottom, w81::accent()),
                        vstrip(state.win_w - o - side - o, side + o, ft, h - bar_bottom, w81::accent()),
                        // accent fill below the title bar: the inner
                        // outline takes its column back
                        vstrip(o, side, bar_bottom, o + side, w81::accent()),
                        vstrip(state.win_w - o - side, side, bar_bottom, o + side, w81::accent()),
                        strip(h - o - side, side, o, o, w81::accent()),
                        // inner outline: sides below the title bar + bottom
                        vstrip(o + side, o, bar_bottom, fb - o, w81::outline()),
                        vstrip(state.win_w - o - side - o, o, bar_bottom, fb - o, w81::outline()),
                        strip(h - fb, o, o + side + o, o + side + o, w81::outline()),
                    ]
                    .boxed()
                };
                // the top inner outline: the 1px row directly below the
                // title bar (its bottom line rises when maximized —
                // 20px buttons + 3px space instead of 9px). Not
                // maximized: it spans the content (between the side
                // inner outlines); maximized: zero insets put the title
                // bar at y=0 and the line spans the full window width.
                let (line_y, line_l, line_r) = if state.maximized {
                    (crate::border::win81_bar_h(true), 0.0, 0.0)
                } else {
                    (ft + crate::border::win81_bar_h(false), fl, fr)
                };
                // the inner outline first, then the frame, then the
                // Win8.1 contextual blocks' 1px "tails": each block
                // continues 1 physical px below the bar, overriding the
                // outline row (drawn last — no Stack size tricks needed).
                // Tails are a non-maximized detail: maximized blocks
                // fill the 23px bar (their spec height doesn't fit).
                let mut layers: Vec<Element<'_, Message>> = vec![
                    strip(line_y, outline_w, line_l, line_r, w81::outline()),
                    framed,
                ];
                if !state.maximized {
                    for (x, w, c) in crate::border::contextual_extenders(state) {
                        layers.push(strip(line_y, outline_w, x, state.win_w - x - w, c));
                    }
                }
                iced::widget::Stack::with_children(layers).boxed()
            }
        }
    } else {
        root.boxed()
    };
    if state.self_border {
        let grab = match state.frame_style {
            // Win8/Win8.1 have the thick sizing border
            crate::model::FrameStyle::Win8 | crate::model::FrameStyle::Win81 => 8.0,
            crate::model::FrameStyle::Win10 => 5.0,
        };
        // a maximized Win8.1 window can't be edge-resized: omit the
        // top resize zones (dead resize cursor over the caption
        // corner) — the corner belongs to the close button instead
        let top_zones = !(state.maximized
            && state.frame_style == crate::model::FrameStyle::Win81);
        let mut s: iced::widget::Stack<Element<'_, Message>> = stack![framed, crate::border::resize_frame(grab, top_zones),
                           crate::border::caption_overlay(state, skin)];
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
            s = s.push(el);
        }
        if let Some(el) = drag_caption(state, skin) {
            s = s.push(el);
        }
        // context menu above everything (SPEC/MENU_SPEC.MD): in-window
        // by default, overlay layer with menu.yaml layout.popup; the
        // Wayland xdg_popup host replaces both (menu drawn off-window)
        if let Some(open) = &state.menu {
            if state.menu_popup {
                // hosted by the side-channel popup — nothing in-window
            } else {
                let el = if state.menu_cfg.layout.popup {
                    crate::menu::host(open, &state.menu_cfg, skin)
                } else {
                    crate::menu::layer(open, &state.menu_cfg, skin)
                };
                s = s.push(el);
            }
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
            return s.push(el).boxed();
        }
        let badge = drag_icon_badge(state, skin);
        match (drag_caption(state, skin), badge) {
            (Some(c), Some(b)) => {
                stack![framed, b, c].boxed()
            }
            (Some(c), None) => stack![framed, c].boxed(),
            (None, Some(b)) => stack![framed, b].boxed(),
            (None, None) => framed,
        }
    }
}

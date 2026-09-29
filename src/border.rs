//! Window chrome: self-drawn frame (Win8 two-tone / Win10 1px), resize grab
//! ring, title bar (window icon, QAT, contextual blocks, title), and the
//! caption buttons (_ [] X) overlay.

use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message};
use crate::ribbon::{self, active_tab_geometry, main_tabs_width, tab_row_w, tab_width, visible_tabs, TabDef, TAB_ROW_H, TITLEBAR_H};
use crate::ui_common::{bg, dbg, elide, icon, skin};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeDir {
    N,
    S,
    E,
    W,
    Ne,
    Nw,
    Se,
    Sw,
}

impl ResizeDir {
    pub fn task(self, id: iced::window::Id) -> iced::Task<Message> {
        use iced::window::Direction;
        let d = match self {
            ResizeDir::N => Direction::North,
            ResizeDir::S => Direction::South,
            ResizeDir::E => Direction::East,
            ResizeDir::W => Direction::West,
            ResizeDir::Ne => Direction::NorthEast,
            ResizeDir::Nw => Direction::NorthWest,
            ResizeDir::Se => Direction::SouthEast,
            ResizeDir::Sw => Direction::SouthWest,
        };
        iced::window::drag_resize(id, d)
    }

    fn cursor(self) -> iced::mouse::Interaction {
        use iced::mouse::Interaction;
        match self {
            ResizeDir::E | ResizeDir::W => Interaction::ResizingHorizontally,
            ResizeDir::N | ResizeDir::S => Interaction::ResizingVertically,
            ResizeDir::Ne | ResizeDir::Sw => Interaction::ResizingDiagonallyUp,
            ResizeDir::Nw | ResizeDir::Se => Interaction::ResizingDiagonallyDown,
        }
    }
}


/// An invisible resize zone aligned to one window edge/corner. The outer
/// container spans the whole window (transparent to events); only the inner
/// strip/corner reacts.
pub(crate) fn resize_zone(
    dir: ResizeDir,
    w: Length,
    h: Length,
    ax: Option<iced::Alignment>,
    ay: Option<iced::Alignment>,
) -> Element<'static, Message> {
    let zone = mouse_area(
        container(Space::new().width(w).height(h))
            .style(|_: &iced::Theme| container::Style::default()),
    )
    .interaction(dir.cursor())
    .on_press(Message::StartResize(dir));
    let mut c = container(zone).width(Length::Fill).height(Length::Fill);
    if let Some(x) = ax {
        c = c.align_x(x);
    }
    if let Some(y) = ay {
        c = c.align_y(y);
    }
    c.into()
}

/// Window frame (exposed border):
///  - Win8: 3px accent border at the window edge; content is inset 3px
///    (internal calculation follows the exposed outer border).
///  - Win10: single 1px dark border; content inset 1px.
/// Frame thickness (top, right, bottom, left). Win8: thin top, THICK
/// sides+bottom (the classic sizing border); Win10: 1px all around.
/// The internal content is inset by exactly these amounts - the "virtual
/// window" shrinks, the frame never covers content.
/// Frame insets without an Explorer (model-side splitter math).
pub fn frame_insets_pub(self_border: bool, style: crate::model::FrameStyle) -> (f32, f32, f32, f32) {
    if !self_border {
        return (0.0, 0.0, 0.0, 0.0);
    }
    match style {
        crate::model::FrameStyle::Win8 => (1.0, 6.0, 6.0, 6.0),
        crate::model::FrameStyle::Win10 => (1.0, 1.0, 1.0, 1.0),
    }
}

pub(crate) fn frame_insets(state: &Explorer) -> (f32, f32, f32, f32) {
    if !state.self_border {
        return (0.0, 0.0, 0.0, 0.0);
    }
    match state.frame_style {
        crate::model::FrameStyle::Win8 => (1.0, 6.0, 6.0, 6.0),
        crate::model::FrameStyle::Win10 => (1.0, 1.0, 1.0, 1.0),
    }
}

pub(crate) fn frame_color(state: &Explorer, skin: skin::Skin) -> Color {
    match state.frame_style {
        crate::model::FrameStyle::Win8 => skin.accent,
        crate::model::FrameStyle::Win10 => skin.accent_dark,
    }
}

/// Caption buttons (_ [] X) as an overlay at the window's top-right corner:
/// they contact the top edge (Win8: over the frame) and the right edge.
/// Win8: min/max width = 60% of close width, short (20px), thin glyphs.
/// Win10: uniform 46px, nearly full title-bar height.
pub(crate) fn caption_overlay(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    // Pixel ratios from the real Win8 capture (/tmp/user-win8cap.png):
    // buttons hang below the top frame (y=2), height 21 = 69% of the 30px
    // title bar; adjacent, no gaps; min/max w = 1.33x h, close w = 2.24x h;
    // glyph baseline at ~70% of the button height.
    let (mw, cw, h, glyph, y, right_pad, names) = match state.frame_style {
        crate::model::FrameStyle::Win8 => (
            28.0, 47.0, 21.0, 13.0, 1.0, 6.0,
            ("minimize8", "maximize8", "close8"),
        ),
        crate::model::FrameStyle::Win10 => (46.0, 46.0, 29.0, 15.0, 1.0, 1.0,
            ("minimize", "maximize", "close")),
    };
    // Win8: buttons are opaque at rest (titlebar blue / close red) so the
    // frame's top edge never shows as a line above/through them
    let rest_bg: Option<Color> = match state.frame_style {
        crate::model::FrameStyle::Win8 => Some(skin.accent),
        crate::model::FrameStyle::Win10 => None,
    };
    // glyph bottoms share one baseline at ~70% of the button height
    let one = move |icon_name: &'static str,
                    w: f32,
                    msg: Message,
                    hover_bg: Color,
                    solid: bool|
          -> Element<'static, Message> {
        let rest = if solid { Some(skin.close_hover) } else { rest_bg };
        let blend_white = |c: Color, a: f32| Color {
            r: c.r + (1.0 - c.r) * a,
            g: c.g + (1.0 - c.g) * a,
            b: c.b + (1.0 - c.b) * a,
            a: 1.0,
        };
        button(
            container(crate::icon::view(icon_name, glyph, &skin))
                .width(Length::Fixed(w))
                .height(Length::Fixed(h))
                .center_x(Length::Fixed(w))
                .center_y(Length::Fixed(h)),
        )
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .padding(0)
        .style(move |_t, status| button::Style {
            background: match (solid, status) {
                (true, button::Status::Hovered) => Some(skin.close_hover.into()),
                (true, button::Status::Pressed) => Some(skin.close_pressed.into()),
                (true, _) => Some(skin.close_idle.into()),
                (false, button::Status::Hovered) => Some(match rest {
                    Some(c) => blend_white(c, 0.18).into(),
                    None => hover_bg.into(),
                }),
                (false, button::Status::Pressed) => Some(match rest {
                    Some(c) => blend_white(c, 0.32).into(),
                    None => hover_bg.scale_alpha(0.7).into(),
                }),
                (false, _) => rest.map(Into::into),
            },
            text_color: Color::WHITE,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(msg)
        .into()
    };
    let ghost = Color::from_rgba(1.0, 1.0, 1.0, 0.18);
    container(
        row![
            one(names.0, mw, Message::Minimize, ghost, false),
            one(names.1, mw, Message::Maximize, ghost, false),
            one(names.2, cw, Message::Close, skin.close_hover, true),
        ]
        .height(Length::Fixed(h)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: y,
        right: right_pad,
        ..Default::default()
    })
    .align_x(iced::Alignment::End)
    .align_y(iced::Alignment::Start)
    .into()
}

/// Invisible resize ring around the window edges (custom border mode).
pub(crate) fn resize_frame(margin: f32) -> Element<'static, Message> {
    use iced::Alignment;
    let m = Length::Fixed(margin);
    let c = Length::Fixed(margin * 2.0);
    let fill = Length::Fill;
    stack![
        resize_zone(ResizeDir::N, fill, m, None, None),
        resize_zone(ResizeDir::S, fill, m, None, Some(Alignment::End)),
        resize_zone(ResizeDir::W, m, fill, None, None),
        resize_zone(ResizeDir::E, m, fill, Some(Alignment::End), None),
        // corners on top (they overlap the edge strips)
        resize_zone(ResizeDir::Nw, c, c, None, None),
        resize_zone(ResizeDir::Ne, c, c, Some(Alignment::End), None),
        resize_zone(ResizeDir::Sw, c, c, None, Some(Alignment::End)),
        resize_zone(ResizeDir::Se, c, c, Some(Alignment::End), Some(Alignment::End)),
    ]
    .into()
}

// ---------------- --ui-debug element outlines ----------------


pub(crate) fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // Win8: contextual tab labels are colored blocks on the title bar,
    // right-aligned before the caption buttons. Title centers in the
    // space left of them (so it shifts when they appear).
    let qat_btn: f32 = 22.0; // QAT square button (also the chevron row height)
    let icon_zone_w: f32 = 28.0; // window icon zone at the bar's left
    let mut ctx_blocks = row![].height(Length::Fixed(TITLEBAR_H));
    let mut first_block = true;
    for tab in visible_tabs(state) {
        let Some(accent) = tab.color.as_deref().and_then(ribbon::color_by_name) else { continue };
        if !first_block {
            // space between two contextual blocks, aligning with the tab row
            ctx_blocks = ctx_blocks.push(Space::new().width(Length::Fixed(ribbon::CTX_TAB_GAP)));
        }
        first_block = false;
        let active = state.active_tab == tab.id;
        let width = tab_width(&tab.label);
        // Win8: pastel accent block at 90% of the title bar height, no edge
        fn tint(accent: Color, t: f32) -> Color {
            Color::from_rgb(
                accent.r + (1.0 - accent.r) * t,
                accent.g + (1.0 - accent.g) * t,
                accent.b + (1.0 - accent.b) * t,
            )
        }
        let label_color =
            Color::from_rgb(accent.r * 0.75, accent.g * 0.75, accent.b * 0.75);
        // The title-bar block is ALWAYS solid pastel (idle or active); the
        // fade belongs to the contextual tab, never to this block. 80% of
        // the 30px title bar (24px), lowered so its bottom edge is flush
        // with the title-bar bottom = connects to the tab top.
        let label = container(
            text(tab.label.clone())
                .size(12.0)
                .color(label_color)
                .width(Length::Shrink),
        )
        .width(Length::Fixed(width))
        .center_x(Length::Fixed(width));
        let solid = container(label)
            .width(Length::Fixed(width))
            .height(Length::Fixed(TAB_ROW_H))
            .center_x(Length::Fixed(width))
            .center_y(Length::Fixed(TAB_ROW_H))
            .style(move |_| container::Style {
                background: Some(tint(accent, if active { 0.45 } else { 0.62 }).into()),
                ..Default::default()
            });
        // lowered: 24px solid block bottom-aligned in the 30px title bar
        let block: Element<Message> = container(solid)
            .width(Length::Fixed(width))
            .height(Length::Fixed(TITLEBAR_H))
            .align_y(iced::Alignment::End)
            .into();
        ctx_blocks = ctx_blocks.push(
            dbg(
                Box::leak(format!("ctx_block:{}", tab.label).into_boxed_str()),
                mouse_area(block).on_press(Message::BlockPress(tab.id.clone())),
            ),
        );
    }

    // Quick-access buttons (Undo / Redo) framed by thin separators
    let vline = || {
        container(
            container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(15.0)))
                .style(|_| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.35).into()),
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(6.0))
        .center_x(Length::Fixed(6.0))
    };
    let mut qat = row![].spacing(2).align_y(iced::Alignment::Center);
    qat = qat.push(vline());
    for q in &state.quick_access {
        let id = q.clone();
        qat = qat.push(
            mouse_area(
                container(crate::icon::view(q, 16.0, &skin))
                    .width(Length::Fixed(qat_btn))
                    .height(Length::Fixed(qat_btn))
                    .center_x(Length::Fixed(qat_btn))
                    .center_y(Length::Fixed(qat_btn)),
            )
            .on_press(Message::QuickAccess(id)),
        );
    }
    qat = qat.push(
        mouse_area(
            container(crate::icon::view("chevron_down", 10.0, &skin))
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(qat_btn))
                .center_x(Length::Fixed(14.0))
                .center_y(Length::Fixed(qat_btn)),
        )
        .on_press(Message::ToggleRibbonCollapse),
    );
    qat = qat.push(vline());
    // exact: sep 6+2, per button qat_btn+2, chevron 14+2, sep 6
    let qat_w: f32 = state.quick_access.len() as f32 * (qat_btn + 2.0) + 30.0;

    // Win8: the contextual blocks sit exactly above their tab-row stubs
    // (which start right after the main tabs); the title centers between
    // the icon+QAT and the blocks. When the title is too long it elides
    // and recenters AFTER the blocks: `... contextual  title  _^x`.
    // Win8: min/max are 60% of the close button's width; Win10: uniform 46
    let captions_w = match state.frame_style {
        crate::model::FrameStyle::Win8 => 28.0 + 28.0 + 47.0 + 6.0,
        crate::model::FrameStyle::Win10 => 46.0 * 3.0,
    };
    let n_ctx = visible_tabs(state)
        .iter()
        .filter(|t| t.color.is_some())
        .count();
    let blocks_w: f32 = if n_ctx == 0 {
        0.0
    } else {
        visible_tabs(state)
            .iter()
            .filter(|t| t.color.is_some())
            .map(|t| tab_width(&t.label))
            .sum::<f32>()
            + n_ctx.saturating_sub(1) as f32 * 5.0
    };
    // `icon | QAT | title  contextual  _^x`; the title lives in the zone
    // above the main tabs. When it doesn't fit there it elides and moves
    // AFTER the blocks: `icon | QAT | contextual  title  _^x`.
    let title_w = state.title().chars().count() as f32 * 7.5 + 8.0;
    let title_zone = (main_tabs_width(state) - 28.0 - qat_w).max(60.0);
    let tight = blocks_w > 1.0 && title_w > title_zone - 8.0;
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = state.win_w - fr - fl;
    if tight {
        let elide_w = (content_w - 28.0 - qat_w - 138.0 - blocks_w - 16.0).max(30.0);
        // blocks keep their aligned position above the tab stubs; the title
        // moves right of them. Every non-interactive strip drags the window.
        let gap_w = (main_tabs_width(state) - 28.0 - qat_w).max(0.0);
        let drag_gap: Element<Message> = mouse_area(
            Space::new().width(Length::Fixed(gap_w)).height(Length::Fixed(TITLEBAR_H)),
        )
        .on_press(Message::TitlebarPress)
        .on_double_click(Message::TitlebarDoubleClick)
        .into();
        let title = container(
            text(elide(&state.title(), elide_w))
                .size(12.0)
                .color(skin.title_text)
                .width(Length::Shrink),
        )
        .height(Length::Fill)
        .center_y(Length::Fill);
        let drag_title: Element<Message> = mouse_area(
            container(title).width(Length::Fill).center_x(Length::Fill),
        )
        .on_press(Message::TitlebarPress)
        .on_double_click(Message::TitlebarDoubleClick)
        .into();
        return container(
            row![
                container(icon("folder", 16.0, skin))
                    .width(Length::Fixed(icon_zone_w))
                    .height(Length::Fill)
                    .center_x(Length::Fixed(icon_zone_w))
                    .center_y(Length::Fill),
                container(qat).width(Length::Fixed(qat_w)).height(Length::Fill).center_y(Length::Fill),
                drag_gap,
                ctx_blocks,
                drag_title,
                Space::new().width(Length::Fixed(captions_w)),
            ]
            .height(Length::Fixed(TITLEBAR_H)),
        )
        .width(Length::Fill)
        .height(Length::Fixed(TITLEBAR_H))
        .style(bg(skin.accent))
        .into();
    }

    let title = container(
        text(elide(&state.title(), title_zone - 8.0))
            .size(12.0)
            .color(skin.title_text)
            .width(Length::Shrink),
    )
    .width(Length::Fixed(title_zone))
    .height(Length::Fill)
    .center_x(Length::Fixed(title_zone))
    .center_y(Length::Fill);
    let drag_title = mouse_area(title)
        .on_press(Message::TitlebarPress)
        .on_double_click(Message::TitlebarDoubleClick);

    container(
        row![
            container(icon("folder", 16.0, skin))
                .width(Length::Fixed(icon_zone_w))
                .height(Length::Fill)
                .center_x(Length::Fixed(icon_zone_w))
                .center_y(Length::Fill),
            container(qat).width(Length::Fixed(qat_w)).height(Length::Fill).center_y(Length::Fill),
            drag_title,
            ctx_blocks,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(TITLEBAR_H)))
                .on_press(Message::TitlebarPress)
                .on_double_click(Message::TitlebarDoubleClick),
            Space::new().width(Length::Fixed(captions_w)),
        ]
        .height(Length::Fixed(TITLEBAR_H)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TITLEBAR_H))
    .style(bg(skin.accent))
    .into()
}

// ---------------- ribbon ----------------


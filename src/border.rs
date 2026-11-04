//! Window chrome: self-drawn frame (Win8 two-tone / Win10 1px), resize grab
//! ring, title bar (window icon, QAT, contextual blocks, title), and the
//! caption buttons (_ [] X) overlay.

use iced::Widget as _;
use iced::widget::{button, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{bg, elide, icon, skin};

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
    c.boxed()
}

/// Window frame (exposed border):
///  - Win8: 3px accent border at the window edge; content is inset 3px
///    (internal calculation follows the exposed outer border).
///  - Win10: single 1px dark border; content inset 1px.
///
/// Frame thickness (top, right, bottom, left). Win8: thin top, THICK
/// sides+bottom (the classic sizing border); Win10: 1px all around.
/// The internal content is inset by exactly these amounts - the "virtual
/// window" shrinks, the frame never covers content.
/// Frame insets without an Explorer (model-side splitter math).
pub fn frame_insets_pub(self_border: bool, style: crate::model::FrameStyle) -> (f32, f32, f32, f32) {
    if !self_border {
        return (0.0, 0.0, 0.0, 0.0);
    }
    // the frame border keeps a constant physical width (see
    // SPEC/Border_DPI.md): scale the insets like any 1px border
    let w = crate::theme::border_w;
    match style {
        // Win8: the top stays a 1px hairline, but the thick sizing
        // border on the sides and bottom is ordinary UI geometry and
        // follows the DPI
        crate::model::FrameStyle::Win8 => (w(1.0), 6.0, 6.0, 6.0),
        crate::model::FrameStyle::Win10 => (w(1.0), w(1.0), w(1.0), w(1.0)),
    }
}

pub(crate) fn frame_insets(state: &Explorer) -> (f32, f32, f32, f32) {
    frame_insets_pub(state.self_border, state.frame_style)
}

pub(crate) fn frame_color(state: &Explorer, skin: skin::Skin) -> Color {
    match state.frame_style {
        crate::model::FrameStyle::Win8 => skin.accent,
        crate::model::FrameStyle::Win10 => skin.accent_dark,
    }
}

// ---------------------------------------------------------------------------
// caption glyphs
//
// The ONLY hard-coded icons in the app (the D14 exception): the caption
// buttons are window chrome — they follow the frame style and the
// title-bar colors, not the icon pack (a Breeze user still gets the
// Win8/Win10 frame buttons). Every other icon resolves through the
// icon registry.

/// One caption button in both frame variants. The Win8 glyphs are drawn
/// pixel-per-pixel from a real Win8 capture (ratios against the 21px
/// button height, rendered in a 16-unit box); `{title}` is the title-bar
/// text color (dark on the light-blue Win8 bar). The Win10 glyphs are
/// white on the dark title bar.
struct CaptionGlyph {
    /// Win8 frame; `{title}` = title-bar text color.
    win8: &'static str,
    /// Win10 frame (white on the title bar).
    win10: &'static str,
}

/// `_` `[]` `X` — index-parallel to the caption button row in
/// [`caption_overlay`].
static CAPTION_GLYPHS: [CaptionGlyph; 3] = [
    CaptionGlyph {
        // minimize: 7.3 x 2.1 px low bar
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <rect x='3.5' y='11.4' width='9' height='2.6' fill='{title}'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <line x1='3' y1='8' x2='13' y2='8' stroke='#ffffff' stroke-width='1.4'/></svg>",
    },
    CaptionGlyph {
        // maximize: 10.5 x 8.4 px outline, top bar 2.1 px, sides/bottom
        // 1.05 px (a mini window with a thicker title bar)
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <rect x='1.55' y='2.85' width='12.9' height='2.6' fill='{title}'/>\
               <rect x='1.55' y='2.85' width='1.3' height='10.35' fill='{title}'/>\
               <rect x='13.15' y='2.85' width='1.3' height='10.35' fill='{title}'/>\
               <rect x='1.55' y='11.9' width='12.9' height='1.3' fill='{title}'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <rect x='3' y='3' width='10' height='10' fill='none' stroke='#ffffff' stroke-width='1.4'/></svg>",
    },
    CaptionGlyph {
        // close: 8.4 x 7.3 px, stroke 2.1, flat butt ends cut short of
        // the corners (win10: thinner X)
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <line x1='2.85' y1='3.5' x2='13.15' y2='12.55' stroke='#ffffff' stroke-width='2.6'/>\
               <line x1='13.15' y1='3.5' x2='2.85' y2='12.55' stroke='#ffffff' stroke-width='2.6'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <line x1='3.5' y1='3.5' x2='12.5' y2='12.5' stroke='#ffffff' stroke-width='1.5'/>\
                <line x1='12.5' y1='3.5' x2='3.5' y2='12.5' stroke='#ffffff' stroke-width='1.5'/></svg>",
    },
];

/// Render a caption glyph for the frame style: `{title}` takes the
/// title-bar text color (Win8); Win10 templates have no tokens.
fn caption_svg(template: &'static str, skin: skin::Skin) -> iced::widget::svg::Handle {
    let doc = if template.contains("{title}") {
        template.replace("{title}", &crate::icons::hex(skin.title_text))
    } else {
        template.to_string()
    };
    iced::widget::svg::Handle::from_memory(doc.into_bytes())
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
    let (mw, cw, h, glyph, y, right_pad) = match state.frame_style {
        crate::model::FrameStyle::Win8 => (28.0, 47.0, 21.0, 13.0, 1.0, 6.0),
        crate::model::FrameStyle::Win10 => (46.0, 46.0, 29.0, 15.0, 1.0, 1.0),
    };
    let glyphs: [&'static str; 3] = match state.frame_style {
        crate::model::FrameStyle::Win8 => [
            CAPTION_GLYPHS[0].win8,
            CAPTION_GLYPHS[1].win8,
            CAPTION_GLYPHS[2].win8,
        ],
        crate::model::FrameStyle::Win10 => [
            CAPTION_GLYPHS[0].win10,
            CAPTION_GLYPHS[1].win10,
            CAPTION_GLYPHS[2].win10,
        ],
    };
    // Win8: buttons are opaque at rest (titlebar blue / close red) so the
    // frame's top edge never shows as a line above/through them
    let rest_bg: Option<Color> = match state.frame_style {
        crate::model::FrameStyle::Win8 => Some(skin.accent),
        crate::model::FrameStyle::Win10 => None,
    };
    // glyph bottoms share one baseline at ~70% of the button height
    let one = move |glyph_tpl: &'static str,
                    w: f32,
                    msg: Message,
                    hover_bg: Color,
                    solid: bool|
          -> Element<'static, Message> {
        // Win8 keeps the close button red even at rest; Win10 shows it
        // like the other caption buttons (accent bar showing through)
        // and only turns red on hover/press.
        let solid_rest = solid && rest_bg.is_some();
        let rest = if solid_rest { Some(skin.close_idle) } else { rest_bg };
        let blend_white = |c: Color, a: f32| Color {
            r: c.r + (1.0 - c.r) * a,
            g: c.g + (1.0 - c.g) * a,
            b: c.b + (1.0 - c.b) * a,
            a: 1.0,
        };
        button(
            container(
                iced::widget::svg(caption_svg(glyph_tpl, skin))
                    .width(Length::Fixed(glyph))
                    .height(Length::Fixed(glyph)),
            )
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
                (true, _) => rest.map(Into::into),
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
        .boxed()
    };
    let ghost = skin.glass_ghost;
    container(
        row![
            one(glyphs[0], mw, Message::Minimize, ghost, false),
            one(glyphs[1], mw, Message::Maximize, ghost, false),
            one(glyphs[2], cw, Message::Close, skin.close_hover, true),
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
    .boxed()
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
    .boxed()
}

// ---------------- title bar ----------------

/// Title bar height — the ribbon no longer merges its tab row into the
/// title bar (the rs-ribbon widget owns the tab strip), so the bar is a
/// plain drag band: window icon, centered title, caption space.
pub(crate) const TITLEBAR_H: f32 = 30.0;

/// Contextual tab-set blocks (Win8): a pastel accent block per visible
/// contextual set, bottom-aligned in the title bar, positioned exactly
/// above its ribbon tabs (geometry published by the ribbon widget).
fn contextual_blocks(state: &Explorer) -> Element<'static, Message> {
    let (_, fr, _, fl) = frame_insets(state);
    if state.win_w - fl - fr < state.ribbon.sacrifice().0 * state.dpi_multiplier() {
        // ribbon sacrificed: no tabs, no blocks
        return Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(0.0)).boxed();
    }
    let selected = state.ribbon.framework().view().selected_tab();
    let mut r: iced::widget::Row<Element<'_, Message>> = row![].height(Length::Fixed(TITLEBAR_H));
    let mut prev_end = 0.0f32;
    for b in state.ribbon.contextual_blocks() {
        // the block spans exactly its tab run on the strip below
        let bx = b.x;
        let bw = b.w;
        let gap = (bx - prev_end).max(0.0);
        if gap > 0.0 {
            r = r.push(Space::new().width(Length::Fixed(gap)).boxed());
        }
        prev_end = bx + bw;
        let accent = b.color;
        let active = selected >= b.first_tab && selected <= b.last_tab;
        fn tint(c: Color, t: f32) -> Color {
            Color::from_rgb(
                c.r + (1.0 - c.r) * t,
                c.g + (1.0 - c.g) * t,
                c.b + (1.0 - c.b) * t,
            )
        }
        let label_color = Color::from_rgb(accent.r * 0.6, accent.g * 0.6, accent.b * 0.6);
        let solid = container(
            container(
                text(b.label.clone()).size(state.fonts.titlebar.block).color(label_color).width(Length::Shrink),
            )
            .width(Length::Fixed(bw))
            .height(Length::Fixed(24.0))
            .center_x(Length::Fixed(bw))
            .center_y(Length::Fixed(24.0)),
        )
        .width(Length::Fixed(bw))
        .height(Length::Fixed(24.0))
        .style(move |_| container::Style {
            background: Some(tint(accent, if active { 0.45 } else { 0.62 }).into()),
            ..Default::default()
        });
        let block = container(solid)
            .width(Length::Fixed(bw))
            .height(Length::Fixed(TITLEBAR_H))
            .align_y(iced::Alignment::End);
        r = r.push(
            mouse_area(block).on_press(Message::BlockPress(b.first_tab)).boxed(),
        );
    }
    r.boxed()
}

/// Title-bar QAT ("show above the ribbon"): icon buttons right of the
/// window icon, framed by thin separators; the chevron docks the QAT
/// back below the command pane.
fn titlebar_qat(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let sheen = skin.glass_sheen;
    let vline = || {
        container(
            container(Space::new().width(Length::Fixed(crate::theme::border_w(1.0))).height(Length::Fixed(15.0)))
                .style(move |_| container::Style {
                    background: Some(sheen.into()),
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(6.0))
        .center_x(Length::Fixed(6.0))
    };
    let mut qat: iced::widget::Row<Element<'static, Message>> = row![].spacing(2).align_y(iced::Alignment::Center);
    qat = qat.push(vline().boxed());
    for (cmd, icon_name, _enabled) in state.ribbon.qat_buttons() {
        qat = qat.push(
            mouse_area(
                container(crate::icon::view(&icon_name, 16.0, &skin))
                    .width(Length::Fixed(22.0))
                    .height(Length::Fixed(22.0))
                    .center_x(Length::Fixed(22.0))
                    .center_y(Length::Fixed(22.0)),
            )
            .on_press(Message::Ribbon(crate::ribbon::RibbonEvent::Execute {
                command: cmd,
                verb: crate::ribbon::enums::ExecutionVerb::Execute,
                key: None,
                value: None,
            }))
            .boxed(),
        );
    }
    qat = qat.push(
        mouse_area(
            container(crate::icon::view("chevron_down", 10.0, &skin))
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(22.0))
                .center_x(Length::Fixed(14.0))
                .center_y(Length::Fixed(22.0)),
        )
        .on_press(Message::ToggleQatDock)
        .boxed(),
    );
    qat = qat.push(vline().boxed());
    container(qat).height(Length::Fill).center_y(Length::Fill).boxed()
}

pub(crate) fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let icon_zone_w: f32 = 28.0; // window icon zone at the bar's left
    let captions_w = match state.frame_style {
        crate::model::FrameStyle::Win8 => 28.0 + 28.0 + 47.0 + 6.0,
        crate::model::FrameStyle::Win10 => 46.0 * 3.0,
    };
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fr - fl).max(240.0);
    let qat_visible = state.ribbon.qat_docked_top()
        && state.win_w - fl - fr
            >= state.ribbon.sacrifice().0 * state.dpi_multiplier();
    // QAT head width when docked at the top: two separators, the
    // chevron and n 22px buttons with 2px row spacing
    // (mirrors titlebar_qat).
    let head_w = icon_zone_w
        + if qat_visible {
            24.0 * state.ribbon.qat_buttons().len() as f32 + 30.0
        } else {
            0.0
        };
    // the centered title must clear the contextual blocks (they sit
    // above the ribbon tabs, left of the title area) and the QAT head
    let blocks_right = state
        .ribbon
        .contextual_blocks()
        .iter()
        .map(|b| b.x + b.w)
        .fold(0.0f32, f32::max);
    let title_left = (blocks_right + 8.0).max(head_w);
    let title_w = (content_w - title_left - captions_w - 16.0).max(60.0);
    let drag_title: Element<Message> = mouse_area(
        container(
            text(elide(&state.title(), title_w, 8.75))
                .size(state.fonts.titlebar.title)
                .color(skin.title_text)
                .width(Length::Shrink),
        )
        .width(Length::Fixed(title_w))
        .height(Length::Fill)
        .center_x(Length::Fixed(title_w))
        .center_y(Length::Fill),
    )
    .on_press(Message::TitlebarPress)
    .on_double_click(Message::TitlebarDoubleClick)
    .boxed();
    let mut head = row![
        // the window icon follows the current folder (16x16)
        container(icon(state.location_icon(), 16.0, skin))
            .width(Length::Fixed(icon_zone_w))
            .height(Length::Fill)
            .center_x(Length::Fixed(icon_zone_w))
            .center_y(Length::Fill),
    ];
    if qat_visible {
        head = head.push(titlebar_qat(state, skin));
    }
    let base = container(
        row![
            head,
            Space::new().width(Length::Fixed((title_left - head_w).max(0.0))),
            drag_title,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(TITLEBAR_H)))
                .on_press(Message::TitlebarPress)
                .on_double_click(Message::TitlebarDoubleClick),
            Space::new().width(Length::Fixed(captions_w)),
        ]
        .height(Length::Fixed(TITLEBAR_H)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TITLEBAR_H))
    .style(bg(skin.accent));
    stack![base, contextual_blocks(state)].boxed()
}

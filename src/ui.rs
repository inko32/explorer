//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.

pub mod skin {
    use iced::color;

    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    pub struct Skin {
        pub accent: iced::Color,
        pub accent_dark: iced::Color,
        pub titlebar_inactive: iced::Color,
        pub window_bg: iced::Color,
        pub ribbon_bg: iced::Color,
        pub nav_bg: iced::Color,
        pub toolbar_bg: iced::Color,
        pub status_bg: iced::Color,
        pub col_header_bg: iced::Color,
        pub hover: iced::Color,
        pub pressed: iced::Color,
        /// ribbon command hover: fill + border (clone: E8EFF7/A4CEF9)
        pub hover_btn: iced::Color,
        pub hover_btn_border: iced::Color,
        /// circular nav-button hover (clone: E5F3FB/70C0E7)
        pub nav_hover: iced::Color,
        pub nav_hover_border: iced::Color,
        /// File tab (clone: 0066B4, hover 007DDD)
        pub file_tab: iced::Color,
        pub file_tab_hover: iced::Color,
        pub sel: iced::Color,
        pub sel_border: iced::Color,
        pub sep: iced::Color,
        /// group separator line (lighter than sep - a separator, not a border)
        pub group_sep: iced::Color,
        pub text: iced::Color,
        pub subtle: iced::Color,
        pub disabled: iced::Color,
        pub folder: iced::Color,
        pub folder_edge: iced::Color,
        pub close_hover: iced::Color,
        pub address_border: iced::Color,
        /// title-bar text color (dark on the light Win8 blue, white on dark)
        pub title_text: iced::Color,
    }

    pub const LIGHT: Skin = Skin {
        // Win8 100% reference (windows-8-file-explorer-drive-tools.png):
        // light sky-blue chrome, darker File tab, deep outer frame edge
        accent: color!(0x6BADF6),
        accent_dark: color!(0x368BE9),
        titlebar_inactive: color!(0xC8C8C8),
        window_bg: color!(0xFFFFFF),
        ribbon_bg: color!(0xF5F6F7),
        nav_bg: color!(0xFFFFFF),
        toolbar_bg: color!(0xFFFFFF),
        status_bg: color!(0xFFFFFF),
        col_header_bg: color!(0xF7F7F7),
        hover: color!(0xE5F3FF),
        pressed: color!(0xA4CEF9),
        hover_btn: color!(0xE8EFF7),
        hover_btn_border: color!(0xA4CEF9),
        nav_hover: color!(0xE5F3FB),
        nav_hover_border: color!(0x70C0E7),
        file_tab: color!(0x1979CA),
        file_tab_hover: color!(0x2B87D8),
        sel: color!(0xCCE8FF),
        sel_border: color!(0x1979CA),
        sep: color!(0xDADBDC),
        group_sep: color!(0xE2E3E4),
        // note: separators DADBDC outer / E2E3E4 inner (clone)
        text: color!(0x1A1A1A),
        subtle: color!(0x494949),
        disabled: color!(0xA0A0A0),
        folder: color!(0xF6C356),
        folder_edge: color!(0xC88E1E),
        close_hover: color!(0xE81123),
        address_border: color!(0xA9A9A9),
        title_text: color!(0x1A1A1A),
    };

    pub const DARK: Skin = Skin {
        accent: color!(0x4A90D9),
        accent_dark: color!(0x2C5F9E),
        titlebar_inactive: color!(0x3A3A3A),
        window_bg: color!(0x1E1E1E),
        ribbon_bg: color!(0x2D2D2D),
        nav_bg: color!(0x252526),
        toolbar_bg: color!(0x2D2D2D),
        status_bg: color!(0x252526),
        col_header_bg: color!(0x2B2B2C),
        hover: color!(0x3A4046),
        pressed: color!(0x4A525A),
        hover_btn: color!(0x3A4046),
        hover_btn_border: color!(0x4A90D9),
        nav_hover: color!(0x3A4046),
        nav_hover_border: color!(0x4A90D9),
        file_tab: color!(0x1979CA),
        file_tab_hover: color!(0x2B87D8),
        sel: color!(0x2F5480),
        sel_border: color!(0x4A90D9),
        sep: color!(0x454545),
        group_sep: color!(0x505050),
        text: color!(0xE8E8E8),
        subtle: color!(0xA0A0A0),
        disabled: color!(0x6E6E6E),
        folder: color!(0xC89B3C),
        folder_edge: color!(0x8A6618),
        close_hover: color!(0xE81123),
        address_border: color!(0x555555),
        title_text: color!(0xFFFFFF),
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

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space,
};
use iced::{Color, Element, Length};

use crate::fs::{self, Entry};
use crate::model::{self, Explorer, Message, ViewMode};
use crate::ribbon::{self, GroupDef, TabDef};

/// Edge-resize directions for the self-drawn border.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
fn resize_zone(
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

fn frame_insets(state: &Explorer) -> (f32, f32, f32, f32) {
    if !state.self_border {
        return (0.0, 0.0, 0.0, 0.0);
    }
    match state.frame_style {
        crate::model::FrameStyle::Win8 => (1.0, 6.0, 6.0, 6.0),
        crate::model::FrameStyle::Win10 => (1.0, 1.0, 1.0, 1.0),
    }
}

fn frame_color(state: &Explorer, skin: skin::Skin) -> Color {
    match state.frame_style {
        crate::model::FrameStyle::Win8 => skin.accent,
        crate::model::FrameStyle::Win10 => skin.accent_dark,
    }
}

/// Caption buttons (_ [] X) as an overlay at the window's top-right corner:
/// they contact the top edge (Win8: over the frame) and the right edge.
/// Win8: min/max width = 60% of close width, short (20px), thin glyphs.
/// Win10: uniform 46px, nearly full title-bar height.
fn caption_overlay(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
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
                (true, button::Status::Hovered) => {
                    Some(Color::from_rgb8(0xE8, 0x11, 0x23).into())
                }
                (true, button::Status::Pressed) => {
                    Some(Color::from_rgb8(0x8F, 0x0A, 0x15).into())
                }
                (true, _) => Some(Color::from_rgb8(0xC2, 0x0E, 0x1C).into()),
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
fn resize_frame(margin: f32) -> Element<'static, Message> {
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

static UI_DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn ui_debug() -> bool {
    UI_DEBUG.load(std::sync::atomic::Ordering::Relaxed)
}

/// Deterministic distinct color per element name (FNV-1a hash -> palette).
fn debug_color(name: &str) -> Color {
    const PALETTE: [[f32; 3]; 12] = [
        [0.90, 0.15, 0.15], // red
        [0.95, 0.50, 0.05], // orange
        [0.85, 0.75, 0.05], // yellow
        [0.35, 0.75, 0.10], // lime
        [0.05, 0.65, 0.35], // green
        [0.05, 0.75, 0.70], // teal
        [0.10, 0.55, 0.95], // blue
        [0.25, 0.30, 0.95], // indigo
        [0.60, 0.25, 0.90], // purple
        [0.90, 0.20, 0.70], // magenta
        [0.65, 0.35, 0.20], // brown
        [0.10, 0.10, 0.10], // black
    ];
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    let c = PALETTE[(h as usize) % PALETTE.len()];
    Color::from_rgb(c[0], c[1], c[2])
}

/// Wrap an element in a 1px outline (drawn inside — no layout shift).
/// No-op unless `--ui-debug` was passed. Same name = same color.
fn dbg<'a>(name: &'static str, e: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let e = e.into();
    if !ui_debug() {
        return e;
    }
    let color = debug_color(name);
    container(e)
        .style(move |_t: &iced::Theme| container::Style {
            border: iced::Border {
                color,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn view(state: &Explorer) -> Element<'_, Message> {
    UI_DEBUG.store(state.ui_debug, std::sync::atomic::Ordering::Relaxed);
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    let mut root = column![];
    if state.self_border {
        root = root.push(dbg("titlebar", titlebar(state, skin)));
    }
    root = root.push(dbg("ribbon", ribbon(state, skin)));
    root = root.push(dbg("address_bar", address_bar(state, skin)));
    let nav = dbg("nav_pane", nav_pane(state, skin));
    let fv = dbg("file_view", file_view(state, skin));
    root = root.push(dbg("main_row", row![nav, fv].height(Length::Fill)));
    root = root.push(dbg("statusbar", statusbar(state, skin)));
    // ribbon dropdown menu (above everything; backdrop click closes)
    let menu_layer: Option<Element<Message>> = state.open_menu.as_ref().map(|m| {
        let menu_w = m
            .items
            .iter()
            .map(|id| {
                state
                    .ribbon
                    .commands
                    .get(id)
                    .map(|_| id)
                    .unwrap_or(id)
            })
            .map(|_| 0.0f32)
            .sum::<f32>(); // placeholder, computed below
        let _ = menu_w;
        menu_overlay(state, m, skin)
    });
    let (ft, fr, fb, fl) = frame_insets(state);
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
                    background: Some(frame_color(state, skin).into()),
                    ..Default::default()
                })
                .into()
        }
    } else {
        root.into()
    };
    if state.self_border {
        let grab = if state.frame_style == crate::model::FrameStyle::Win8 { 8.0 } else { 5.0 };
        let mut s = stack![dbg("frame", framed), dbg("resize_ring", resize_frame(grab)),
                           dbg("captions", caption_overlay(state, skin))];
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
/// popup anchored under the ribbon at x.
fn menu_overlay(state: &Explorer, m: &crate::model::OpenMenu, skin: skin::Skin) -> Element<'static, Message> {
    let has_sel = !state.selected.is_empty();
    let has_clip = state.clipboard.is_some();
    let has_items = !state.entries.is_empty();
    let mut labels: Vec<(String, String, bool)> = Vec::new(); // (id, label, enabled)
    for id in &m.items {
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
        labels.push((id.clone(), label, enabled));
        let _ = icon_name;
    }
    let w = labels
        .iter()
        .map(|(_, l, _)| l.chars().count() as f32 * 7.0 + 34.0)
        .fold(m.source_w.max(120.0), f32::max);
    let mut items_col = column![].spacing(0);
    for (id, label, enabled) in labels {
        let item_label: iced::widget::Text<'_, iced::Theme, iced::Renderer> = text(label)
            .size(11.5)
            .color(if enabled { skin.text } else { skin.disabled });
        let item = button(item_label)
        .width(Length::Fixed(w))
        .height(Length::Fixed(22.0))
        .padding([0.0, 10.0])
        .style(move |_t: &iced::Theme, status: button::Status| button::Style {
            background: match (enabled, status) {
                (true, button::Status::Hovered) => Some(skin.nav_hover.into()),
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
    let popup = container(items_col)
        .width(Length::Fixed(w))
        .style(move |_| container::Style {
            background: Some(skin.window_bg.into()),
            border: iced::Border {
                color: skin.sep,
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
    let (ft, _, _, fl) = frame_insets(state);
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

fn bg(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: Some(color.into()),
        ..Default::default()
    }
}

fn icon(name: &str, size: f32, skin: skin::Skin) -> Element<'static, Message> {
    crate::icon::view(name, size, &skin)
}

// ---------------- title bar ----------------

/// Middle-ellipsis text clamp to a pixel width.
fn elide(s: &str, max_w: f32) -> String {
    let w = s.chars().count() as f32 * 7.5;
    if w <= max_w {
        return s.to_string();
    }
    let keep = ((max_w / 7.5).max(1.0) as usize).saturating_sub(3);
    let take = keep / 2;
    let start: String = s.chars().take(take).collect();
    let tail: String = s
        .chars()
        .rev()
        .take(keep.saturating_sub(take))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}\u{2026}{}", start, tail)
}

fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // Win8: contextual tab labels are colored blocks on the title bar,
    // right-aligned before the caption buttons. Title centers in the
    // space left of them (so it shifts when they appear).
    let mut ctx_blocks = row![].height(Length::Fixed(30.0));
    let mut first_block = true;
    for tab in visible_tabs(state) {
        let Some(accent) = tab.color.as_deref().and_then(ribbon::color_by_name) else { continue };
        if !first_block {
            // space between two contextual blocks, aligning with the tab row
            ctx_blocks = ctx_blocks.push(Space::new().width(Length::Fixed(5.0)));
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
            .height(Length::Fixed(24.0))
            .center_x(Length::Fixed(width))
            .center_y(Length::Fixed(24.0))
            .style(move |_| container::Style {
                background: Some(tint(accent, if active { 0.45 } else { 0.62 }).into()),
                ..Default::default()
            });
        // lowered: 24px solid block bottom-aligned in the 30px title bar
        let block: Element<Message> = container(solid)
            .width(Length::Fixed(width))
            .height(Length::Fixed(30.0))
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
                    .width(Length::Fixed(22.0))
                    .height(Length::Fixed(22.0))
                    .center_x(Length::Fixed(22.0))
                    .center_y(Length::Fixed(22.0)),
            )
            .on_press(Message::QuickAccess(id)),
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
        .on_press(Message::ToggleRibbonCollapse),
    );
    qat = qat.push(vline());
    // exact: sep 6+2, per button 22+2, chevron 14+2, sep 6
    let qat_w: f32 = state.quick_access.len() as f32 * 24.0 + 30.0;

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
            Space::new().width(Length::Fixed(gap_w)).height(Length::Fixed(30.0)),
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
                    .width(Length::Fixed(28.0))
                    .height(Length::Fill)
                    .center_x(Length::Fixed(28.0))
                    .center_y(Length::Fill),
                container(qat).width(Length::Fixed(qat_w)).height(Length::Fill).center_y(Length::Fill),
                drag_gap,
                ctx_blocks,
                drag_title,
                Space::new().width(Length::Fixed(captions_w)),
            ]
            .height(Length::Fixed(30.0)),
        )
        .width(Length::Fill)
        .height(Length::Fixed(30.0))
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
                .width(Length::Fixed(28.0))
                .height(Length::Fill)
                .center_x(Length::Fixed(28.0))
                .center_y(Length::Fill),
            container(qat).width(Length::Fixed(qat_w)).height(Length::Fill).center_y(Length::Fill),
            drag_title,
            ctx_blocks,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(30.0)))
                .on_press(Message::TitlebarPress)
                .on_double_click(Message::TitlebarDoubleClick),
            Space::new().width(Length::Fixed(captions_w)),
        ]
        .height(Length::Fixed(30.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(30.0))
    .style(bg(skin.accent))
    .into()
}

// ---------------- ribbon ----------------

fn ribbon(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // clone/Win8: a 1px line under the tab row; the body has NO top border
    // (the active tab merges into the body - a same-color bridge covers the
    // line beneath it, so no bottom edge on the focused tab).
    let line: Element<Message> = container(
        Space::new().width(Length::Fill).height(Length::Fixed(1.0)),
    )
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.sep.into()),
        ..Default::default()
    })
    .into();
    let body = container(ribbon_body(state, skin))
        .width(Length::Fill)
        .height(Length::Fixed(94.0))
        .style(move |_| container::Style {
            background: Some(skin.ribbon_bg.into()),
            ..Default::default()
        });
    // bottom edge of the ribbon body (the top edge is `line` above)
    let bottom_line: Element<Message> = container(
        Space::new().width(Length::Fill).height(Length::Fixed(1.0)),
    )
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.sep.into()),
        ..Default::default()
    })
    .into();
    if state.ribbon_collapsed {
        return column![tab_row(state, skin), line].into();
    }
    let col = column![tab_row(state, skin), line, body, bottom_line];
    match active_tab_geometry(state) {
        Some((x, w)) => {
            let bridge = container(
                column![
                    Space::new().width(Length::Fill).height(Length::Fixed(26.0)),
                    container(
                        row![
                            Space::new().width(Length::Fixed(x)),
                            container(Space::new().width(Length::Fixed(w)).height(Length::Fixed(3.0)))
                                .style(move |_| container::Style {
                                    background: Some(skin.ribbon_bg.into()),
                                    ..Default::default()
                                }),
                        ]
                        .width(Length::Fill),
                    )
                    .height(Length::Fixed(3.0)),
                ]
                .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill);
            stack![col, bridge].into()
        }
        None => col.into(),
    }
}

fn tab_button(
    label: &str,
    width: f32,
    active: bool,
    is_file: bool,
    accent: Option<Color>,
    highlight_bar: bool,
    msg: Message,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // Win8 contextual tabs: colored label + colored highlight bar at the
    // top of the tab, touching the title bar (only with the self-drawn
    // border — native decorations have no title area for the bar).
    let text_color = if is_file {
        Color::WHITE
    } else if active {
        skin.text
    } else {
        skin.subtle
    };
    let text_color = accent.map(|a| if active { a } else { a.scale_alpha(0.75) }).unwrap_or(text_color);
    let btn = button(
        container(text(label.to_string()).size(12.0).color(text_color))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(28.0))
    .padding(0)
        .style(move |_t, status| {
            // clone: File tab solid brand color; main tabs transparent with a
            // 1px #EBECEC outline on hover; active tab fills the body color
            // and borders top/sides #DADBDC, bottom merges into the body.
            let outline = Color::from_rgb8(0xEB, 0xEC, 0xEC);
            let frame = skin.sep;
            let (bg, border_color, border_w) = if is_file {
                let c = match status {
                    button::Status::Hovered => skin.file_tab_hover,
                    _ => skin.file_tab,
                };
                (Some(c), c, 0.0)
            } else {
                match (active, status) {
                    (true, _) => (Some(skin.ribbon_bg), frame, 1.0),
                    (false, button::Status::Hovered) => (None, outline, 1.0),
                    _ => (None, Color::TRANSPARENT, 0.0),
                }
            };
            button::Style {
                background: bg.map(Into::into),
                text_color,
                border: iced::Border {
                    color: if is_file { Color::TRANSPARENT } else { accent.filter(|_| active && highlight_bar).unwrap_or(border_color) },
                    width: if is_file { 0.0 } else { border_w },
                    radius: 0.0.into(),
                },
                shadow: Default::default(),
                snap: true,
            }
        })
        .on_press(msg);
    match (accent, highlight_bar) {
        (Some(a), true) => column![
            container(Space::new().width(Length::Fixed(width)).height(Length::Fill))
                .width(Length::Fixed(width))
                .height(Length::Fixed(if active { 3.0 } else { 2.0 }))
                .style(move |_| container::Style {
                    background: Some(a.scale_alpha(if active { 1.0 } else { 0.45 }).into()),
                    ..Default::default()
                }),
            btn,
        ]
        .width(Length::Fixed(width))
        .into(),
        _ => btn.into(),
    }
}

/// Tabs visible right now: main tabs + contextual tabs whose `when` matches.

fn tab_width(label: &str) -> f32 {
    // Win8 tabs are roomy: 76px minimum, extending when the text is longer
    (20.0 + label.chars().count() as f32 * 7.5).max(76.0)
}

/// Exact rendered width of a tab (File gets the wide Win8 block).
fn tab_row_w(tab: &ribbon::TabDef) -> f32 {
    if tab.kind.as_deref() == Some("file") {
        62.0
    } else {
        tab_width(&tab.label)
    }
}

/// Width of the main tabs region in the tab row — contextual title-bar
/// blocks align directly above their stubs.
fn main_tabs_width(state: &Explorer) -> f32 {
    // main tabs: no spacing between them
    state.ribbon.tabs.iter().map(tab_row_w).sum()
}

/// x offset + width of the currently active tab within the tab row.
fn active_tab_geometry(state: &Explorer) -> Option<(f32, f32)> {
    let mut x = 0.0f32;
    let mut prev_ctx = false;
    for tab in visible_tabs(state) {
        let ctx = tab.color.is_some();
        if ctx && prev_ctx {
            x += 5.0; // space only between two contextual tabs
        }
        let w = tab_row_w(tab);
        if tab.id == state.active_tab {
            return Some((x, w));
        }
        x += w;
        prev_ctx = ctx;
    }
    None
}

fn visible_tabs(state: &Explorer) -> Vec<&TabDef> {
    let ctx = state.context();
    state
        .ribbon
        .tabs
        .iter()
        .chain(
            state
                .ribbon
                .contextual_tabs
                .iter()
                .filter(|t| t.when.as_deref().map(|w| ribbon::eval_when(w, ctx)).unwrap_or(true)),
        )
        .collect()
}

fn tab_row(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // ribbon minimize chevron (Win8: also double-click a tab)
    let collapse = button(icon(if state.ribbon_collapsed { "chevron_down" } else { "up" }, 16.0, skin))
        .width(Length::Fixed(26.0))
        .height(Length::Fixed(26.0))
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
        .on_press(Message::ToggleRibbonCollapse);
    let help = button(icon("help", 18.0, skin))
        .width(Length::Fixed(26.0))
        .height(Length::Fixed(26.0))
        .style(|_t, _status| button::Style {
            background: None,
            text_color: Color::TRANSPARENT,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        });
    let mut tabs_row = row![].height(Length::Fixed(28.0));
    let mut prev_ctx = false;
    for tab in visible_tabs(state) {
        let width = tab_row_w(tab);
        let accent = tab.color.as_deref().and_then(ribbon::color_by_name);
        let ctx = accent.is_some();
        if ctx && prev_ctx {
            // space only between two contextual tabs (Win8 reference)
            tabs_row = tabs_row.push(Space::new().width(Length::Fixed(5.0)));
        }
        prev_ctx = ctx;
        if let Some(accent) = accent.filter(|_| state.self_border) {
            // Win8: connecting stub under the title-bar block, labeled with
            // the first group's caption.
            let active = state.active_tab == tab.id;
            let stub_label = tab.groups.first().map(|g| g.label.clone()).unwrap_or_default();
            let label = container(
                text(stub_label)
                    .size(12.0)
                    .color(accent.scale_alpha(0.9))
                    .width(Length::Shrink),
            )
            .width(Length::Fixed(width - 2.0))
            .height(Length::Fill)
            .center_x(Length::Fixed(width - 2.0))
            .center_y(Length::Fill);
            // idle contextual tab: its BORDER fades - solid top, sides fading
            let stub: Element<Message> = if active {
                container(label)
                    .width(Length::Fixed(width))
                    .height(Length::Fixed(28.0))
                    .style(move |_| container::Style {
                        background: Some(skin.ribbon_bg.into()),
                        border: iced::Border {
                            color: accent,
                            width: 1.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            } else {
                let side = move || {
                    container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
                        .width(Length::Fixed(1.0))
                        .height(Length::Fill)
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Gradient(
                                iced::Gradient::Linear(
                                    iced::gradient::Linear::new(iced::Radians(
                                        std::f32::consts::FRAC_PI_2,
                                    ))
                                    .add_stop(0.0, accent)
                                    .add_stop(0.55, accent.scale_alpha(0.5))
                                    .add_stop(1.0, Color::TRANSPARENT),
                                ),
                            )),
                            ..Default::default()
                        })
                };
                let top = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .style(move |_| container::Style {
                        background: Some(accent.into()),
                        ..Default::default()
                    });
                column![
                    top,
                    row![side(), label, side(),].height(Length::Fill),
                ]
                .width(Length::Fixed(width))
                .height(Length::Fixed(28.0))
                .into()
            };
            tabs_row = tabs_row.push(dbg(
                Box::leak(format!("tab_stub:{}", tab.id).into_boxed_str()),
                mouse_area(stub).on_press(Message::SetTab(tab.id.clone())),
            ));
        } else {
            let t = tab_button(
                &tab.label,
                width,
                state.active_tab == tab.id,
                tab.kind.as_deref() == Some("file"),
                accent,
                state.self_border,
                Message::SetTab(tab.id.clone()),
                skin,
            );
            tabs_row = tabs_row.push(dbg(
                Box::leak(format!("tab:{}", tab.id).into_boxed_str()),
                t,
            ));
        }
    }
    container(
        row![
            tabs_row,
            Space::new().width(Length::Fill),
            container(collapse).height(Length::Fixed(28.0)).center_y(Length::Fixed(28.0)),
            container(help).height(Length::Fixed(28.0)).center_y(Length::Fixed(28.0)).padding([0.0, 6.0]),
        ]
        .height(Length::Fixed(28.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(28.0))
    .style(bg(skin.window_bg)) // white strip - the accent frame must not bleed through
    .into()
}

fn cmd_style(
    enabled: bool,
    skin: skin::Skin,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    move |_t, status| {
        let (bgc, border_c, tc) = if !enabled {
            (None, Color::TRANSPARENT, skin.disabled)
        } else {
            match status {
                button::Status::Hovered => (Some(skin.hover_btn), skin.hover_btn_border, skin.text),
                button::Status::Pressed => (Some(skin.pressed), skin.hover_btn_border, skin.text),
                _ => (None, Color::TRANSPARENT, skin.text),
            }
        };
        button::Style {
            background: bgc.map(Into::into),
            text_color: tc,
            border: iced::Border {
                color: border_c,
                width: if border_c == Color::TRANSPARENT { 0.0 } else { 1.0 },
                radius: 0.0.into(),
            },
            shadow: Default::default(),
            snap: true,
        }
    }
}


fn big_cmd_w(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // icon zone 46 (icon a bit higher) + always-2-line label zone 30 = 76px,
    // exactly three 23px small rows + spacing + group pad
    let content = column![
        container(icon_el)
            .width(Length::Fixed(width))
            .height(Length::Fixed(46.0))
            .center_x(Length::Fixed(width))
            .center_y(Length::Fixed(46.0)),
        dbg(
            "big_label",
            container(
                text(label.to_string())
                    .size(11.0)
                    .line_height(1.15)
                    .color(if enabled { skin.text } else { skin.disabled })
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fixed(width))
            .height(Length::Fixed(30.0))
            .center_x(Length::Fixed(width))
            .center_y(Length::Fixed(30.0)),
        ),
    ]
    .width(Length::Fixed(width));
    let btn = button(dbg(
        Box::leak(format!("big_cmd:{label}").into_boxed_str()),
        content,
    ))
    .width(Length::Fixed(width))
    .height(Length::Fixed(76.0))
    .padding(0)
    .style(cmd_style(enabled, skin));
    match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    }
}


fn small_cmd_active(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    height: f32,
    enabled: bool,
    active: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let row = row![
        icon_el,
        text(label.to_string()).size(11.0).color(if enabled { skin.text } else { skin.disabled }),
    ]
    .spacing(5)
    .align_y(iced::Alignment::Center);
    // iced buttons top-align their content: center it in the full row height
    let content = container(row)
        .height(Length::Fixed(height))
        .center_y(Length::Fixed(height));
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .padding([0.0, 4.0])
        .style(move |_t, status| {
            let mut style = cmd_style(enabled, skin)(_t, status);
            if active {
                style.background = Some(skin.pressed.into());
                style.border = iced::Border {
                    color: skin.accent,
                    width: 1.0,
                    radius: 0.0.into(),
                };
            }
            style
        });
    let el: Element<Message> = match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    };
    dbg(Box::leak(format!("cmd:{label}").into_boxed_str()), el)
}

fn group(caption: &str, width: f32, content: Element<'static, Message>, skin: skin::Skin) -> Element<'static, Message> {
    // clone: groups pad 5px, separator line on the RIGHT, caption #494949
    // a | line with space at the top and bottom, not a full-height square
    let strip = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(64.0)))
        .style(bg(skin.group_sep))
        .height(Length::Fill)
        .center_y(Length::Fill);
    row![
        column![
            container(dbg(Box::leak(format!("group_body:{caption}").into_boxed_str()), content))
                .width(Length::Fixed(width))
                .height(Length::Fixed(78.0))
                .padding(iced::Padding { top: 6.0, right: 5.0, bottom: 0.0, left: 5.0 }),
            container(text(caption.to_string()).size(11.0).color(skin.subtle))
                .width(Length::Fixed(width))
                .height(Length::Fixed(15.0))
                .center_x(Length::Fixed(width))
                .style(bg(skin.ribbon_bg)),
        ],
        strip,
    ]
    .into()
}

fn ribbon_body(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let Some(tab) = visible_tabs(state).into_iter().find(|t| t.id == state.active_tab) else {
        return Space::new().width(Length::Fill).height(Length::Fixed(94.0)).into();
    };
    let mut groups_row = row![];
    let mut x = 0.0f32;
    for g in &tab.groups {
        let name: &'static str = Box::leak(format!("group:{}", g.label).into_boxed_str());
        groups_row = groups_row.push(dbg(name, group_from_config(g, state, skin, x)));
        x += g.width.unwrap_or(110.0) + 1.0; // group + separator strip
    }
    groups_row.into()
}

/// Build one group's content from YAML: big commands stand alone, small
/// commands stack into `columns` columns (sequential chunks).
fn group_from_config(g: &GroupDef, state: &Explorer, skin: skin::Skin, x_base: f32) -> Element<'static, Message> {
    let has_sel = !state.selected.is_empty();
    let has_clip = state.clipboard.is_some();
    let has_items = !state.entries.is_empty();
    let enabled_of = |cmd: &ribbon::CmdRef| {
        cmd.enabled.unwrap_or(true) && ribbon::command_enabled(&cmd.id, has_sel, has_clip, has_items)
    };
    let menu_source = state.open_menu.as_ref().map(|m| m.source.as_str());
    let active_of = |id: &str| match id {
        _ if Some(id) == menu_source => true, // pressed while its menu is open
        "toggle_border" => state.self_border,
        "toggle_dark" => state.dark,
        "toggle_hidden" => state.show_hidden,
        "toggle_frame" => state.frame_style == crate::model::FrameStyle::Win8,
        _ => false,
    };

    let mut content = row![].spacing(2);
    let mut bigs_w = 0.0f32;
    for cmd in &g.commands {
        if cmd.separator == Some(true) {
            // internal | separator inside the group (inset top+bottom)
            let strip = container(
                container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(62.0)))
                    .style(bg(skin.group_sep)),
            )
            .width(Length::Fixed(11.0))
            .height(Length::Fixed(76.0))
            .center_x(Length::Fixed(11.0))
            .center_y(Length::Fixed(76.0));
            content = content.push(strip);
            bigs_w += 13.0;
            continue;
        }
        if cmd.size.as_deref() == Some("big") {
            let enabled = enabled_of(cmd);
            let w = cmd.width.unwrap_or(62.0);
            let has_action = state.ribbon.commands.contains_key(&cmd.id);
            let msg = enabled.then(|| Message::RunCommand(cmd.id.clone()));
            let el: Element<Message> = if let Some(items) = &cmd.dropdown {
                // Win8 split big button: icon = action, label+v = menu
                big_cmd_split(
                    crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 40.0, &skin),
                    &cmd.label,
                    w,
                    enabled,
                    if has_action { msg } else { None },
                    enabled.then(|| Message::OpenMenu {
                        x: menu_anchor(state, x_base + 5.0 + bigs_w),
                        y: 30.0 + 28.0 + 1.0 + 6.0 + 76.0,
                        source: cmd.id.clone(),
                        source_w: w,
                        items: items.clone(),
                    }),
                    skin,
                )
            } else {
                big_cmd_w(
                    crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 40.0, &skin),
                    &cmd.label,
                    w,
                    enabled,
                    msg,
                    skin,
                )
            };
            bigs_w += w + 2.0;
            content = content.push(el);
        }
    }

    let small_cmds: Vec<&ribbon::CmdRef> =
        g.commands.iter().filter(|c| c.size.as_deref() != Some("big")).collect();
    // `rows` = vertical slots per column (Win8: max 3, never four).
    // Buttons tile column-major; a column with fewer buttons stays compact.
    // Row height is FIXED (Win8/10: 2-row and 3-row columns look identical).
    let rows = g.rows.unwrap_or(3).clamp(1, 3) as usize;
    let columns = small_cmds.len().div_ceil(rows).max(1);
    let row_h = 23.0;
    let col_w = small_cmds
        .iter()
        .map(|c| c.width.unwrap_or(100.0))
        .fold(0.0f32, f32::max);
    let mut smalls: Vec<Element<Message>> = Vec::new();
    for (j, cmd) in small_cmds.iter().enumerate() {
        let enabled = enabled_of(cmd);
        let active = active_of(&cmd.id);
        let w = cmd.width.unwrap_or(100.0);
        let has_action = state.ribbon.commands.contains_key(&cmd.id);
        let msg = enabled.then(|| Message::RunCommand(cmd.id.clone()));
        let el: Element<Message> = if cmd.kind.as_deref() == Some("checkbox") {
            checkbox_cmd(&cmd.label, w, row_h, enabled, active, msg, skin)
        } else if let Some(items) = &cmd.dropdown {
            // split small: label runs the command (or opens the menu when it
            // has no action); the arrow always opens the menu
            let label_msg = if has_action { msg.clone() } else { None };
            // menu directly under this button: left edge of its column,
            // y = the button's bottom (titlebar 30 + tabs 28 + line 1 +
            // group pad 6 + row*(23+1) + 23)
            let col = (j / rows) as f32;
            let row_i = (j % rows) as f32;
            let ax = x_base + 5.0 + bigs_w + col * (col_w + 2.0);
            let ay = 30.0 + 28.0 + 1.0 + 6.0 + row_i * 24.0 + 23.0;
            let menu_msg = enabled.then(|| Message::OpenMenu {
                x: menu_anchor(state, ax),
                y: ay,
                source: cmd.id.clone(),
                source_w: w,
                items: items.clone(),
            });
            let arrow: Element<Message> = small_cmd_active(
                drop_arrow_el(if enabled { skin.subtle } else { skin.disabled }, 0.0),
                "",
                18.0,
                row_h,
                enabled,
                false,
                menu_msg.clone().or(if has_action { None } else { msg.clone() }),
                skin,
            );
            let main: Element<Message> = small_cmd_active(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 16.0, &skin),
                &cmd.label,
                w - 18.0,
                row_h,
                enabled,
                active,
                label_msg.or(menu_msg),
                skin,
            );
            row![main, arrow].width(Length::Fixed(w)).into()
        } else {
            small_cmd_active(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 16.0, &skin),
                &cmd.label,
                w,
                row_h,
                enabled,
                active,
                msg,
                skin,
            )
        };
        smalls.push(el);
    }
    let mut it = smalls.into_iter();
    for _ in 0..columns {
        let mut col = column![].spacing(1);
        for _ in 0..rows {
            if let Some(s) = it.next() {
                col = col.push(s);
            }
        }
        content = content.push(col);
    }
    if g.kind.as_deref() == Some("gallery") {
        // gallery: the tiled grid plus an expand chevron that opens the
        // full list as a dropdown pane (Win8 Layout group)
        let items: Vec<String> = g.commands.iter().map(|c| c.id.clone()).collect();
        let gw = g.width.unwrap_or(300.0);
        let expand = button(crate::icon::view("chevron_down", 12.0, &skin))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(76.0))
            .style(cmd_style(true, skin))
            .on_press(Message::OpenMenu {
                x: menu_anchor(state, x_base + gw - 60.0),
                y: 30.0 + 28.0 + 1.0 + 6.0 + 76.0,
                source: g.label.clone(),
                source_w: gw,
                items,
            });
        let grid = container(content)
            .width(Length::Fixed(gw - 24.0))
            .height(Length::Fixed(78.0));
        return group(
            &g.label.clone(),
            gw,
            row![grid, expand].height(Length::Fixed(78.0)).into(),
            skin,
        );
    }
    group(&g.label.clone(), g.width.unwrap_or(110.0), content.into(), skin)
}

/// Checkbox command row: a 13px box + label, standard row height. The
/// "holding" (checked) state is the command's active state.
fn checkbox_cmd(
    label: &str,
    width: f32,
    height: f32,
    enabled: bool,
    checked: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let box_el = container(
        text(if checked { "\u{2713}" } else { "" })
            .size(10.0)
            .color(Color::WHITE),
    )
    .width(Length::Fixed(13.0))
    .height(Length::Fixed(13.0))
    .center_x(Length::Fixed(13.0))
    .center_y(Length::Fixed(13.0))
    .style(move |_| container::Style {
        background: Some(
            (if checked { skin.accent } else { skin.window_bg }).into(),
        ),
        border: iced::Border {
            color: if checked { skin.accent } else { skin.disabled },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });
    let row = row![
        box_el,
        text(label.to_string()).size(11.0).color(if enabled { skin.text } else { skin.disabled }),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let content = container(row)
        .height(Length::Fixed(height))
        .center_y(Length::Fixed(height));
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .padding([0.0, 4.0])
        .style(cmd_style(enabled, skin));
    let el: Element<Message> = match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    };
    dbg(Box::leak(format!("checkbox:{label}").into_boxed_str()), el)
}

/// Win8 split big button: the icon area runs the default action, the
/// label+v strip opens the dropdown menu.
fn big_cmd_split(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    action: Option<Message>,
    menu: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let tc = if enabled { skin.text } else { skin.disabled };
    let top = button(
        container(
            container(icon_el)
                .width(Length::Fixed(44.0))
                .height(Length::Fixed(44.0))
                .center_x(Length::Fixed(44.0))
                .center_y(Length::Fixed(44.0)),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(46.0))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(46.0)),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(46.0))
    .padding(0)
    .style(cmd_style(enabled, skin));
    let top: Element<Message> = match (enabled, action) {
        (true, Some(m)) => top.on_press(m).into(),
        _ => top.into(),
    };
    // one word: `Delete` / `v`; two words: `Navigation` / `Pane v`
    let words: Vec<&str> = label.split_whitespace().collect();
    let (line1, line2): (String, Option<String>) = if words.len() > 1 {
        (
            words[..words.len() - 1].join(" "),
            Some(words[words.len() - 1].to_string()),
        )
    } else {
        (label.to_string(), None)
    };
    let line2_row: Element<Message> = match line2 {
        Some(last) => row![
            text(last).size(11.0).color(tc),
            drop_arrow_el(tc, 5.5),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Start)
        .into(),
        // arrow alone on line 2 (one-word labels): same offset and height as
        // the "word v" row, so all big split buttons' line 2 aligns
        None => container(drop_arrow_el(tc, 5.5))
            .height(Length::Fixed(13.2))
            .align_y(iced::Alignment::Start)
            .into(),
    };
    let bottom_col = column![
        container(text(line1).size(11.0).color(tc))
            .width(Length::Fixed(width))
            .center_x(Length::Fixed(width)),
        container(line2_row)
            .width(Length::Fixed(width))
            .center_x(Length::Fixed(width)),
    ]
    .width(Length::Fixed(width));
    let bottom = button(bottom_col)
    .width(Length::Fixed(width))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(cmd_style(enabled, skin));
    let bottom: Element<Message> = match (enabled, menu) {
        (true, Some(m)) => bottom.on_press(m).into(),
        _ => bottom.into(),
    };
    let name: &'static str = Box::leak(format!("big_split:{label}").into_boxed_str());
    dbg(name, column![dbg("big_split_icon", top), dbg("big_split_label", bottom)]
        .width(Length::Fixed(width))
        .align_x(iced::Alignment::Center))
    .into()
}

/// 90-degree dropdown triangle, 7x4 px, top-padded so its base touches the
/// x-height TOP of an adjacent 11px label (5.5 px below the line top).
fn drop_arrow_el(c: Color, top_pad: f32) -> Element<'static, Message> {
    container(
        iced::widget::svg(crate::icons::drop_arrow(c))
            .width(Length::Fixed(7.0))
            .height(Length::Fixed(4.0)),
    )
    .padding(iced::Padding {
        top: top_pad,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })
    .into()
}

/// Clamp a dropdown menu anchor x so the menu stays inside the window.
fn menu_anchor(state: &Explorer, x: f32) -> f32 {
    let (_, fr, _, fl) = frame_insets(state);
    x.clamp(4.0, (state.win_w - fl - fr - 220.0).max(4.0))
}

// ---------------- address bar ----------------

fn nav_circle(icon_name: &'static str, enabled: bool, msg: Message, skin: skin::Skin) -> Element<'static, Message> {
    let btn = button(crate::icon::view(icon_name, 18.0, &skin))
        .width(Length::Fixed(30.0))
        .height(Length::Fixed(30.0))
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
                radius: 15.0.into(),
            },
            shadow: Default::default(),
            snap: true,
        });
    if enabled {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

fn address_bar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let drag = state.bar_split;
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fl - fr).max(320.0);
    let search_w = (content_w * state.search_share).clamp(120.0, content_w - 280.0);
    let refresh = button(icon("refresh", 16.0, skin))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
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
                background: None,
                ..Default::default()
            })
            .into();
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
                .into()
        };
        crumbs = crumbs.push(cell_el);
        if !is_last {
            crumbs = crumbs.push(icon("chevron_right", 9.0, skin));
        }
    }

    let crumb_bar = container(
        row![crumbs, Space::new().width(Length::Fill), refresh]
            .align_y(iced::Alignment::Center)
            .padding([0.0, 4.0]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(30.0))
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: Color::from_rgb8(0xD9, 0xD9, 0xD9),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // search box: placeholder left, recessed magnifier button at the right
    // end (Win8: light-blue tint + thin divider)
    let search_btn = container(icon("search", 13.0, skin))
        .width(Length::Fixed(26.0))
        .height(Length::Fill)
        .center_x(Length::Fixed(26.0))
        .center_y(Length::Fixed(30.0))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(Color::from_rgb8(0xE8, 0xF1, 0xFB).into()),
            border: iced::Border {
                color: Color::from_rgb8(0xA9, 0xC4, 0xE8),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    let search = container(
        row![
            text(format!("Search {}", state.title()))
                .size(12.0)
                .color(skin.disabled),
            Space::new().width(Length::Fill),
            search_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(search_w))
    .height(Length::Fixed(30.0))
    .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 8.0 })
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: Color::from_rgb8(0xD9, 0xD9, 0xD9),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // splitter between the bars: a 1px line (accent on hover/drag) in a
    // small grab target, with the horizontal-resize cursor
    let line_c = if drag || state.splitter_hover { skin.accent_dark } else { skin.group_sep };
    let splitter = mouse_area(
        container(
            container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(24.0)))
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(30.0))
                .center_x(Length::Fixed(1.0))
                .center_y(Length::Fixed(30.0))
                .style(bg(line_c)),
        )
        .width(Length::Fixed(7.0))
        .height(Length::Fixed(30.0))
        .center_x(Length::Fixed(7.0))
        .center_y(Length::Fixed(30.0)),
    )
    .on_press(Message::BeginBarSplit)
    .on_enter(Message::SplitterHover(true))
    .on_exit(Message::SplitterHover(false))
    .interaction(iced::mouse::Interaction::ResizingHorizontally);
    container(
        row![
            nav_circle("back", !state.back_is_empty(), Message::Back, skin),
            nav_circle("forward", !state.forward_is_empty(), Message::Forward, skin),
            nav_circle("up", state.cwd.parent().is_some(), Message::Up, skin),
            crumb_bar,
            splitter,
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

// ---------------- nav pane ----------------

fn nav_item(
    label: String,
    icon_name: &'static str,
    selected: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let item: Element<Message> = container(
        row![
            crate::icon::view(icon_name, 16.0, &skin),
            text(label).size(12.0).color(skin.text),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([2.0, 0.0]),
    )
    .padding([3.0, 8.0])
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into();
    match msg {
        Some(m) => mouse_area(item).on_press(m).into(),
        None => item,
    }
}

fn nav_pane(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut list = column![].spacing(0).padding(4);

    list = list.push(
        row![
            icon("star", 14.0, skin),
            text("Favorites").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([2.0, 6.0]),
    );
    let mut favs: Vec<(&str, &'static str, Option<PathBuf>)> = vec![
        ("Home", "home", home.clone()),
        ("Desktop", "desktop", home.as_ref().map(|h| h.join("Desktop"))),
        ("Documents", "documents", home.as_ref().map(|h| h.join("Documents"))),
        ("Downloads", "downloads", home.as_ref().map(|h| h.join("Downloads"))),
        ("Recent places", "recent", None),
    ];
    for (label, handle, target) in favs.drain(..) {
        let selected = target.as_deref() == Some(state.cwd.as_path());
        let msg = target.map(Message::Navigate);
        list = list.push(nav_item(label.to_string(), handle, selected, msg, skin));
    }

    list = list.push(
        row![
            icon("computer", 14.0, skin),
            text("This PC").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item(
        "Computer".to_string(),
        "computer",
        state.cwd == Path::new("/"),
        Some(Message::Navigate(PathBuf::from("/"))),
        skin,
    ));
    for v in &state.volumes {
        let selected = state.cwd == v.mount;
        let mount = v.mount.clone();
        list = list.push(nav_item(
            v.name.clone(),
            "drive",
            selected,
            Some(Message::Navigate(mount)),
            skin,
        ));
    }

    list = list.push(
        row![
            icon("network", 14.0, skin),
            text("Network").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item("Network".to_string(), "network", false, None, skin));

    container(list)
        .width(Length::Fixed(210.0))
        .height(Length::Fill)
        .style(bg(skin.nav_bg))
        .into()
}

// ---------------- file view ----------------

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

    let body: Element<Message> = if state.loading {
        container(
            container(text("Loading…").size(12.0).color(skin.subtle))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        match state.view {
        ViewMode::Details => {
            // Virtualized: build only the rows in view (+overscan), with
            // spacers preserving total scroll height (D13).
            const ROW_H: f32 = 24.0;
            let total = state.entries.len();
            let view_rows = (state.viewport_h / ROW_H).ceil() as usize + 4;
            let first = ((state.scroll_y / ROW_H).floor() as usize).saturating_sub(2);
            let last = (first + view_rows).min(total);
            let mut rows = column![].spacing(0);
            if first > 0 {
                rows = rows.push(Space::new().height(Length::Fixed(first as f32 * ROW_H)));
            }
            for i in first..last {
                if let Some(e) = state.entries.get(i) {
                    rows = rows.push(file_row(i, e, state, skin));
                }
            }
            if last < total {
                rows = rows.push(Space::new().height(Length::Fixed((total - last) as f32 * ROW_H)));
            }
            scrollable(rows)
                .height(Length::Fill)
                .on_scroll(|vp| Message::Scrolled {
                    y: vp.absolute_offset().y,
                    height: vp.bounds().height,
                })
                .into()
        }
        ViewMode::Large => {
            let mut grid = row![].spacing(8).padding(8);
            let mut current = column![].spacing(8);
            for (i, e) in state.entries.iter().enumerate() {
                current = current.push(large_cell(i, e, state, skin));
                if (i + 1) % 6 == 0 {
                    grid = grid.push(current);
                    current = column![].spacing(8);
                }
            }
            grid = grid.push(current);
            scrollable(grid).height(Length::Fill).into()
        }
        }
    };

    container(column![header, body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into()
}


fn file_row<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let hovered = state.hovered == Some(i);
    let bgc = if selected {
        skin.sel
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };

    let name_cell: Element<Message> = if state.renaming.as_ref().map(|(ri, _)| *ri) == Some(i) {
        let buf = state.renaming.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
        text_input("Name", &buf)
            .on_input(Message::RenameInput)
            .on_submit(Message::CommitRename(buf))
            .size(12.0)
            .padding(2)
            .into()
    } else {
        text(e.name.clone()).size(12.0).color(skin.text).into()
    };

    let content = row![
        container(crate::icon::for_entry(e, 16.0, &skin))
            .width(Length::Fixed(26.0))
            .center_y(Length::Fixed(16.0)),
        container(name_cell).width(Length::Fill).padding([2.0, 4.0]),
        container(text(e.modified.map(model::format_date).unwrap_or_default()).size(12.0).color(skin.text))
            .width(Length::Fixed(170.0))
            .padding([2.0, 4.0]),
        container(text(model::type_label(e)).size(12.0).color(skin.text))
            .width(Length::Fixed(140.0))
            .padding([2.0, 4.0]),
        container(
            text(if e.kind == fs::EntryKind::Dir { String::new() } else { model::format_size(e.size) })
                .size(12.0)
                .color(skin.text),
        )
        .width(Length::Fixed(90.0))
        .padding([2.0, 4.0]),
    ]
    .height(Length::Fixed(24.0))
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

    let ma = mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::Hover(None));
    dbg("file_row", ma).into()
}

fn large_cell<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let content = column![
        crate::icon::for_entry(e, 64.0, &skin),
        text(e.name.clone()).size(11.0).color(skin.text).width(Length::Fixed(96.0)),
    ]
    .spacing(6)
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(104.0))
    .height(Length::Fixed(110.0));
    let styled = container(content)
        .padding(4)
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
        .into()
}

// ---------------- status bar ----------------

fn statusbar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let mut left = format!("{} items", state.entries.len());
    if !state.selected.is_empty() {
        let bytes: u64 = state.selected.iter().filter_map(|&i| state.entries.get(i)).map(|e| e.size).sum();
        left += &format!(
            "    {} selected    {}",
            state.selected.len(),
            model::format_size(bytes)
        );
    }
    if !state.status.is_empty() {
        left += &format!("    {}", state.status);
    }

    let view_btn = |icon_name: &'static str, active: bool, msg: Message| {
        button(crate::icon::view(icon_name, 18.0, &skin))
            .width(Length::Fixed(26.0))
            .height(Length::Fixed(26.0))
            .style(move |_t, status| button::Style {
                background: if active {
                    Some(skin.pressed.into())
                } else {
                    match status {
                        button::Status::Hovered => Some(skin.hover.into()),
                        _ => None,
                    }
                },
                text_color: skin.text,
                border: iced::Border {
                    color: if active { skin.accent } else { Color::TRANSPARENT },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Default::default(),
                snap: true,
            })
            .on_press(msg)
    };

    container(
        row![
            text(left).size(11.0).color(skin.subtle),
            Space::new().width(Length::Fill),
            view_btn("view_details", state.view == ViewMode::Details, Message::SetView(ViewMode::Details)),
            view_btn("view_large", state.view == ViewMode::Large, Message::SetView(ViewMode::Large)),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 15.0 }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(24.0))
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

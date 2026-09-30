//! Shared UI plumbing: the skin (light/dark palettes — dark mode lives here),
//! `--ui-debug` element outlines, and small element helpers used everywhere.

use iced::widget::container;
use iced::{Color, Element, Length};

use crate::model::Message;

/// Square chrome tool button size (ribbon minimize/help, nav refresh,
/// status-bar view switcher) — one size everywhere.
pub(crate) const CHROME_BTN: f32 = 26.0;

/// MS Paint `TPopupMenu.setWidth(140)` — the menu width floor.
pub(crate) const MENU_W: f32 = 140.0;
/// `TMenuItem`: 16 icon + iconTextGap 16 + `Insets(3,8,3,5)` → 22px rows.
pub(crate) const MENU_ITEM_H: f32 = 22.0;
/// `TMenuSection`: font 12 + 16 = 28 (two 1px lines + 26px content).
pub(crate) const MENU_SECTION_H: f32 = 28.0;

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
        /// selected/pressed ribbon button (MS Paint: C9E0F7/62A2E4)
        pub sel_btn: iced::Color,
        pub sel_btn_border: iced::Color,
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
        /// close button at rest (Win8: solid red, deeper than the hover)
        pub close_idle: iced::Color,
        /// close button pressed (Win8: dark red)
        pub close_pressed: iced::Color,
        /// main-tab hover outline (clone: EBECED; dark: subtle gray)
        pub tab_hover_outline: iced::Color,
        /// dropdown menu (MS Paint: FBFCFD fill / DCDDDE border)
        pub menu_bg: iced::Color,
        pub menu_border: iced::Color,
        /// menu section header (MS Paint TMenuSection: F5F6F7 fill / D9DADB lines)
        pub menu_section_border: iced::Color,
        /// color-plate swatch hover (MS Paint ColorPlateButton: CBECFD / 64A5E7)
        pub plate_hover: iced::Color,
        pub plate_hover_border: iced::Color,
        /// nav crumb bar / search box border (clone: D9D9D9)
        pub nav_border: iced::Color,
        /// search box recessed button (clone: E8F1FB fill / A9C4E8 divider)
        pub search_btn_bg: iced::Color,
        pub search_btn_border: iced::Color,
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
        sel_btn: color!(0xC9E0F7),
        sel_btn_border: color!(0x62A2E4),
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
        close_idle: color!(0xC20E1C),
        close_pressed: color!(0x8F0A15),
        tab_hover_outline: color!(0xEBECED),
        nav_border: color!(0xD9D9D9),
        search_btn_bg: color!(0xE8F1FB),
        search_btn_border: color!(0xA9C4E8),
        menu_bg: color!(0xFBFCFD),
        menu_border: color!(0xDCDDDE),
        menu_section_border: color!(0xD9DADB),
        plate_hover: color!(0xCBECFD),
        plate_hover_border: color!(0x64A5E7),
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
        sel_btn: color!(0x3D5A78),
        sel_btn_border: color!(0x5A89C0),
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
        close_idle: color!(0xA50D19),
        close_pressed: color!(0x6E080F),
        tab_hover_outline: color!(0x4A4A4A),
        nav_border: color!(0x555555),
        search_btn_bg: color!(0x2E3F55),
        search_btn_border: color!(0x46647F),
        menu_bg: color!(0x383838),
        menu_border: color!(0x505050),
        menu_section_border: color!(0x505050),
        plate_hover: color!(0x3A506E),
        plate_hover_border: color!(0x4A7AB5),
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


pub(crate) static UI_DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn ui_debug() -> bool {
    UI_DEBUG.load(std::sync::atomic::Ordering::Relaxed)
}

/// Deterministic distinct color per element name (FNV-1a hash -> palette).
pub(crate) fn debug_color(name: &str) -> Color {
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
pub(crate) fn dbg<'a>(name: &'static str, e: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
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


pub(crate) fn bg(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: Some(color.into()),
        ..Default::default()
    }
}

pub(crate) fn icon(name: &str, size: f32, skin: skin::Skin) -> Element<'static, Message> {
    crate::icon::view(name, size, &skin)
}

// ---------------- title bar ----------------

/// Middle-ellipsis text clamp to a pixel width.
pub(crate) fn elide(s: &str, max_w: f32) -> String {
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


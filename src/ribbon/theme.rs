//! Ribbon visual themes + color helpers.
//!
//! Two themes are provided, matching the two `UIRibbon.dll` eras:
//!
//! * [`Theme::windows10`] (default) — the flat, accent-colored rendering
//!   the framework gets from the Windows 10 uxtheme classes
//!   (`ExplorerFrame`/`Ribbon`). Solid fills, no gradients, no hover
//!   borders; the active tab is marked by an accent underline, checked
//!   buttons by an accent-tinted fill, keytips by dark badges.
//! * [`Theme::scenic`] — the Windows 7 "Scenic" glass look: gradients,
//!   glossy orb, light-blue hover borders, yellow checked state.
//!
//! `UI_PKEY_GlobalBackgroundColor` / `GlobalHighlightColor` /
//! `GlobalTextColor` (HSB, see [`crate::ribbon::ui_hsb`]) override either theme at
//! runtime, mirroring `IUIFramework::SetUICommandProperty` on command 0.

use iced::Color;

use crate::ribbon::enums::ui_hsb;

/// Convert a packed `UI_HSBCOLOR` (hue|sat<<8|bri<<16) to RGB [`Color`].
pub fn hsb_to_rgb(hsb: u32) -> Color {
    let h = (hsb & 0xff) as f32 / 255.0 * 360.0;
    let s = ((hsb >> 8) & 0xff) as f32 / 255.0;
    let v = ((hsb >> 16) & 0xff) as f32 / 255.0;
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::from_rgb(r + m, g + m, b + m)
}

/// Packed `0x00RRGGBB` -> [`Color`].
pub fn rgb(packed: u32) -> Color {
    Color::from_rgb8(
        ((packed >> 16) & 0xff) as u8,
        ((packed >> 8) & 0xff) as u8,
        (packed & 0xff) as u8,
    )
}

/// Mix a color towards white (`amount` = white fraction).
pub fn tint(color: Color, amount: f32) -> Color {
    Color {
        r: color.r + (1.0 - color.r) * amount,
        g: color.g + (1.0 - color.g) * amount,
        b: color.b + (1.0 - color.b) * amount,
        a: color.a,
    }
}

/// Multiply a color's channels (`f < 1.0` darkens).
pub fn shade(color: Color, f: f32) -> Color {
    Color {
        r: (color.r * f).min(1.0),
        g: (color.g * f).min(1.0),
        b: (color.b * f).min(1.0),
        a: color.a,
    }
}

/// Contextual tab group colors (`Table Tools` orange, `Picture Tools`
/// purple, ... like the Windows ribbon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextualColor {
    Orange,
    Green,
    Blue,
    Purple,
    Teal,
    Red,
    Gold,
}

impl ContextualColor {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "orange" => Self::Orange,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "purple" => Self::Purple,
            "teal" => Self::Teal,
            "red" => Self::Red,
            "gold" => Self::Gold,
            _ => return None,
        })
    }
    pub fn accent(self) -> Color {
        match self {
            Self::Orange => rgb(0xE97132),
            Self::Green => rgb(0x9BBB59),
            Self::Blue => rgb(0x4BACC6),
            Self::Purple => rgb(0xB07CC6),
            Self::Teal => rgb(0x31859C),
            Self::Red => rgb(0xC0504D),
            Self::Gold => rgb(0xD8A53A),
        }
    }
}

/// How the active tab is visually marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabStyle {
    /// Windows 7 / 8: the active tab is a filled box connected to the
    /// ribbon body.
    #[default]
    Filled,
    /// Windows 10: the active tab keeps the strip background and is
    /// marked by an accent underline.
    Underline,
}

/// A ribbon theme.
#[derive(Debug, Clone)]
pub struct Theme {
    /// System accent color (Windows 10 default blue): active tab
    /// underline, orb, checked state, list selection.
    pub accent: Color,
    /// Tab strip background (top, bottom).
    pub tab_strip: (Color, Color),
    /// Ribbon body background (top, bottom).
    pub ribbon: (Color, Color),
    /// Active tab background fill.
    pub tab_active: Color,
    pub tab_hover: Color,
    pub tab_text: Color,
    pub text: Color,
    pub text_disabled: Color,
    pub group_label: Color,
    pub border: Color,
    pub group_separator: Color,
    /// Application menu orb gradient (top, bottom).
    pub orb: (Color, Color),
    pub hover_bg: Color,
    /// `Some(color)` draws a border around hovered buttons (Win7);
    /// Windows 10 uses `None`.
    pub hover_border: Option<Color>,
    pub pressed_bg: Color,
    pub checked_bg: Color,
    pub checked_border: Option<Color>,
    pub popup_bg: Color,
    pub popup_border: Color,
    pub tooltip_bg: Color,
    pub tooltip_border: Color,
    /// Gallery item / list selection.
    pub selection: Color,
    pub qat_bg: (Color, Color),
    pub keytip_bg: Color,
    pub keytip_text: Color,
    /// Corner radius of buttons and button-like chrome.
    pub button_radius: f32,
    /// Corner radius of input fields (combo/spinner), frames and swatches.
    pub field_radius: f32,
    /// Radius of the fully round chrome (help / minimize buttons).
    pub round_radius: f32,
    /// Active tab rendering.
    pub tab_style: TabStyle,
    /// Color of the line connecting the tab bar with the command pane:
    /// a 1px line under the tab bar that wraps around the active tab
    /// (left, top, right — open at the pane). Also used for the pane's
    /// bottom hairline. When a contextual tab is active the widget draws
    /// this line in the contextual accent instead.
    pub tab_line: Color,
    /// Radius of the application-menu orb.
    pub orb_radius: f32,
}

/// Windows 10 ribbon look: flat chrome driven by the accent color, as
/// rendered by the Windows 10 `UIRibbon.dll` through uxtheme.
impl Theme {
    pub fn windows10() -> Self {
        let accent = rgb(0x0078D7);
        Self {
            accent,
            tab_strip: (Color::WHITE, Color::WHITE),
            ribbon: (Color::WHITE, Color::WHITE),
            tab_active: Color::WHITE, // activity is shown by the underline
            tab_hover: rgb(0xE6E6E6),
            tab_text: rgb(0x444444),
            text: rgb(0x333333),
            text_disabled: rgb(0xA6A6A6),
            group_label: rgb(0x666666),
            border: rgb(0xCCCCCC),
            group_separator: rgb(0xE1E1E1),
            orb: (accent, accent),
            hover_bg: rgb(0xDADADA),
            hover_border: None,
            pressed_bg: rgb(0xC8C8C8),
            checked_bg: tint(accent, 0.80),
            checked_border: None,
            popup_bg: Color::WHITE,
            popup_border: rgb(0xCCCCCC),
            tooltip_bg: Color::WHITE,
            tooltip_border: rgb(0x999999),
            selection: accent,
            qat_bg: (Color::WHITE, Color::WHITE),
            keytip_bg: rgb(0x3B3B3B),
            keytip_text: Color::WHITE,
            button_radius: 2.0,
            field_radius: 0.0,
            round_radius: 9.0,
            tab_style: TabStyle::Underline,
            tab_line: rgb(0xE1E1E1),
            orb_radius: 11.0,
        }
    }

    /// Windows 8 ribbon look: flat like Windows 10, but the active tab
    /// is a filled tab connected to the body (no accent underline), and
    /// every corner is square.
    pub fn windows8() -> Self {
        let accent = rgb(0x2673EC);
        Self {
            accent,
            // Windows 8: the tab bar is white, the command pane is light
            // gray, and the active tab carries the pane color so it
            // connects to it.
            tab_strip: (Color::WHITE, Color::WHITE),
            ribbon: (rgb(0xF5F6F7), rgb(0xF5F6F7)),
            tab_active: rgb(0xF5F6F7),
            tab_hover: rgb(0xDADADA),
            tab_text: rgb(0x444444),
            text: rgb(0x333333),
            text_disabled: rgb(0xA6A6A6),
            group_label: rgb(0x666666),
            border: rgb(0xCCCCCC),
            group_separator: rgb(0xE1E1E1),
            orb: (accent, accent),
            hover_bg: rgb(0xDADADA),
            hover_border: None,
            pressed_bg: rgb(0xC8C8C8),
            checked_bg: tint(accent, 0.80),
            checked_border: None,
            popup_bg: Color::WHITE,
            popup_border: rgb(0xCCCCCC),
            tooltip_bg: Color::WHITE,
            tooltip_border: rgb(0x999999),
            selection: accent,
            qat_bg: (Color::WHITE, Color::WHITE),
            keytip_bg: rgb(0x3B3B3B),
            keytip_text: Color::WHITE,
            button_radius: 0.0,
            field_radius: 0.0,
            round_radius: 0.0,
            tab_style: TabStyle::Filled,
            tab_line: rgb(0xD0D0D0),
            orb_radius: 11.0,
        }
    }

    /// Windows 7 "Scenic" glass ribbon look.
    pub fn scenic() -> Self {
        Self {
            accent: rgb(0x2A7BD6),
            tab_strip: (rgb(0xF1F3F6), rgb(0xD6DCE4)),
            ribbon: (rgb(0xFBFBFB), rgb(0xEAEAEA)),
            tab_active: rgb(0xFCFCFC),
            tab_hover: rgb(0xEAF4FB),
            tab_text: rgb(0x3A3A3A),
            text: rgb(0x262626),
            text_disabled: rgb(0xA8A8A8),
            group_label: rgb(0x595959),
            border: rgb(0x9B9B9B),
            group_separator: rgb(0xC4C4C4),
            orb: (rgb(0x3B7BD4), rgb(0x1A4FA0)),
            hover_bg: rgb(0xE3F0FA),
            hover_border: Some(rgb(0x6BAAD3)),
            pressed_bg: rgb(0xC4E0F3),
            checked_bg: rgb(0xFCE096),
            checked_border: Some(rgb(0xE5C365)),
            popup_bg: rgb(0xFCFCFC),
            popup_border: rgb(0x8A8A8A),
            tooltip_bg: rgb(0xFFFFE1),
            tooltip_border: rgb(0x7F7F7F),
            selection: rgb(0x2A7BD6),
            qat_bg: (rgb(0xE8EDF4), rgb(0xD9E0EA)),
            keytip_bg: rgb(0xF8F8F8),
            keytip_text: rgb(0x262626),
            button_radius: 3.0,
            field_radius: 1.0,
            round_radius: 9.0,
            tab_style: TabStyle::Filled,
            tab_line: rgb(0xC4C4C4),
            orb_radius: 11.0,
        }
    }

    /// Apply `UI_PKEY_GlobalBackgroundColor` / `GlobalHighlightColor` /
    /// `GlobalTextColor` (packed HSB, `ui_hsb`).
    pub fn apply_global_colors(
        &mut self,
        background: Option<u32>,
        highlight: Option<u32>,
        text: Option<u32>,
    ) {
        if let Some(bg) = background {
            let c = hsb_to_rgb(bg);
            self.ribbon = (c, shade(c, 0.96));
            self.tab_strip = (shade(c, 1.03), shade(c, 0.94));
        }
        if let Some(hl) = highlight {
            let c = hsb_to_rgb(hl);
            self.accent = c;
            self.selection = c;
            self.orb = (c, shade(c, 0.85));
            self.checked_bg = tint(c, 0.80);
            self.hover_bg = tint(c, 0.78);
        }
        if let Some(t) = text {
            let c = hsb_to_rgb(t);
            self.text = c;
            self.tab_text = c;
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::windows10()
    }
}

/// Default Office theme colors (`UI_PKEY_ThemeColors`).
pub const THEME_COLORS: [u32; 10] = [
    0x000000, 0x808080, 0xEEECE1, 0x4F81BD, 0xC0504D, 0x9BBB59, 0x8064A2,
    0x4BACC6, 0xF79646, 0xFFFFFF,
];

/// Default standard colors (`UI_PKEY_StandardColors`).
pub const STANDARD_COLORS: [u32; 10] = [
    0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050, 0x00B0F0,
    0x0070C0, 0x002060, 0x7030A0,
];

/// `UI_HSB` sample: a highlight blue, for demos of the global color keys.
pub const SAMPLE_HSB_HIGHLIGHT: u32 = ui_hsb(213, 190, 200);

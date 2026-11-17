//! Theme tokens: the single source of truth for every color in the app.
//!
//! The palettes are data-driven: `assets/theme.yaml` holds the light and
//! dark token sets; a complete copy at `~/.config/explorer/theme.yaml`
//! overrides the embedded one (same pattern as `ribbon.yaml`). Ribbon
//! buttons, folder view, nav pane, menus, the title bar and the future
//! file-operation dialogs (token
//! groups `dialog` and `state`) all draw from [`light`]/[`dark`].
//!
//! Token names mirror the spec where a mapping exists (`text` =
//! text.primary, `window_bg` = surface.content, ...); app-specific
//! chrome keeps its own names.

use std::sync::OnceLock;

use iced::Color;
use serde::Deserialize;

const EMBEDDED: &str = include_str!("../assets/theme.yaml");

/// All theme tokens for one mode (light or dark).
#[allow(dead_code)] // dialog/state tokens are for the future D1-D6 dialogs
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub accent: Color,
    pub accent_dark: Color,
    pub sel: Color,
    pub sel_border: Color,
    /// Selection fill/border when the window is unfocused
    pub sel_inactive: Color,
    pub sel_inactive_border: Color,
    pub window_bg: Color,
    pub ribbon_bg: Color,
    pub nav_bg: Color,
    pub toolbar_bg: Color,
    pub status_bg: Color,
    pub col_header_bg: Color,
    pub menu_bg: Color,
    pub menu_border: Color,
    pub menu_section_border: Color,
    pub text: Color,
    pub subtle: Color,
    pub disabled: Color,
    pub title_text: Color,
    pub titlebar_inactive: Color,
    pub hover: Color,
    pub pressed: Color,
    pub hover_btn: Color,
    pub hover_btn_border: Color,
    pub sel_btn: Color,
    pub sel_btn_border: Color,
    pub nav_hover: Color,
    pub nav_hover_border: Color,
    /// Breadcrumb text fill while its dropdown arrow is hovered;
    /// the pair shows grey text + blue arrow.
    pub nav_grey: Color,
    pub plate_hover: Color,
    pub plate_hover_border: Color,
    pub search_btn_bg: Color,
    pub search_btn_border: Color,
    pub file_tab: Color,
    pub file_tab_hover: Color,
    pub tab_hover_outline: Color,
    pub sep: Color,
    pub group_sep: Color,
    pub nav_border: Color,
    pub address_border: Color,
    pub close_idle: Color,
    pub close_hover: Color,
    pub close_pressed: Color,
    pub glass_ghost: Color,
    pub glass_sheen: Color,
    pub folder: Color,
    pub folder_edge: Color,
    /// ribbon-widget-specific chrome; `ribbon/mod.rs::theme` derives the
    /// rest of the widget theme from the shared tokens above
    pub ribbon: Ribbon,
    /// SPEC §2.5 dialog tokens (future D1-D6 dialogs)
    pub dialog: Dialog,
    /// SPEC §2.7 semantic state colors (progress/paused/error/warning)
    pub state: State,
}

/// Ribbon chrome tokens whose values do not match a shared token.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Ribbon {
    pub accent: Color,
    pub accent_dark: Color,
    pub text: Color,
    pub tab_text: Color,
    pub border: Color,
    pub tab_line: Color,
    pub tooltip_border: Color,
    pub group_label: Color,
    pub text_disabled: Color,
    pub keytip_bg: Color,
    pub keytip_text: Color,
}

/// SPEC §2.5 neutral/surface tokens for the file-operation dialogs.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Dialog {
    pub surface_dialog: Color,
    pub divider: Color,
    pub button_fill: Color,
    pub button_border: Color,
    pub button_border_bottom: Color,
    pub textbox_fill: Color,
    pub textbox_border: Color,
    pub tooltip_fill: Color,
    pub tooltip_border: Color,
    pub focus_stroke: Color,
    pub checkbox_border: Color,
    pub checkbox_fill: Color,
}

/// SPEC §2.7 semantic state colors — deliberately not accent-derived.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub progress_fill: Color,
    pub paused_fill: Color,
    pub error_fill: Color,
    pub warning_icon: Color,
}

/// The full theme: one [`Palette`] per mode.
#[derive(Deserialize)]
pub struct Theme {
    /// When false (default) border widths ignore the UI scale: a "1px"
    /// border is always 1 physical pixel.
    /// Accepts `true`/`false` or `1`/`0`.
    #[serde(default, deserialize_with = "de_bool01")]
    pub border_follow_dpi: bool,
    /// Fade-out duration for overlays (ms): ribbon popups fade out on
    /// focus loss. 0 disables the
    /// animation. General animation token; future fades reuse it.
    #[serde(default = "default_fade_ms")]
    pub fade_ms: u32,
    /// Double-click window (ms): two clicks within this count as one
    /// double-click (title-bar maximize, ribbon tab minimize toggle,
    /// file open). The OS double-click time is not exposed
    /// cross-platform (iced hardcodes 300ms in mouse::Click), so this
    /// is the single app-wide setting for it.
    #[serde(default = "default_double_click_ms")]
    pub double_click_ms: u32,
    pub light: Palette,
    pub dark: Palette,
}

fn default_fade_ms() -> u32 {
    300
}

fn default_double_click_ms() -> u32 {
    400
}

/// `border_follow_dpi: true/false` or `1/0`.
fn de_bool01<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    use serde::de::Error as _;
    match serde_yaml::Value::deserialize(d)? {
        serde_yaml::Value::Bool(b) => Ok(b),
        serde_yaml::Value::Number(n) => match n.as_u64() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err(D::Error::custom(
                "border_follow_dpi: expected 0/1 or false/true",
            )),
        },
        _ => Err(D::Error::custom(
            "border_follow_dpi: expected 0/1 or false/true",
        )),
    }
}

// ---------------------------------------------------------------------------
// border width factor
//
// One app unit occupies `scale_dpi` device pixels (`Explorer::scale_dpi()`:
// the absolute --dpi factor, or the system scale when --dpi is not given).
// When the theme says borders must not follow the DPI, widths are
// multiplied by `not_follow_dpi = 1 / scale_dpi` so they keep a
// constant physical size.

static BORDER_SCALE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(1.0f32.to_bits());

/// Publish the effective factor: `1.0` when borders follow the DPI (or
/// no scale is known), else `1 / scale_dpi`.
pub(crate) fn set_border_scale(follow_dpi: bool, scale_dpi: f32) {
    let f = if follow_dpi || scale_dpi <= 0.0 {
        1.0
    } else {
        1.0 / scale_dpi
    };
    BORDER_SCALE.store(f.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Border/line width in logical pixels, honoring `border_follow_dpi`:
/// `border_w(1.0)` is always 1 physical pixel when borders don't follow
/// the DPI. All chrome borders go through this.
pub(crate) fn border_w(px: f32) -> f32 {
    f32::from_bits(BORDER_SCALE.load(std::sync::atomic::Ordering::Relaxed)) * px
}

impl Palette {
    /// Theme seed for iced 0.15's generated palettes.
    pub fn palette(self) -> iced::theme::palette::Seed {
        iced::theme::palette::Seed {
            background: self.window_bg,
            text: self.text,
            primary: self.accent,
            success: iced::color!(0x1F9E3E),
            warning: iced::color!(0xFFB900),
            danger: self.close_hover,
        }
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

pub fn get() -> &'static Theme {
    THEME.get_or_init(load)
}

/// Fade-out duration for overlays (ms), theme.yaml `fade_ms`.
pub fn fade_ms() -> u32 {
    get().fade_ms
}

/// Double-click window (ms), theme.yaml `double_click_ms`.
pub fn double_click_ms() -> u64 {
    u64::from(get().double_click_ms)
}

pub fn light() -> Palette {
    get().light
}

pub fn dark() -> Palette {
    get().dark
}

/// User override first, embedded copy as the fallback. The embedded
/// YAML ships with the binary, so failing to parse it is a build bug.
fn load() -> Theme {
    let user = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/theme.yaml"));
    if let Some(p) = user
        && let Ok(text) = std::fs::read_to_string(&p)
    {
        match serde_yaml::from_str::<Theme>(&text) {
            Ok(t) => return t,
            Err(e) => eprintln!("theme: ignoring {}: {e}", p.display()),
        }
    }
    serde_yaml::from_str(EMBEDDED).expect("embedded assets/theme.yaml must parse")
}

/// Packed sRGB hex color: `#RRGGBB` or `#RRGGBBAA` (leading `#` optional).
pub(crate) struct Hex(pub(crate) Color);

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let h = s.strip_prefix('#').unwrap_or(&s);
        let invalid = || serde::de::Error::custom(format!("invalid hex color {s:?}"));
        let v = u32::from_str_radix(h, 16).map_err(|_| invalid())?;
        let (r, g, b, a) = match h.len() {
            6 => ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff, 255),
            8 => ((v >> 24) & 0xff, (v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff),
            _ => return Err(invalid()),
        };
        Ok(Hex(Color::from_rgba8(
            r as u8,
            g as u8,
            b as u8,
            a as f32 / 255.0,
        )))
    }
}

// serde front-ends: same shape as the public structs, colors as hex
// strings, converted in the `From` impls below.

#[derive(Deserialize)]
struct PaletteDto {
    accent: Hex,
    accent_dark: Hex,
    sel: Hex,
    sel_border: Hex,
    sel_inactive: Hex,
    sel_inactive_border: Hex,
    window_bg: Hex,
    ribbon_bg: Hex,
    nav_bg: Hex,
    toolbar_bg: Hex,
    status_bg: Hex,
    col_header_bg: Hex,
    menu_bg: Hex,
    menu_border: Hex,
    menu_section_border: Hex,
    text: Hex,
    subtle: Hex,
    disabled: Hex,
    title_text: Hex,
    titlebar_inactive: Hex,
    hover: Hex,
    pressed: Hex,
    hover_btn: Hex,
    hover_btn_border: Hex,
    sel_btn: Hex,
    sel_btn_border: Hex,
    nav_hover: Hex,
    nav_hover_border: Hex,
    nav_grey: Hex,
    plate_hover: Hex,
    plate_hover_border: Hex,
    search_btn_bg: Hex,
    search_btn_border: Hex,
    file_tab: Hex,
    file_tab_hover: Hex,
    tab_hover_outline: Hex,
    sep: Hex,
    group_sep: Hex,
    nav_border: Hex,
    address_border: Hex,
    close_idle: Hex,
    close_hover: Hex,
    close_pressed: Hex,
    glass_ghost: Hex,
    glass_sheen: Hex,
    folder: Hex,
    folder_edge: Hex,
    ribbon: RibbonDto,
    dialog: DialogDto,
    state: StateDto,
}

#[derive(Deserialize)]
struct RibbonDto {
    accent: Hex,
    accent_dark: Hex,
    text: Hex,
    tab_text: Hex,
    border: Hex,
    tab_line: Hex,
    tooltip_border: Hex,
    group_label: Hex,
    text_disabled: Hex,
    keytip_bg: Hex,
    keytip_text: Hex,
}

#[derive(Deserialize)]
struct DialogDto {
    surface_dialog: Hex,
    divider: Hex,
    button_fill: Hex,
    button_border: Hex,
    button_border_bottom: Hex,
    textbox_fill: Hex,
    textbox_border: Hex,
    tooltip_fill: Hex,
    tooltip_border: Hex,
    focus_stroke: Hex,
    checkbox_border: Hex,
    checkbox_fill: Hex,
}

#[derive(Deserialize)]
struct StateDto {
    progress_fill: Hex,
    paused_fill: Hex,
    error_fill: Hex,
    warning_icon: Hex,
}

impl<'de> Deserialize<'de> for Palette {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from(PaletteDto::deserialize(d)?))
    }
}

impl From<PaletteDto> for Palette {
    fn from(v: PaletteDto) -> Self {
        // `Hex` is an internal wrapper; each line unwraps one field.
        Self {
            accent: v.accent.0,
            accent_dark: v.accent_dark.0,
            sel: v.sel.0,
            sel_border: v.sel_border.0,
            sel_inactive: v.sel_inactive.0,
            sel_inactive_border: v.sel_inactive_border.0,
            window_bg: v.window_bg.0,
            ribbon_bg: v.ribbon_bg.0,
            nav_bg: v.nav_bg.0,
            toolbar_bg: v.toolbar_bg.0,
            status_bg: v.status_bg.0,
            col_header_bg: v.col_header_bg.0,
            menu_bg: v.menu_bg.0,
            menu_border: v.menu_border.0,
            menu_section_border: v.menu_section_border.0,
            text: v.text.0,
            subtle: v.subtle.0,
            disabled: v.disabled.0,
            title_text: v.title_text.0,
            titlebar_inactive: v.titlebar_inactive.0,
            hover: v.hover.0,
            pressed: v.pressed.0,
            hover_btn: v.hover_btn.0,
            hover_btn_border: v.hover_btn_border.0,
            sel_btn: v.sel_btn.0,
            sel_btn_border: v.sel_btn_border.0,
            nav_hover: v.nav_hover.0,
            nav_hover_border: v.nav_hover_border.0,
            nav_grey: v.nav_grey.0,
            plate_hover: v.plate_hover.0,
            plate_hover_border: v.plate_hover_border.0,
            search_btn_bg: v.search_btn_bg.0,
            search_btn_border: v.search_btn_border.0,
            file_tab: v.file_tab.0,
            file_tab_hover: v.file_tab_hover.0,
            tab_hover_outline: v.tab_hover_outline.0,
            sep: v.sep.0,
            group_sep: v.group_sep.0,
            nav_border: v.nav_border.0,
            address_border: v.address_border.0,
            close_idle: v.close_idle.0,
            close_hover: v.close_hover.0,
            close_pressed: v.close_pressed.0,
            glass_ghost: v.glass_ghost.0,
            glass_sheen: v.glass_sheen.0,
            folder: v.folder.0,
            folder_edge: v.folder_edge.0,
            ribbon: Ribbon::from(v.ribbon),
            dialog: Dialog::from(v.dialog),
            state: State::from(v.state),
        }
    }
}

impl From<RibbonDto> for Ribbon {
    fn from(v: RibbonDto) -> Self {
        Self {
            accent: v.accent.0,
            accent_dark: v.accent_dark.0,
            text: v.text.0,
            tab_text: v.tab_text.0,
            border: v.border.0,
            tab_line: v.tab_line.0,
            tooltip_border: v.tooltip_border.0,
            group_label: v.group_label.0,
            text_disabled: v.text_disabled.0,
            keytip_bg: v.keytip_bg.0,
            keytip_text: v.keytip_text.0,
        }
    }
}

impl From<DialogDto> for Dialog {
    fn from(v: DialogDto) -> Self {
        Self {
            surface_dialog: v.surface_dialog.0,
            divider: v.divider.0,
            button_fill: v.button_fill.0,
            button_border: v.button_border.0,
            button_border_bottom: v.button_border_bottom.0,
            textbox_fill: v.textbox_fill.0,
            textbox_border: v.textbox_border.0,
            tooltip_fill: v.tooltip_fill.0,
            tooltip_border: v.tooltip_border.0,
            focus_stroke: v.focus_stroke.0,
            checkbox_border: v.checkbox_border.0,
            checkbox_fill: v.checkbox_fill.0,
        }
    }
}

impl From<StateDto> for State {
    fn from(v: StateDto) -> Self {
        Self {
            progress_fill: v.progress_fill.0,
            paused_fill: v.paused_fill.0,
            error_fill: v.error_fill.0,
            warning_icon: v.warning_icon.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_theme_parses() {
        let t = get();
        // spot-check a few tokens against the reference design
        assert_eq!(t.light.ribbon_bg, Color::from_rgb8(0xF5, 0xF6, 0xF7));
        assert_eq!(t.dark.ribbon_bg, Color::from_rgb8(0x2D, 0x2D, 0x2D));
        assert!((t.light.glass_ghost.a - 0x2E as f32 / 255.0).abs() < 1e-6);
        assert_eq!(t.light.dialog.button_fill, Color::from_rgb8(0xFD, 0xFD, 0xFD));
        assert_eq!(t.dark.state.error_fill, Color::from_rgb8(0xFF, 0x99, 0xA4));
    }

    #[test]
    fn hex_accepts_hash_and_alpha() {
        #[derive(Deserialize)]
        struct T {
            c: Hex,
        }
        let t: T = serde_yaml::from_str("c: \"#010203\"").unwrap();
        assert_eq!(t.c.0, Color::from_rgba8(1, 2, 3, 1.0));
        let t: T = serde_yaml::from_str("c: 01020380").unwrap();
        assert_eq!(t.c.0, Color::from_rgba8(1, 2, 3, 0x80 as f32 / 255.0));
        assert!(serde_yaml::from_str::<T>("c: '#12345'").is_err());
    }
}

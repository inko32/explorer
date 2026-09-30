//! MS Paint-style ribbon backend.
//!
//! A reimplementation of the ribbon from the Java MS Paint clone
//! (/tmp/zramfs_inkodaily/MSPaint-master). Noun mapping and port notes:
//! docs/RIBBON-MSPAINT.md. The Win8-clone backend is preserved in
//! ribbon_classic.rs; switch in main.rs (`#[path]` attribute).
//!
//! MS Paint facts this backend follows (all sizes logical px, 96 dpi):
//! - toolbar bg F5F6F7 (= skin.ribbon_bg), border lines DADBDC (= skin.sep)
//! - button hover E8EFF7/A4CEF9 (= skin.hover_btn/hover_btn_border),
//!   selected C9E0F7/62A2E4 (= skin.sel_btn/sel_btn_border)
//! - group separators: a 9px zone with a FULL-HEIGHT 1px line (gap 0)
//! - toolbar padding 5 top / 5 sides / 3 bottom; caption row under content
//! - big split button (Paste): big icon on top, "label ▾" strip below, one
//!   shared selected/hover border around both halves + a divider line
//! - galleries: white 1px-bordered panel with a grid of square cells
//! - color plate: color1/color2 selectors + 10x2 color grid (+ chooser)
//! Dark mode: every color goes through the skin. High dpi: 1px lines are
//! snapped to whole physical pixels via `phys_px`.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct RibbonConfig {
    pub tabs: Vec<TabDef>,
    #[serde(default)]
    pub contextual_tabs: Vec<TabDef>,
    #[serde(default)]
    pub commands: HashMap<String, CommandDef>,
}

#[derive(Debug, Deserialize)]
pub struct TabDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub kind: Option<String>,
    /// Contextual-tab accent color (name or #RRGGBB).
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
    /// When to AUTO-activate the tab (defaults to `when).
    #[serde(default)]
    pub activate_when: Option<String>,
    #[serde(default)]
    pub groups: Vec<GroupDef>,
}

#[derive(Debug, Deserialize)]
pub struct GroupDef {
    pub label: String,
    /// Manual width override in px — normally omitted: computed from content.
    #[serde(default)]
    pub width: Option<f32>,
    /// Vertical slots per small-command column (default 3).
    #[serde(default)]
    pub rows: Option<usize>,
    /// "gallery": white grid panel + expand dropdown.
    /// "color_plate": MS Paint Colors group (kept for later use).
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub commands: Vec<CmdRef>,
}

#[derive(Debug, Deserialize)]
pub struct CmdRef {
    /// Empty + `separator: true` renders a full-height separator INSIDE a group.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub separator: Option<bool>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Radio-group name (MS Paint `GroupTButtons`): selecting this button
    /// deselects the other group members. Selection lives in
    /// Explorer::ribbon_sel (Message::RibbonSelect).
    #[serde(default)]
    pub group: Option<String>,
    /// "checkbox": toggle button — active state = checked.
    #[serde(default)]
    pub kind: Option<String>,
    /// Dropdown menu items (command ids) — a split button when the command
    /// also has an action, a plain "label ▾" dropdown otherwise.
    #[serde(default)]
    pub dropdown: Option<Vec<String>>,
    /// Custom cursor hook (MS Paint `setViewCursor`) — parsed and kept for
    /// later; not wired to the window yet.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Manual width override in px — normally omitted.
    #[serde(default)]
    pub width: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandDef {
    /// Menu-item label when the command never appears as a ribbon button.
    #[serde(default)]
    pub label: Option<String>,
    pub internal: Option<String>,
    #[serde(default)]
    pub shell: Option<String>,
}

/// Default ribbon (embedded). A user file at ~/.config/explorer/ribbon.yaml
/// overrides it when present and valid.
pub fn load() -> RibbonConfig {
    let user_path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/ribbon.yaml"));
    if let Some(p) = user_path {
        if let Ok(text) = std::fs::read_to_string(&p) {
            match serde_yaml::from_str(&text) {
                Ok(cfg) => return cfg,
                Err(e) => eprintln!("ribbon.yaml parse error ({}): {e}", p.display()),
            }
        }
    }
    serde_yaml::from_str(include_str!("../assets/ribbon.yaml"))
        .expect("embedded ribbon.yaml is valid")
}

// ---------------- context + `when` expressions (same as classic) ----------------

#[derive(Clone, Copy, Default)]
pub struct Context {
    pub is_drive: bool,
    pub inside_archive: bool,
    pub has_archive: bool,
    pub has_image: bool,
    pub has_video: bool,
    pub has_audio: bool,
    pub has_selection: bool,
    pub is_empty: bool,
}

impl Context {
    fn get(self, name: &str) -> Option<bool> {
        Some(match name {
            "is_drive" => self.is_drive,
            "inside_archive" => self.inside_archive,
            "has_archive" => self.has_archive,
            "has_image" => self.has_image,
            "has_video" => self.has_video,
            "has_audio" => self.has_audio,
            "has_selection" => self.has_selection,
            "is_empty" => self.is_empty,
            _ => return None,
        })
    }
}

/// Evaluate a `when` expression: identifiers + and/or/not + parentheses.
pub fn eval_when(expr: &str, ctx: Context) -> bool {
    let tokens = tokenize(expr);
    let mut pos = 0;
    let v = parse_or(&tokens, &mut pos, ctx);
    v.unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            c if c.is_alphabetic() || c == '_' => {
                let mut ident = String::from(c);
                while let Some(&c2) = chars.peek() {
                    if c2.is_alphanumeric() || c2 == '_' {
                        ident.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(match ident.as_str() {
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Ident(ident),
                });
            }
            _ => {}
        }
    }
    out
}

fn parse_or(toks: &[Tok], pos: &mut usize, ctx: Context) -> Option<bool> {
    let mut v = parse_and(toks, pos, ctx)?;
    while *pos < toks.len() && toks[*pos] == Tok::Or {
        *pos += 1;
        let rhs = parse_and(toks, pos, ctx)?;
        v = v || rhs;
    }
    Some(v)
}

fn parse_and(toks: &[Tok], pos: &mut usize, ctx: Context) -> Option<bool> {
    let mut v = parse_not(toks, pos, ctx)?;
    while *pos < toks.len() && toks[*pos] == Tok::And {
        *pos += 1;
        let rhs = parse_not(toks, pos, ctx)?;
        v = v && rhs;
    }
    Some(v)
}

fn parse_not(toks: &[Tok], pos: &mut usize, ctx: Context) -> Option<bool> {
    if *pos < toks.len() && toks[*pos] == Tok::Not {
        *pos += 1;
        return Some(!parse_not(toks, pos, ctx)?);
    }
    parse_atom(toks, pos, ctx)
}

fn parse_atom(toks: &[Tok], pos: &mut usize, ctx: Context) -> Option<bool> {
    if *pos >= toks.len() {
        return None;
    }
    match &toks[*pos] {
        Tok::Ident(name) => {
            *pos += 1;
            ctx.get(name)
        }
        Tok::LParen => {
            *pos += 1;
            let v = parse_or(toks, pos, ctx)?;
            if *pos < toks.len() && toks[*pos] == Tok::RParen {
                *pos += 1;
            }
            Some(v)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            inside_archive: true,
            has_image: false,
            has_selection: true,
            ..Default::default()
        }
    }

    #[test]
    fn when_exprs() {
        let c = ctx();
        assert!(eval_when("inside_archive", c));
        assert!(!eval_when("has_image", c));
        assert!(eval_when("inside_archive or has_image", c));
        assert!(eval_when("has_selection and not has_image", c));
        assert!(eval_when("(has_image or has_selection) and inside_archive", c));
        assert!(!eval_when("unknown_pred", c));
    }

    #[test]
    fn embedded_yaml_loads() {
        let cfg: RibbonConfig = serde_yaml::from_str(include_str!("../assets/ribbon.yaml"))
            .expect("embedded ribbon.yaml is valid");
        assert!(!cfg.tabs.is_empty());
    }
}

/// Contextual tab accent colors (Win8 palette).
pub fn color_by_name(name: &str) -> Option<iced::Color> {
    Some(match name {
        "gold" => iced::color!(0xE8A33D),
        "green" => iced::color!(0x3FA046),
        "blue" => iced::color!(0x0078D7),
        "orange" => iced::color!(0xE88B00),
        "purple" => iced::color!(0x8E5BB5),
        "teal" => iced::color!(0x2E9E9E),
        "red" => iced::color!(0xD04343),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let v = u32::from_str_radix(&hex[1..], 16).ok()?;
            iced::Color::from_rgb8(((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
        }
        _ => return None,
    })
}

// ---------------- enablement (same as classic) ----------------

/// Dynamic enablement: static `enabled: false` wins; then per-command rules.
pub fn command_enabled(id: &str, has_selection: bool, has_clipboard: bool, has_items: bool) -> bool {
    match id {
        "cut" | "copy" | "delete" | "delete_permanently" | "rename" | "open_picture" => {
            has_selection
        }
        "paste" => has_clipboard,
        "select_all" | "invert_selection" => has_items,
        "select_none" => has_selection,
        _ => true,
    }
}

// ---------------- geometry (MS Paint metrics; logical px @ 96dpi) ----------------

/// Horizontal space between two CONTEXTUAL tabs (and their title-bar
/// blocks) — shared with border.rs.
pub(crate) const CTX_TAB_GAP: f32 = 3.0;

// ---- chrome geometry shared with border.rs (single source of fact) ----
/// Title bar height — border.rs draws the bar; the ribbon's dropdown
/// anchors are positioned relative to it.
pub(crate) const TITLEBAR_H: f32 = 30.0;
/// Tab row height (below the title bar, above the tab-row line).
pub(crate) const TAB_ROW_H: f32 = 24.0;
/// The 1px line between the tab row and the ribbon body (also the body's
/// bottom edge).
pub(crate) const TAB_LINE_H: f32 = 1.0;

/// Window-y of the ribbon body top: below the title bar, the tab row and
/// the tab-row line. Dropdown menus anchor under the ribbon from this.
pub(crate) fn ribbon_body_y() -> f32 {
    TITLEBAR_H + TAB_ROW_H + TAB_LINE_H
}

// ---- MS Paint toolbar metrics ----
/// Total ribbon body height (unchanged from the classic backend so the
/// rest of the layout is unaffected): 5 top pad + 77 content + 15 caption
/// + 3 bottom pad.
pub(crate) const RIBBON_H: f32 = 100.0;
/// Toolbar padding: top, sides, bottom (MS Paint EmptyBorder(5,5,3,5)).
pub(crate) const PAD_TOP: f32 = 5.0;
pub(crate) const PAD_SIDE: f32 = 5.0;
pub(crate) const PAD_BOTTOM: f32 = 3.0;
/// Group caption row height.
pub(crate) const CAPTION_H: f32 = 15.0;
/// Group command-area height.
pub(crate) const GROUP_CONTENT_H: f32 = RIBBON_H - PAD_TOP - PAD_BOTTOM - CAPTION_H; // 77
/// Group separator zone width — a FULL-HEIGHT 1px line centered in it
/// (MS Paint TSeparator(9, 92, gap 0)).
pub(crate) const SEP_W: f32 = 9.0;
/// Small labeled command row height (three rows fill the content area).
pub(crate) const SMALL_H: f32 = 25.0;
/// Vertical spacing between two small-command rows in a column.
pub(crate) const SMALL_GAP: f32 = 1.0;
/// Icon-only tool button (MS Paint "Tools" grid squares).
pub(crate) const ICON_BTN: f32 = 25.0;
/// Big split button (MS Paint Paste): icon zone + 1px divider + strip.
pub(crate) const BIG_STRIP_H: f32 = 21.0;
pub(crate) const BIG_DIVIDER_H: f32 = 1.0;
pub(crate) const BIG_ICON: f32 = 32.0;
/// Gallery: white panel grid of square cells (MS Paint ShapeTools panel).
pub(crate) const GALLERY_CELL: f32 = 22.0;
/// Color plate: swatch cell size (MS Paint uses 14px cells, 1px insets).
pub(crate) const COLOR_CELL: f32 = 14.0;

/// The MS Paint color-plate palette, row by row (20 colors; the Java
/// original reserves 30 cells with the last row empty).
pub(crate) const COLOR_PLATE: [[u32; 10]; 2] = [
    [0x000000, 0x808080, 0x880015, 0xFF0000, 0xFF7F27, 0xFFFF00, 0x00FF00, 0x00A2E8, 0x3F48CC, 0xA349A4],
    [0xFFFFFF, 0xC0C0C0, 0x00FFFF, 0xB97A57, 0xFFC90E, 0xEFE4B0, 0xFC9EA0, 0xB5E61D, 0x7092BE, 0xC8BFE6],
];

// ---------------- ribbon UI ----------------

use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::border::frame_insets;
use crate::model::{Explorer, Message};
use crate::ui_common::{bg, dbg, icon, skin, CHROME_BTN};

/// Logical size of `n` physical pixels at the current total UI scale: a
/// 1px line must cover a whole number of device pixels, otherwise it lands
/// on a fractional device width and renders as a blurry gray hairline.
pub(crate) fn phys_px(state: &Explorer, n: f32) -> f32 {
    let scale = if state.system_scale > 0.0 {
        state.system_scale * state.dpi_multiplier()
    } else {
        1.0
    };
    n / scale
}

/// Approximate advance width of a string at 11px in the system sans
/// (DejaVu Sans family) — used for button width estimation.
pub(crate) fn text11_w(s: &str) -> f32 {
    s.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' => 2.5f32,
            't' => 3.7,
            'f' => 3.4,
            'r' => 4.0,
            'm' => 9.4,
            'w' => 8.7,
            'M' => 9.3,
            'W' => 11.6,
            ' ' => 3.4,
            c if c.is_ascii_uppercase() => 7.7,
            _ => 6.3,
        })
        .sum()
}

pub(crate) fn view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // geometry + accent of the active tab (the accent also drives the line)
    let active_geo = active_tab_geometry(state);
    // MS Paint draws a 1px border line top+bottom over the toolbar. With an
    // active CONTEXTUAL tab the line takes that tab's accent color — the
    // active tab covers its own segment (see the bridge below).
    let line_color = active_geo.and_then(|(_, _, a)| a).unwrap_or(skin.sep);
    let line = container(Space::new().width(Length::Fill).height(Length::Fixed(phys_px(state, TAB_LINE_H))))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(line_color.into()),
            ..Default::default()
        });
    let body = container(mspaint_body(state, skin))
        .width(Length::Fill)
        .height(Length::Fixed(RIBBON_H))
        .padding(iced::Padding {
            top: PAD_TOP,
            right: PAD_SIDE,
            bottom: PAD_BOTTOM,
            left: PAD_SIDE,
        })
        .style(move |_| container::Style {
            background: Some(skin.ribbon_bg.into()),
            ..Default::default()
        });
    let bottom_line = container(Space::new().width(Length::Fill).height(Length::Fixed(phys_px(state, TAB_LINE_H))))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.sep.into()),
            ..Default::default()
        });
    if state.ribbon_collapsed {
        return column![tab_row(state, skin), line].into();
    }
    let col = column![tab_row(state, skin), line, body, bottom_line];
    match active_geo {
        Some((x, w, accent)) => {
            // border color of the active tab: frame for main tabs, accent
            // for contextual ones
            let border_c = accent.unwrap_or(skin.sep);
            let leg = |c: Color| {
                container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(1.0)))
                    .style(move |_| container::Style {
                        background: Some(c.into()),
                        ..Default::default()
                    })
            };
            let strip = container(
                row![
                    Space::new().width(Length::Fixed(x)),
                    // the tab's +height interferes with the tab-row line:
                    // the side LINES continue down through the zone (no gap
                    // below the tab), the middle swallows the line
                    leg(border_c),
                    container(
                        Space::new()
                            .width(Length::Fixed((w - 2.0).max(0.0)))
                            .height(Length::Fixed(1.0)),
                    )
                    .style(move |_| container::Style {
                        background: Some(skin.ribbon_bg.into()),
                        ..Default::default()
                    }),
                    leg(border_c),
                ]
                .width(Length::Fill),
            )
            .height(Length::Fixed(1.0));
            let bridge = container(
                column![
                    Space::new().width(Length::Fill).height(Length::Fixed(TAB_ROW_H)),
                    strip,
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

// ---------------- MS Paint widgets ----------------

/// MS Paint `TButton` chrome: hover fill+border, selected (radio/toggle/
/// pressed) fill+border, disabled plain.
fn tbtn_style(
    enabled: bool,
    selected: bool,
    skin: skin::Skin,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    move |_t, status| {
        let (bgc, border_c, tc) = if !enabled {
            (None, Color::TRANSPARENT, skin.disabled)
        } else if selected {
            (Some(skin.sel_btn), skin.sel_btn_border, skin.text)
        } else {
            match status {
                button::Status::Hovered => (Some(skin.hover_btn), skin.hover_btn_border, skin.text),
                button::Status::Pressed => (Some(skin.sel_btn), skin.sel_btn_border, skin.text),
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

/// MS Paint `TSeparator`: a `SEP_W`-wide zone with a full-height 1px line.
fn separator(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let h = phys_px(state, GROUP_CONTENT_H + CAPTION_H);
    container(
        container(Space::new().width(Length::Fixed(phys_px(state, 1.0))).height(Length::Fixed(h)))
            .style(bg(skin.sep)),
    )
    .width(Length::Fixed(SEP_W))
    .height(Length::Fill)
    .center_x(Length::Fixed(SEP_W))
    .into()
}

/// Small labeled command (MS Paint Cut/Copy rows): icon + label, 25px high.
/// `label: ""` renders an MS Paint "Tools"-style icon-only square.
fn small_cmd(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    selected: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let content: Element<Message> = if label.is_empty() {
        container(icon_el)
            .width(Length::Fixed(ICON_BTN))
            .height(Length::Fixed(ICON_BTN))
            .center_x(Length::Fixed(ICON_BTN))
            .center_y(Length::Fixed(ICON_BTN))
            .into()
    } else {
        let r = row![
            icon_el,
            text(label.to_string())
                .size(11.0)
                .color(if enabled { skin.text } else { skin.disabled }),
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center);
        container(r)
            .height(Length::Fixed(SMALL_H))
            .center_y(Length::Fixed(SMALL_H))
            .into()
    };
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(if label.is_empty() { ICON_BTN } else { SMALL_H }))
        .padding([0.0, 4.0])
        .style(tbtn_style(enabled, selected, skin));
    let el: Element<Message> = match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    };
    dbg(
        Box::leak(format!("cmd:{label}").into_boxed_str()),
        el,
    )
}

/// Checkbox command: a 13px box + label toggle (checked = selected).
fn checkbox_cmd(
    label: &str,
    width: f32,
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
        background: Some((if checked { skin.accent } else { skin.window_bg }).into()),
        border: iced::Border {
            color: if checked { skin.accent } else { skin.disabled },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });
    let r = row![
        box_el,
        text(label.to_string())
            .size(11.0)
            .color(if enabled { skin.text } else { skin.disabled }),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let content = container(r)
        .height(Length::Fixed(SMALL_H))
        .center_y(Length::Fixed(SMALL_H));
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(SMALL_H))
        .padding([0.0, 4.0])
        .style(tbtn_style(enabled, false, skin));
    let el: Element<Message> = match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    };
    dbg(
        Box::leak(format!("checkbox:{label}").into_boxed_str()),
        el,
    )
}

/// MS Paint `TButtonAndDropTools`: big icon button on top, "label ▾" strip
/// below. Hover/selected paint ONE shared border around both halves plus a
/// divider line (paintComponent draws the rect + the split line).
#[allow(clippy::too_many_arguments)]
fn big_split(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    selected: bool,
    action: Option<Message>,
    menu: Option<Message>,
    state: &Explorer,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let icon_zone_h = GROUP_CONTENT_H - BIG_STRIP_H - BIG_DIVIDER_H;
    // top half: the big icon runs the action
    let icon_zone = container(icon_el)
        .width(Length::Fixed(width))
        .height(Length::Fixed(icon_zone_h))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(icon_zone_h));
    let top = button(icon_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(icon_zone_h))
        .padding(0)
        .style(tbtn_style(enabled, false, skin));
    let top: Element<Message> = match (enabled, action) {
        (true, Some(m)) => top.on_press(m).into(),
        _ => top.into(),
    };
    // divider between the halves (part of the unified chrome)
    let divider = container(
        container(Space::new().width(Length::Fixed(phys_px(state, 1.0))).height(Length::Fill))
            .style(bg(skin.sep)),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(BIG_DIVIDER_H))
    .center_x(Length::Fixed(width));
    // bottom half: "label ▾" opens the dropdown
    let strip_label = row![
        text(label.to_string())
            .size(11.0)
            .color(if enabled { skin.text } else { skin.disabled }),
        drop_arrow_el(if enabled { skin.text } else { skin.disabled }, 4.5),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Start);
    let strip_zone = container(strip_label)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_STRIP_H))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(BIG_STRIP_H));
    let bottom = button(strip_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_STRIP_H))
        .padding(0)
        .style(tbtn_style(enabled, false, skin));
    let bottom: Element<Message> = match (enabled, menu) {
        (true, Some(m)) => bottom.on_press(m).into(),
        _ => bottom.into(),
    };
    // shared hover/selected border around the whole control (drawn over the
    // two button halves + divider)
    let frame: Element<Message> = container(Space::new().width(Length::Fixed(width)).height(Length::Fixed(GROUP_CONTENT_H)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(GROUP_CONTENT_H))
        .style(move |_| container::Style {
            background: None,
            border: iced::Border {
                color: if selected {
                    skin.sel_btn_border
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into();
    let name: &'static str = Box::leak(format!("big_split:{label}").into_boxed_str());
    dbg(
        name,
        stack![column![top, divider, bottom].width(Length::Fixed(width)), frame]
            .width(Length::Fixed(width)),
    )
    .into()
}

/// 90-degree dropdown triangle, 6x3 px, top-padded so its base sits below
/// an adjacent 11px label (same glyph as the classic backend).
pub(crate) fn drop_arrow_el(c: Color, top_pad: f32) -> Element<'static, Message> {
    container(
        iced::widget::svg(crate::icons::drop_arrow(c))
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(3.0)),
    )
    .padding(iced::Padding {
        top: top_pad,
        right: 1.0,
        bottom: 0.0,
        left: 1.0,
    })
    .into()
}

/// MS Paint `DropTools` small dropdown: "label ▾" (arrow on the right);
/// the whole button opens the menu unless `action` is set (then the label
/// runs the action and only the arrow opens the menu).
fn small_dropdown(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    action: Option<Message>,
    menu: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let tc = if enabled { skin.text } else { skin.disabled };
    let label_zone = row![
        icon_el,
        text(label.to_string()).size(11.0).color(tc),
        Space::new().width(Length::Fill),
        drop_arrow_el(tc, 4.5),
    ]
    .spacing(5)
    .align_y(iced::Alignment::Center)
    .width(Length::Fixed(width - 18.0));
    let content = container(label_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(SMALL_H))
        .center_y(Length::Fixed(SMALL_H))
        .padding([0.0, 4.0]);
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(SMALL_H))
        .padding(0)
        .style(tbtn_style(enabled, false, skin));
    let el: Element<Message> = match (enabled, action.or(menu)) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    };
    dbg(
        Box::leak(format!("drop:{label}").into_boxed_str()),
        el,
    )
}

/// Width of one small command (icon + label + padding; icon-only square).
fn small_width(cmd: &CmdRef) -> f32 {
    if cmd.label.is_empty() && cmd.icon.is_some() {
        return ICON_BTN;
    }
    let mut w = 8.0; // horizontal padding
    if cmd.kind.as_deref() == Some("checkbox") {
        w += 13.0 + 6.0;
    } else if cmd.icon.is_some() {
        w += 16.0 + 5.0;
    }
    w += text11_w(&cmd.label);
    if cmd.dropdown.is_some() {
        w += 18.0;
    }
    w.ceil()
}

/// Width of the big split button from its strip label.
fn big_split_width(label: &str) -> f32 {
    (text11_w(label) + 6.0 + 8.0 + 18.0).max(44.0).ceil()
}

/// Effective command width (YAML `width` wins).
fn cmd_width(cmd: &CmdRef) -> f32 {
    cmd.width.unwrap_or_else(|| {
        if cmd.size.as_deref() == Some("big") {
            big_split_width(&cmd.label)
        } else {
            small_width(cmd)
        }
    })
}

/// Effective group width: content + the group padding MS Paint leaves
/// around each group (2px sides — visually from the grid gaps).
pub(crate) fn group_width(g: &GroupDef) -> f32 {
    if let Some(w) = g.width {
        return w;
    }
    match g.kind.as_deref() {
        Some("gallery") => {
            let rows = g.rows.unwrap_or(3).clamp(1, 3) as f32;
            let cols = (g.commands.len() as f32 / rows).ceil();
            (PAD_SIDE + cols * GALLERY_CELL + 20.0 + PAD_SIDE).ceil()
        }
        Some("color_plate") => (2.0 * (COLOR_CELL + 2.0) + 2.0 + 10.0 * (COLOR_CELL + 2.0) + 2.0 + 20.0).ceil(),
        _ => {
            let rows = g.rows.unwrap_or(3).clamp(1, 3);
            let mut items_w = 0.0f32;
            let mut n_items = 0usize;
            let mut smalls: Vec<&CmdRef> = Vec::new();
            for cmd in &g.commands {
                if cmd.separator == Some(true) {
                    items_w += SEP_W;
                    n_items += 1;
                } else if cmd.size.as_deref() == Some("big") {
                    items_w += cmd_width(cmd);
                    n_items += 1;
                } else {
                    smalls.push(cmd);
                }
            }
            let columns = smalls.len().div_ceil(rows).max(1);
            let col_w = smalls.iter().map(|c| cmd_width(c)).fold(0.0f32, f32::max);
            if !smalls.is_empty() {
                items_w += columns as f32 * col_w;
                n_items += columns;
            }
            let spacing = 2.0 * n_items.saturating_sub(1) as f32;
            (PAD_SIDE * 2.0 + items_w + spacing).ceil()
        }
    }
}

/// Clamp a dropdown menu anchor x so the menu stays inside the window.
pub(crate) fn menu_anchor(state: &Explorer, x: f32) -> f32 {
    let (_, fr, _, fl) = frame_insets(state);
    x.clamp(4.0, (state.win_w - fl - fr - 220.0).max(4.0))
}

/// Build one group's content from YAML. Small commands stack into
/// `rows`-tall columns (MS Paint clipboard/tools arrangement).
fn group_from_config(g: &GroupDef, state: &Explorer, skin: skin::Skin, x_base: f32) -> Element<'static, Message> {
    let has_sel = !state.selected.is_empty();
    let has_clip = state.clipboard.is_some();
    let has_items = !state.entries.is_empty();
    let enabled_of = |cmd: &CmdRef| {
        cmd.enabled.unwrap_or(true) && command_enabled(&cmd.id, has_sel, has_clip, has_items)
    };
    let menu_source = state.open_menu.as_ref().map(|m| m.source.as_str());
    let active_of = |id: &str| match id {
        _ if Some(id) == menu_source => true, // pressed while its menu is open
        "toggle_border" => state.self_border,
        "toggle_dark" => state.dark,
        "toggle_hidden" => state.show_hidden,
        "toggle_frame" => state.frame_style == crate::model::FrameStyle::Win8,
        "view_large" => state.view == crate::model::ViewMode::Large,
        "view_details" => state.view == crate::model::ViewMode::Details,
        _ => false,
    };
    // selected = radio-group selection (MS Paint GroupTButtons) or a
    // model-backed toggle
    let selected_of = |cmd: &CmdRef| {
        if let Some(grp) = &cmd.group {
            match state.ribbon_sel.get(grp) {
                Some(id) => *id == cmd.id,
                None => active_of(&cmd.id), // default until first click
            }
        } else {
            active_of(&cmd.id)
        }
    };
    let press_of = |cmd: &CmdRef, msg: Message| match &cmd.group {
        Some(grp) => Message::RibbonSelect {
            group: grp.clone(),
            id: cmd.id.clone(),
        },
        None => msg,
    };

    if g.kind.as_deref() == Some("color_plate") {
        return color_plate(g, state, skin);
    }
    if g.kind.as_deref() == Some("gallery") {
        return gallery(g, state, skin, x_base);
    }

    let mut content = row![].spacing(2);
    let mut bigs_w = 0.0f32;
    for cmd in &g.commands {
        if cmd.separator == Some(true) {
            content = content.push(separator(state, skin));
            bigs_w += SEP_W + 2.0;
            continue;
        }
        if cmd.size.as_deref() == Some("big") {
            let enabled = enabled_of(cmd);
            let w = cmd_width(cmd);
            let has_action = state.ribbon.commands.contains_key(&cmd.id);
            let selected = selected_of(cmd);
            let action = enabled.then(|| {
                press_of(cmd, Message::RunCommand(cmd.id.clone()))
            });
            let menu = cmd.dropdown.as_ref().and_then(|items| {
                enabled.then(|| Message::OpenMenu {
                    x: menu_anchor(state, x_base + PAD_SIDE + bigs_w),
                    y: ribbon_body_y() + PAD_TOP + GROUP_CONTENT_H,
                    source: cmd.id.clone(),
                    source_w: w,
                    items: items.clone(),
                })
            });
            let el = big_split(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), BIG_ICON, &skin),
                &cmd.label,
                w,
                enabled,
                selected,
                if has_action { action } else { None },
                menu,
                state,
                skin,
            );
            bigs_w += w + 2.0;
            content = content.push(el);
        }
    }

    let small_cmds: Vec<&CmdRef> =
        g.commands.iter().filter(|c| c.size.as_deref() != Some("big") && c.separator != Some(true)).collect();
    let rows = g.rows.unwrap_or(3).clamp(1, 3) as usize;
    let columns = small_cmds.len().div_ceil(rows).max(1);
    let col_w = small_cmds
        .iter()
        .map(|c| cmd_width(c))
        .fold(0.0f32, f32::max);
    let mut smalls: Vec<Element<Message>> = Vec::new();
    for (j, cmd) in small_cmds.iter().enumerate() {
        let enabled = enabled_of(cmd);
        let selected = selected_of(cmd);
        let w = cmd_width(cmd);
        let has_action = state.ribbon.commands.contains_key(&cmd.id);
        let msg = enabled.then(|| press_of(cmd, Message::RunCommand(cmd.id.clone())));
        let col = (j / rows) as f32;
        let row_i = (j % rows) as f32;
        let el: Element<Message> = if cmd.kind.as_deref() == Some("checkbox") {
            checkbox_cmd(&cmd.label, w, enabled, selected, msg, skin)
        } else if let Some(items) = &cmd.dropdown {
            // MS Paint has no split small: whole button opens the menu,
            // unless the command itself has an action (then it wins).
            let menu_msg = enabled.then(|| Message::OpenMenu {
                x: menu_anchor(state, x_base + PAD_SIDE + bigs_w + col * (col_w + 2.0)),
                y: ribbon_body_y() + PAD_TOP + row_i * (SMALL_H + SMALL_GAP) + SMALL_H,
                source: cmd.id.clone(),
                source_w: w,
                items: items.clone(),
            });
            small_dropdown(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 16.0, &skin),
                &cmd.label,
                w,
                enabled,
                if has_action { msg } else { None },
                menu_msg,
                skin,
            )
        } else {
            small_cmd(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 16.0, &skin),
                &cmd.label,
                w,
                enabled,
                selected,
                msg,
                skin,
            )
        };
        smalls.push(el);
    }
    let mut it = smalls.into_iter();
    for _ in 0..columns {
        let mut col = column![].spacing(SMALL_GAP);
        for _ in 0..rows {
            if let Some(s) = it.next() {
                col = col.push(s);
            }
        }
        content = content.push(col);
    }
    group_frame(&g.label.clone(), group_width(g), content.into(), skin)
}

/// MS Paint gallery (ShapeTools): a white 1px-bordered panel with a grid
/// of square cells, plus an expand chevron opening the full list.
fn gallery(g: &GroupDef, state: &Explorer, skin: skin::Skin, x_base: f32) -> Element<'static, Message> {
    let rows = g.rows.unwrap_or(3).clamp(1, 3);
    let columns = g.commands.len().div_ceil(rows).max(1);
    let cells: Vec<Element<Message>> = g
        .commands
        .iter()
        .map(|cmd| {
            let enabled = cmd.enabled.unwrap_or(true);
            let btn = button(
                container(crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), 16.0, &skin))
                    .width(Length::Fixed(GALLERY_CELL))
                    .height(Length::Fixed(GALLERY_CELL))
                    .center_x(Length::Fixed(GALLERY_CELL))
                    .center_y(Length::Fixed(GALLERY_CELL)),
            )
            .width(Length::Fixed(GALLERY_CELL))
            .height(Length::Fixed(GALLERY_CELL))
            .padding(0)
            .style(tbtn_style(enabled, false, skin));
            let el: Element<Message> = match enabled {
                true => btn.on_press(Message::RunCommand(cmd.id.clone())).into(),
                false => btn.into(),
            };
            dbg(Box::leak(format!("cell:{}", cmd.id).into_boxed_str()), el)
        })
        .collect();
    let mut it = cells.into_iter();
    let mut grid = column![].spacing(1);
    for _ in 0..rows {
        let mut line = row![].spacing(1);
        for _ in 0..columns {
            if let Some(c) = it.next() {
                line = line.push(c);
            }
        }
        grid = grid.push(line);
    }
    let panel = container(grid)
        .padding(1.0)
        .style(move |_| container::Style {
            background: Some(skin.window_bg.into()),
            border: iced::Border {
                color: skin.sep,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    let items: Vec<String> = g.commands.iter().map(|c| c.id.clone()).collect();
    let gw = group_width(g);
    let expand = button(crate::icon::view("chevron_down", 12.0, &skin))
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(GROUP_CONTENT_H))
        .style(tbtn_style(true, false, skin))
        .on_press(Message::OpenMenu {
            x: menu_anchor(state, x_base + gw - 26.0),
            y: ribbon_body_y() + PAD_TOP + GROUP_CONTENT_H,
            source: g.label.clone(),
            source_w: gw,
            items,
        });
    group_frame(
        &g.label.clone(),
        gw,
        row![panel, expand].height(Length::Fixed(GROUP_CONTENT_H)).into(),
        skin,
    )
}

/// MS Paint Colors group: color1/color2 selectors, the 10x2 palette grid
/// and a chooser button (the group's first command, if any). A swatch
/// click dispatches `RunCommand("color_plate:<RRGGBB>")` — define a
/// matching command in ribbon.yaml to wire it (kept for later use).
fn color_plate(g: &GroupDef, _state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let sel_box = |c: Color| {
        container(Space::new().width(Length::Fixed(COLOR_CELL)).height(Length::Fixed(COLOR_CELL)))
            .width(Length::Fixed(COLOR_CELL + 2.0))
            .height(Length::Fixed(COLOR_CELL + 2.0))
            .style(move |_| container::Style {
                background: Some(c.into()),
                border: iced::Border {
                    color: skin.sep,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
    };
    let mut rows = column![].spacing(2);
    for r in 0..2 {
        let mut line = row![].spacing(2);
        for c in 0..10 {
            let hex = COLOR_PLATE[r][c];
            let c = Color::from_rgb8(((hex >> 16) & 0xFF) as u8, ((hex >> 8) & 0xFF) as u8, (hex & 0xFF) as u8);
            let swatch = button(
                container(Space::new().width(Length::Fixed(COLOR_CELL)).height(Length::Fixed(COLOR_CELL)))
                    .style(bg(c)),
            )
            .width(Length::Fixed(COLOR_CELL))
            .height(Length::Fixed(COLOR_CELL))
            .padding(0)
            .style(tbtn_style(true, false, skin))
            .on_press(Message::RunCommand(format!("color_plate:{hex:06X}")));
            line = line.push(swatch);
        }
        rows = rows.push(line);
    }
    let chooser: Element<Message> = match g.commands.first() {
        Some(cmd) => button(crate::icon::view(cmd.icon.as_deref().unwrap_or("gear"), 16.0, &skin))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(tbtn_style(true, false, skin))
            .on_press(Message::RunCommand(cmd.id.clone()))
            .into(),
        None => Space::new().width(Length::Fixed(20.0)).height(Length::Fixed(20.0)).into(),
    };
    let content = row![
        column![sel_box(Color::BLACK), sel_box(Color::WHITE)].spacing(2),
        rows,
        container(chooser).height(Length::Fixed(GROUP_CONTENT_H)).align_y(iced::Alignment::End),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);
    group_frame(&g.label.clone(), group_width(g), content.into(), skin)
}

/// A group: content on top, centered caption under it (MS Paint grid:
/// glue above the caption pushes it to the bottom edge).
fn group_frame(caption: &str, width: f32, content: Element<'static, Message>, skin: skin::Skin) -> Element<'static, Message> {
    column![
        container(dbg(
            Box::leak(format!("group_body:{caption}").into_boxed_str()),
            content,
        ))
        .width(Length::Fixed(width))
        .height(Length::Fixed(GROUP_CONTENT_H)),
        container(text(caption.to_string()).size(11.0).color(skin.subtle))
            .width(Length::Fixed(width))
            .height(Length::Fixed(CAPTION_H))
            .center_x(Length::Fixed(width))
            .style(bg(skin.ribbon_bg)),
    ]
    .into()
}

fn mspaint_body(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let Some(tab) = visible_tabs(state).into_iter().find(|t| t.id == state.active_tab) else {
        return Space::new().width(Length::Fill).height(Length::Fixed(RIBBON_H)).into();
    };
    let mut body = row![];
    let mut x = 0.0f32;
    let mut first = true;
    for g in &tab.groups {
        if !first {
            body = body.push(separator(state, skin));
            x += SEP_W;
        }
        first = false;
        let name: &'static str = Box::leak(format!("group:{}", g.label).into_boxed_str());
        body = body.push(dbg(name, group_from_config(g, state, skin, x)));
        x += group_width(g);
    }
    body.into()
}

// ---------------- tab strip (explorer chrome — same as classic) ----------------

/// Active tab's merge overlay: repaints the MIDDLE (`width - 2`) of the
/// tab's bottom border in the command-pane color, leaving the bottom
/// corners untouched so the side border lines keep running (into the
/// bridge legs in `view`).
fn tab_merge_overlay(width: f32, skin: skin::Skin) -> Element<'static, Message> {
    container(
        container(
            Space::new()
                .width(Length::Fixed((width - 2.0).max(0.0)))
                .height(Length::Fixed(1.0)),
        )
        .style(move |_| container::Style {
            background: Some(skin.ribbon_bg.into()),
            ..Default::default()
        }),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(TAB_ROW_H))
    .align_x(iced::Alignment::Center)
    .align_y(iced::Alignment::End)
    .into()
}

/// Contextual tab frame (stub / tab-button backing): the caption sits
/// BETWEEN the side borders, so the text stays at the SAME position idle
/// and active. Top line: always 1px solid accent. Bottom line: 1px in
/// `bottom` (the pane color when idle — the border fades to 0% there — or
/// the accent when active, where `tab_merge_overlay` repaints its middle).
/// Sides: the distinctive `th`-px border — a linear 100%->0% top-to-bottom
/// fade when `fade` (idle, 2px), solid when active (1px, thinner than idle).
fn ctx_frame(
    width: f32,
    th: f32,
    accent: Color,
    bottom: Color,
    fade: bool,
    middle: Element<'static, Message>,
) -> Element<'static, Message> {
    let side = move || {
        container(Space::new().width(Length::Fixed(th)).height(Length::Fill))
            .width(Length::Fixed(th))
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(if fade {
                    iced::Background::Gradient(iced::Gradient::Linear(
                        iced::gradient::Linear::new(iced::Radians(
                            std::f32::consts::FRAC_PI_2,
                        ))
                        .add_stop(0.0, accent)
                        .add_stop(1.0, Color::TRANSPARENT),
                    ))
                } else {
                    accent.into()
                }),
                ..Default::default()
            })
    };
    column![
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(move |_| container::Style {
                background: Some(accent.into()),
                ..Default::default()
            }),
        row![side(), middle, side(),].height(Length::Fill),
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(move |_| container::Style {
                background: Some(bottom.into()),
                ..Default::default()
            }),
    ]
    .width(Length::Fixed(width))
    .height(Length::Fixed(TAB_ROW_H))
    .into()
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
        container(text(label.to_string()).size(11.0).color(text_color))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(TAB_ROW_H))
    .padding(0)
        .style(move |_t, status| {
            // clone: File tab solid brand color; main tabs transparent with a
            // 1px outline on hover; active tab fills the body color and
            // borders top/sides, bottom merges into the body.
            let outline = skin.tab_hover_outline;
            let frame = skin.sep;
            let (bgc, border_color, border_w) = if is_file {
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
                background: bgc.map(Into::into),
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
    // Active tab (normal AND contextual) merges into the command pane:
    // the middle of its bottom border changes to the pane color — the
    // corners keep the border so the side lines run on. The bridge in
    // `view` adds the interference zone under it.
    let btn: Element<Message> = if active && !is_file {
        stack![btn, tab_merge_overlay(width, skin)]
            .width(Length::Fixed(width))
            .into()
    } else if !active && accent.is_some() {
        // inactive contextual tab: a rectangle border THICKER (2px fading
        // sides) than the activated one (1px solid) — see ctx_frame
        let frame = ctx_frame(
            width,
            2.0,
            accent.unwrap(),
            skin.ribbon_bg,
            true,
            Space::new().width(Length::Fill).height(Length::Fill).into(),
        );
        stack![frame, btn].width(Length::Fixed(width)).into()
    } else {
        btn.into()
    };
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
pub(crate) fn visible_tabs(state: &Explorer) -> Vec<&TabDef> {
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
                .filter(|t| t.when.as_deref().map(|w| eval_when(w, ctx)).unwrap_or(true)),
        )
        .collect()
}

pub(crate) fn tab_width(label: &str) -> f32 {
    // Win8 tabs are roomy: 76px minimum, extending when the text is longer
    (17.0 + label.chars().count() as f32 * 6.43).max(65.0)
}

/// Exact rendered width of a tab (File gets the wide Win8 block).
pub(crate) fn tab_row_w(tab: &TabDef) -> f32 {
    if tab.kind.as_deref() == Some("file") {
        53.0
    } else {
        tab_width(&tab.label)
    }
}

/// Width of the main tabs region in the tab row — contextual title-bar
/// blocks align directly above their stubs.
pub(crate) fn main_tabs_width(state: &Explorer) -> f32 {
    // main tabs: no spacing between them
    state.ribbon.tabs.iter().map(tab_row_w).sum()
}

/// x offset + width (+ contextual accent color, if any) of the currently
/// active tab within the tab row.
pub(crate) fn active_tab_geometry(state: &Explorer) -> Option<(f32, f32, Option<Color>)> {
    let mut x = 0.0f32;
    let mut prev_ctx = false;
    for tab in visible_tabs(state) {
        let ctx = tab.color.is_some();
        if ctx && prev_ctx {
            x += CTX_TAB_GAP; // space only between two contextual tabs
        }
        let w = tab_row_w(tab);
        if tab.id == state.active_tab {
            return Some((x, w, tab.color.as_deref().and_then(color_by_name)));
        }
        x += w;
        prev_ctx = ctx;
    }
    None
}

fn tab_row(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // ribbon minimize chevron (Win8: also double-click a tab)
    let collapse = button(icon(if state.ribbon_collapsed { "chevron_down" } else { "up" }, 16.0, skin))
        .width(Length::Fixed(CHROME_BTN))
        .height(Length::Fixed(CHROME_BTN))
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
        .width(Length::Fixed(CHROME_BTN))
        .height(Length::Fixed(CHROME_BTN))
        .style(|_t, _status| button::Style {
            background: None,
            text_color: Color::TRANSPARENT,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        });
    let mut tabs_row = row![].height(Length::Fixed(TAB_ROW_H));
    let mut prev_ctx = false;
    for tab in visible_tabs(state) {
        let width = tab_row_w(tab);
        let accent = tab.color.as_deref().and_then(color_by_name);
        let ctx = accent.is_some();
        if ctx && prev_ctx {
            // space only between two contextual tabs (Win8 reference)
            tabs_row = tabs_row.push(Space::new().width(Length::Fixed(CTX_TAB_GAP)));
        }
        prev_ctx = ctx;
        if let Some(accent) = accent.filter(|_| state.self_border) {
            // Win8: connecting stub under the title-bar block, labeled with
            // the first group's caption.
            let active = state.active_tab == tab.id;
            let stub_label = tab.groups.first().map(|g| g.label.clone()).unwrap_or_default();
            // both states are the same ctx_frame — only the border differs
            // (2px fading sides when idle, 1px solid when active), so the
            // caption keeps the exact same position
            let stub: Element<Message> = if active {
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
                // active: SOLID 1px accent frame (thinner than the idle 2px
                // fading sides), built from the same ctx_frame so the
                // caption stays at the same position; tab_merge_overlay
                // repaints the middle of the bottom line in the pane color
                // and the bridge continues the side lines
                let framed = ctx_frame(width, 1.0, accent, accent, false, label.into());
                stack![framed, tab_merge_overlay(width, skin)]
                    .width(Length::Fixed(width))
                    .into()
            } else {
                // idle: same frame, 2px fading sides, pane-colored bottom
                let label = container(
                    text(stub_label)
                        .size(12.0)
                        .color(accent.scale_alpha(0.9))
                        .width(Length::Shrink),
                )
                .width(Length::Fixed(width - 4.0))
                .height(Length::Fill)
                .center_x(Length::Fixed(width - 4.0))
                .center_y(Length::Fill);
                ctx_frame(width, 2.0, accent, skin.ribbon_bg, true, label.into())
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
            container(collapse).height(Length::Fixed(TAB_ROW_H)).center_y(Length::Fixed(TAB_ROW_H)),
            container(help).height(Length::Fixed(TAB_ROW_H)).center_y(Length::Fixed(TAB_ROW_H)).padding([0.0, 6.0]),
        ]
        .height(Length::Fixed(TAB_ROW_H)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TAB_ROW_H))
    .style(bg(skin.window_bg)) // white strip - the accent frame must not bleed through
    .into()
}

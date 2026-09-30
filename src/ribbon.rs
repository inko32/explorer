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
    #[allow(dead_code)]
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

// ---- MS Paint toolbar metrics — EVERY number from the Java source ----
/// HomeToolBar `EmptyBorder(5, 5, 3, 5)`: top, sides, bottom.
pub(crate) const PAD_TOP: f32 = 5.0;
pub(crate) const PAD_SIDE: f32 = 5.0;
pub(crate) const PAD_BOTTOM: f32 = 3.0;
/// Content row height = `TSeparator(9, 92, 0, ...)` height: 92.
pub(crate) const CONTENT_H: f32 = 92.0;
/// Total toolbar height (5 + 92 + 3) — unchanged from the classic backend
/// so the rest of the app layout is unaffected.
pub(crate) const RIBBON_H: f32 = PAD_TOP + CONTENT_H + PAD_BOTTOM; // 100
/// Group caption row: JLabel at Dialog 12 ≈ 17.
pub(crate) const CAPTION_H: f32 = 17.0;
/// Group command area.
pub(crate) const GROUP_CONTENT_H: f32 = CONTENT_H - CAPTION_H; // 75
/// `TSeparator`: a 9px zone with a FULL-HEIGHT 1px line (gap 0).
pub(crate) const SEP_W: f32 = 9.0;
/// `TButton` `Insets(3,5,3,5)` + 16px icon → 22px rows; `GridBagPanel` has
/// ZERO insets → rows pack with NO gaps.
pub(crate) const SMALL_H: f32 = 22.0;
/// `MainTools` margin `(2,2,2,2)` + 16 icon → 20px tool squares.
pub(crate) const ICON_BTN: f32 = 20.0;
/// `TButtonAndDropTools` (ImageTools selectTool): bigIcon 32 +
/// `Insets(2,4,2,4)` → 40px icon zone; the Paste bigIcon is 25x31.
pub(crate) const BIG_ICON: f32 = 32.0;
pub(crate) const BIG_ICON_ZONE: f32 = 40.0;
/// `DropTools` VERTICAL strip: text 16 + `Insets(3,5,13,5)` → 32 (the 13
/// is the bottom arrow zone).
pub(crate) const BIG_STRIP_H: f32 = 32.0;
pub(crate) const BIG_DIVIDER_H: f32 = 1.0;
/// `ShapeTools`: scroll pane preferred 162x64, `listPanel` WHITE, 1px
/// `LineBorder(borderColor)`, 7 columns, `ShapeButton` margin 0 + 16 icon
/// → cells 162/7 x 21.
pub(crate) const GALLERY_W: f32 = 162.0;
pub(crate) const GALLERY_CELL_W: f32 = GALLERY_W / 7.0; // 23.14
pub(crate) const GALLERY_CELL_H: f32 = 21.0;
/// `ColorPlateTool`: `ColorPlateButton` 20x20 (white fill, A0A0A0 edge),
/// colorPanel `Insets(1,1,1,1)` 10x3 grid; `ColorButton` image 32 with the
/// swatch at (9,7) 28x28, margins (3,5,3,5); group cell insets (7,7,5,7);
/// swatch hover CBECFD fill / 64A5E7 border (skin.plate_hover*).
pub(crate) const COLOR_CELL: f32 = 20.0;
pub(crate) const COLOR_SWATCH: f32 = 32.0;
/// `TopMenuButton` (the MS Paint File button): 58x23 — reference only;
/// the explorer keeps its Win8 File tab.
#[allow(dead_code)]
pub(crate) const FILE_BTN_W: f32 = 58.0;
#[allow(dead_code)]
pub(crate) const FILE_BTN_H: f32 = 23.0;
/// Swing default font: Dialog 12 (menu sections bold 12).
pub(crate) const LABEL_SIZE: f32 = 12.0;

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
use crate::ui_common::{bg, dbg, icon, skin, CHROME_BTN, MENU_W};

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

/// Advance width at the Swing Dialog-12 size the ribbon labels use.
pub(crate) fn text_w(s: &str) -> f32 {
    text11_w(s) * (LABEL_SIZE / 11.0)
}

pub(crate) fn view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // geometry + accent of the active tab (the accent also drives the line)
    let active_geo = active_tab_geometry(state);
    // HomeToolBar.paintComponent: 1px border lines top (y=0) and bottom
    // (y=height-1) over the toolbar, plus a bg-colored bridge over the top
    // line under the File button. With an active CONTEXTUAL tab the line
    // takes that tab's accent color — the active tab covers its own segment
    // (see the bridge below).
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

/// MS Paint `TSeparator(9, 92, gap 0)`: a 9px zone with a full-height 1px
/// line centered in it (height 92 = the toolbar content row).
fn separator(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let h = phys_px(state, CONTENT_H);
    container(
        container(Space::new().width(Length::Fixed(phys_px(state, 1.0))).height(Length::Fixed(h)))
            .style(bg(skin.sep)),
    )
    .width(Length::Fixed(SEP_W))
    .height(Length::Fill)
    .center_x(Length::Fixed(SEP_W))
    .into()
}

/// Small labeled command (`TButton` text+icon, Insets(3,5,3,5) + 16px icon
/// → 22px rows). `label: ""` renders a MainTools-style icon-only 20px
/// square (Insets(2,2,2,2)).
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
                .size(LABEL_SIZE)
                .color(if enabled { skin.text } else { skin.disabled }),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        container(r)
            .height(Length::Fixed(SMALL_H))
            .center_y(Length::Fixed(SMALL_H))
            .into()
    };
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(if label.is_empty() { ICON_BTN } else { SMALL_H }))
        .padding([0.0, 5.0])
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

/// Checkbox command (our noun — MS Paint has none): a 13px box + label
/// toggle on a 22px row; checked = selected.
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
            .size(LABEL_SIZE)
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
        .padding([0.0, 5.0])
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

/// MS Paint `TButtonAndDropTools`: a 40px big-icon button on top, a 1px
/// divider, then the 32px `DropTools` strip ("label" + ▾ at the bottom
/// center, Insets(3,5,13,5)). The PARENT paints the chrome: one border
/// around both halves + the divider line, when selected (SELECTABLE half
/// also fills C9E0F7) or while the menu is open (paintUrgent, no fill).
/// Note: the unified HOVER border (parent mouseEntered) is approximated —
/// each half shows its own hover fill; the shared border appears only when
/// active.
#[allow(clippy::too_many_arguments)]
fn big_split(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    selectable: bool,
    active: bool,
    action: Option<Message>,
    menu: Option<Message>,
    state: &Explorer,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // top half: the big icon runs the action
    let icon_zone = container(icon_el)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_ICON_ZONE))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(BIG_ICON_ZONE));
    let top = button(icon_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_ICON_ZONE))
        .padding(0)
        .style(tbtn_style(enabled, selectable && active, skin));
    let top: Element<Message> = match (enabled, action) {
        (true, Some(m)) => top.on_press(m).into(),
        _ => top.into(),
    };
    // divider between the halves — drawn only in the active state
    // (SELECTED_BORDER line at the icon height), else transparent
    let divider = container(
        container(
            Space::new()
                .width(Length::Fixed(phys_px(state, 1.0)))
                .height(Length::Fill),
        )
        .style(move |_| container::Style {
            background: Some(
                (if active { skin.sel_btn_border } else { Color::TRANSPARENT }).into(),
            ),
            ..Default::default()
        }),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(BIG_DIVIDER_H))
    .center_x(Length::Fixed(width));
    // bottom half: the DropTools strip — text, ▾ centered at the bottom
    let tc = if enabled { skin.text } else { skin.disabled };
    let strip_zone = column![
        container(
            text(label.to_string())
                .size(LABEL_SIZE)
                .color(tc),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(16.0))
        .center_x(Length::Fixed(width))
        .align_y(iced::Alignment::End),
        container(drop_arrow_el(tc, 0.0))
            .width(Length::Fixed(width))
            .height(Length::Fixed(BIG_STRIP_H - 16.0))
            .center_x(Length::Fixed(width))
            .align_y(iced::Alignment::Center),
    ]
    .width(Length::Fixed(width));
    let bottom = button(
        container(strip_zone)
            .width(Length::Fixed(width))
            .height(Length::Fixed(BIG_STRIP_H)),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(BIG_STRIP_H))
    .padding(0)
    .style(tbtn_style(enabled, false, skin));
    let bottom: Element<Message> = match (enabled, menu) {
        (true, Some(m)) => bottom.on_press(m).into(),
        _ => bottom.into(),
    };
    // the parent's unified border (selected / menu-open)
    let frame: Element<Message> = container(
        Space::new()
            .width(Length::Fixed(width))
            .height(Length::Fixed(GROUP_CONTENT_H)),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(GROUP_CONTENT_H))
    .style(move |_| container::Style {
        background: None,
        border: iced::Border {
            color: if active {
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
        stack![
            column![top, divider, bottom].width(Length::Fixed(width)),
            frame
        ]
        .width(Length::Fixed(width)),
    )
    .into()
}

/// 90-degree dropdown triangle (the MS Paint DropTools glyph: three 1px
/// lines, 5x3 px), padded so it sits below an adjacent label.
pub(crate) fn drop_arrow_el(c: Color, top_pad: f32) -> Element<'static, Message> {
    container(
        iced::widget::svg(crate::icons::drop_arrow(c))
            .width(Length::Fixed(5.0))
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

/// MS Paint `DropTools` HORIZONTAL (Insets(3,5,3,15) — the 15 is the arrow
/// zone): icon + label with a ▾ at width-10. The whole button opens the
/// menu unless `action` is set (then the label runs the action).
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
        text(label.to_string()).size(LABEL_SIZE).color(tc),
        Space::new().width(Length::Fill),
        container(drop_arrow_el(tc, 4.5))
            .width(Length::Fixed(10.0))
            .align_x(iced::Alignment::End),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .width(Length::Fixed(width - 15.0));
    let content = container(label_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(SMALL_H))
        .center_y(Length::Fixed(SMALL_H))
        .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 });
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

/// Width of one small command, margins included (TButton Insets(3,5,3,5),
/// iconTextGap 4; DropTools keeps its ▾ in the 15px right margin).
fn small_width(cmd: &CmdRef) -> f32 {
    if cmd.label.is_empty() && cmd.icon.is_some() {
        return ICON_BTN;
    }
    let mut w = 10.0; // left + right margins
    if cmd.kind.as_deref() == Some("checkbox") {
        w += 13.0 + 6.0;
    } else if cmd.icon.is_some() {
        w += 16.0 + 4.0;
    }
    w += text_w(&cmd.label);
    w.ceil()
}

/// Width of the big split button: the icon zone is 4+32+4 = 40; the strip
/// is 5+text+5 (arrow sits in the 13px bottom margin).
fn big_split_width(label: &str) -> f32 {
    (text_w(label) + 10.0).max(BIG_ICON_ZONE + 1.0).ceil()
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

/// Effective group width. MS Paint groups have NO side padding — GridBag
/// zero insets pack the components flush; the toolbar padding frames the
/// row. GridBag also puts NO gaps between grid columns; the only visual
/// gaps come from the components' own margins.
pub(crate) fn group_width(g: &GroupDef) -> f32 {
    if let Some(w) = g.width {
        return w;
    }
    match g.kind.as_deref() {
        Some("gallery") => {
            let rows = g.rows.unwrap_or(3).clamp(1, 3);
            let cols = g.commands.len().div_ceil(rows).max(1);
            (cols as f32 * GALLERY_CELL_W + 2.0 + 13.0).ceil()
        }
        Some("color_plate") => {
            // ColorPlateTool cell insets (7,7,5,7): 7 leading; ColorButton
            // 32+margins 10 = 42; gap 7+7 = 14; color2 42; gap 7; panel
            // 10*20 + insets 2 = 202; chooser 30+margins 10 = 40.
            (7.0 + 42.0 + 14.0 + 42.0 + 7.0 + 202.0 + 40.0_f32).ceil()
        }
        _ => {
            let rows = g.rows.unwrap_or(3).clamp(1, 3);
            let mut items_w = 0.0f32;
            let mut smalls: Vec<&CmdRef> = Vec::new();
            for cmd in &g.commands {
                if cmd.separator == Some(true) {
                    items_w += SEP_W;
                } else if cmd.size.as_deref() == Some("big") {
                    items_w += cmd_width(cmd);
                } else {
                    smalls.push(cmd);
                }
            }
            let columns = smalls.len().div_ceil(rows).max(1);
            let col_w = smalls.iter().map(|c| cmd_width(c)).fold(0.0f32, f32::max);
            if !smalls.is_empty() {
                items_w += columns as f32 * col_w;
            }
            items_w.ceil()
        }
    }
}

/// Clamp a dropdown menu anchor x so the menu stays inside the window.
pub(crate) fn menu_anchor(state: &Explorer, x: f32) -> f32 {
    let (_, fr, _, fl) = frame_insets(state);
    x.clamp(4.0, (state.win_w - fl - fr - 220.0).max(4.0))
}

/// Convert YAML dropdown entries to menu rows; `">Name"` entries become
/// MS Paint `TMenuSection` headers.
fn menu_entries(items: &[String]) -> Vec<crate::model::MenuEntry> {
    items
        .iter()
        .map(|s| match s.strip_prefix('>') {
            Some(name) => crate::model::MenuEntry::Section(name.to_string()),
            None => crate::model::MenuEntry::Item(s.clone()),
        })
        .collect()
}

/// Build one group's content from YAML. Small commands stack into
/// `rows`-tall columns (ClipboardTools arrangement), packed with no gaps.
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

    let mut content = row![];
    let mut bigs_w = 0.0f32;
    for cmd in &g.commands {
        if cmd.separator == Some(true) {
            content = content.push(separator(state, skin));
            bigs_w += SEP_W;
            continue;
        }
        if cmd.size.as_deref() == Some("big") {
            let enabled = enabled_of(cmd);
            let w = cmd_width(cmd);
            let has_action = state.ribbon.commands.contains_key(&cmd.id);
            let selectable = cmd.group.is_some();
            let active = selected_of(cmd);
            let action = enabled.then(|| {
                press_of(cmd, Message::RunCommand(cmd.id.clone()))
            });
            let menu = cmd.dropdown.as_ref().and_then(|items| {
                enabled.then(|| Message::OpenMenu {
                    x: menu_anchor(state, x_base + bigs_w),
                    y: ribbon_body_y() + PAD_TOP + GROUP_CONTENT_H,
                    source: cmd.id.clone(),
                    source_w: w.max(MENU_W),
                    items: menu_entries(items),
                })
            });
            let el = big_split(
                crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), BIG_ICON, &skin),
                &cmd.label,
                w,
                enabled,
                selectable,
                active,
                if has_action { action } else { None },
                menu,
                state,
                skin,
            );
            bigs_w += w;
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
            // MS Paint has no split small: the DropTools button opens the
            // menu, unless the command itself has an action (then it wins).
            let menu_msg = enabled.then(|| Message::OpenMenu {
                x: menu_anchor(state, x_base + bigs_w + col * col_w),
                y: ribbon_body_y() + PAD_TOP + row_i * SMALL_H + SMALL_H,
                source: cmd.id.clone(),
                source_w: w.max(MENU_W),
                items: menu_entries(items),
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
        let mut col = column![];
        for _ in 0..rows {
            if let Some(s) = it.next() {
                col = col.push(s);
            }
        }
        content = content.push(col);
    }
    group_frame(&g.label.clone(), group_width(g), content.into(), skin)
}

/// MS Paint gallery (ShapeTools): a white panel with a 1px border and a
/// grid of 23x21 cells (16px icons, margin 0). Our extension: a 13px strip
/// with an expand chevron where the Java scroll pane has its 13px scrollbar.
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
                    .width(Length::Fixed(GALLERY_CELL_W))
                    .height(Length::Fixed(GALLERY_CELL_H))
                    .center_x(Length::Fixed(GALLERY_CELL_W))
                    .center_y(Length::Fixed(GALLERY_CELL_H)),
            )
            .width(Length::Fixed(GALLERY_CELL_W))
            .height(Length::Fixed(GALLERY_CELL_H))
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
    let mut grid = column![];
    for _ in 0..rows {
        let mut line = row![];
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
        .width(Length::Fixed(13.0))
        .height(Length::Fixed(GROUP_CONTENT_H))
        .style(tbtn_style(true, false, skin))
        .on_press(Message::OpenMenu {
            x: menu_anchor(state, x_base + gw - 20.0),
            y: ribbon_body_y() + PAD_TOP + GROUP_CONTENT_H,
            source: g.label.clone(),
            source_w: gw.max(MENU_W),
            items: menu_entries(&items),
        });
    group_frame(
        &g.label.clone(),
        gw,
        row![panel, expand].height(Length::Fixed(GROUP_CONTENT_H)).into(),
        skin,
    )
}

/// MS Paint Colors group (ColorPlateTool): two 32px color buttons (black
/// frame + 28px swatch, "Color 1"/"Color 2" labels under), the 10-column
/// plate of 20px cells (white fill, A0A0A0 edge; hover CBECFD/64A5E7), and
/// the Edit Color chooser (30px icon + label). Cell insets (7,7,5,7).
/// A swatch click dispatches `RunCommand("color_plate:<RRGGBB>")` — kept
/// for later wiring.
fn color_plate(g: &GroupDef, _state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let color_btn = |c: Color, label: &'static str| {
        let swatch = container(
            container(Space::new().width(Length::Fixed(28.0)).height(Length::Fixed(28.0)))
                .width(Length::Fixed(COLOR_SWATCH))
                .height(Length::Fixed(COLOR_SWATCH))
                .center_x(Length::Fixed(COLOR_SWATCH))
                .center_y(Length::Fixed(COLOR_SWATCH))
                .style(move |_| container::Style {
                    background: Some(c.into()),
                    border: iced::Border {
                        color: Color::from_rgb8(0x21, 0x21, 0x21),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(COLOR_SWATCH + 10.0))
        .height(Length::Fixed(COLOR_SWATCH + 10.0))
        .center_x(Length::Fixed(COLOR_SWATCH + 10.0))
        .center_y(Length::Fixed(COLOR_SWATCH + 10.0));
        let content = column![
            swatch,
            container(text(label).size(LABEL_SIZE).color(skin.text))
                .width(Length::Fixed(COLOR_SWATCH + 10.0))
                .center_x(Length::Fixed(COLOR_SWATCH + 10.0)),
        ]
        .spacing(2);
        button(content)
            .width(Length::Fixed(COLOR_SWATCH + 10.0))
            .padding(0)
            .style(tbtn_style(true, false, skin))
    };
    let mut plate = column![];
    for r in 0..2 {
        let mut line = row![];
        for c in 0..10 {
            let hex = COLOR_PLATE[r][c];
            let c = Color::from_rgb8(((hex >> 16) & 0xFF) as u8, ((hex >> 8) & 0xFF) as u8, (hex & 0xFF) as u8);
            let swatch = button(
                container(
                    Space::new()
                        .width(Length::Fixed(COLOR_CELL - 2.0))
                        .height(Length::Fixed(COLOR_CELL - 2.0)),
                )
                .width(Length::Fixed(COLOR_CELL))
                .height(Length::Fixed(COLOR_CELL))
                .center_x(Length::Fixed(COLOR_CELL))
                .center_y(Length::Fixed(COLOR_CELL))
                .style(move |_t| container::Style {
                    background: Some(c.into()),
                    border: iced::Border {
                        color: Color::from_rgb8(0xA0, 0xA0, 0xA0),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
            )
            .width(Length::Fixed(COLOR_CELL))
            .height(Length::Fixed(COLOR_CELL))
            .padding(0)
            .style(move |_t, status| {
                let mut style = tbtn_style(true, false, skin)(_t, status);
                if status == button::Status::Hovered {
                    style.background = Some(skin.plate_hover.into());
                    style.border = iced::Border {
                        color: skin.plate_hover_border,
                        width: 1.0,
                        radius: 0.0.into(),
                    };
                }
                style
            })
            .on_press(Message::RunCommand(format!("color_plate:{hex:06X}")));
            line = line.push(swatch);
        }
        plate = plate.push(line);
    }
    let panel = container(plate)
        .padding(1.0)
        .style(move |_| container::Style {
            background: Some(skin.window_bg.into()),
            border: iced::Border {
                color: Color::from_rgb8(0xA0, 0xA0, 0xA0),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    // chooser: the group's first command, if any (MS Paint opens the Edit
    // Colors dialog here)
    let chooser: Element<Message> = match g.commands.first() {
        Some(cmd) => {
            let content = column![
                container(crate::icon::view(cmd.icon.as_deref().unwrap_or("gear"), 30.0, &skin))
                    .width(Length::Fixed(40.0))
                    .height(Length::Fixed(30.0))
                    .center_x(Length::Fixed(40.0))
                    .center_y(Length::Fixed(30.0)),
                container(
                    text("Edit Color").size(LABEL_SIZE).color(skin.text),
                )
                .width(Length::Fixed(40.0))
                .center_x(Length::Fixed(40.0)),
            ]
            .spacing(2);
            button(content)
                .width(Length::Fixed(40.0))
                .padding(0)
                .style(tbtn_style(true, false, skin))
                .on_press(Message::RunCommand(cmd.id.clone()))
                .into()
        }
        None => Space::new().width(Length::Fixed(40.0)).height(Length::Fixed(30.0)).into(),
    };
    let content = container(
        row![
            Space::new().width(Length::Fixed(7.0)),
            color_btn(Color::BLACK, "Color 1"),
            color_btn(Color::WHITE, "Color 2"),
            Space::new().width(Length::Fixed(7.0)),
            panel,
            chooser,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(GROUP_CONTENT_H))
    .align_y(iced::Alignment::Center);
    group_frame(&g.label.clone(), group_width(g), content.into(), skin)
}

/// A group: content on top, centered Dialog-12 caption under it (the Java
/// groups pin the caption to the bottom edge via a weighted glue row).
fn group_frame(caption: &str, width: f32, content: Element<'static, Message>, skin: skin::Skin) -> Element<'static, Message> {
    column![
        container(dbg(
            Box::leak(format!("group_body:{caption}").into_boxed_str()),
            content,
        ))
        .width(Length::Fixed(width))
        .height(Length::Fixed(GROUP_CONTENT_H)),
        container(text(caption.to_string()).size(LABEL_SIZE).color(skin.subtle))
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

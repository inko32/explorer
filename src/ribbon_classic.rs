//! YAML-driven ribbon (D12): config schema, loader, `when` expression
//! evaluator, enablement rules, icon mapping, command dispatch.

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
    /// When to AUTO-activate the tab (defaults to `when`). Use a stricter
    /// expression to show the tab idle on weaker contexts: e.g. the Archive
    /// tab shows when an archive is selected but only activates inside one.
    #[serde(default)]
    pub activate_when: Option<String>,
    #[serde(default)]
    pub groups: Vec<GroupDef>,
}

#[derive(Debug, Deserialize)]
pub struct GroupDef {
    pub label: String,
    /// Manual width override in px — normally omitted: the width is computed
    /// from the group's content (buttons, columns, spacing, padding).
    #[serde(default)]
    pub width: Option<f32>,
    /// Vertical slots per small-command column (default 3, max 3 -
    /// four buttons in one column are not allowed).
    #[serde(default)]
    pub rows: Option<usize>,
    /// "gallery": tile the commands as an in-ribbon gallery grid with an
    /// expand button opening the full list as a dropdown.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub commands: Vec<CmdRef>,
}

#[derive(Debug, Deserialize)]
pub struct CmdRef {
    /// Empty + `separator: true` renders a vertical separator INSIDE a group.
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
    /// Manual width override in px — normally omitted: big buttons size
    /// from label+icon (`big_width`), small ones from icon+label (+checkbox
    /// box / dropdown arrow) via `small_width`.
    #[serde(default)]
    pub width: Option<f32>,
    /// "checkbox": renders a check box + label (active state = checked).
    #[serde(default)]
    pub kind: Option<String>,
    /// Dropdown menu items (command ids) — renders a split button with an
    /// arrow that opens the menu below the ribbon.
    #[serde(default)]
    pub dropdown: Option<Vec<String>>,
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

// ---------------- context + `when` expressions ----------------

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

// ---------------- enablement ----------------

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



// ---------------- ribbon UI (extracted from ui.rs) ----------------

use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::border::frame_insets;
use crate::model::{Explorer, Message};
use crate::ui_common::{bg, dbg, icon, skin, CHROME_BTN};

pub(crate) fn view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // geometry + accent of the active tab (the accent also drives the line)
    let active_geo = active_tab_geometry(state);
    // clone/Win8: a 1px line under the tab row; the body has NO top border.
    // With an active CONTEXTUAL tab the whole line takes that tab's accent
    // color — the active tab covers its own segment (see the bridge below),
    // which is also its +1px pane-colored bottom line.
    let line_color = active_geo.and_then(|(_, _, a)| a).unwrap_or(skin.sep);
    let line: Element<Message> = container(
        Space::new().width(Length::Fill).height(Length::Fixed(TAB_LINE_H)),
    )
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(line_color.into()),
        ..Default::default()
    })
    .into();
    let body = container(ribbon_body(state, skin))
        .width(Length::Fill)
        .height(Length::Fixed(RIBBON_H))
        .style(move |_| container::Style {
            background: Some(skin.ribbon_bg.into()),
            ..Default::default()
        });
    // bottom edge of the ribbon body (the top edge is `line` above)
    let bottom_line: Element<Message> = container(
        Space::new().width(Length::Fill).height(Length::Fixed(TAB_LINE_H)),
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
                    // the tab's +2px height interferes with the tab-row line:
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
                    // start at y=24: right below the tab row
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
            // 1px #EBECEC outline on hover; active tab fills the body color
            // and borders top/sides #DADBDC, bottom merges into the body.
            let outline = skin.tab_hover_outline;
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
    // Active tab (normal AND contextual) merges into the command pane:
    // the middle of its bottom border changes to the pane color (the
    // corners keep the border so the side lines run on). The bridge in
    // `view` adds the +2px interference zone under it.
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

/// Horizontal space between two CONTEXTUAL tabs (and between their
/// title-bar blocks) — single source of fact for the tab row, the
/// active-tab geometry and the title-bar blocks in border.rs.
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

// ---- group metrics — the width math in group_width() and the rendering
// in group()/group_from_config() must agree exactly, so these are consts ----
/// Group content top padding (the buttons start this far below the body top).
pub(crate) const GROUP_TOP_PAD: f32 = 4.0;
/// Group horizontal padding, each side.
pub(crate) const GROUP_PAD: f32 = 5.0;
/// Width of the zone holding a group separator line (right edge) or an
/// internal `separator: true` strip.
pub(crate) const GROUP_SEP_W: f32 = 11.0;
/// Horizontal spacing between the items inside a group.
pub(crate) const GROUP_ITEM_SPACING: f32 = 2.0;
/// Height of the tall content zone (internal separator, gallery expand).
pub(crate) const GROUP_TALL_ZONE_H: f32 = 76.0;
/// Height of one small-command row.
pub(crate) const SMALL_ROW_H: f32 = 23.0;
/// Vertical spacing between two small-command rows in a column.
pub(crate) const SMALL_ROW_SPACING: f32 = 1.0;

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
                // active: SOLID 1px accent frame (thinner than the idle 2px
                // fading sides), built from the same ctx_frame so the
                // caption stays at the same position; tab_merge_overlay
                // repaints the middle of the bottom line in the pane color
                // and the bridge continues the side lines
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

/// Active tab's merge overlay: repaints the MIDDLE (`width - 2`) of the
/// tab's bottom border in the command-pane color, leaving the bottom
/// corners untouched so the side border lines keep running (into the
/// bridge legs in `view`). `width` = full tab width, 24px tall, aligned
/// to the bottom edge.
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
/// and active (a container border does not offset content in iced layout).
/// Top line: always 1px solid accent. Bottom line: 1px in `bottom` (the
/// pane color when idle — the border fades to 0% there — or the accent
/// when active, where `tab_merge_overlay` repaints its middle). Sides: the
/// distinctive `th`-px border — a linear 100%->0% top-to-bottom fade when
/// `fade` (idle, 2px), solid when active (1px, thinner than idle).
/// `middle` sits between the sides (a Space for a plain tab, the caption
/// label for a stub).
#[allow(clippy::too_many_arguments)]
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

pub(crate) fn cmd_style(
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


/// Approximate advance width of a string at 11px in the system sans
/// (DejaVu Sans family) — used for big-button width estimation.
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

/// Split a label into its two display lines. Manual break wins: a `\n` in
/// the YAML label (double-quoted "\n" escapes to a real newline) splits
/// there. Otherwise: 1 word = one line; 2 words = one per line; 3+ words =
/// the width-balanced break (minimize the wider line; ties go to the
/// latest break - "Run as\nAdministrator", "Execute\nthe file").
pub(crate) fn label_lines(label: &str) -> (String, Option<String>) {
    if let Some((a, b)) = label.split_once('\n') {
        return (a.to_string(), Some(b.replace('\n', " ")));
    }
    let words: Vec<&str> = label.split_whitespace().collect();
    match words.len() {
        0 => (String::new(), None),
        1 => (label.to_string(), None),
        2 => (words[0].to_string(), Some(words[1].to_string())),
        _ => {
            let mut best = 1usize;
            let mut best_score = f32::MAX;
            for k in 1..words.len() {
                let l1 = words[..k].join(" ");
                let l2 = words[k..].join(" ");
                let score = text11_w(&l1).max(text11_w(&l2));
                if score <= best_score {
                    best_score = score;
                    best = k;
                }
            }
            (words[..best].join(" "), Some(words[best..].join(" ")))
        }
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn label_split_two_words() {
        assert_eq!(label_lines("Move to"), ("Move".to_string(), Some("to".to_string())));
        assert_eq!(label_lines("Delete"), ("Delete".to_string(), None));
    }

    #[test]
    fn label_split_three_words_balanced() {
        // "Run as Administrator" breaks after "as"; "Execute the file" after "Execute"
        assert_eq!(
            label_lines("Run as Administrator"),
            ("Run as".to_string(), Some("Administrator".to_string()))
        );
        assert_eq!(
            label_lines("Execute the file"),
            ("Execute".to_string(), Some("the file".to_string()))
        );
    }

    #[test]
    fn label_split_manual_newline() {
        assert_eq!(
            label_lines("Run as\nAdministrator"),
            ("Run as".to_string(), Some("Administrator".to_string()))
        );
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn embedded_yaml_loads() {
        let cfg: RibbonConfig = serde_yaml::from_str(include_str!("../assets/ribbon.yaml"))
            .expect("embedded ribbon.yaml is valid");
        assert!(!cfg.tabs.is_empty());
    }

    #[test]
    fn auto_widths_from_content() {
        // big: icon floor 48 for short labels, wider for long ones
        assert_eq!(big_width("Cut", false), 48.0);
        assert_eq!(big_width("New folder", false), 48.0); // icon floor wins
        assert!(big_width("Administrator", false) > 48.0);
        // small: 8 pad + 21 icon zone + label; +18 with a dropdown arrow
        let mk = |label: &str, icon: bool, dd: bool| CmdRef {
            id: String::new(),
            separator: None,
            label: label.to_string(),
            icon: icon.then(|| "file".to_string()),
            size: None,
            enabled: None,
            width: None,
            kind: None,
            dropdown: dd.then(Vec::new),
        };
        let plain = small_width(&mk("Copy", true, false));
        assert_eq!(plain, (8.0 + 21.0 + text11_w("Copy")).ceil());
        assert_eq!(small_width(&mk("Copy", true, true)), plain + 18.0);
        assert_eq!(small_width(&mk("Copy", false, false)), plain - 21.0);
        // manual override always wins
        let mut c = mk("Copy", true, false);
        c.width = Some(123.0);
        assert_eq!(cmd_width(&c), 123.0);
    }

    #[test]
    fn auto_group_width_sums_content() {
        let g: GroupDef = serde_yaml::from_str(
            r#"
label: Clip
rows: 3
commands:
  - { id: a, label: Paste, icon: paste, size: big }
  - { id: b, label: Cut, icon: cut, size: small }
  - { id: c, label: Copy, icon: copy, size: small }
"#,
        )
        .unwrap();
        // 10 pad + big 48 + spacing 2 + one small column
        let col = small_width(&g.commands[1]).max(small_width(&g.commands[2]));
        assert_eq!(group_width(&g), (GROUP_PAD * 2.0 + 48.0 + GROUP_ITEM_SPACING + col).ceil());
        // manual override wins
        let g2: GroupDef = serde_yaml::from_str("label: X\nwidth: 200\ncommands: []").unwrap();
        assert_eq!(group_width(&g2), 200.0);
    }
}

/// Win8 big-button width from content (spec): min = 1 border + 3 + icon 40
/// + 3 + 1 border = 48; text gets 1px margins; the arrow adds 2 spacing +
/// (1 + 6 + 1) zone.
pub(crate) fn big_width(label: &str, has_arrow: bool) -> f32 {
    let (line1, line2) = label_lines(label);
    let arrow = if has_arrow { 10.0 } else { 0.0 };
    let mut w = text11_w(&line1) + 2.0;
    if let Some(l2) = &line2 {
        w = w.max(text11_w(l2) + arrow + 2.0);
    } else if has_arrow {
        w = w.max(10.0);
    }
    w.max(48.0)
}

/// Small-button width from content: 4px padding each side + 16px icon with
/// a 5px gap (checkboxes swap the icon for a 13px box with a 6px gap) +
/// the 11px label; a dropdown split adds the 18px arrow button.
pub(crate) fn small_width(cmd: &CmdRef) -> f32 {
    let mut w = 8.0; // horizontal padding (button padding [0, 4])
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

/// Effective button width: the manual YAML `width` wins, otherwise it is
/// computed from the content (icon + label + extras).
pub(crate) fn cmd_width(cmd: &CmdRef) -> f32 {
    cmd.width.unwrap_or_else(|| {
        if cmd.size.as_deref() == Some("big") {
            big_width(&cmd.label, cmd.dropdown.is_some())
        } else {
            small_width(cmd)
        }
    })
}

/// Effective group width: the manual YAML `width` wins, otherwise it is
/// computed from the content — 10px group padding + big buttons, internal
/// separators and small-command columns with 2px spacing between items
/// (+ the 24px expand-chevron zone for galleries).
pub(crate) fn group_width(g: &GroupDef) -> f32 {
    if let Some(w) = g.width {
        return w;
    }
    let rows = g.rows.unwrap_or(3).clamp(1, 3);
    let smalls: Vec<&CmdRef> = g
        .commands
        .iter()
        .filter(|c| c.separator != Some(true) && c.size.as_deref() != Some("big"))
        .collect();
    let columns = smalls.len().div_ceil(rows).max(1);
    let col_w = smalls.iter().map(|c| cmd_width(c)).fold(0.0f32, f32::max);
    let mut items_w = 0.0f32;
    let mut n_items = 0usize;
    for cmd in &g.commands {
        if cmd.separator == Some(true) {
            items_w += GROUP_SEP_W;
            n_items += 1;
        } else if cmd.size.as_deref() == Some("big") {
            items_w += cmd_width(cmd);
            n_items += 1;
        }
    }
    if !smalls.is_empty() {
        items_w += columns as f32 * col_w;
        n_items += columns;
    }
    let spacing = GROUP_ITEM_SPACING * n_items.saturating_sub(1) as f32;
    // galleries: width = grid + 24 (expand-chevron zone absorbs the padding
    // slack); normal groups: the group padding around the content.
    let extra = if g.kind.as_deref() == Some("gallery") {
        24.0
    } else {
        GROUP_PAD * 2.0
    };
    (items_w + spacing + extra).ceil()
}


/// Big-button geometry (user spec): [1px top border, 3px, icon square,
/// 1px shared icon/label line, 3px, line1 12px, 4px line space, line2 12px,
/// 3px, 1px bottom border] = 80px total. Icon zone 44, label zone 36.
pub(crate) const BIG_ICON_ZONE: f32 = 44.0;
pub(crate) const BIG_LABEL_ZONE: f32 = 36.0;
pub(crate) const BIG_BTN_H: f32 = BIG_ICON_ZONE + BIG_LABEL_ZONE; // 80
/// group content = 4px pad + button 80; caption row 15; +1 top line
pub(crate) const GROUP_CONTENT_H: f32 = 84.0;
pub(crate) const RIBBON_H: f32 = GROUP_CONTENT_H + 15.0 + TAB_LINE_H; // 100

/// Compact two-line label for big buttons (split or plain): line 1 = all
/// but the last word, line 2 = last word (plain) or "word v" (split) or the
/// lone arrow. 11px text at line_height 1.1, two 12px rows in the 30px zone.
fn big_label_lines(
    label: &str,
    line2_extra: Option<Element<'static, Message>>, // e.g. the dropdown arrow
    width: f32,
    tc: Color,
) -> (Element<'static, Message>, Element<'static, Message>) {
    let (line1, line2) = label_lines(label);
    let l1: Element<Message> = container(
        text(line1)
            .size(11.0)
            .line_height(1.1)
            .color(tc)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(12.0))
    .center_x(Length::Fixed(width))
    .into();
    let l2: Element<Message> = match (line2, line2_extra) {
        (Some(last), extra) => {
            let mut r = row![text(last)
                .size(11.0)
                .line_height(1.1)
                .color(tc)]
            .spacing(2)
            .align_y(iced::Alignment::Start);
            if let Some(e) = extra {
                r = r.push(e);
            }
            container(r)
                .width(Length::Fixed(width))
                .height(Length::Fixed(12.0))
                .center_x(Length::Fixed(width))
                .into()
        }
        (None, Some(extra)) => container(extra)
            .width(Length::Fixed(width))
            .height(Length::Fixed(12.0))
            .center_x(Length::Fixed(width))
            .align_y(iced::Alignment::Start)
            .into(),
        (None, None) => Space::new()
            .width(Length::Fixed(width))
            .height(Length::Fixed(12.0))
            .into(),
    };
    (l1, l2)
}

fn big_cmd_w(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // icon zone: 1px border + 3px space + 40px icon, bottom edge = the shared line
    let icon_zone = container(icon_el)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_ICON_ZONE))
        .center_x(Length::Fixed(width))
        .align_y(iced::Alignment::Start)
        .padding(iced::Padding { top: 4.0, right: 0.0, bottom: 0.0, left: 0.0 });
    // label zone: 3px space, line1 12, 4px gap, line2 12, 3px + border
    let tc = if enabled { skin.text } else { skin.disabled };
    let (l1, l2) = big_label_lines(label, None, width, tc);
    let label_zone = container(column![l1, l2].spacing(4).width(Length::Fixed(width)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_LABEL_ZONE))
        .center_x(Length::Fixed(width))
        .align_y(iced::Alignment::Start)
        .padding(iced::Padding { top: 3.0, right: 0.0, bottom: 0.0, left: 0.0 });
    let content = column![
        dbg("big_icon", icon_zone),
        dbg("big_label", label_zone),
    ]
    .width(Length::Fixed(width));
    let btn = button(dbg(
        Box::leak(format!("big_cmd:{label}").into_boxed_str()),
        content,
    ))
    .width(Length::Fixed(width))
    .height(Length::Fixed(BIG_BTN_H))
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

fn group(caption: &str, width: f32, content: Element<'static, Message>, state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    // clone: groups pad 5px, separator line on the RIGHT, caption #494949.
    // The separator runs the ENTIRE command-pane height: 2px of air below
    // the pane top (kept), then straight down to the pane's bottom edge,
    // crossing the caption row — not just the button area. Its 1px
    // thickness is snapped to a whole PHYSICAL pixel for crispness at any
    // dpi.
    let th = phys_px(state, 1.0);
    let strip = container(
        container(Space::new().width(Length::Fixed(th)).height(Length::Fixed(RIBBON_H - 4.0)))
            .style(bg(skin.group_sep)),
    )
    .width(Length::Fixed(GROUP_SEP_W))
    .height(Length::Fixed(RIBBON_H))
    .align_y(iced::Alignment::Start)
    .padding(iced::Padding { top: 2.0, right: 0.0, bottom: 0.0, left: 0.0 })
    .center_x(Length::Fixed(GROUP_SEP_W));
    row![
        column![
            container(dbg(Box::leak(format!("group_body:{caption}").into_boxed_str()), content))
                .width(Length::Fixed(width))
                .height(Length::Fixed(GROUP_CONTENT_H))
                .padding(iced::Padding { top: GROUP_TOP_PAD, right: GROUP_PAD, bottom: 0.0, left: GROUP_PAD }),
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
        return Space::new().width(Length::Fill).height(Length::Fixed(RIBBON_H)).into();
    };
    let mut groups_row = row![];
    let mut x = 0.0f32;
    for g in &tab.groups {
        let name: &'static str = Box::leak(format!("group:{}", g.label).into_boxed_str());
        groups_row = groups_row.push(dbg(name, group_from_config(g, state, skin, x)));
        x += group_width(g) + 1.0; // group + separator strip
    }
    groups_row.into()
}

/// Build one group's content from YAML: big commands stand alone, small
/// commands stack into `columns` columns (sequential chunks).
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
        _ => false,
    };

    let mut content = row![].spacing(GROUP_ITEM_SPACING);
    let mut bigs_w = 0.0f32;
    for cmd in &g.commands {
        if cmd.separator == Some(true) {
            // internal | separator inside the group (inset top+bottom)
            let strip = container(
                container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(GROUP_TALL_ZONE_H - 14.0)))
                    .style(bg(skin.group_sep)),
            )
            .width(Length::Fixed(GROUP_SEP_W))
            .height(Length::Fixed(GROUP_TALL_ZONE_H))
            .center_x(Length::Fixed(GROUP_SEP_W))
            .center_y(Length::Fixed(GROUP_TALL_ZONE_H));
            content = content.push(strip);
            bigs_w += GROUP_SEP_W + GROUP_ITEM_SPACING;
            continue;
        }
        if cmd.size.as_deref() == Some("big") {
            let enabled = enabled_of(cmd);
            let w = cmd_width(cmd);
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
                        x: menu_anchor(state, x_base + GROUP_PAD + bigs_w),
                        y: ribbon_body_y() + GROUP_TOP_PAD + BIG_BTN_H,
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
            bigs_w += w + GROUP_ITEM_SPACING;
            content = content.push(el);
        }
    }

    let small_cmds: Vec<&CmdRef> =
        g.commands.iter().filter(|c| c.size.as_deref() != Some("big")).collect();
    // `rows` = vertical slots per column (Win8: max 3, never four).
    // Buttons tile column-major; a column with fewer buttons stays compact.
    // Row height is FIXED (Win8/10: 2-row and 3-row columns look identical).
    let rows = g.rows.unwrap_or(3).clamp(1, 3) as usize;
    let columns = small_cmds.len().div_ceil(rows).max(1);
    let row_h = SMALL_ROW_H;
    let col_w = small_cmds
        .iter()
        .map(|c| cmd_width(c))
        .fold(0.0f32, f32::max);
    let mut smalls: Vec<Element<Message>> = Vec::new();
    for (j, cmd) in small_cmds.iter().enumerate() {
        let enabled = enabled_of(cmd);
        let active = active_of(&cmd.id);
        let w = cmd_width(cmd);
        let has_action = state.ribbon.commands.contains_key(&cmd.id);
        let msg = enabled.then(|| Message::RunCommand(cmd.id.clone()));
        let el: Element<Message> = if cmd.kind.as_deref() == Some("checkbox") {
            checkbox_cmd(&cmd.label, w, row_h, enabled, active, msg, skin)
        } else if let Some(items) = &cmd.dropdown {
            // split small: label runs the command (or opens the menu when it
            // has no action); the arrow always opens the menu
            let label_msg = if has_action { msg.clone() } else { None };
            // menu directly under this button: left edge of its column,
            // y = the button's bottom (ribbon body top + group pad +
            // row*(row h + spacing) + row h)
            let col = (j / rows) as f32;
            let row_i = (j % rows) as f32;
            let ax = x_base + GROUP_PAD + bigs_w + col * (col_w + GROUP_ITEM_SPACING);
            let ay = ribbon_body_y() + GROUP_TOP_PAD + row_i * (SMALL_ROW_H + SMALL_ROW_SPACING) + SMALL_ROW_H;
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
        let mut col = column![].spacing(SMALL_ROW_SPACING);
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
        let gw = group_width(g);
        let expand = button(crate::icon::view("chevron_down", 12.0, &skin))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(GROUP_TALL_ZONE_H))
            .style(cmd_style(true, skin))
            .on_press(Message::OpenMenu {
                x: menu_anchor(state, x_base + gw - 60.0),
                y: ribbon_body_y() + GROUP_TOP_PAD + BIG_BTN_H,
                source: g.label.clone(),
                source_w: gw,
                items,
            });
        let grid = container(content)
            .width(Length::Fixed(gw - 24.0))
            .height(Length::Fixed(GROUP_CONTENT_H));
        return group(
            &g.label.clone(),
            gw,
            row![grid, expand].height(Length::Fixed(GROUP_CONTENT_H)).into(),
            state,
            skin,
        );
    }
    group(&g.label.clone(), group_width(g), content.into(), state, skin)
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
    // icon half: top border 1 + 3 space + 40 icon; bottom edge = shared line
    let icon_zone = container(icon_el)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_ICON_ZONE))
        .center_x(Length::Fixed(width))
        .align_y(iced::Alignment::Start)
        .padding(iced::Padding { top: 4.0, right: 0.0, bottom: 0.0, left: 0.0 });
    let top = button(icon_zone)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_ICON_ZONE))
        .padding(0)
        .style(cmd_style(enabled, skin));
    let top: Element<Message> = match (enabled, action) {
        (true, Some(m)) => top.on_press(m).into(),
        _ => top.into(),
    };
    // label half: shared line + 3 space + line1 12 + 4 gap + line2 12 + 3 +
    // bottom border; the arrow is 6x3 with 1px margins each side
    let (l1, l2) = big_label_lines(label, Some(drop_arrow_el(tc, 4.5)), width, tc);
    let bottom_col = container(column![l1, l2].spacing(4).width(Length::Fixed(width)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_LABEL_ZONE))
        .align_y(iced::Alignment::Start)
        .padding(iced::Padding { top: 3.0, right: 0.0, bottom: 0.0, left: 0.0 });
    let bottom = button(bottom_col)
        .width(Length::Fixed(width))
        .height(Length::Fixed(BIG_LABEL_ZONE))
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
pub(crate) fn drop_arrow_el(c: Color, top_pad: f32) -> Element<'static, Message> {
    // 6x3 triangle, always 1px margin left and right
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

/// Logical size of `n` physical pixels at the current total UI scale
/// (system scale x the --dpi multiplier): a 1px separator must cover a
/// whole number of device pixels, otherwise it lands on a fractional
/// device width and renders as a blurry gray hairline.
pub(crate) fn phys_px(state: &Explorer, n: f32) -> f32 {
    let scale = if state.system_scale > 0.0 {
        state.system_scale * state.dpi_multiplier()
    } else {
        1.0
    };
    n / scale
}

/// Clamp a dropdown menu anchor x so the menu stays inside the window.
pub(crate) fn menu_anchor(state: &Explorer, x: f32) -> f32 {
    let (_, fr, _, fl) = frame_insets(state);
    x.clamp(4.0, (state.win_w - fl - fr - 220.0).max(4.0))
}

// ---------------- address bar ----------------



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
    #[serde(default)]
    pub groups: Vec<GroupDef>,
}

#[derive(Debug, Deserialize)]
pub struct GroupDef {
    pub label: String,
    #[serde(default)]
    pub width: Option<f32>,
    /// How many columns small commands stack into (default 1).
    #[serde(default)]
    pub columns: Option<usize>,
    #[serde(default)]
    pub commands: Vec<CmdRef>,
}

#[derive(Debug, Deserialize)]
pub struct CmdRef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandDef {
    #[serde(default)]
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
        "cut" | "copy" | "delete" | "rename" | "open_picture" => has_selection,
        "paste" => has_clipboard,
        "select_all" | "invert_selection" => has_items,
        "select_none" => has_selection,
        _ => true,
    }
}

// ---------------- icon mapping ----------------

pub fn icon_by_name(name: &str, skin: &crate::ui::skin::Skin) -> iced::widget::svg::Handle {
    use crate::icons;
    match name {
        "paste" => icons::paste(skin),
        "cut" => icons::cut(skin),
        "copy" => icons::copy(skin),
        "copy_path" => icons::copy_path(skin),
        "paste_shortcut" => icons::paste_shortcut(skin),
        "move_to" => icons::move_to(skin),
        "copy_to" => icons::copy_to(skin),
        "delete" => icons::delete(skin),
        "rename" => icons::rename(skin),
        "new_folder" => icons::new_folder(skin),
        "new_item" => icons::new_item(skin),
        "easy_access" => icons::easy_access(skin),
        "properties" => icons::properties(skin),
        "open" => icons::open(skin),
        "edit" => icons::edit(skin),
        "history" => icons::history(skin),
        "select_all" => icons::select_all(skin),
        "select_none" => icons::select_none(skin),
        "invert_sel" => icons::invert_sel(skin),
        "view_large" => icons::view_large(skin),
        "view_details" => icons::view_details(skin),
        "computer" => icons::computer(skin),
        "file" => icons::file(skin),
        "gear" => icons::gear(skin),
        "pictures" => icons::pictures(skin),
        _ => icons::file(skin),
    }
}

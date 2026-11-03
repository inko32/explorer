//! Shared UI plumbing: `--ui-debug` element outlines and small element
//! helpers used everywhere. The light/dark palettes live in
//! `assets/theme.yaml`, parsed by `crate::theme`; `skin` below is a
//! re-export so existing call sites stay unchanged.

use iced::Widget as _;
use iced::widget::container;
use iced::{Color, Element};

use crate::model::Message;

/// Square chrome tool button size (ribbon minimize/help, nav refresh,
/// status-bar view switcher) — one size everywhere.
pub(crate) const CHROME_BTN: f32 = 26.0;


pub mod skin {
    pub use crate::theme::{dark, light, Palette as Skin};
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
pub(crate) fn dbg<'a>(name: &'static str, e: Element<'a, Message>) -> Element<'a, Message> {
    let e = e;
    if !ui_debug() {
        return e;
    }
    let color = debug_color(name);
    container(e)
        .style(move |_t: &iced::Theme| container::Style {
            border: iced::Border {
                color,
                width: crate::theme::border_w(1.0),
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .boxed()
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

/// End ellipsis for file names: if the name is wider than `max_w`
/// (estimated at `per_char` px/char), cut the tail and append "...",
/// reserving 3 characters for it. Used by the text-right families
/// (Content / Tiles / Details / SmallIcon).
pub(crate) fn elide_end(s: &str, max_w: f32, per_char: f32) -> String {
    elide_cut(s, max_w, |_| per_char)
}

/// Icon-top label cut, used in EVERY item state (hover/selected look
/// identical): greedily wrap the name at `line_w` px (per-char em
/// widths — glyphs differ in width), and when it exceeds `max_lines`
/// lines, keep the first `max_lines` lines, filling the last one and
/// ending it with "..." (3 chars reserved). The returned string is
/// what the text widget wraps for real, so the ellipsis always sits at
/// the end of the last visible line.
pub(crate) fn wrap_elide(s: &str, line_w: f32, max_lines: usize, font_size: f32) -> String {
    let adv = |c: char| char_em(c) * font_size;
    // greedy fill of estimated lines
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0f32;
    for c in s.chars() {
        let cw = adv(c);
        if cur_w + cw > line_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        cur_w += cw;
        cur.push(c);
    }
    lines.push(cur);
    if lines.len() <= max_lines {
        return s.to_string();
    }
    // first max_lines-1 lines in full; the last line tail-cut to "..."
    let mut out: String = lines[..max_lines - 1].concat();
    let mut w = 0.0f32;
    for c in lines[max_lines - 1].chars() {
        let cw = adv(c);
        if w + cw + 3.0 * adv('.') > line_w {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push_str("...");
    out
}

fn elide_cut(s: &str, max_w: f32, advance: impl Fn(char) -> f32) -> String {
    let w: f32 = s.chars().map(&advance).sum();
    if w <= max_w {
        return s.to_string();
    }
    let dots = 3.0 * advance('.');
    let mut out = String::new();
    let mut used = 0.0;
    for c in s.chars() {
        let cw = advance(c);
        if used + cw + dots > max_w {
            break;
        }
        used += cw;
        out.push(c);
    }
    out.push_str("...");
    out
}

/// Estimated number of wrapped lines of `s` at `line_w` px (greedy
/// fill with the per-char em widths — glyphs differ in width).
/// Uncapped: the caller clamps to its line limit. Drives the icon-top
/// dynamic row heights (§8.3).
pub(crate) fn wrap_line_count(s: &str, line_w: f32, font_size: f32) -> usize {
    let adv = |c: char| char_em(c) * font_size;
    let mut lines = 1usize;
    let mut w = 0.0f32;
    for c in s.chars() {
        let cw = adv(c);
        if w + cw > line_w {
            lines += 1;
            w = 0.0;
        }
        w += cw;
    }
    lines
}

/// Per-character advance estimate in ems for the UI font: characters
/// differ in width ('i' ~0.4em, 'W' ~0.9em, CJK/emoji 1em), which the
/// icon-top wrap cut must account for.
pub(crate) fn char_em(c: char) -> f32 {
    const NARROW: &str = "ijlftI.,:;'\"|!()[]{}\\/`-_@";
    match c {
        ' ' => 0.30,
        'm' | 'w' => 0.85,
        'M' | 'W' => 0.90,
        _ if NARROW.contains(c) => 0.40,
        _ if c.is_ascii_uppercase() => 0.70,
        _ if c.is_ascii() => 0.58,
        _ if is_wide(c) => 1.0,
        _ => 0.60,
    }
}

/// Rough East-Asian Wide / emoji ranges (full-width glyphs advance one
/// em in the UI font).
fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF | 0x20000..=0x2FFFD | 0x30000..=0x3FFFD)
}

/// List-view name cut: a fixed unit budget (ASCII = 1 unit, any
/// non-ASCII char = 2 units); over-budget names are tail-cut with
/// "...", reserving 3 units for it. No pixel elision in List — the
/// full name is shown on one line.
pub(crate) fn truncate_units(s: &str, budget: usize) -> String {
    let units = |c: char| if c.is_ascii() { 1 } else { 2 };
    if s.chars().map(units).sum::<usize>() <= budget {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let u = units(c);
        if used + u + 3 > budget {
            break;
        }
        used += u;
        out.push(c);
    }
    out.push_str("...");
    out
}

/// Middle-ellipsis text clamp to a pixel width.
pub(crate) fn elide(s: &str, max_w: f32, per_char: f32) -> String {
    let w = s.chars().count() as f32 * per_char;
    if w <= max_w {
        return s.to_string();
    }
    let keep = ((max_w / per_char).max(1.0) as usize).saturating_sub(3);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elide_end_reserves_three_chars() {
        // 72px at 7.2 px/char = 10 chars: keep 7, reserve 3 for "..."
        let out = elide_end(&"a".repeat(100), 72.0, 7.2);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), 10);
        // fits the width: unchanged
        assert_eq!(elide_end("short", 72.0, 7.2), "short");
    }

    #[test]
    fn wrap_elide_keeps_full_lines_and_ends_with_dots() {
        // 'a' = 0.58em; at 10px font, 5 chars fill a 30px line
        let name = "a".repeat(15); // 3 estimated lines
        // fits 3 lines: unchanged
        assert_eq!(wrap_elide(&name, 30.0, 3, 10.0), name);
        // only 2 lines allowed: first line full, second tail-cut to "..."
        let out = wrap_elide(&name, 30.0, 2, 10.0);
        assert!(out.ends_with("..."));
        assert_eq!(out, format!("{}{}", "a".repeat(8), "..."));
    }

    #[test]
    fn truncate_units_counts_non_ascii_double() {
        // 100 ASCII units: shown in full
        let ascii = "a".repeat(100);
        assert_eq!(truncate_units(&ascii, 100), ascii);
        // 101 ASCII: tail-cut, 3 units reserved for "..." (97 + ... = 100)
        assert_eq!(truncate_units(&"a".repeat(101), 100).chars().count(), 100);
        // 60 non-ASCII chars = 120 units: cut at 48 chars (96 units) + "..."
        let out = truncate_units(&"é".repeat(60), 100);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), 51);
    }
}


//! Shared UI plumbing: small element helpers used everywhere. The light/dark palettes live in
//! `assets/theme.yaml`, parsed by `crate::theme`; `skin` below is a
//! re-export so existing call sites stay unchanged.

use iced::widget::container;
use iced::{Color, Element};

use crate::model::Message;

/// Square chrome tool button size (ribbon minimize/help, nav refresh,
/// status-bar view switcher) — one size everywhere.
pub(crate) const CHROME_BTN: f32 = 26.0;


pub mod skin {
    pub use crate::theme::{dark, light, Palette as Skin};
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
// Name-label cuts (single-line ellipsis and icon-top hard wrap) live
// in `crate::text_metrics` — exact cosmic-text widths, not estimates.

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


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


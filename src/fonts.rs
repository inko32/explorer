//! Font backend (reserved seam). One place decides which faces are loaded
//! and which is the UI default.
//!
//! Currently: Segoe UI for all text (temporarily, to match the Win8
//! references pixel-for-pixel). Later: register fallback faces in
//! `extra_fonts()` for CJK / Thai / … — cosmic-text falls back through the
//! registered font system by codepoint, so mixing scripts just works.
//!
//! Provenance: Segoe UI is Microsoft's artwork (like the extracted icon
//! PNGs, D14) — fine for private use, must be replaced with a
//! metric-compatible free face before any public release.

use iced::Font;

/// Family name of the UI default font.
pub const UI_FONT_NAME: &str = "Segoe UI";

/// Embedded UI font (copied from the reference clone's assets; see module
/// docs for provenance).
pub const UI_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/segoeui.ttf");

/// Extra faces registered as script fallbacks (CJK, Thai, …). Empty for now.
pub fn extra_fonts() -> &'static [&'static [u8]] {
    &[]
}

/// The UI default font (all widgets that don't override it).
pub fn default_font() -> Font {
    Font::with_name(UI_FONT_NAME)
}

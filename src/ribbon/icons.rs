//! Ribbon chrome glyph set.
//!
//! Command icons live in the **explorer icon registry** like every
//! other app icon: `assets/icons.yaml` maps each ribbon `image:` name
//! to a pack entry (embedded PNG, colorful SVG template in
//! `assets/svgs/`, or a freedesktop name for a system pack) — see
//! `src/icon.rs` / `src/icons.rs`.
//!
//! This set is only the ribbon framework's own chrome, the glyphs the
//! icon registry intentionally does NOT know, resolved as the last
//! step of `src/ribbon/image.rs`:
//!
//!   blank        placeholder image (no icon), also UiImage::default
//!   keytip_arrow keytip menu chevron
//!   font_color   fallback image of the generic color-swatch control
//!                (no such control in ribbon.yaml today)
//!   save_as, print_preview  the generic framework-test ribbon
//!                (src/ribbon/framework_tests.yaml) — not app commands
//!
//! Icons are small hand-drawn SVGs that scale to both 16px (small) and
//! 32px (large) slots.

macro_rules! icon {
    ($name:literal, $body:literal) => {
        (
            $name,
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">"#,
                $body,
                "</svg>"
            ),
        )
    };
}


/// Chrome glyphs: `(name, svg source)`.
pub const ICONS: &[(&str, &str)] = &[
    icon!("blank", ""),
    icon!("keytip_arrow", r##"<path d="M4 2l6 6-6 6" fill="none" stroke="#333333" stroke-width="1.6"/>"##),
    icon!("font_color", r##"<path d="M3.4 12.6L6.8 3.4l3.4 9.2" fill="none" stroke="#333333" stroke-width="1.3"/><path d="M2.6 13.8h8.4" stroke="#2A6DB5" stroke-width="2"/>"##),
    // generic framework-test ribbon glyphs (not app commands)
    icon!("save_as", r##"<path d="M3 2h8l2 2v6H3z" fill="#fff" stroke="#333333"/><path d="M5 2v4h5V2" fill="none" stroke="#333333"/><path d="M6 13.6h4M8 11.8v3.6" stroke="#2A6DB5" stroke-width="1.3"/>"##),
    icon!("print_preview", r##"<rect x="2" y="3" width="12" height="10" fill="#fff" stroke="#333333"/><circle cx="7.4" cy="7.4" r="2.6" fill="none" stroke="#2A6DB5" stroke-width="1.2"/><path d="M9.4 9.4L13 12" stroke="#2A6DB5" stroke-width="1.4"/>"##),
];

/// Look up a chrome glyph by name.
pub fn get(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

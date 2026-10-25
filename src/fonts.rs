//! Font backend. One place decides which faces are registered, which
//! family is the UI default (the fonts.yaml fallback list), and the
//! font sizes (FontSizes, assets/fonts.yaml).
//!
//! Primary = the system Noto Sans family (per fonts.yaml); Segoe UI is
//! NOT embedded by default — build with `make build FEATURES=embed-segoe`
//! to embed it (assets/fonts/segoeui.ttf), then naming "Segoe UI" in the
//! fallback list (e.g. as primary) uses it. Per-glyph fallback across
//! system fonts is cosmic-text's job.
//!
//! Provenance: Segoe UI is Microsoft's artwork (like the extracted icon
//! PNGs, D14) — fine for private use, must be replaced with a
//! metric-compatible free face before any public release.

use iced::Font;

/// Family name of the Win8 reference font.
pub const UI_FONT_NAME: &str = "Segoe UI";

/// Embedded Segoe UI face — only with the `embed-segoe` cargo feature
/// (off by default, so the binary ships no Microsoft artwork).
#[cfg(feature = "embed-segoe")]
pub const SEGOE_BYTES: &[u8] = include_bytes!("../assets/fonts/segoeui.ttf");

/// Shipped fallback list (fonts.yaml `family:`): Segoe UI first — it
/// resolves only when embedded (feature `embed-segoe`) or installed,
/// otherwise it is skipped.
pub const DEFAULT_FAMILIES: &[&str] = &[
    "Segoe UI",
    "Noto Sans",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans CJK TC",
    "Noto Sans CJK KR",
    "sans-serif",
];

/// Resolve the primary UI family from the configured fallback list:
/// the first family that is available wins — Segoe UI when embedded
/// (feature) or installed, otherwise the first system-installed entry;
/// "sans-serif" (or an exhausted list) defers to the system default.
/// Per-glyph fallback stays with cosmic-text's system fontdb.
pub fn resolve_primary(families: &[String]) -> Font {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let installed = |name: &str| {
        db.faces().any(|f| {
            f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
        })
    };
    for name in families {
        if name.eq_ignore_ascii_case("sans-serif") {
            break; // system fallback: let cosmic-text pick
        }
        #[cfg(feature = "embed-segoe")]
        if name == UI_FONT_NAME {
            return Font::with_name(UI_FONT_NAME);
        }
        if installed(name) {
            // Font::with_name needs 'static: startup-only, leaked once
            return Font::with_name(name.clone().leak());
        }
    }
    Font::DEFAULT // SansSerif
}

/// The UI default font (all widgets that don't override it).
pub fn default_font() -> Font {
    resolve_primary(&FontSizes::load().family)
}

// ---------------------------------------------------------------------------
// font SIZES (assets/fonts.yaml)

/// Font sizes in px at 100% scaling (96 DPI), from `assets/fonts.yaml`
/// (override ~/.config/explorer/fonts.yaml). Everything defaults to 12.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct FontSizes {
    /// Default for anything not listed (reserved).
    pub base: f32,
    /// font-family fallback list (see resolve_primary).
    pub family: Vec<String>,
    pub ribbon: RibbonFonts,
    pub titlebar: TitlebarFonts,
    pub statusbar: f32,
    pub pane: PaneFonts,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(default)]
pub struct RibbonFonts {
    pub tab: f32,
    pub button_large: f32,
    pub button_small: f32,
    pub group_label: f32,
    pub keytip: f32,
    pub menu: f32,
    pub glyph: f32,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(default)]
pub struct TitlebarFonts {
    pub title: f32,
    pub block: f32,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(default)]
pub struct PaneFonts {
    pub title: f32,
    pub body: f32,
}

impl Default for RibbonFonts {
    fn default() -> Self {
        Self {
            tab: 12.0,
            button_large: 12.0,
            button_small: 12.0,
            group_label: 12.0,
            keytip: 12.0,
            menu: 12.0,
            glyph: 12.0,
        }
    }
}

impl Default for TitlebarFonts {
    fn default() -> Self {
        Self { title: 12.0, block: 12.0 }
    }
}

impl Default for PaneFonts {
    fn default() -> Self {
        Self { title: 12.0, body: 12.0 }
    }
}

impl FontSizes {
    /// Load the font sizes: user file overrides the embedded copy; a
    /// broken file falls back to the defaults (all 12px).
    pub fn load() -> Self {
        let text = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .and_then(|h| std::fs::read_to_string(h.join(".config/explorer/fonts.yaml")).ok())
            .unwrap_or_else(|| include_str!("../assets/fonts.yaml").to_string());
        match serde_yaml::from_str::<FontSizes>(&text) {
            Ok(mut f) => {
                if f.family.is_empty() {
                    f.family = DEFAULT_FAMILIES.iter().map(|s| s.to_string()).collect();
                }
                // `base` seeds zero-valued fields (serde default is 0.0 for
                // missing scalars inside present sections)
                let base = if f.base > 0.0 { f.base } else { 12.0 };
                let fix = |v: &mut f32| {
                    if *v <= 0.0 {
                        *v = base;
                    }
                };
                fix(&mut f.ribbon.tab);
                fix(&mut f.ribbon.button_large);
                fix(&mut f.ribbon.button_small);
                fix(&mut f.ribbon.group_label);
                fix(&mut f.ribbon.keytip);
                fix(&mut f.ribbon.menu);
                fix(&mut f.ribbon.glyph);
                fix(&mut f.titlebar.title);
                fix(&mut f.titlebar.block);
                fix(&mut f.statusbar);
                fix(&mut f.pane.title);
                fix(&mut f.pane.body);
                f.base = base;
                f
            }
            Err(e) => {
                eprintln!("fonts.yaml: {e} — using 12px everywhere");
                FontSizes::load_default()
            }
        }
    }

    fn load_default() -> Self {
        FontSizes {
            base: 12.0,
            family: DEFAULT_FAMILIES.iter().map(|s| s.to_string()).collect(),
            ribbon: RibbonFonts::default(),
            titlebar: TitlebarFonts::default(),
            statusbar: 12.0,
            pane: PaneFonts::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shipped_fonts_yaml_is_all_12() {
        let f: super::FontSizes =
            serde_yaml::from_str(include_str!("../assets/fonts.yaml")).unwrap();
        let all = [
            f.base,
            f.ribbon.tab,
            f.ribbon.button_large,
            f.ribbon.button_small,
            f.ribbon.group_label,
            f.ribbon.keytip,
            f.ribbon.menu,
            f.ribbon.glyph,
            f.titlebar.title,
            f.titlebar.block,
            f.statusbar,
            f.pane.title,
            f.pane.body,
        ];
        assert!(all.iter().all(|&s| s == 12.0), "{all:?}");
    }
}

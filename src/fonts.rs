//! Font backend. One place decides which faces are registered, which
//! family is the UI default (the fonts.yaml fallback list), and the
//! font sizes (FontSizes, assets/fonts.yaml).
//!
//! Primary = Selawik (per fonts.yaml) — Microsoft's OPEN Segoe
//! alternative (SIL OFL, metric-compatible), used when embedded or
//! system-installed. Embedded only with `make build FEATURES=embed-selawik`
//! (assets/fonts/selawik.regular.ttf) — off by default. "Segoe UI" stays
//! in the fallback list for systems that have it installed. Per-glyph
//! fallback (CJK, …) is cosmic-text's system-font job.

use iced::Font;

/// Family name of the embedded face (Selawik: the open Segoe-metric font).
#[cfg(feature = "embed-selawik")]
pub const UI_FONT_NAME: &str = "Selawik";

/// The embedded face — only with the `embed-selawik` cargo feature
/// (off by default; Selawik is SIL OFL, free to ship).
#[cfg(feature = "embed-selawik")]
pub const EMBED_BYTES: &[u8] = include_bytes!("../assets/fonts/selawik.regular.ttf");

/// Shipped fallback list (fonts.yaml `family:`): Segoe UI first — it
/// resolves only when embedded (feature `embed-selawik`) or installed,
/// otherwise it is skipped. Tahoma/Arial are Wine's built-in fonts, so
/// a fresh wine prefix renders text with no font setup at all.
pub const DEFAULT_FAMILIES: &[&str] = &[
    "Selawik",
    "Segoe UI",
    "Tahoma",   // Wine built-in (fresh prefixes)
    "Arial",    // Windows + Wine built-in
    "Noto Sans",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans CJK TC",
    "Noto Sans CJK KR",
    "sans-serif",
];

/// Windows-only: `(registry value name, font file path)` pairs from
/// `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts`.
///
/// Why this exists: fontdb/cosmic-text discover fonts by scanning font
/// DIRECTORIES with plain file I/O (`%SYSTEMROOT%\Fonts`), never
/// through the OS font API. Wine serves its fonts (bundled + the
/// host's, via fontconfig) through gdi32 instead and leaves
/// `C:\Windows\Fonts` empty — so a fresh prefix has zero loadable
/// faces and renders no text. The registry lists every font as an
/// absolute path (wine: `Z:\usr\share\wine\fonts\msyh.ttf`, which
/// the PE binary's std::fs opens through the Z: drive mapping; real
/// Windows: bare filenames, resolved against `%SYSTEMROOT%\Fonts`).
#[cfg(not(unix))]
pub(crate) fn registry_fonts() -> Vec<(String, std::path::PathBuf)> {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
        RegQueryInfoKeyW,
    };

    let mut out = Vec::new();
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts\0"
        .encode_utf16()
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    // wine prefixes without the key (or non-Windows hosts) just fall through
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
        return out;
    }
    let mut values = 0u32;
    let mut max_name = 0u32; // WCHARs
    let mut max_data = 0u32; // bytes
    let info = unsafe {
        RegQueryInfoKeyW(
            key,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut values,
            &mut max_name,
            &mut max_data,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if info != 0 {
        unsafe { RegCloseKey(key) };
        return out;
    }
    let name_cap = max_name + 1;
    let data_cap = max_data + 2; // WCHAR terminator safety
    let mut name_buf = vec![0u16; name_cap as usize];
    let mut data_buf = vec![0u8; data_cap as usize];
    for i in 0..values {
        let mut name_len = name_cap;
        let mut data_len = data_cap;
        let mut typ = 0u32;
        let r = unsafe {
            RegEnumValueW(
                key,
                i,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut typ,
                data_buf.as_mut_ptr(),
                &mut data_len,
            )
        };
        if r != 0 || typ != REG_SZ {
            continue;
        }
        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        // data is little-endian UTF-16 (x86-64 windows hosts and wine alike)
        let units: Vec<u16> = data_buf[..data_len as usize]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let value = String::from_utf16_lossy(&units);
        let path = std::path::PathBuf::from(value.trim_end_matches('\0'));
        // bare filenames (real Windows) live in %SYSTEMROOT%\Fonts;
        // absolute paths (wine's Z:\...) are used as-is
        let path = if path.is_absolute() {
            path
        } else {
            std::env::var_os("SYSTEMROOT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "C:\\Windows".into())
                .join("Fonts")
                .join(path)
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if matches!(ext.as_deref(), Some("ttf" | "ttc" | "otf")) && path.is_file() {
            out.push((name, path));
        }
    }
    unsafe { RegCloseKey(key) };
    out
}

/// Windows-only: font files from the registry, picked for the shipped
/// fallback list (entries whose name mentions a fallback family first,
/// then a few small others), capped so startup stays cheap. Skipped
/// entirely when the direct font-directory scan found faces — that is
/// the real-Windows case, where this would only re-load what fontdb
/// already discovered.
#[cfg(not(unix))]
pub(crate) fn supplementary_font_paths() -> Vec<std::path::PathBuf> {
    const MAX_FILES: usize = 16;
    const MAX_FILE_BYTES: u64 = 24 * 1024 * 1024;
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    if db.faces().next().is_some() {
        return Vec::new();
    }
    let entries = registry_fonts();
    if std::env::var_os("EXPLORER_DEBUG").is_some() {
        eprintln!(
            "[fonts] font-dir scan found 0 faces; {} registry font entries",
            entries.len()
        );
    }
    let mut picked: Vec<&(String, std::path::PathBuf)> = Vec::new();
    // pass 1: families from the shipped fallback list, in list order
    // ("Noto Sans" alone matches hundreds of host variants — the cap
    // keeps startup cheap; Tahoma/Arial are the first real hits)
    'families: for fam in DEFAULT_FAMILIES {
        if fam.eq_ignore_ascii_case("sans-serif") {
            continue;
        }
        for e in &entries {
            if picked.len() >= MAX_FILES {
                break 'families;
            }
            if e.0.to_ascii_lowercase().contains(&fam.to_ascii_lowercase())
                && !picked.iter().any(|p| p.1 == e.1)
            {
                picked.push(e);
            }
        }
    }
    // pass 1b: Wine's built-in CJK-capable UI font (covers CJK file
    // names when the host has no small CJK font to offer)
    if picked.len() < MAX_FILES {
        for e in &entries {
            if picked.len() >= MAX_FILES {
                break;
            }
            if e.0.to_ascii_lowercase().contains("microsoft yahei")
                && !picked.iter().any(|p| p.1 == e.1)
            {
                picked.push(e);
            }
        }
    }
    // pass 2: fill up with the remaining small files (wine's bundled
    // set: marlett/webdings are useless for text but cheap to skip here —
    // the per-file size cap keeps the big CJK collections out)
    for e in &entries {
        if picked.len() >= MAX_FILES {
            break;
        }
        if picked.iter().any(|p| p.1 == e.1) {
            continue;
        }
        let small = std::fs::metadata(&e.1).map(|m| m.len() <= MAX_FILE_BYTES).unwrap_or(false);
        if small {
            picked.push(e);
        }
    }
    let paths: Vec<std::path::PathBuf> = picked.into_iter().map(|e| e.1.clone()).collect();
    if std::env::var_os("EXPLORER_DEBUG").is_some() {
        for p in &paths {
            eprintln!("[fonts]   + {}", p.display());
        }
    }
    paths
}

/// Supplementary fonts as bytes, for the renderer's `font::load` (iced
/// owns that font system; this is the only way in). Empty on unix.
pub fn supplementary_font_bytes() -> Vec<Vec<u8>> {
    #[cfg(not(unix))]
    {
        supplementary_font_paths()
            .iter()
            .filter_map(|p| std::fs::read(p).ok())
            .collect()
    }
    #[cfg(unix)]
    {
        Vec::new()
    }
}

/// Resolve the primary UI family from the configured fallback list:
/// the first family that is available wins — Segoe UI when embedded
/// (feature) or installed, otherwise the first system-installed entry;
/// "sans-serif" (or an exhausted list) defers to the system default.
/// Per-glyph fallback stays with cosmic-text's system fontdb.
pub fn resolve_primary(families: &[String]) -> Font {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    #[cfg(not(unix))]
    if db.faces().next().is_none() {
        // fresh wine prefix: the scan found nothing, fall back to the
        // registry-listed files so the fallback list can resolve
        for path in supplementary_font_paths() {
            let _ = db.load_font_file(&path);
        }
    }
    let installed = |name: &str| {
        db.faces().any(|f| {
            f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
        })
    };
    for name in families {
        if name.eq_ignore_ascii_case("sans-serif") {
            break; // system fallback: let cosmic-text pick
        }
        #[cfg(feature = "embed-selawik")]
        if name == UI_FONT_NAME {
            return Font::new(UI_FONT_NAME);
        }

        if installed(name) {
            // Font::with_name needs 'static: startup-only, leaked once
            return Font::new(name.clone().leak());
        }
    }
    Font::DEFAULT // SansSerif
}

/// The UI default font (all widgets that don't override it).
pub fn default_font() -> Font {
    resolve_primary(&FontSizes::load().family)
}

/// The resolved primary family, probed once (the ribbon's text
/// measurement calls this per glyph string — keep it cached).
pub fn primary() -> Font {
    static PRIMARY: std::sync::OnceLock<Font> = std::sync::OnceLock::new();
    *PRIMARY.get_or_init(default_font)
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

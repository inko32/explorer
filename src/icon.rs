//! Standalone icon module (D14): the ONLY seam through which icons render.
//!
//! `assets/icons.yaml` is the theme selector AND the internal-name
//! registry: `theme:` picks the active icon pack (`explorer` = the
//! embedded all-SVG pack, or a system theme name such as `breeze`);
//! `themes.*.icons` map the app's internal names to pack entries —
//! `svg: builtin` (skin-colored template, one file per icon in
//! assets/icons/svgs/), `file:` (reserved raster-asset slot), or
//! `icon:` (freedesktop icon-theme name, resolved by
//! src/icon_theme.rs with the Inherits chain, whatever the theme
//! ships — SVG or PNG). A user override at
//! ~/.config/explorer/icons.yaml replaces `theme:` and any entry.
//!
//! The active pack stands alone — there is NO fallback between packs:
//! an icon that is unavailable (no entry, theme lookup miss, missing
//! PNG asset or template) renders as a solid circle. Ribbon chrome that
//! is not part of any pack (the keytip chevron, the blank
//! placeholder) resolves to nothing here so the ribbon can fall back
//! to its own glyph set.
//!
//! Thumbnails will plug into [`for_entry`] later; nothing outside this
//! module decides how an entry looks.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use iced::Widget as _;
use iced::widget::{image, svg};
use iced::{Element, Length};

use crate::ui_common::skin::Skin;

#[derive(Debug, serde::Deserialize)]
struct IconDef {
    file: Option<String>,
    svg: Option<String>,
    /// System icon-theme name for `icon:` entries (freedesktop lookup).
    icon: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct ThemeFile {
    #[serde(default)]
    icons: HashMap<String, IconDef>,
}

#[derive(Debug, serde::Deserialize)]
struct IconsFile {
    /// Active theme at startup ("explorer" or a system theme name).
    #[serde(default = "default_theme_name")]
    theme: String,
    #[serde(default)]
    themes: HashMap<String, ThemeFile>,
}

fn default_theme_name() -> String {
    "explorer".to_string()
}

const EXPLORER: &str = "explorer";

struct Icons {
    /// `theme:` from icons.yaml (user override wins).
    default_theme: String,
    /// defs of the embedded explorer pack.
    explorer: HashMap<String, IconDef>,
    /// defs of the system themes (e.g. "breeze").
    system: HashMap<String, HashMap<String, IconDef>>,
}

static ICONS: OnceLock<Icons> = OnceLock::new();

/// Runtime theme override (the ribbon "Breeze icons" toggle); None =
/// icons.yaml `theme:`.
static ACTIVE: RwLock<Option<String>> = RwLock::new(None);

fn load() -> Icons {
    let mut file: IconsFile =
        serde_yaml::from_str(include_str!("../assets/icons.yaml")).expect("icons.yaml valid");

    // user override: same shape — `theme:` plus any theme's entries
    if let Some(p) = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/icons.yaml"))
        && let Ok(text) = std::fs::read_to_string(&p)
    {
        match serde_yaml::from_str::<IconsFile>(&text) {
            Ok(user) => {
                file.theme = user.theme;
                for (theme, defs) in user.themes {
                    file.themes.entry(theme).or_default().icons.extend(defs.icons);
                }
            }
            Err(e) => eprintln!("icons.yaml parse error ({}): {e}", p.display()),
        }
    }

    let explorer = file
        .themes
        .remove(EXPLORER)
        .map(|t| t.icons)
        .unwrap_or_default();
    let system = file
        .themes
        .into_iter()
        .map(|(name, t)| (name, t.icons))
        .collect();

    Icons {
        default_theme: file.theme,
        explorer,
        system,
    }
}

fn icons() -> &'static Icons {
    ICONS.get_or_init(load)
}

/// The theme in effect right now (the runtime override or the
/// icons.yaml default).
pub fn active_theme() -> String {
    ACTIVE
        .read()
        .ok()
        .and_then(|o| o.clone())
        .unwrap_or_else(|| icons().default_theme.clone())
}

/// Switch between the embedded explorer pack (off) and the system icon
/// theme (on, e.g. breeze). The system side is icons.yaml's `theme:`
/// when that already names one, otherwise `breeze`.
pub fn set_system(on: bool) {
    let target = if on {
        let d = icons().default_theme.clone();
        if d == EXPLORER { "breeze".to_string() } else { d }
    } else {
        EXPLORER.to_string()
    };
    if let Ok(mut a) = ACTIVE.write() {
        *a = Some(target);
    }
}

/// Whether the system icon theme is in effect (the ribbon toggle state).
pub fn system_active() -> bool {
    active_theme() != EXPLORER
}

/// A resolved icon at the handle level (for non-widget renderers, e.g.
/// the ribbon scene graph): raster PNG variant or vector generator.
#[derive(Clone)]
pub enum IconHandle {
    Png(iced::widget::image::Handle),
    Svg(svg::Handle),
}

/// Where an icon came from, before it is wrapped into a widget.
enum Source {
    Png(iced::widget::image::Handle),
    Svg(svg::Handle),
    /// The pack declares the icon but it cannot be produced (theme
    /// lookup miss, `file:` without a PNG asset, template missing) —
    /// the caller draws the solid-circle marker.
    Missing,
}

/// The shared resolution (see the module docs): the ACTIVE pack only —
/// the explorer pack is not consulted for a system theme unless its map
/// says so explicitly. `None` only when the name is unknown to the pack
/// (the ribbon then falls back to its own glyphs).
fn resolve(name: &str, size: u32, skin: &Skin) -> Option<Source> {
    let icons = icons();
    let active = active_theme();
    let def = if active == EXPLORER {
        icons.explorer.get(name)
    } else {
        icons.system.get(&active).and_then(|m| m.get(name))
    }?;
    if let Some(icon) = &def.icon {
        if let Some(path) = crate::icon_theme::lookup(&active, icon, size) {
            // path handles decode lazily at render time
            let src = if path.extension().is_some_and(|e| e == "png") {
                Source::Png(iced::widget::image::Handle::from_path(path))
            } else {
                Source::Svg(svg::Handle::from_path(path))
            };
            return Some(src);
        }
        return Some(Source::Missing);
    }
    if def.svg.is_some() || def.file.is_none() {
        return Some(match crate::icons::builtin_svg(name, skin) {
            Some(h) => Source::Svg(h),
            None => Source::Missing,
        });
    }
    Some(Source::Missing) // `file:` entry without a PNG asset
}

/// Solid circle drawn when the active pack cannot produce an icon.
fn missing_svg(skin: &Skin) -> svg::Handle {
    svg::Handle::from_memory(
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
             <circle cx='8' cy='8' r='7' fill='{}'/></svg>",
            crate::icons::hex(skin.subtle),
        )
        .into_bytes(),
    )
}

/// Handle for a named icon at the requested size. `None` means the name
/// is unknown to the active pack — callers with their own glyph set
/// (the ribbon chrome) fall back to it. A name the pack declares but
/// cannot produce resolves to the solid-circle marker.
pub fn handle(name: &str, size: u32, skin: &Skin) -> Option<IconHandle> {
    match resolve(name, size, skin)? {
        Source::Png(h) => Some(IconHandle::Png(h)),
        Source::Svg(h) => Some(IconHandle::Svg(h)),
        Source::Missing => Some(IconHandle::Svg(missing_svg(skin))),
    }
}

/// Render a named icon at the given size. Unknown and unavailable names
/// render the solid-circle marker.
pub fn view<Message: 'static>(name: &str, size: f32, skin: &Skin) -> Element<'static, Message> {
    view_opacity(name, size, skin, 1.0)
}

/// `view` at a custom opacity — the drag badge draws its icons
/// translucent together with the badge background.
pub fn view_opacity<Message: 'static>(
    name: &str,
    size: f32,
    skin: &Skin,
    opacity: f32,
) -> Element<'static, Message> {
    let want = size.round().max(1.0) as u32;
    let handle = match resolve(name, want, skin) {
        Some(Source::Png(h)) => IconHandle::Png(h),
        Some(Source::Svg(h)) => IconHandle::Svg(h),
        Some(Source::Missing) | None => IconHandle::Svg(missing_svg(skin)),
    };
    match handle {
        IconHandle::Png(handle) => image(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .opacity(opacity)
            .boxed(),
        IconHandle::Svg(handle) => svg(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .opacity(opacity)
            .boxed(),
    }
}

/// The internal icon name for an entry: directories get the folder
/// glyph, regular files map by extension.
fn name_for(is_dir: bool, name: &str) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, x)| x.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "png",
        "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" => "file_image",
        "txt" | "md" | "log" => "text",
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "zst" => "archive",
        "mp3" | "flac" | "ogg" | "wav" => "audio",
        "mp4" | "mkv" | "mov" | "webm" | "avi" => "video",
        "exe" | "msi" | "appimage" => "exe",
        "iso" | "img" => "disc",
        _ => "file",
    }
}

/// Icon for a filesystem entry — extension-based names today; thumbnails
/// plug in here later (check the thumbnail cache first, fall back to this).
pub fn for_entry<Message: 'static>(e: &crate::fs::Entry, size: f32, skin: &Skin) -> Element<'static, Message> {
    view(name_for(e.is_dir(), &e.name), size, skin)
}

/// [`for_entry`] at a custom opacity (the drag badge).
pub fn for_name_opacity<Message: 'static>(
    is_dir: bool,
    name: &str,
    size: f32,
    skin: &Skin,
    opacity: f32,
) -> Element<'static, Message> {
    view_opacity(name_for(is_dir, name), size, skin, opacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_loads_both_themes() {
        let icons = icons();
        // explorer pack: all-SVG template map
        assert!(icons.explorer.contains_key("folder"));
        // system theme map is present for the breeze switch
        assert!(icons.system.contains_key("breeze"));
        assert!(icons.system["breeze"].contains_key("paste"));
    }

    #[test]
    fn resolution_switches_with_the_theme() {
        let skin = crate::ui_common::skin::light();
        // embedded pack serves SVG templates
        assert!(matches!(handle("folder", 16, &skin), Some(IconHandle::Svg(_))));
        // unknown names miss entirely (the ribbon keeps its own glyphs)
        assert!(handle("definitely_unknown_icon", 16, &skin).is_none());

        set_system(true);
        assert!(system_active());
        // with breeze active the mapped names resolve: a system-theme
        // file when the theme has one, the circle marker otherwise —
        // never the explorer pack (no fallback between packs)
        assert!(handle("folder", 16, &skin).is_some());
        assert!(handle("paste", 32, &skin).is_some());
        // and it switches back
        set_system(false);
        assert!(!system_active());
        assert!(matches!(handle("folder", 16, &skin), Some(IconHandle::Svg(_))));
    }

    #[test]
    fn missing_marker_is_a_solid_circle() {
        let skin = crate::ui_common::skin::light();
        let handle = missing_svg(&skin);
        let iced::advanced::svg::Data::Bytes(bytes) = handle.data() else {
            panic!("expected in-memory svg");
        };
        let text = std::str::from_utf8(bytes).unwrap();
        assert!(text.contains("<circle"), "the unavailable marker is a circle: {text}");
    }
}

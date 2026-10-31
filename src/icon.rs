//! Standalone icon module (D14): the ONLY seam through which icons render.
//!
//! `assets/icons.yaml` is the theme selector AND the internal-name
//! registry: `theme:` picks the active icon pack (`explorer` = the
//! embedded Win8 PNG pack, or a system theme name such as `breeze`);
//! `themes.*.icons` map the app's internal names to pack entries —
//! `file:` (embedded PNG, breeze-style `<size>/<name>.png` layout under
//! assets/icons/), `svg: builtin` (skin-colored template, one file per
//! icon in assets/svgs/), or `icon:` (freedesktop icon-theme name,
//! resolved by src/icon_theme.rs with the Inherits chain). A user
//! override at ~/.config/explorer/icons.yaml replaces `theme:` and any
//! entry.
//!
//! The active pack stands alone — there is NO fallback between packs:
//! an icon that is unavailable (no entry, theme lookup miss, missing
//! PNG asset or template) renders as a solid circle. Ribbon chrome that
//! is not part of any pack (the orb, keytip chevrons) resolves to
//! nothing here so the ribbon can fall back to its own glyph set.
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
    /// explorer `file:` name → sorted (size, decoded handle) - handles
    /// are cached: decoding 245 PNGs on every view rebuild made dropdown
    /// menus blink.
    pngs: HashMap<String, Vec<(u32, iced::widget::image::Handle)>>,
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

    // preload png variants (decode once, reuse the handles forever)
    let mut pngs: HashMap<String, Vec<(u32, iced::widget::image::Handle)>> = HashMap::new();
    for (name, def) in &explorer {
        let Some(file) = &def.file else { continue };
        let mut variants = Vec::new();
        for size in [16u32, 20, 24, 32, 40, 48, 64] {
            if let Some(bytes) = asset_bytes(file, size) {
                variants.push((size, iced::widget::image::Handle::from_bytes(bytes)));
            }
        }
        if !variants.is_empty() {
            pngs.insert(name.clone(), variants);
        }
    }

    Icons {
        default_theme: file.theme,
        explorer,
        system,
        pngs,
    }
}

macro_rules! embedded_pngs {
    ($($name:literal),* $(,)?) => {
        fn asset_bytes(file: &str, size: u32) -> Option<&'static [u8]> {
            // breeze-style layout: assets/icons/<size>/<file>.png
            let path = format!("{size}/{file}.png");
            Some(match path.as_str() {
                $( $name => include_bytes!(concat!("../assets/icons/", $name)), )*
                _ => return None,
            })
        }
    };
}

// Keep in sync with files present in assets/icons/.
embedded_pngs!(
    "16/audio.png",
    "16/computer.png",
    "16/copy.png",
    "16/copy_to.png",
    "16/cut.png",
    "16/delete.png",
    "16/disc.png",
    "16/easy_access.png",
    "16/edit.png",
    "16/email.png",
    "16/exe.png",
    "16/file_image.png",
    "16/file.png",
    "16/folder_new.png",
    "16/folder.png",
    "16/history.png",
    "16/invert_sel.png",
    "16/libraries.png",
    "16/move_to.png",
    "16/new_item.png",
    "16/paste.png",
    "16/paste_shortcut.png",
    "16/png.png",
    "16/properties.png",
    "16/select_all.png",
    "16/select_none.png",
    "16/share.png",
    "16/star.png",
    "16/text.png",
    "16/video.png",
    "16/view_details.png",
    "16/view_large.png",
    "16/view_list.png",
    "16/view_tiles.png",
    "16/zip.png",
    "20/audio.png",
    "20/computer.png",
    "20/copy.png",
    "20/copy_to.png",
    "20/cut.png",
    "20/delete.png",
    "20/disc.png",
    "20/easy_access.png",
    "20/edit.png",
    "20/email.png",
    "20/exe.png",
    "20/file_image.png",
    "20/file.png",
    "20/folder_new.png",
    "20/folder.png",
    "20/history.png",
    "20/invert_sel.png",
    "20/libraries.png",
    "20/move_to.png",
    "20/new_item.png",
    "20/paste.png",
    "20/paste_shortcut.png",
    "20/png.png",
    "20/properties.png",
    "20/select_all.png",
    "20/select_none.png",
    "20/share.png",
    "20/star.png",
    "20/text.png",
    "20/video.png",
    "20/view_details.png",
    "20/view_large.png",
    "20/view_list.png",
    "20/view_tiles.png",
    "20/zip.png",
    "24/audio.png",
    "24/computer.png",
    "24/copy.png",
    "24/copy_to.png",
    "24/cut.png",
    "24/delete.png",
    "24/disc.png",
    "24/easy_access.png",
    "24/edit.png",
    "24/email.png",
    "24/exe.png",
    "24/file_image.png",
    "24/file.png",
    "24/folder_new.png",
    "24/folder.png",
    "24/history.png",
    "24/invert_sel.png",
    "24/libraries.png",
    "24/move_to.png",
    "24/new_item.png",
    "24/paste.png",
    "24/paste_shortcut.png",
    "24/png.png",
    "24/properties.png",
    "24/select_all.png",
    "24/select_none.png",
    "24/share.png",
    "24/star.png",
    "24/text.png",
    "24/video.png",
    "24/view_details.png",
    "24/view_large.png",
    "24/view_list.png",
    "24/view_tiles.png",
    "24/zip.png",
    "32/audio.png",
    "32/computer.png",
    "32/copy.png",
    "32/copy_to.png",
    "32/cut.png",
    "32/delete.png",
    "32/disc.png",
    "32/easy_access.png",
    "32/edit.png",
    "32/email.png",
    "32/exe.png",
    "32/file_image.png",
    "32/file.png",
    "32/folder_new.png",
    "32/folder.png",
    "32/history.png",
    "32/invert_sel.png",
    "32/libraries.png",
    "32/move_to.png",
    "32/new_item.png",
    "32/paste.png",
    "32/paste_shortcut.png",
    "32/png.png",
    "32/properties.png",
    "32/select_all.png",
    "32/select_none.png",
    "32/share.png",
    "32/star.png",
    "32/text.png",
    "32/video.png",
    "32/view_details.png",
    "32/view_large.png",
    "32/view_list.png",
    "32/view_tiles.png",
    "32/zip.png",
    "40/audio.png",
    "40/computer.png",
    "40/copy.png",
    "40/copy_to.png",
    "40/cut.png",
    "40/delete.png",
    "40/disc.png",
    "40/easy_access.png",
    "40/edit.png",
    "40/email.png",
    "40/exe.png",
    "40/file_image.png",
    "40/file.png",
    "40/folder_new.png",
    "40/folder.png",
    "40/history.png",
    "40/invert_sel.png",
    "40/libraries.png",
    "40/move_to.png",
    "40/new_item.png",
    "40/paste.png",
    "40/paste_shortcut.png",
    "40/png.png",
    "40/properties.png",
    "40/select_all.png",
    "40/select_none.png",
    "40/share.png",
    "40/star.png",
    "40/text.png",
    "40/video.png",
    "40/view_details.png",
    "40/view_large.png",
    "40/view_list.png",
    "40/view_tiles.png",
    "40/zip.png",
    "48/audio.png",
    "48/computer.png",
    "48/copy.png",
    "48/copy_to.png",
    "48/cut.png",
    "48/delete.png",
    "48/disc.png",
    "48/easy_access.png",
    "48/edit.png",
    "48/email.png",
    "48/exe.png",
    "48/file_image.png",
    "48/file.png",
    "48/folder_new.png",
    "48/folder.png",
    "48/history.png",
    "48/invert_sel.png",
    "48/libraries.png",
    "48/move_to.png",
    "48/new_item.png",
    "48/paste.png",
    "48/paste_shortcut.png",
    "48/png.png",
    "48/properties.png",
    "48/select_all.png",
    "48/select_none.png",
    "48/share.png",
    "48/star.png",
    "48/text.png",
    "48/video.png",
    "48/view_details.png",
    "48/view_large.png",
    "48/view_list.png",
    "48/view_tiles.png",
    "48/zip.png",
    "64/audio.png",
    "64/computer.png",
    "64/copy.png",
    "64/copy_to.png",
    "64/cut.png",
    "64/delete.png",
    "64/disc.png",
    "64/easy_access.png",
    "64/edit.png",
    "64/email.png",
    "64/exe.png",
    "64/file_image.png",
    "64/file.png",
    "64/folder_new.png",
    "64/folder.png",
    "64/history.png",
    "64/invert_sel.png",
    "64/libraries.png",
    "64/move_to.png",
    "64/new_item.png",
    "64/paste.png",
    "64/paste_shortcut.png",
    "64/png.png",
    "64/properties.png",
    "64/select_all.png",
    "64/select_none.png",
    "64/share.png",
    "64/star.png",
    "64/text.png",
    "64/video.png",
    "64/view_details.png",
    "64/view_large.png",
    "64/view_list.png",
    "64/view_tiles.png",
    "64/zip.png",
);

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
    if let Some(variants) = icons.pngs.get(name) {
        // nearest variant >= requested, else the largest available
        let handle = variants
            .iter()
            .find(|(s, _)| *s >= size)
            .or(variants.last())
            .map(|(_, h)| h.clone())
            .unwrap();
        return Some(Source::Png(handle));
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
        // explorer pack: PNG variants decoded from the breeze-style dirs
        assert!(icons.pngs.contains_key("folder"));
        assert_eq!(icons.pngs["folder"].len(), 7, "16..64 variants");
        // system theme map is present for the breeze switch
        assert!(icons.system.contains_key("breeze"));
        assert!(icons.system["breeze"].contains_key("paste"));
    }

    #[test]
    fn resolution_switches_with_the_theme() {
        let skin = crate::ui_common::skin::light();
        // embedded pack serves PNG handles
        assert!(matches!(handle("folder", 16, &skin), Some(IconHandle::Png(_))));
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
        assert!(matches!(handle("folder", 16, &skin), Some(IconHandle::Png(_))));
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

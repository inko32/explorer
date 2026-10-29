//! Standalone icon module (D14): the ONLY seam through which icons render.
//!
//! Resolution order per icon name:
//!   1. user override  ~/.config/explorer/icons.yaml
//!   2. embedded       assets/icons.yaml  → `file`: PNG under assets/icons/
//!      (nearest size variant ≥ requested) or `svg`: builtin theme-colored
//!      generator (src/icons.rs)
//!
//! Thumbnails will plug into [`for_entry`] later; nothing outside this
//! module decides how an entry looks.

use std::collections::HashMap;
use std::sync::OnceLock;

use iced::Widget as _;
use iced::widget::{image, svg, Space};
use iced::{Element, Length};

use crate::ui_common::skin::Skin;

#[derive(Debug, serde::Deserialize)]
struct IconDef {
    file: Option<String>,
    svg: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct IconsFile {
    #[serde(default)]
    icons: HashMap<String, IconDef>,
}

struct Icons {
    defs: HashMap<String, IconDef>,
    /// name → sorted (size, decoded handle) - handles are cached: decoding
    /// 245 PNGs on every view rebuild made dropdown menus blink.
    pngs: HashMap<String, Vec<(u32, iced::widget::image::Handle)>>,
}

static ICONS: OnceLock<Icons> = OnceLock::new();

fn load() -> Icons {
    let mut defs: HashMap<String, IconDef> = HashMap::new();

    // embedded registry
    let embedded: IconsFile =
        serde_yaml::from_str(include_str!("../assets/icons.yaml")).expect("icons.yaml valid");
    defs.extend(embedded.icons);

    // user override
    if let Some(p) = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/icons.yaml"))
        && let Ok(text) = std::fs::read_to_string(&p)
    {
        match serde_yaml::from_str::<IconsFile>(&text) {
            Ok(user) => defs.extend(user.icons),
            Err(e) => eprintln!("icons.yaml parse error ({}): {e}", p.display()),
        }
    }

    // preload png variants (decode once, reuse the handles forever)
    let mut pngs: HashMap<String, Vec<(u32, iced::widget::image::Handle)>> = HashMap::new();
    for (name, def) in &defs {
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

    Icons { defs, pngs }
}

macro_rules! embedded_pngs {
    ($($name:literal),* $(,)?) => {
        fn asset_bytes(file: &str, size: u32) -> Option<&'static [u8]> {
            let path = format!("{file}-{size}.png");
            Some(match path.as_str() {
                $( $name => include_bytes!(concat!("../assets/icons/", $name)), )*
                _ => return None,
            })
        }
    };
}

// Keep in sync with files present in assets/icons/.
embedded_pngs!(
    "audio-16.png",
    "audio-20.png",
    "audio-24.png",
    "audio-32.png",
    "audio-40.png",
    "audio-48.png",
    "audio-64.png",
    "computer-16.png",
    "computer-20.png",
    "computer-24.png",
    "computer-32.png",
    "computer-40.png",
    "computer-48.png",
    "computer-64.png",
    "copy-16.png",
    "copy-20.png",
    "copy-24.png",
    "copy-32.png",
    "copy-40.png",
    "copy-48.png",
    "copy-64.png",
    "copy_to-16.png",
    "copy_to-20.png",
    "copy_to-24.png",
    "copy_to-32.png",
    "copy_to-40.png",
    "copy_to-48.png",
    "copy_to-64.png",
    "cut-16.png",
    "cut-20.png",
    "cut-24.png",
    "cut-32.png",
    "cut-40.png",
    "cut-48.png",
    "cut-64.png",
    "delete-16.png",
    "delete-20.png",
    "delete-24.png",
    "delete-32.png",
    "delete-40.png",
    "delete-48.png",
    "delete-64.png",
    "disc-16.png",
    "disc-20.png",
    "disc-24.png",
    "disc-32.png",
    "disc-40.png",
    "disc-48.png",
    "disc-64.png",
    "easy_access-16.png",
    "easy_access-20.png",
    "easy_access-24.png",
    "easy_access-32.png",
    "easy_access-40.png",
    "easy_access-48.png",
    "easy_access-64.png",
    "edit-16.png",
    "edit-20.png",
    "edit-24.png",
    "edit-32.png",
    "edit-40.png",
    "edit-48.png",
    "edit-64.png",
    "email-16.png",
    "email-20.png",
    "email-24.png",
    "email-32.png",
    "email-40.png",
    "email-48.png",
    "email-64.png",
    "exe-16.png",
    "exe-20.png",
    "exe-24.png",
    "exe-32.png",
    "exe-40.png",
    "exe-48.png",
    "exe-64.png",
    "file-16.png",
    "file-20.png",
    "file-24.png",
    "file-32.png",
    "file-40.png",
    "file-48.png",
    "file-64.png",
    "file_image-16.png",
    "file_image-20.png",
    "file_image-24.png",
    "file_image-32.png",
    "file_image-40.png",
    "file_image-48.png",
    "file_image-64.png",
    "folder-16.png",
    "folder-20.png",
    "folder-24.png",
    "folder-32.png",
    "folder-40.png",
    "folder-48.png",
    "folder-64.png",
    "folder_new-16.png",
    "folder_new-20.png",
    "folder_new-24.png",
    "folder_new-32.png",
    "folder_new-40.png",
    "folder_new-48.png",
    "folder_new-64.png",
    "history-16.png",
    "history-20.png",
    "history-24.png",
    "history-32.png",
    "history-40.png",
    "history-48.png",
    "history-64.png",
    "invert_sel-16.png",
    "invert_sel-20.png",
    "invert_sel-24.png",
    "invert_sel-32.png",
    "invert_sel-40.png",
    "invert_sel-48.png",
    "invert_sel-64.png",
    "libraries-16.png",
    "libraries-20.png",
    "libraries-24.png",
    "libraries-32.png",
    "libraries-40.png",
    "libraries-48.png",
    "libraries-64.png",
    "move_to-16.png",
    "move_to-20.png",
    "move_to-24.png",
    "move_to-32.png",
    "move_to-40.png",
    "move_to-48.png",
    "move_to-64.png",
    "new_item-16.png",
    "new_item-20.png",
    "new_item-24.png",
    "new_item-32.png",
    "new_item-40.png",
    "new_item-48.png",
    "new_item-64.png",
    "paste-16.png",
    "paste-20.png",
    "paste-24.png",
    "paste-32.png",
    "paste-40.png",
    "paste-48.png",
    "paste-64.png",
    "paste_shortcut-16.png",
    "paste_shortcut-20.png",
    "paste_shortcut-24.png",
    "paste_shortcut-32.png",
    "paste_shortcut-40.png",
    "paste_shortcut-48.png",
    "paste_shortcut-64.png",
    "png-16.png",
    "png-20.png",
    "png-24.png",
    "png-32.png",
    "png-40.png",
    "png-48.png",
    "png-64.png",
    "properties-16.png",
    "properties-20.png",
    "properties-24.png",
    "properties-32.png",
    "properties-40.png",
    "properties-48.png",
    "properties-64.png",
    "select_all-16.png",
    "select_all-20.png",
    "select_all-24.png",
    "select_all-32.png",
    "select_all-40.png",
    "select_all-48.png",
    "select_all-64.png",
    "select_none-16.png",
    "select_none-20.png",
    "select_none-24.png",
    "select_none-32.png",
    "select_none-40.png",
    "select_none-48.png",
    "select_none-64.png",
    "share-16.png",
    "share-20.png",
    "share-24.png",
    "share-32.png",
    "share-40.png",
    "share-48.png",
    "share-64.png",
    "star-16.png",
    "star-20.png",
    "star-24.png",
    "star-32.png",
    "star-40.png",
    "star-48.png",
    "star-64.png",
    "text-16.png",
    "text-20.png",
    "text-24.png",
    "text-32.png",
    "text-40.png",
    "text-48.png",
    "text-64.png",
    "video-16.png",
    "video-20.png",
    "video-24.png",
    "video-32.png",
    "video-40.png",
    "video-48.png",
    "video-64.png",
    "view_details-16.png",
    "view_details-20.png",
    "view_details-24.png",
    "view_details-32.png",
    "view_details-40.png",
    "view_details-48.png",
    "view_details-64.png",
    "view_large-16.png",
    "view_large-20.png",
    "view_large-24.png",
    "view_large-32.png",
    "view_large-40.png",
    "view_large-48.png",
    "view_large-64.png",
    "view_list-16.png",
    "view_list-20.png",
    "view_list-24.png",
    "view_list-32.png",
    "view_list-40.png",
    "view_list-48.png",
    "view_list-64.png",
    "view_tiles-16.png",
    "view_tiles-20.png",
    "view_tiles-24.png",
    "view_tiles-32.png",
    "view_tiles-40.png",
    "view_tiles-48.png",
    "view_tiles-64.png",
    "zip-16.png",
    "zip-20.png",
    "zip-24.png",
    "zip-32.png",
    "zip-40.png",
    "zip-48.png",
    "zip-64.png",
);

fn icons() -> &'static Icons {
    ICONS.get_or_init(load)
}

/// A resolved icon at the handle level (for non-widget renderers, e.g.
/// the ribbon scene graph): raster PNG variant or vector generator.
#[derive(Clone)]
pub enum IconHandle {
    Png(iced::widget::image::Handle),
    Svg(svg::Handle),
}

/// Handle for a named icon at the requested size, or `None` when the name
/// is unknown (callers may fall back to their own glyph set). Same
/// resolution order as [`view`] but without wrapping into an Element.
pub fn handle(name: &str, size: u32, skin: &Skin) -> Option<IconHandle> {
    let icons = icons();
    if let Some(variants) = icons.pngs.get(name) {
        // nearest variant >= requested, else the largest available
        let handle = variants
            .iter()
            .find(|(s, _)| *s >= size)
            .or(variants.last())
            .map(|(_, h)| h.clone())
            .unwrap();
        return Some(IconHandle::Png(handle));
    }
    if let Some(def) = icons.defs.get(name) {
        if def.svg.is_some() || def.file.is_none() {
            return Some(IconHandle::Svg(crate::icons::builtin_svg(name, skin)));
        }
        return None; // registered as file but no PNG asset is present
    }
    // not in the registry — builtin generators may still know the name
    if crate::icons::has_builtin(name) {
        return Some(IconHandle::Svg(crate::icons::builtin_svg(name, skin)));
    }
    None
}

/// Render a named icon at the given size.
pub fn view<Message: 'static>(name: &str, size: f32, skin: &Skin) -> Element<'static, Message> {
    let icons = icons();
    if let Some(variants) = icons.pngs.get(name) {
        let want = size.round().max(1.0) as u32;
        // nearest variant >= requested, else the largest available
        let handle = variants
            .iter()
            .find(|(s, _)| *s >= want)
            .or(variants.last())
            .map(|(_, h)| h.clone())
            .unwrap();
        return image(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .boxed();
    }
    if let Some(def) = icons.defs.get(name) {
        if def.svg.is_some() || def.file.is_none() {
            return svg(crate::icons::builtin_svg(name, skin))
                .width(Length::Fixed(size))
                .height(Length::Fixed(size))
                .boxed();
        }
    } else {
        // not in the registry — builtin SVGs may still serve it
        // (unknown names render the placeholder file glyph)
        return svg(crate::icons::builtin_svg(name, skin))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .boxed();
    }
    // unknown name: blank
    Space::new().width(Length::Fixed(size)).height(Length::Fixed(size)).boxed()
}

/// Icon for a filesystem entry — extension-based names today; thumbnails
/// plug in here later (check the thumbnail cache first, fall back to this).
pub fn for_entry<Message: 'static>(e: &crate::fs::Entry, size: f32, skin: &Skin) -> Element<'static, Message> {
    let name = if e.is_dir() {
        "folder"
    } else {
        let ext = e
            .name
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
    };
    view(name, size, skin)
}

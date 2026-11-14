//! Builtin theme-colored SVG icon templates — the `svg: builtin` entries
//! of the explorer pack in icons.yaml. Every template is a file under
//! `assets/icons/svgs/<name>.svg` holding `{token}` placeholders that are
//! substituted from the active skin at render time:
//!
//!   {fg}          icon glyph color (skin.text)
//!   {bg}          page/screen fill  (skin.window_bg)
//!   {subtle}      secondary strokes (skin.subtle)
//!   {folder}      folder fill       (skin.folder)
//!   {folder_edge} folder outline    (skin.folder_edge)
//!   {hover}       screen content    (skin.hover)
//!   {title}       caption glyph on the title bar (skin.title_text)
//!
//! Design colors that must not follow the skin (the manila paste board,
//! the green check, the white caption glyphs drawn on the dark title
//! bar) stay as literal hex in the templates. A name with no template
//! resolves to nothing — the icon module draws its unavailable marker.
//! Swap a template file to change the glyph; add one to grow the set.

use iced::Color;
use iced::widget::svg;

use crate::ui_common::skin::Skin;

macro_rules! svg_templates {
    ($($name:literal => $file:literal),* $(,)?) => {
        fn template(name: &str) -> Option<&'static str> {
            Some(match name {
                $( $name => include_str!(concat!("../assets/icons/svgs/", $file)), )*
                _ => return None,
            })
        }
    };
}

// Keep in sync with files present in assets/svgs/.
svg_templates!(
    "home" => "home.svg",
    "desktop" => "computer.svg",
    "documents" => "documents.svg",
    "downloads" => "downloads.svg",
    "pictures" => "pictures.svg",
    "music" => "music.svg",
    "videos" => "videos.svg",
    "recent" => "recent.svg",
    "network" => "network.svg",
    "drive" => "drive.svg",
    "back" => "back.svg",
    "forward" => "forward.svg",
    "up" => "up.svg",
    "refresh" => "refresh.svg",
    "search" => "search.svg",
    "chevron_down" => "chevron_down.svg",
    "chevron_right" => "chevron_right.svg",
    "nav_pane" => "nav_pane.svg",
    "preview_pane" => "preview_pane.svg",
    "details_pane" => "details_pane.svg",
    "sort_by" => "sort_by.svg",
    "undo" => "undo.svg",
    "redo" => "redo.svg",
    "help" => "help.svg",
    "close" => "close.svg",
    "gear" => "gear.svg",
    "open" => "open.svg",
    "copy_path" => "copy_path.svg",
    "rename" => "rename.svg",
    // ribbon command icons (colorful; page/outline shapes follow the
    // skin via {bg}/{fg}, accents stay literal design colors)
    "new" => "new.svg",
    "save" => "save.svg",
    "print" => "print.svg",
    "exit" => "exit.svg",
    "find" => "find.svg",
    "pin" => "pin.svg",
    "paste_special" => "paste_special.svg",
    "terminal" => "terminal.svg",
    "extract" => "extract.svg",
    // clipboard / organize commands
    "paste" => "paste.svg",
    "cut" => "cut.svg",
    "copy" => "copy.svg",
    "paste_shortcut" => "paste_shortcut.svg",
    "move_to" => "move_to.svg",
    "copy_to" => "copy_to.svg",
    "delete" => "delete.svg",
    "new_folder" => "new_folder.svg",
    "new_item" => "new_item.svg",
    "easy_access" => "easy_access.svg",
    "properties" => "properties.svg",
    "edit" => "edit.svg",
    "history" => "history.svg",
    "select_all" => "select_all.svg",
    "select_none" => "select_none.svg",
    "invert_sel" => "invert_sel.svg",
    "share" => "share.svg",
    "email" => "email.svg",
    // view modes
    "view_details" => "view_details.svg",
    "view_large" => "view_large.svg",
    "view_list" => "view_list.svg",
    "view_tiles" => "view_tiles.svg",
    // favorites / This PC
    "star" => "star.svg",
    "computer" => "computer.svg",
    "libraries" => "libraries.svg",
    // items (mime)
    "folder" => "folder.svg",
    "file" => "file.svg",
    "file_image" => "file_image.svg",
    "text" => "text.svg",
    "zip" => "zip.svg",
    "audio" => "audio.svg",
    "video" => "video.svg",
    "exe" => "exe.svg",
    "disc" => "disc.svg",
    "png" => "png.svg",
);

/// Render a builtin template with the skin colors baked in; `None` when
/// no template exists for `name` (the caller draws the unavailable
/// marker).
pub fn builtin_svg(name: &str, s: &Skin) -> Option<svg::Handle> {
    let tpl = template(name)?;
    Some(svg::Handle::from_memory(render(tpl, s).into_bytes()))
}

/// The skin-colored SVG source of a builtin icon — for off-window
/// rasterizers (the Wayland drag-image surface) that cannot consume an
/// iced `svg::Handle`.
pub fn builtin_svg_xml(name: &str, s: &Skin) -> Option<String> {
    Some(render(template(name)?, s))
}

fn render(tpl: &str, s: &Skin) -> String {
    // {folder_edge} must be substituted before its {folder} prefix
    tpl.replace("{folder_edge}", &hex(s.folder_edge))
        .replace("{folder}", &hex(s.folder))
        .replace("{fg}", &hex(s.text))
        .replace("{bg}", &hex(s.window_bg))
        .replace("{subtle}", &hex(s.subtle))
        .replace("{hover}", &hex(s.hover))
        .replace("{title}", &hex(s.title_text))
}

pub(crate) fn hex(c: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manifest_entry_resolves_and_substitutes() {
        let skin = crate::ui_common::skin::light();
        for name in [
            "home", "desktop", "documents", "downloads", "pictures", "music", "videos", "recent",
            "network", "drive", "back", "forward", "up", "refresh", "search", "chevron_down",
            "chevron_right", "nav_pane", "preview_pane", "details_pane", "sort_by", "undo",
            "redo", "help", "close", "gear", "open", "copy_path", "rename",
            "new", "save", "print", "exit", "find", "pin", "paste_special",
            "terminal", "extract",
            "paste", "cut", "copy", "paste_shortcut", "move_to", "copy_to",
            "delete", "new_folder", "new_item", "easy_access", "properties",
            "edit", "history", "select_all", "select_none", "invert_sel",
            "share", "email",
            "view_details", "view_large", "view_list", "view_tiles",
            "star", "computer", "libraries",
            "folder", "file", "file_image", "text", "zip", "audio", "video",
            "exe", "disc", "png",
        ] {
            let handle = builtin_svg(name, &skin)
                .unwrap_or_else(|| panic!("template {name} must exist"));
            let iced::advanced::svg::Data::Bytes(bytes) = handle.data() else {
                panic!("template {name} must render in memory");
            };
            let text = std::str::from_utf8(bytes).expect("utf-8 svg");
            // every placeholder replaced: no raw skin color remains unknown
            assert!(!text.contains("{"), "template {name} has an unreplaced token");
            assert!(text.starts_with("<svg"), "template {name} is an svg document");
            // must parse with the same pipeline the renderer uses
            let parsed = resvg::usvg::Tree::from_str(
                text,
                &resvg::usvg::Options::default(),
            );
            assert!(parsed.is_ok(), "template {name} must parse: {:?}", parsed.err());
        }
        // unknown names have no template
        assert!(builtin_svg("definitely_unknown", &skin).is_none());
    }
}

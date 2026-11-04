//! Builtin theme-colored SVG icon templates — the `svg: builtin` entries
//! of the explorer pack in icons.yaml. Every template is a file under
//! `assets/svgs/<name>.svg` holding `{token}` placeholders that are
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
                $( $name => include_str!(concat!("../assets/svgs/", $file)), )*
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
);

/// Render a builtin template with the skin colors baked in; `None` when
/// no template exists for `name` (the caller draws the unavailable
/// marker).
pub fn builtin_svg(name: &str, s: &Skin) -> Option<svg::Handle> {
    let tpl = template(name)?;
    Some(svg::Handle::from_memory(render(tpl, s).into_bytes()))
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

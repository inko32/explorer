//! Side panes right of the folder view: the Preview pane and the
//! Details pane (ribbon View → Panes). Mutually exclusive: toggling one
//! hides the other, toggling the active one hides it.
//!
//! Content is deliberately simple today and extended here later:
//! Preview = text files (first bytes) / images / a "no preview" note;
//! Details = icon, file name, size.
//!
//! The preview payload is loaded by the MODEL on selection change
//! (`Explorer::sync_pane`) and cached — view() never touches the disk.

use std::path::Path;

use iced::widget::{column, container, scrollable, text};
use iced::{Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{bg, skin};

/// Default pane width (px); the splitter between the folder view and
/// the pane resizes it (`Explorer::pane_width`).
pub const DEFAULT_WIDTH: f32 = 240.0;
/// Drag clamp: never narrower than this, never wider than 60% of the
/// content width.
pub const MIN_WIDTH: f32 = 160.0;

/// The configured width clamped to the window.
pub fn clamped(w: f32, content_w: f32) -> f32 {
    w.clamp(MIN_WIDTH, (content_w * 0.6).max(MIN_WIDTH))
}

/// Which side pane is shown (None = neither; they are exclusive).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    None,
    Preview,
    Details,
}

/// Cached preview payload for the selected entry.
pub enum Content {
    /// No selection / unsupported type / read error.
    None,
    Text(String),
    Image(iced::widget::image::Handle),
}

/// Extensions previewed as text / images (lowercase).
const TEXT_EXTS: &[&str] = &[
    "txt", "md", "log", "rs", "toml", "yaml", "yml", "json", "csv", "sh", "py", "c", "h", "cpp",
];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];
/// Preview reads at most this many bytes.
const TEXT_CAP: usize = 8192;

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Load the preview payload for `path` (model side; a capped read or an
/// image handle — both cheap and synchronous by design).
pub fn load(path: &Path, is_dir: bool) -> Content {
    if is_dir {
        return Content::None;
    }
    let ext = ext_of(path);
    if TEXT_EXTS.contains(&ext.as_str()) {
        return match std::fs::read(path) {
            Ok(bytes) => {
                let cut = bytes.len().min(TEXT_CAP);
                // don't split a UTF-8 sequence at the cap
                let mut s = String::from_utf8_lossy(&bytes[..cut]).into_owned();
                if bytes.len() > TEXT_CAP {
                    s.push_str("\n…");
                }
                Content::Text(s)
            }
            Err(_) => Content::None,
        };
    }
    if IMAGE_EXTS.contains(&ext.as_str()) {
        return Content::Image(iced::widget::image::Handle::from_path(path));
    }
    Content::None
}

/// The pane (right of the folder view). Called only when
/// `state.pane != Mode::None`.
pub fn view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let entry = state
        .selected
        .iter()
        .next()
        .and_then(|&i| state.entries.get(i));
    let content: Element<Message> = match (state.pane, entry) {
        (Mode::Preview, Some(e)) => preview(state, e, skin),
        (Mode::Details, Some(e)) => details(e, skin, &state.fonts),
        (_, _) => container(
            text("Select an item")
                .size(state.fonts.pane.body)
                .color(skin.subtle)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into(),
    };
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into()
}

/// Preview pane: text / image / fallback note.
fn preview<'a>(state: &'a Explorer, e: &'a crate::fs::Entry, skin: skin::Skin) -> Element<'a, Message> {
    match &state.pane_content {
        Content::Text(s) => scrollable(
            container(text(s.clone()).size(state.fonts.pane.body).color(skin.text))
                .padding(8.0)
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .into(),
        Content::Image(handle) => container(
            iced::widget::image(handle.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(8.0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        Content::None => column![
            container(crate::icon::for_entry(e, 48.0, &skin))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text(e.name.clone()).size(state.fonts.pane.title).color(skin.text))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(
                text(if e.is_dir() { "Folder" } else { "No preview available" })
                    .size(state.fonts.pane.body)
                    .color(skin.subtle),
            )
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        ]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .into(),
    }
}

/// Details pane: icon, file name, size.
fn details<'a>(e: &'a crate::fs::Entry, skin: skin::Skin, fonts: &crate::fonts::FontSizes) -> Element<'a, Message> {
    column![
        container(crate::icon::for_entry(e, 48.0, &skin))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        container(
            text(e.name.clone())
                .size(fonts.pane.title)
                .color(skin.text)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill),
        container(
            text(if e.is_dir() {
                String::new()
            } else {
                crate::model::format_size(e.size)
            })
            .size(fonts.pane.body)
            .color(skin.subtle),
        )
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center),
    ]
    .spacing(8)
    .padding(8)
    .width(Length::Fill)
    .into()
}

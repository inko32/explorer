//! D4 — Delete Confirmation: single item, with a preview (spec D4).

use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length, Padding};

use super::format;
use super::tokens::Tokens;
use super::widgets::{self, PageIcon};
use super::Message;

use iced::widget::Widget as _;
pub const WINDOW_W: f32 = 482.0;

/// What is about to be deleted (metadata for the preview).
#[derive(Debug, Clone)]
pub struct DeleteItem {
    pub name: String,
    pub is_dir: bool,
    pub type_label: String,
    pub size: u64,
    pub modified: String,
}

pub fn view<'a>(item: &'a DeleteItem, recycle: bool, t: Tokens) -> Element<'a, Message> {
    let thing = if item.is_dir { "folder" } else { "file" };
    let question = if recycle {
        format!("Are you sure you want to move this {thing} to the Recycle Bin?")
    } else {
        format!("Are you sure you want to permanently delete this {thing}?")
    };

    let icon = widgets::delete_icon(t);

    let meta_lines: Vec<String> = [
        Some(item.name.clone()),
        Some(format!("Type: {}", item.type_label)),
        Some(format!("Size: {}", format::size(item.size, 1))),
        Some(format!("Date modified: {}", item.modified)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let meta = column(
        meta_lines
            .iter()
            .map(|l| text(l.clone()).size(12).color(t.text_primary).boxed())
            .collect::<Vec<Element<Message>>>(),
    )
    .spacing(3);

    let preview = row![
        widgets::page_icon(PageIcon {
            ext: item
                .name
                .rsplit_once('.')
                .map(|(_, e)| e.to_uppercase())
                .unwrap_or_default(),
            size: 48.0,
            folder: item.is_dir,
            t,
        }),
        meta
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let buttons = row![
        widgets::push_btn("Yes", 87.0, true, true, Message::DeleteYes, t),
        widgets::push_btn("No", 87.0, false, true, Message::DeleteNo, t),
    ]
    .spacing(15)
    .align_y(Alignment::Center);

    column![
        container(row![
            container(icon).padding(Padding { left: 17.0, top: 11.0, ..Default::default() }),
            container(text(question).size(12).color(t.text_primary))
                .padding(Padding { left: 14.0, top: 10.0, right: 16.0, ..Default::default() })
                .width(Length::Fixed(WINDOW_W - 70.0)),
        ]),
        container(preview).padding(Padding {
            left: 63.0,
            top: 8.0,
            ..Default::default()
        }),
        container(buttons).padding(Padding {
            left: WINDOW_W - 16.0 - 87.0 - 15.0 - 87.0,
            top: 12.0,
            ..Default::default()
        }),
    ]
    .width(Length::Fixed(WINDOW_W))
    .boxed()
}

/// Window height from the metadata line count (spec D4.2 layout rule).
pub fn window_height(lines: usize) -> f32 {
    // last baseline = 79 + (n-1)*15 relative to a 31px header; buttons top
    // = last baseline + 20, bottom margin 15, button 24
    let last_baseline = 79.0 + (lines.max(1) as f32 - 1.0) * 15.0;
    last_baseline + 20.0 + 24.0 + 14.0
}

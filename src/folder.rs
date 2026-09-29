//! File view pane: details list and large-icons grid.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Space};
use iced::{Color, Element, Length};

use crate::fs::{self, Entry, EntryKind};
use crate::model::{self, Explorer, Message, ViewMode};
use crate::ui_common::{bg, dbg, skin};

pub(crate) fn file_view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let header = container(
        row![
            container(text("Name").size(11.0).color(skin.subtle))
                .width(Length::Fill)
                .padding([4.0, 8.0]),
            container(text("Date modified").size(11.0).color(skin.subtle))
                .width(Length::Fixed(170.0))
                .padding([4.0, 8.0]),
            container(text("Type").size(11.0).color(skin.subtle))
                .width(Length::Fixed(140.0))
                .padding([4.0, 8.0]),
            container(text("Size").size(11.0).color(skin.subtle))
                .width(Length::Fixed(90.0))
                .padding([4.0, 8.0]),
        ]
        .height(Length::Fixed(24.0))
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.col_header_bg.into()),
        border: iced::Border {
            color: skin.sep,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    let body: Element<Message> = if state.loading {
        container(
            container(text("Loading…").size(12.0).color(skin.subtle))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        match state.view {
        ViewMode::Details => {
            // Virtualized: build only the rows in view (+overscan), with
            // spacers preserving total scroll height (D13).
            const ROW_H: f32 = 24.0;
            let total = state.entries.len();
            let view_rows = (state.viewport_h / ROW_H).ceil() as usize + 4;
            let first = ((state.scroll_y / ROW_H).floor() as usize).saturating_sub(2);
            let last = (first + view_rows).min(total);
            let mut rows = column![].spacing(0);
            if first > 0 {
                rows = rows.push(Space::new().height(Length::Fixed(first as f32 * ROW_H)));
            }
            for i in first..last {
                if let Some(e) = state.entries.get(i) {
                    rows = rows.push(file_row(i, e, state, skin));
                }
            }
            if last < total {
                rows = rows.push(Space::new().height(Length::Fixed((total - last) as f32 * ROW_H)));
            }
            scrollable(rows)
                .height(Length::Fill)
                .on_scroll(|vp| Message::Scrolled {
                    y: vp.absolute_offset().y,
                    height: vp.bounds().height,
                })
                .into()
        }
        ViewMode::Large => {
            let mut grid = row![].spacing(8).padding(8);
            let mut current = column![].spacing(8);
            for (i, e) in state.entries.iter().enumerate() {
                current = current.push(large_cell(i, e, state, skin));
                if (i + 1) % 6 == 0 {
                    grid = grid.push(current);
                    current = column![].spacing(8);
                }
            }
            grid = grid.push(current);
            scrollable(grid).height(Length::Fill).into()
        }
        }
    };

    container(column![header, body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into()
}


fn file_row<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let hovered = state.hovered == Some(i);
    let bgc = if selected {
        skin.sel
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };

    let name_cell: Element<Message> = if state.renaming.as_ref().map(|(ri, _)| *ri) == Some(i) {
        let buf = state.renaming.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
        text_input("Name", &buf)
            .on_input(Message::RenameInput)
            .on_submit(Message::CommitRename(buf))
            .size(12.0)
            .padding(2)
            .into()
    } else {
        text(e.name.clone()).size(12.0).color(skin.text).into()
    };

    let content = row![
        container(crate::icon::for_entry(e, 16.0, &skin))
            .width(Length::Fixed(26.0))
            .center_y(Length::Fixed(16.0)),
        container(name_cell).width(Length::Fill).padding([2.0, 4.0]),
        container(text(e.modified.map(model::format_date).unwrap_or_default()).size(12.0).color(skin.text))
            .width(Length::Fixed(170.0))
            .padding([2.0, 4.0]),
        container(text(model::type_label(e)).size(12.0).color(skin.text))
            .width(Length::Fixed(140.0))
            .padding([2.0, 4.0]),
        container(
            text(if e.kind == fs::EntryKind::Dir { String::new() } else { model::format_size(e.size) })
                .size(12.0)
                .color(skin.text),
        )
        .width(Length::Fixed(90.0))
        .padding([2.0, 4.0]),
    ]
    .height(Length::Fixed(24.0))
    .align_y(iced::Alignment::Center);

    let styled = container(content)
        .width(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    let ma = mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::Hover(None));
    dbg("file_row", ma).into()
}

fn large_cell<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let content = column![
        crate::icon::for_entry(e, 64.0, &skin),
        text(e.name.clone()).size(11.0).color(skin.text).width(Length::Fixed(96.0)),
    ]
    .spacing(6)
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(104.0))
    .height(Length::Fixed(110.0));
    let styled = container(content)
        .padding(4)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .into()
}

// ---------------- status bar ----------------


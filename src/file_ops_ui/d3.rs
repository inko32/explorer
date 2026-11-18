//! D3 — Conflict Fine Selector ("File Conflicts", spec D3).
//!
//! Modal-ish second window: per-file choice of which version(s) to keep.
//! The state owns a snapshot of the op's conflicts; Continue applies it
//! back to the operation.

use iced::widget::{column, container, mouse_area, row, scrollable, text, tooltip, Space};
use iced::{Alignment, Element, Length, Padding};

use super::engine::Conflict;
use super::format;
use super::tokens::Tokens;
use super::widgets::{self, CheckState, PageIcon};
use super::Message;
use super::Side;

use iced::widget::Widget as _;
pub const WINDOW_W: f32 = 528.0;
pub const WINDOW_H: f32 = 525.0;
pub const GROUP_H: f32 = 104.0;

#[derive(Debug, Clone)]
pub struct D3State {
    pub op_id: u64,
    pub src_label: String,
    pub dst_label: String,
    pub conflicts: Vec<Conflict>,
    pub skip_same: bool,
    pub focus: Option<(usize, Side)>,
}

/// (left checked, right checked)
fn sel(c: &Conflict) -> (bool, bool) {
    (c.keep_src, c.keep_dst)
}

fn status_label(c: &Conflict) -> &'static str {
    match sel(c) {
        (true, true) => "Will keep both",
        (true, false) => "Will replace",
        _ => "Will skip",
    }
}

/// "Replace 1, keep both 1, skip 9" (spec D3.3 Continue description).
fn summary(conflicts: &[Conflict], skip_same: bool) -> String {
    let mut replace = 0;
    let mut both = 0;
    let mut skip = 0;
    for c in conflicts {
        if skip_same && c.same_date_size {
            skip += 1;
            continue;
        }
        match sel(c) {
            (true, true) => both += 1,
            (true, false) => replace += 1,
            _ => skip += 1,
        }
    }
    format!("Replace {replace}, keep both {both}, skip {skip}")
}

fn header_state(conflicts: &[Conflict], side: Side, skip_same: bool) -> CheckState {
    let mut eligible = 0;
    let mut checked = 0;
    for c in conflicts {
        if skip_same && c.same_date_size {
            continue;
        }
        eligible += 1;
        let on = match side {
            Side::Left => c.keep_src,
            Side::Right => c.keep_dst,
        };
        if on {
            checked += 1;
        }
    }
    if eligible > 0 && checked == eligible {
        CheckState::Checked
    } else if checked > 0 {
        CheckState::Mixed
    } else {
        CheckState::Unchecked
    }
}

/// One side entry: checkbox + thumbnail + metadata (spec D3.2).
fn entry<'a>(
    op: u64,
    idx: usize,
    side: Side,
    c: &Conflict,
    disabled: bool,
    focused: bool,
    t: Tokens,
) -> Element<'a, Message> {
    let checked = match side {
        Side::Left => c.keep_src,
        Side::Right => c.keep_dst,
    };
    let (date, size) = match side {
        Side::Left => (&c.src_modified, c.src_size),
        Side::Right => (&c.dst_modified, c.dst_size),
    };
    // the whole entry is one hit area (the wrapping mouse_area toggles);
    // the box itself is drawn only — enabled=false keeps its colors
    let check = widgets::check_box(
        if checked { CheckState::Checked } else { CheckState::Unchecked },
        None,
        false,
        Message::Noop,
        t,
    );
    let meta_color = if disabled { t.text_disabled } else { t.text_primary };
    let ext = c
        .name
        .rsplit_once('.')
        .map(|(_, e)| e.to_uppercase())
        .unwrap_or_default();
    let thumb = widgets::page_icon(PageIcon {
        ext,
        size: 64.0,
        folder: false,
        t,
    });
    let content = row![
        check,
        thumb,
        column![
            text(date.clone()).size(12).color(meta_color),
            text(format::size(size, 2)).size(12).color(meta_color),
        ]
        .spacing(3)
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let box_style = move |_: &iced::Theme| container::Style {
        background: if checked {
            Some(t.accent_tint_selected.into())
        } else if focused {
            Some(t.accent_tint_hover.into())
        } else {
            None
        },
        border: iced::Border {
            color: if focused { t.focus_stroke } else { t.accent_tint_selected },
            width: if focused { 1.0 } else { 0.0 },
            radius: 4.0.into(),
        },
        ..Default::default()
    };

    let el: Element<'a, Message> = container(content)
        .padding(Padding {
            left: 3.0,
            right: 6.0,
            top: 3.0,
            bottom: 3.0,
        })
        .style(box_style)
        .boxed();

    if disabled {
        el
    } else {
        mouse_area(el)
            .on_press(Message::D3Toggle { op, idx, side })
            .on_enter(Message::D3Focus { idx, side })
            .boxed()
    }
}

pub fn view<'a>(st: &'a D3State, t: Tokens) -> Element<'a, Message> {
    let instruction = text("Which files do you want to keep?")
        .size(16)
        .color(t.accent_text);
    let sub = text(
        "If you select both versions, the copied file will have a number added to its name.",
    )
    .size(12)
    .color(t.text_primary);

    let left_hdr_state = header_state(&st.conflicts, Side::Left, st.skip_same);
    let right_hdr_state = header_state(&st.conflicts, Side::Right, st.skip_same);
    let col_header = |label: String, state: CheckState, side: Side| {
        row![
            widgets::check_box(
                state,
                None,
                true,
                Message::D3Header { op: st.op_id, side },
                t
            ),
            mouse_area(text(label).size(12).color(t.accent_link))
                .on_press(Message::Noop)
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    };
    let headers = row![
        container(col_header(
            format!("Files from {}", st.src_label),
            left_hdr_state,
            Side::Left
        ))
        .width(Length::Fixed(240.0))
        .padding(Padding { left: 26.0, ..Default::default() }),
        container(col_header(
            format!("Files already in {}", st.dst_label),
            right_hdr_state,
            Side::Right
        ))
        .width(Length::Fixed(260.0)),
    ]
    .padding(Padding {
        top: 27.0,
        bottom: 8.0,
        ..Default::default()
    });

    let same_n = st.conflicts.iter().filter(|c| c.same_date_size).count();

    let mut groups: Vec<Element<'a, Message>> = Vec::new();
    for (i, c) in st.conflicts.iter().enumerate() {
        if i > 0 {
            groups.push(super::d1::divider(t));
        }
        let disabled = st.skip_same && c.same_date_size;
        let status = if disabled {
            "Will skip"
        } else {
            status_label(c)
        };
        let name_row = row![
            text(c.name.clone()).size(12).color(t.text_secondary),
            Space::new().width(Length::Fill).height( 1.0),
            text(status).size(12).color(t.text_secondary),
        ]
        .width(Length::Fixed(WINDOW_W - 30.0))
        .padding(Padding {
            left: 27.0,
            ..Default::default()
        });
        let entries = row![
            container(entry(
                st.op_id,
                i,
                Side::Left,
                c,
                disabled,
                st.focus == Some((i, Side::Left)),
                t
            ))
            .width(Length::Fixed(240.0))
            .padding(Padding { left: 23.0, ..Default::default() }),
            container(entry(
                st.op_id,
                i,
                Side::Right,
                c,
                disabled,
                st.focus == Some((i, Side::Right)),
                t
            ))
            .width(Length::Fixed(260.0))
            .padding(Padding { left: 19.0, ..Default::default() }),
        ]
        .padding(Padding {
            top: 4.0,
            ..Default::default()
        });
        groups.push(
            container(
                tooltip(
                    column![name_row, entries].height(Length::Fixed(GROUP_H - 1.0)),
                    text(format!(
                        "{}\n{} bytes\n{}",
                        c.name,
                        format::count(c.src_size),
                        c.src_modified
                    )),
                )
                .position(tooltip::Position::FollowCursor),
            )
            .boxed(),
        );
    }

    let list = if st.conflicts.len() as f32 * GROUP_H > 350.0 {
        scrollable(column(groups).width(Length::Fixed(WINDOW_W)))
            .height(Length::Fixed(350.0))
            .width(Length::Fixed(WINDOW_W))
            .boxed()
    } else {
        container(column(groups))
            .height(Length::Fixed((st.conflicts.len() as f32 * GROUP_H).max(1.0)))
            .boxed()
    };

    let skip_check: Element<'a, Message> = if same_n > 0 {
        widgets::check_box(
            if st.skip_same { CheckState::Checked } else { CheckState::Unchecked },
            Some(format!(
                "Skip {n} {files} with the same date and size",
                n = same_n,
                files = if same_n == 1 { "file" } else { "files" }
            )),
            true,
            Message::D3SkipSame {
                op: st.op_id,
                checked: !st.skip_same,
            },
            t,
        )
    } else {
        Space::new().width(0.0).height(0.0).boxed()
    };

    let footer = container(
        row![
            skip_check,
            Space::new().width(Length::Fill).height( 1.0),
            tooltip(
                widgets::push_btn(
                    "Continue",
                    91.0,
                    true,
                    true,
                    Message::D3Continue(st.op_id),
                    t
                ),
                text(summary(&st.conflicts, st.skip_same))
            )
            .position(tooltip::Position::Top),
            widgets::push_btn("Cancel", 91.0, false, true, Message::D3Cancel(st.op_id), t),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(WINDOW_W))
    .height(Length::Fixed(42.0))
    .padding(Padding {
        left: 11.0,
        right: 12.0,
        ..Default::default()
    })
    .style(move |_| container::Style {
        background: Some(t.surface_dialog.into()),
        ..Default::default()
    });

    column![
        container(instruction).padding(Padding {
            left: 27.0,
            top: 26.0,
            ..Default::default()
        }),
        container(sub).padding(Padding {
            left: 27.0,
            top: 2.0,
            ..Default::default()
        }),
        headers,
        super::d1::divider(t),
        list,
        super::d1::divider(t),
        footer
    ]
    .width(Length::Fixed(WINDOW_W))
    .boxed()
}

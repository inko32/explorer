//! D2 — Conflict Confirmation: "Replace / Skip / Let me decide" (spec D2).
//!
//! Presentation is either a standalone window (single conflicted operation:
//! just a D1 window with one panel E and a disabled details toggle) or a
//! panel embedded in the D1 progress window. Both share `panel()` here.

use iced::widget::{column, container, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};

use super::engine::{ConflictChoice, FileOp, OpKind, OpState};
use super::tokens::Tokens;
use super::widgets::{cmd_link, CmdIcon, CmdLink, CmdState};
use super::Message;

/// Height of panel E (spec D2.2), including its bottom divider.
use iced::widget::Widget as _;
pub const PANEL_H: f32 = 209.0;

/// The "Copying N items from X to Y" header line with folder links
/// (shared with the D1 running panels).
pub fn header_line<'a>(op: &FileOp, t: Tokens) -> Element<'a, Message> {
    let n = super::format::count(op.total_items);
    let item_word = if op.total_items == 1 { "item" } else { "items" };
    let src = op.src_label.clone();
    let link = |label: String| {
        mouse_area(
            text(label)
                .size(12)
                .color(t.accent_link),
        )
        .on_press(Message::Noop) // open the folder: wired when the shell lands
        .boxed()
    };
    let mut parts: Vec<Element<'a, Message>> = Vec::new();
    match op.kind {
        OpKind::Delete => {
            parts.push(
                text(format!("{} {n} {item_word} from ", op.kind.verb()))
                    .size(12)
                    .color(t.text_primary)
                    .boxed(),
            );
            parts.push(link(src));
        }
        _ => {
            parts.push(
                text(format!("{} {n} {item_word} from ", op.kind.verb()))
                    .size(12)
                    .color(t.text_primary)
                    .boxed(),
            );
            parts.push(link(src));
            parts.push(text(" to ").size(12).color(t.text_primary).boxed());
            parts.push(link(op.dst_label.clone()));
        }
    }
    row(parts)
        .align_y(Alignment::Center)
        .width(Length::Fixed(super::widgets::BAR_W))
        .boxed()
}

/// Panel E content (without the divider row), 449 x 208.
pub fn panel_body<'a>(
    op: &FileOp,
    hover: Option<usize>,
    focus: Option<usize>,
    waiting: bool,
    t: Tokens,
) -> Element<'a, Message> {
    let n = op.conflicts.len();
    let file_word = if n == 1 {
        "a file with the same name"
    } else {
        "files with the same names"
    };
    let headline = text(format!("The destination has {n} {file_word}"))
        .size(16)
        .color(t.text_primary);

    let state_of = |idx: usize| {
        if waiting {
            CmdState::Disabled
        } else if Some(idx) == focus {
            CmdState::Selected
        } else if Some(idx) == hover {
            CmdState::Hovered
        } else {
            CmdState::Rest
        }
    };
    let singular = if n == 1 { "the file" } else { "the files" };
    let decide_text = format!("Let me decide for each {singular}");
    let options = [
        CmdLink {
            icon: CmdIcon::Check,
            text: if n == 1 {
                "Replace the file in the destination".into()
            } else {
                "Replace the files in the destination".into()
            },
            state: state_of(0),
            on_press: Message::ConflictChoice {
                op: op.id,
                choice: ConflictChoice::ReplaceAll,
            },
        },
        CmdLink {
            icon: CmdIcon::Undo,
            text: if n == 1 {
                "Skip this file".into()
            } else {
                "Skip these files".into()
            },
            state: state_of(1),
            on_press: Message::ConflictChoice {
                op: op.id,
                choice: ConflictChoice::SkipAll,
            },
        },
        CmdLink {
            icon: CmdIcon::Compare,
            text: decide_text.into(),
            state: state_of(2),
            on_press: Message::ConflictChoice {
                op: op.id,
                choice: ConflictChoice::PerFile,
            },
        },
    ];

    let mut rows: Vec<Element<'a, Message>> = vec![
        container(header_line(op, t))
            .padding(iced::Padding {
                left: 27.0,
                top: 11.0,
                right: 0.0,
                bottom: 0.0,
            })
            .boxed(),
        container(headline)
            .padding(iced::Padding {
                left: 27.0,
                top: 9.0,
                right: 0.0,
                bottom: 0.0,
            })
            .boxed(),
    ];
    for (i, option) in options.into_iter().enumerate() {
        let el = cmd_link(option, t);
        // hover fill (spec 4.5): track via enter/exit messages
        let el = mouse_area(el)
            .on_enter(Message::D2Hover {
                op: op.id,
                idx: Some(i),
            })
            .on_exit(Message::D2Hover { op: op.id, idx: None });
        rows.push(
            container(el)
                .padding(iced::Padding {
                    left: 27.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .boxed(),
        );
    }
    if waiting {
        rows.push(
            container(
                text("Waiting for your choices in the File Conflicts window...")
                    .size(12)
                    .color(t.text_secondary),
            )
            .padding(iced::Padding {
                left: 27.0,
                top: 4.0,
                right: 0.0,
                bottom: 0.0,
            })
            .boxed(),
        );
    }

    column(rows)
        .width(Length::Fixed(449.0))
        .height(Length::Fixed(PANEL_H - 1.0))
        .boxed()
}

/// Full panel E including its bottom divider, for embedding in D1.
pub fn panel<'a>(
    op: &FileOp,
    hover: Option<usize>,
    focus: Option<usize>,
    t: Tokens,
) -> Element<'a, Message> {
    let waiting = matches!(op.state, OpState::WaitingForD3);
    column![
        panel_body(op, hover, focus, waiting, t),
        container(Space::new().width(449.0f32).height( 1.0f32))
            .width(Length::Fixed(449.0))
            .height(Length::Fixed(1.0))
            .style(move |_| container::Style {
                background: Some(t.divider.into()),
                ..Default::default()
            })
    ]
    .boxed()
}

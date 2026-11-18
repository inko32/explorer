//! D5 — Message Box: generic icon + question + buttons template
//! (spec D5). Instances: multi-item delete, Cancel Actions, error summary.

use iced::widget::{column, container, row, text, Space};
use iced::{Element, Length, Padding};

use super::tokens::Tokens;
use super::widgets;
use super::Message;

use iced::widget::Widget as _;
pub const WINDOW_W: f32 = 482.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Error/Question land with the error-summary instance
pub enum MsgIcon {
    Warning,
    Error,
    Question,
}

#[derive(Debug, Clone)]
pub struct MsgBox {
    pub title: String,
    pub icon: MsgIcon,
    pub text: String,
    /// optional supporting line ({text.secondary})
    pub support: Option<String>,
    pub buttons: MsgButtons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Ok lands with the error-summary instance
pub enum MsgButtons {
    /// destructive yes/no (D5.2 Delete Multiple Items)
    YesNo,
    /// "Keep running" (default, safe) / "Cancel actions"
    KeepRunning,
    /// single OK
    Ok,
}

impl MsgBox {
    pub fn delete_multiple(n: usize, recycle: bool) -> Self {
        Self {
            title: "Delete Multiple Items".to_string(),
            icon: MsgIcon::Warning,
            text: if recycle {
                format!("Are you sure you want to move these {n} items to the Recycle Bin?")
            } else {
                format!("Are you sure you want to permanently delete these {n} items?")
            },
            support: None,
            buttons: MsgButtons::YesNo,
        }
    }

    pub fn cancel_actions(n: usize) -> Self {
        Self {
            title: "Cancel Actions".to_string(),
            icon: MsgIcon::Question,
            text: format!("Do you want to cancel {n} running action{}?", if n == 1 { "" } else { "s" }),
            support: Some("Files already copied will be kept.".to_string()),
            buttons: MsgButtons::KeepRunning,
        }
    }
}

pub fn view<'a>(mb: &'a MsgBox, t: Tokens) -> Element<'a, Message> {
    let icon: Element<Message> = match mb.icon {
        MsgIcon::Warning => widgets::warn_icon(t).boxed(),
        MsgIcon::Error => {
            // error circle with a white "!"
            widgets::err_icon(t).boxed()
        }
        MsgIcon::Question => widgets::question_icon(t).boxed(),
    };

    let mut text_lines: Vec<Element<Message>> = vec![text(mb.text.clone())
        .size(12)
        .color(t.text_primary)
        .boxed()];
    if let Some(s) = &mb.support {
        text_lines.push(
            text(s.clone())
                .size(12)
                .color(t.text_secondary)
                .boxed(),
        );
    }
    let text_col = column(text_lines).spacing(4);

    let buttons: Element<Message> = match mb.buttons {
        MsgButtons::YesNo => row![
            widgets::push_btn("Yes", 87.0, true, true, Message::DeleteYes, t),
            widgets::push_btn("No", 87.0, false, true, Message::DeleteNo, t),
        ]
        .spacing(15)
        .boxed(),
        MsgButtons::KeepRunning => row![
            widgets::push_btn("Keep running", 105.0, true, true, Message::KeepRunning, t),
            widgets::push_btn("Cancel actions", 105.0, false, true, Message::CancelActions, t),
        ]
        .spacing(15)
        .boxed(),
        MsgButtons::Ok => row![widgets::push_btn("OK", 87.0, true, true, Message::DeleteNo, t)]
            .boxed(),
    };

    column![
        container(row![
            container(icon).padding(Padding { left: 14.0, top: 11.0, ..Default::default() }),
            container(text_col).padding(Padding {
                left: 15.0,
                top: 10.0,
                right: 16.0,
                ..Default::default()
            }),
        ]),
        container(Space::new().width(Length::Fill).height( 8.0)),
        container(buttons).padding(Padding {
            left: WINDOW_W - 16.0 - buttons_width(mb.buttons),
            ..Default::default()
        }),
    ]
    .width(Length::Fixed(WINDOW_W))
    .padding(Padding {
        bottom: 14.0,
        ..Default::default()
    })
    .boxed()
}

fn buttons_width(b: MsgButtons) -> f32 {
    match b {
        MsgButtons::YesNo => 87.0 * 2.0 + 15.0,
        MsgButtons::KeepRunning => 105.0 * 2.0 + 15.0,
        MsgButtons::Ok => 87.0,
    }
}

/// H = button bottom + 15 (spec D5.1)
pub fn window_height(mb: &MsgBox) -> f32 {
    let text_h = 12.0 + 16.0 * (wrap_lines(&mb.text, 60) - 1) as f32;
    let support_h = mb
        .support
        .as_deref()
        .map(|s| 16.0 * wrap_lines(s, 60) as f32 + 4.0)
        .unwrap_or(0.0);
    let body = 45.0 + text_h + support_h; // icon row
    let buttons_top = body.max(73.0) + 8.0;
    buttons_top + 24.0 + 15.0
}

fn wrap_lines(s: &str, chars_per_line: usize) -> usize {
    (s.chars().count() / chars_per_line).max(1)
}

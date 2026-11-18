//! D1 — Progress window: all running file operations in one window
//! (spec D1). Panels A-F, footer details fold, attention pill.

use iced::widget::{column, container, mouse_area, row, scrollable, stack, text, Space};
use iced::{Alignment, Element, Length, Padding};

use super::d2;
use super::engine::{FileOp, OpKind, OpState};
use super::format;
use super::tokens::Tokens;
use super::widgets::{self, Bar, Graph};
use super::FileOpApp;
use super::Message;

use iced::widget::Widget as _;

/// One graph column per 1/391 of the total (spec D1.3).
pub const GRAPH_COLS: usize = 391;

pub const WINDOW_W: f32 = 449.0;
/// Max panel-list viewport height (spec D1.4).
pub const VMAX: f32 = 466.0;
const HEADER_H: f32 = 31.0;
const FOOTER_H: f32 = 45.0;

pub const SCROLL_ID: &str = "fileop-panel-list";

/// Height of one op's panel in the current mode (spec D1.2).
pub fn panel_height(op: &FileOp, details: bool) -> f32 {
    match op.state {
        OpState::NeedsDecision | OpState::WaitingForD3 => d2::PANEL_H,
        OpState::Failed(_) => d2::PANEL_H,
        _ => {
            if details {
                211.0
            } else {
                89.0
            }
        }
    }
}

/// Window title per spec D1.1.
pub fn title(app: &FileOpApp) -> String {
    let interrupted = app
        .ops
        .iter()
        .filter(|o| matches!(o.state, OpState::NeedsDecision | OpState::WaitingForD3 | OpState::Failed(_)))
        .count();
    if interrupted > 0 {
        return format!("{interrupted} Interrupted Action{}", if interrupted == 1 { "" } else { "s" });
    }
    if app.ops.len() > 1 {
        return format!("{} Running Actions", app.ops.len());
    }
    match app.ops.first() {
        Some(op) => match op.state {
            OpState::Preparing => {
                format!(
                    "Discovered {} items ({})...",
                    format::count(op.discovered_items()),
                    format::size(op.discovered_bytes(), 1)
                )
            }
            _ => format!("{}% complete", op.fraction_percent()),
        },
        None => String::new(),
    }
}

/// Total window height for the current panel set (spec D1.4).
pub fn window_height(app: &FileOpApp) -> f32 {
    let content: f32 = app
        .ops
        .iter()
        .map(|o| panel_height(o, app.details))
        .sum::<f32>()
        .max(0.0);
    let content = if app.ops.len() > 1 {
        content + (app.ops.len() - 1) as f32 // inter-panel dividers
    } else {
        content
    };
    HEADER_H + content.min(VMAX) + FOOTER_H
}

/// Running/preparing panel content (A-D), without the divider row.
fn running_body<'a>(app: &FileOpApp, op: &FileOp, t: Tokens) -> Element<'a, Message> {
    let paused = matches!(op.state, OpState::Paused);
    let preparing = matches!(op.state, OpState::Preparing);
    let finished = op.is_finished() || matches!(op.state, OpState::Cancelling);

    let header = container(d2::header_line(op, t)).padding(Padding {
        left: 27.0,
        top: 11.0,
        right: 0.0,
        bottom: 0.0,
    });

    let headline = text(op.headline()).size(16).color(t.text_primary);
    let pause_glyph = if paused { "▶" } else { "‖" };
    let pause = widgets::icon_btn(
        pause_glyph,
        !finished,
        if paused { "Resume" } else { "Pause" },
        Message::TogglePause(op.id),
        t,
    );
    let cancel = widgets::icon_btn(
        "×",
        !op.is_finished(),
        "Cancel",
        Message::CancelOp(op.id),
        t,
    );
    let head_row = row![
        container(headline).width(Length::Fixed(310.0)),
        Space::new().width(Length::Fill).height( 1.0),
        pause,
        cancel
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        left: 27.0,
        right: 12.0,
        top: 0.0,
        bottom: 0.0,
    });

    // bar or speed graph
    let visual: Element<'a, Message> = if app.details {
        let speed_label = format!(
            "Speed: {}",
            format::speed(op.rate, op.kind == OpKind::Delete)
        );
        widgets::graph(Graph {
            frac: op.progress(),
            samples: op.samples.clone(),
            scale: op.scale_max,
            speed: op.rate,
            speed_label,
            paused,
            preparing,
            shimmer: app.shimmer,
            t,
        })
        .boxed()
    } else {
        widgets::bar(Bar {
            frac: if preparing { None } else { Some(op.progress()) },
            sweep: app.marquee,
            paused,
            t,
        })
        .boxed()
    };

    let mut parts: Vec<Element<'a, Message>> = vec![
        header.boxed(),
        container(head_row)
            .height(Length::Fixed(34.0))
            .boxed(),
        container(visual)
            .padding(Padding {
                left: 27.0,
                top: 2.0,
                right: 0.0,
                bottom: 0.0,
            })
            .boxed(),
    ];

    if app.details {
        let items_line = format!("Items remaining: {}", op.remaining_items_text());
        let details = column![
            text(format!("Name: {}", op.current_name))
                .size(12)
                .color(t.text_primary),
            text(format!("Time remaining: {}", op.time_text()))
                .size(12)
                .color(t.text_primary),
            text(items_line).size(12).color(t.text_primary),
        ]
        .spacing(3)
        .padding(Padding {
            left: 27.0,
            top: 14.0,
            right: 0.0,
            bottom: 0.0,
        });
        parts.push(details.boxed());
    }

    column(parts)
        .width(Length::Fixed(WINDOW_W))
        .height(Length::Fixed(if app.details { 210.0 } else { 88.0 }))
        .boxed()
}

/// Error panel F content (spec D1.2 type F, Proposed in the spec).
fn error_body<'a>(op: &'a FileOp, t: Tokens) -> Element<'a, Message> {
    let headline = text(format!("Can't copy {}", op.current_name))
        .size(16)
        .color(t.text_primary);
    let reason = op
        .error
        .as_deref()
        .map(|e| text(e).size(12).color(t.text_secondary));
    let options = [
        widgets::CmdLink {
            icon: widgets::CmdIcon::Error,
            text: "Try again".into(),
            state: widgets::CmdState::Rest,
            on_press: Message::Noop,
        },
        widgets::CmdLink {
            icon: widgets::CmdIcon::Undo,
            text: "Skip this file".into(),
            state: widgets::CmdState::Rest,
            on_press: Message::CancelOp(op.id),
        },
        widgets::CmdLink {
            icon: widgets::CmdIcon::Check,
            text: "Cancel the operation".into(),
            state: widgets::CmdState::Rest,
            on_press: Message::CancelOp(op.id),
        },
    ];
    let mut rows: Vec<Element<'a, Message>> = vec![container(headline)
        .padding(Padding {
            left: 27.0,
            top: 40.0,
            right: 0.0,
            bottom: 0.0,
        })
        .boxed()];
    if let Some(r) = reason {
        rows.push(
            container(r)
                .padding(Padding {
                    left: 27.0,
                    top: 2.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                .boxed(),
        );
    }
    for o in options {
        rows.push(
            container(widgets::cmd_link(o, t))
                .padding(Padding {
                    left: 27.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .boxed(),
        );
    }
    column(rows)
        .width(Length::Fixed(WINDOW_W))
        .height(Length::Fixed(d2::PANEL_H - 1.0))
        .boxed()
}

pub fn divider(t: Tokens) -> Element<'static, Message> {
    container(Space::new().width(1.0f32).height( 1.0f32))
        .width(Length::Fixed(WINDOW_W))
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(t.divider.into()),
            ..Default::default()
        })
        .boxed()
}

/// The whole main-window content: panel list (scrollable) + footer.
pub fn view<'a>(app: &'a FileOpApp, t: Tokens) -> Element<'a, Message> {
    let mut panels: Vec<Element<Message>> = Vec::new();
    for (i, op) in app.ops.iter().enumerate() {
        let body: Element<Message> = match op.state {
            OpState::NeedsDecision | OpState::WaitingForD3 => d2::panel(
                op,
                app.d2_hover.filter(|(id, _)| *id == op.id).map(|(_, i)| i),
                app.d2_focus.filter(|(id, _)| *id == op.id).map(|(_, i)| i),
                t,
            ),
            OpState::Failed(_) => column![error_body(op, t), divider(t)].boxed(),
            _ => column![running_body(app, op, t), divider(t)].boxed(),
        };
        if i > 0 {
            panels.push(divider(t));
        }
        panels.push(body);
    }

    let content_h: f32 = app
        .ops
        .iter()
        .map(|o| panel_height(o, app.details))
        .sum::<f32>()
        + (app.ops.len().saturating_sub(1)) as f32;

    let list: Element<Message> = if content_h > VMAX {
        scrollable(column(panels).width(Length::Fixed(WINDOW_W)))
            .id(iced::widget::Id::new(SCROLL_ID))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new(),
            ))
            .on_scroll(|s| Message::MainScrolled(s.viewport.absolute_offset().y))
            .height(Length::Fixed(VMAX))
            .width(Length::Fixed(WINDOW_W))
            .boxed()
    } else {
        container(column(panels))
            .width(Length::Fixed(WINDOW_W))
            .height(Length::Fixed(content_h))
            .boxed()
    };

    // footer: divider + "More/Fewer details" toggle band
    let details_allowed = app
        .ops
        .iter()
        .all(|o| !matches!(o.state, OpState::NeedsDecision | OpState::WaitingForD3 | OpState::Failed(_)))
        && !app.ops.is_empty();
    let expanded = app.details;
    let chevron = text(if expanded { "▴" } else { "▾" })
        .size(11)
        .color(if details_allowed { t.chevron } else { t.chevron_disabled });
    let label = text(if expanded { "Fewer details" } else { "More details" })
        .size(12)
        .color(if details_allowed {
            t.text_primary
        } else {
            t.text_secondary
        });
    let toggle: Element<Message> = if details_allowed {
        mouse_area(
            row![chevron, label]
                .spacing(10)
                .align_y(Alignment::Center)
                .padding(Padding {
                    left: 31.0,
                    right: 10.0,
                    top: 0.0,
                    bottom: 0.0,
                }),
        )
        .on_press(Message::ToggleDetails)
        .boxed()
    } else {
        row![chevron, label]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding(Padding {
                left: 31.0,
                right: 10.0,
                top: 0.0,
                bottom: 0.0,
            })
            .boxed()
    };
    let footer = column![
        divider(t),
        container(toggle)
            .width(Length::Fixed(WINDOW_W))
            .height(Length::Fixed(43.0))
            .center_y(Length::Fixed(43.0)),
        container(Space::new().width(1.0f32).height( 1.0f32))
            .width(Length::Fixed(WINDOW_W))
            .height(Length::Fixed(1.0))
            .style(move |_| container::Style {
                background: Some(t.divider.into()),
                ..Default::default()
            })
    ];

    let body: Element<Message> = column![list, footer]
        .width(Length::Fixed(WINDOW_W))
        .boxed();

    // attention pill (spec D1.8 / 4.9): a decision panel sits below the fold
    let pill_needed = app.ops.iter().enumerate().any(|(i, o)| {
        matches!(o.state, OpState::NeedsDecision)
            && panel_top(app, i) + d2::PANEL_H > HEADER_H + VMAX
            && app.scroll_y + VMAX < panel_top(app, i) + d2::PANEL_H
    });
    if pill_needed {
        let n = app
            .ops
            .iter()
            .filter(|o| matches!(o.state, OpState::NeedsDecision))
            .count();
        let pill_text = format!(
            "{} action{} needs your attention",
            n,
            if n == 1 { "" } else { "s" }
        );
        let pill: Element<Message> = mouse_area(
            container(
                text(format!("▲ {pill_text}"))
                    .size(12)
                    .color(t.text_on_accent),
            )
            .padding(Padding {
                left: 12.0,
                right: 12.0,
                top: 5.0,
                bottom: 5.0,
            })
            .style(move |_| container::Style {
                background: Some(t.accent_fill.into()),
                border: iced::Border {
                    radius: 14.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
        .on_press(Message::PillScroll)
        .boxed();
        stack![
            body,
            container(pill)
                .width(Length::Fixed(WINDOW_W))
                .align_x(iced::alignment::Horizontal::Center)
                .padding(Padding {
                    top: 8.0,
                    ..Default::default()
                })
        ]
        .boxed()
    } else {
        body
    }
}

/// y position of panel `i` within the scrolled content.
fn panel_top(app: &FileOpApp, i: usize) -> f32 {
    let mut y = 0.0;
    for (j, op) in app.ops.iter().enumerate() {
        if j == i {
            break;
        }
        y += panel_height(op, app.details) + 1.0;
    }
    y
}


//! Minimal repro: `window::resize` on Wayland leaves iced's viewport stale.
//!
//! A 300x150 non-resizable window; two seconds in the app resizes it to
//! 300x300. On Wayland (floating window => "stateless" configures) winit
//! applies the size change synchronously and returns `Some(size)` from
//! `request_inner_size`, promising per winit's contract that **no**
//! `WindowEvent::Resized` will follow. `iced_winit`'s
//! `window::Action::Resize` handler ignores that return value, so iced's
//! internal viewport (`WindowState`) is never updated.
//!
//! Observable without screenshots:
//!   * the "iced thinks the size is" readout (polled every second via
//!     `window::size`) stays 300x150 even though the window frame grew;
//!   * the content keeps rendering at the old 150 px height, so the
//!     compositor stretches it (or shows blank space below, depending on
//!     the compositor).
//!
//! Run on Wayland with the window floating (not maximized/tilled).

use iced::widget::{column, container, text, Space};
use iced::{
    Alignment, Border, Color, Element, Length, Padding, Subscription, Task, Widget as _, window,
};
use std::time::Duration;

const W: f32 = 300.0;
const H0: f32 = 150.0; // initial window height
const H1: f32 = 300.0; // height after window::resize

fn main() -> iced::Result {
    iced::daemon(boot, App::update, App::view)
        .title(|_: &App, _: window::Id| "iced_resize — window::resize viewport bug".to_owned())
        .theme(|_: &App, _: window::Id| Some(iced::Theme::Light))
        .subscription(App::subscription)
        .run()
}

fn boot() -> (App, Task<Message>) {
    let (id, open) = window::open(window::Settings {
        size: iced::Size::new(W, H0),
        resizable: false,
        ..Default::default()
    });
    (
        App {
            id,
            resize_sent: false,
            reported: iced::Size::new(W, H0),
            tick: 0,
        },
        open.map(|_| Message::Opened),
    )
}

struct App {
    id: window::Id,
    resize_sent: bool,
    /// what `window::size` reports (iced's internal viewport)
    reported: iced::Size,
    tick: u64,
}

#[derive(Debug, Clone)]
enum Message {
    Opened,
    Tick,
    DoResize,
    Sized(iced::Size),
    Closed,
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened => Task::none(),
            Message::Tick => {
                self.tick += 1;
                // poll what iced believes the size is, once per second
                let poll = window::size(self.id).map(Message::Sized);
                if self.tick == 2 && !self.resize_sent {
                    Task::batch([poll, Task::done(Message::DoResize)])
                } else {
                    poll
                }
            }
            Message::DoResize => {
                self.resize_sent = true;
                eprintln!(
                    "[repro] sending window::resize -> {W}x{H1} \
                     (window::size believes: {:?})",
                    self.reported
                );
                window::resize(self.id, iced::Size::new(W, H1))
            }
            Message::Sized(size) => {
                if size != self.reported {
                    eprintln!("[repro] window::size now reports: {size:?}");
                    self.reported = size;
                }
                Task::none()
            }
            Message::Closed => iced::exit(),
        }
    }

    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        let stale = self.resize_sent && (self.reported.height - H1).abs() > 1.0;
        let status = if !self.resize_sent {
            format!("waiting… app will resize to {W}x{H1} in ~2s")
        } else if stale {
            format!("BUG: iced thinks the size is still {:?} (stale)", self.reported)
        } else {
            format!("OK: iced sees the new size {:?}", self.reported)
        };

        // The fixed-height block exactly fills the window before the
        // resize; a red hairline marks its bottom edge. If the viewport
        // goes stale, this 150 px-tall buffer is stretched over the
        // 300 px-tall window and the red line lands at the window's
        // bottom edge instead of halfway.
        let content = column![
            text("iced window::resize repro").size(16),
            text(status).size(12),
            Space::new().height(Length::Fill),
            container(Space::new())
                .width(Length::Fixed(W))
                .height(Length::Fixed(1.0))
                .style(|_: &iced::Theme| container::Style {
                    background: Some(Color::from_rgb(0.9, 0.1, 0.1).into()),
                    ..Default::default()
                }),
        ]
        .width(Length::Fixed(W))
        .height(Length::Fixed(H0))
        .align_x(Alignment::Start)
        .padding(Padding::new(10.0));

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &iced::Theme| container::Style {
                background: Some(Color::from_rgb(0.92, 0.92, 0.92).into()),
                border: Border {
                    color: Color::from_rgb(0.4, 0.4, 0.4),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .boxed()
    }
}

impl App {
    fn subscription(_: &App) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(tick_stream).map(|()| Message::Tick),
            window::close_events().map(|_| Message::Closed),
        ])
    }
}

/// One message per second (std-only; no async runtime needed).
fn tick_stream() -> impl iced::futures::Stream<Item = ()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || loop {
        if tx.send(()).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    });
    iced::futures::stream::unfold(rx, |rx| async move { rx.recv().ok().map(|_| ((), rx)) })
}

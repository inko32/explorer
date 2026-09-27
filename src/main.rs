//! Explorer — iced spike. Validates: tiny-skia renderer, borderless window
//! with self-drawn title bar, details file list, hover/selection, dark mode,
//! hi-dpi text quality, idle CPU.

mod model;
mod ui;

use model::{Explorer, Message};

fn main() -> iced::Result {
    iced::application(
        || (Explorer::new(), iced::Task::none()),
        Explorer::update,
        ui::view,
    )
    .title(|state: &Explorer| state.title())
    .theme(|state: &Explorer| state.theme())
    .subscription(|_state: &Explorer| {
        iced::window::open_events().map(Message::WindowOpened)
    })
    .window(iced::window::Settings {
        size: iced::Size::new(1152.0, 720.0),
        min_size: Some(iced::Size::new(560.0, 360.0)),
        decorations: false, // border module draws our frame
        resizable: true,
        visible: true,
        ..Default::default()
    })
    .run()
}

//! Explorer — iced. Windows 8 style file explorer for POSIX (Wayland first).

mod fs;
mod icons;
mod model;
mod ui;

use model::{Explorer, Message};

/// Fs event stream, taken by `Subscription::run` exactly once.
pub static FS_STREAM_RX: std::sync::OnceLock<
    std::sync::Mutex<Option<fs::EventReceiver>>,
> = std::sync::OnceLock::new();

fn fs_stream_builder() -> fs::EventReceiver {
    FS_STREAM_RX
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| iced::futures::channel::mpsc::unbounded().1)
}

fn main() -> iced::Result {
    iced::application(
        || (Explorer::new(), iced::Task::none()),
        Explorer::update,
        ui::view,
    )
    .title(|state: &Explorer| state.title())
    .theme(|state: &Explorer| state.theme())
    .subscription(|_state: &Explorer| {
        iced::Subscription::batch(vec![
            iced::window::open_events().map(Message::WindowOpened),
            iced::event::listen().map(Message::Event),
            iced::Subscription::run(fs_stream_builder).map(Message::FsEvent),
        ])
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

/// Global keyboard shortcuts (binding table). When an inline rename editor is
/// open, only Escape reaches the app — everything else belongs to the editor.
pub fn handle_key(event: iced::Event, renaming: bool) -> Option<Message> {
    use iced::keyboard::{key::Named, Event as KeyEvent, Key};
    let iced::Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    if renaming {
        return (key == Key::Named(Named::Escape)).then_some(Message::CancelRename);
    }
    match key {
        Key::Named(Named::F2) => Some(Message::RenameSelected),
        Key::Named(Named::Delete) => Some(Message::DeleteSelection),
        Key::Named(Named::Backspace) => Some(Message::Up),
        Key::Named(Named::F5) => Some(Message::Refresh),
        Key::Named(Named::ArrowLeft) if modifiers.alt() => Some(Message::Back),
        Key::Named(Named::ArrowRight) if modifiers.alt() => Some(Message::Forward),
        Key::Character(c) if modifiers.control() => match c.as_str() {
            "c" => Some(Message::CopySelection),
            "x" => Some(Message::CutSelection),
            "v" => Some(Message::Paste),
            "a" => Some(Message::SelectAll),
            "n" if modifiers.shift() => Some(Message::NewFolder),
            _ => None,
        },
        _ => None,
    }
}

//! Explorer — iced. Windows 8 style file explorer for POSIX (Wayland first).

mod fs;
mod icon;
mod icons;
mod model;
mod ribbon;
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
    // Renderer selection (D17): GPU (wgpu) is primary, CPU (tiny-skia) is the
    // reserved fallback — both are compiled in and picked at startup.
    //   explorer             -> wgpu, auto-fallback to tiny-skia if GPU init fails
    //   explorer --render gpu -> wgpu only; startup ERROR if unavailable
    //   explorer --render cpu -> tiny-skia only
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--render" => {}
            "--ui-debug" => std::env::set_var("EXPLORER_UI_DEBUG", "1"),
            "--help" | "-h" => {
                eprintln!("Windows 8 style file explorer\n\nUSAGE:\n    explorer [--render <gpu|cpu>] [--ui-debug] [path]\n\nOPTIONS:\n    --render gpu    GPU rendering (wgpu); fails at startup if unavailable\n    --render cpu    CPU rendering (tiny-skia); always works\n    --ui-debug      outline every UI element with colored 1px borders\n    (no flag)       GPU with automatic CPU fallback");
                return Ok(());
            }
            v if v.starts_with("--render=") => set_renderer(&v["--render=".len()..]),
            v if v.starts_with("--render") && v.len() > "--render".len() => {
                set_renderer(&v["--render".len()..].trim_start_matches('='))
            }
            _ => {}
        }
    }
    if std::env::var_os("EXPLORER_DEBUG").is_some() || std::env::var_os("ICED_BACKEND").is_some() {
        eprintln!(
            "renderer: {}",
            std::env::var("ICED_BACKEND").unwrap_or_else(|_| "gpu (auto-fallback to cpu)".into())
        );
    }
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
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size.width, size.height)),
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

/// `--render gpu|cpu` -> iced's `ICED_BACKEND` (wgpu / tiny-skia). An
/// explicit choice is strict: `--render gpu` errors out instead of silently
/// falling back; no flag = auto-fallback.
fn set_renderer(choice: &str) {
    let backend = match choice {
        "gpu" | "wgpu" => "wgpu",
        "cpu" | "tiny-skia" | "tiny_skia" => "tiny-skia",
        other => {
            eprintln!("unknown --render value '{other}' (expected gpu|cpu)");
            std::process::exit(2);
        }
    };
    // SAFETY: single-threaded, before iced::application run.
    std::env::set_var("ICED_BACKEND", backend);
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
        Key::Named(Named::Escape) => Some(Message::CancelMenu),
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

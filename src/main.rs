//! Explorer — iced. Windows 8 style file explorer for POSIX (Wayland first).

mod border;
mod commands;
// SPEC/DRAGGING.md core. The UriList/Text/Custom payloads, the native
// adapter DragSources, the bounds-based registry API and the
// Open/Import/Run actions are the Phase 1/3 surface — consumed by the
// future platform adapter (external DnD) and targets (tabs,
// executables, favorites), not yet wired in the UI.
#[allow(dead_code)]
mod dnd;
mod dnd_wayland;
mod details_view;
mod folder;
mod fonts;
mod fs;
mod icon;
mod icon_theme;
mod icons;
mod menu;
mod model;
mod nav_pane;
// TODO(perf): assets/performance.yaml rate caps — disabled, see model/explorer.rs
#[allow(dead_code)]
mod perf;
mod navigation;
mod pane;
// Ribbon: the vendored rs-ribbon framework (Windows Ribbon Framework
// re-implementation, vendor/rs-ribbon) + the explorer commanding layer
// in ribbon.rs. The ribbon markup is assets/ribbon.yaml.
mod ribbon;
mod scrollbar;
mod status;
mod text_metrics;
mod theme;
mod ui;
mod ui_common;
mod view_step;

use model::{Explorer, FocusDir, Message};

/// Fs event stream, taken by `Subscription::run` exactly once.
pub static FS_STREAM_RX: std::sync::OnceLock<
    std::sync::Mutex<Option<fs::EventReceiver>>,
> = std::sync::OnceLock::new();

/// Wayland DnD side-channel event stream, taken by `Subscription::run`
/// exactly once. The channel is pre-created in `Explorer::new` (like
/// the fs worker) so the stream is live before the window exists; the
/// thread connects to the sender later via `DND_EVENT_TX`.
pub static DND_STREAM_RX: std::sync::OnceLock<
    std::sync::Mutex<Option<dnd_wayland::EventReceiver>>,
> = std::sync::OnceLock::new();

/// Sender half of the DnD event channel, handed to the side-channel
/// thread once the raw Wayland handles arrive (`Message::WlHandles`).
pub static DND_EVENT_TX: std::sync::OnceLock<
    std::sync::Mutex<Option<dnd_wayland::EventSender>>,
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

fn dnd_stream_builder() -> dnd_wayland::EventReceiver {
    DND_STREAM_RX
        .get()
        .and_then(|rx| rx.lock().unwrap().take())
        .unwrap_or_else(|| iced::futures::channel::mpsc::unbounded().1)
}

/// ~60fps tick stream for the Details column reorder's shift
/// animation. The thread-pool iced executor has no timer (its
/// `time::every` is an empty module), so a plain std thread + channel
/// does the interval; the thread exits when the stream is dropped.
fn reorder_tick_stream() -> impl iced::futures::Stream<Item = ()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || loop {
        if tx.send(()).is_err() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    });
    iced::futures::stream::unfold(rx, |rx| async move { rx.recv().ok().map(|_| ((), rx)) })
}

fn main() -> iced::Result {
    // Renderer selection (D17): GPU (wgpu) is primary, CPU (tiny-skia) is the
    // reserved fallback — both are compiled in and picked at startup.
    //   explorer             -> wgpu, auto-fallback to tiny-skia if GPU init fails
    //   explorer --render gpu -> wgpu only; startup ERROR if unavailable
    //   explorer --render cpu -> tiny-skia only
    let mut want_dpi = false;
    for arg in std::env::args().skip(1) {
        if want_dpi {
            want_dpi = false;
            set_dpi(&arg);
            continue;
        }
        match arg.as_str() {
            "--render" => {}
            "--dpi" => want_dpi = true,
            v if v.starts_with("--dpi=") => set_dpi(&v["--dpi=".len()..]),
            v if v.starts_with("--size=") => set_size(&v["--size=".len()..]),
            "--help" | "-h" => {
                eprintln!("Windows 8 style file explorer\n\nUSAGE:\n    explorer [--render <gpu|cpu>] [path]\n\nOPTIONS:\n    --render gpu    GPU rendering (wgpu); fails at startup if unavailable\n    --render cpu    CPU rendering (tiny-skia); always works\n    --dpi=N         absolute UI scale: percent of 96 dpi (100, 125, 150, 123...)\n                    or a direct factor <= 10 (1.5); default = system scale\n    --size=WxH      window size in logical pixels (default 1152x720)\n    (no flag)       GPU with automatic CPU fallback");
                return Ok(());
            }
            v if v.starts_with("--render=") => set_renderer(&v["--render=".len()..]),
            v if v.starts_with("--render") && v.len() > "--render".len() => {
                set_renderer(v["--render".len()..].trim_start_matches('='))
            }
            _ => {}
        }
    }
    // The Win8.1 frame can be translucent (win81theme.yaml
    // border_opacity): the frame accent/outlines draw with alpha and
    // the content behind the window shows through them. The frame
    // style is a runtime toggle, but winit only accepts transparency
    // at window creation — decide from the yaml now and reuse the
    // flag for the window settings and the clear color below.
    let translucent = crate::border::win81::opacity() < 1.0;
    if std::env::var_os("EXPLORER_DEBUG").is_some() || std::env::var_os("ICED_BACKEND").is_some() {
        eprintln!(
            "renderer: {}",
            std::env::var("ICED_BACKEND").unwrap_or_else(|_| "gpu (auto-fallback to cpu)".into())
        );
    }
    let app = iced::application(
        || (Explorer::new(), iced::Task::none()),
        Explorer::update,
        ui::view,
    );
    // Selawik is embedded only with the `embed-selawik` cargo feature
    // (off by default; it is SIL OFL — free to ship when enabled)
    #[cfg(feature = "embed-selawik")]
    let app = app.fonts([fonts::EMBED_BYTES]);
    app.scale_factor(|state: &Explorer| state.dpi_multiplier())
    // fonts.yaml fallback list: first installed family wins
    .font(fonts::default_font())
    .title(|state: &Explorer| state.title())
    .theme(|state: &Explorer| state.theme())
    // The clear color must be transparent on a translucent window:
    // the default style clears with the theme's OPAQUE background,
    // which swallows the border's alpha (the border fades against
    // white instead of reaching the compositor). The view still draws
    // opaque fills over the whole window, so nothing else changes.
    .style(move |_state: &Explorer, theme: &iced::Theme| iced::theme::Style {
        background_color: if translucent {
            iced::Color::TRANSPARENT
        } else {
            theme.palette().background.base.color
        },
        text_color: theme.palette().background.base.text,
    })
    .subscription(|state: &Explorer| {
        let mut subs = vec![
            iced::window::open_events().map(Message::WindowOpened),
            iced::event::listen().map(Message::Event),
            iced::Subscription::run(fs_stream_builder).map(Message::FsEvent),
            iced::Subscription::run(dnd_stream_builder).map(Message::Dnd),
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size.width, size.height)),
        ];
        // the column reorder's shift animation ticks only while it is
        // actually animating (a move restarts it, so the subscription
        // resumes then)
        if state
            .col_reorder
            .as_ref()
            .is_some_and(|d| d.anim_t < 1.0)
        {
            subs.push(
                iced::Subscription::run(reorder_tick_stream)
                    .map(|_| Message::ColumnReorderTick),
            );
        }
        iced::Subscription::batch(subs)
    })
    .window(iced::window::Settings {
        size: window_size(),
        min_size: Some(iced::Size::new(160.0, 120.0)),
        decorations: false, // border module draws our frame
        resizable: true,
        visible: true,
        // translucent Win8.1 border (see `translucent` above)
        transparent: translucent,
        // the program icon: the pack's libraries template
        // (assets/icons/svgs/libraries.svg) rasterized at startup.
        // Note: winit 0.30 ignores window icons on Wayland (no
        // xdg-toplevel-icon support); the app id below drives the
        // taskbar lookup there.
        icon: window_icon(),
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "explorer".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
}

/// `--dpi=N` -> `EXPLORER_DPI`: absolute scale (percent of 96 dpi, or a
/// direct factor <= 10). Applied as a multiplier over the system scale.
fn set_dpi(value: &str) {
    let v: f32 = match value.parse() {
        Ok(v) if v > 0.0 && v <= 1000.0 => v,
        _ => {
            eprintln!("invalid --dpi value '{value}' (expected percent like 100/125/150 or factor <= 10)");
            std::process::exit(2);
        }
    };
    let factor = if v <= 10.0 { v } else { v / 100.0 };
    unsafe {
        std::env::set_var("EXPLORER_DPI", factor.to_string());
    }
}

/// `--size=WxH` -> `EXPLORER_SIZE`: initial window size (logical pixels).
fn set_size(value: &str) {
    let (w, h) = value
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
        .filter(|(w, h)| *w >= 160.0 && *h >= 120.0 && *w <= 10000.0 && *h <= 10000.0)
        .unwrap_or_else(|| {
            eprintln!("invalid --size value '{value}' (expected like 1152x720, min 160x120)");
            std::process::exit(2);
        });
    unsafe {
        std::env::set_var("EXPLORER_SIZE", format!("{w}x{h}"));
    }
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
    unsafe {
        std::env::set_var("ICED_BACKEND", backend);
    }
}

/// Base LOGICAL window size. `--dpi` scales the CONTENT; the window size is
/// set independently (`--size=WxH` / EXPLORER_SIZE, default below).
pub const BASE_LOGICAL: (f32, f32) = (1152.0, 720.0);

/// Initial window size: `--size=WxH` if given, else the base.
fn window_size() -> iced::Size {
    if let Ok(v) = std::env::var("EXPLORER_SIZE")
        && let Some((w, h)) = v
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
    {
        return iced::Size::new(w, h);
    }
    iced::Size::new(BASE_LOGICAL.0, BASE_LOGICAL.1)
}

/// The window/taskbar icon: the pack's `libraries` template rendered at
/// 48px through the same usvg/resvg pipeline the renderer uses (the
/// pack is all-SVG since the PNG assets were replaced; there is no
/// raster app icon to load). `None` on any failure — a window without
/// an icon still runs.
fn window_icon() -> Option<iced::window::Icon> {
    const SIZE: u32 = 48;
    let handle = crate::icons::builtin_svg("libraries", &crate::ui_common::skin::light())?;
    let iced::advanced::svg::Data::Bytes(svg) = handle.data() else {
        return None;
    };
    let tree = resvg::usvg::Tree::from_str(
        std::str::from_utf8(svg).ok()?,
        &resvg::usvg::Options::default(),
    )
    .ok()?;
    let scale = resvg::tiny_skia::Transform::from_scale(
        SIZE as f32 / tree.size().width(),
        SIZE as f32 / tree.size().height(),
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    // premultiplied pixmap -> straight RGBA for winit
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    iced::window::icon::from_rgba(rgba, SIZE, SIZE).ok()
}

/// Global keyboard shortcuts (binding table). When an inline rename editor is
/// open, only Escape reaches the app — everything else belongs to the editor.
pub fn handle_key(
    event: iced::Event,
    renaming: bool,
    menu_open: bool,
    address_editing: bool,
    search_editing: bool,
) -> Option<Message> {
    use iced::keyboard::{key::Named, Event as KeyEvent, Key};
    let iced::Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    if menu_open {
        return (key == Key::Named(Named::Escape)).then_some(Message::CloseMenu);
    }
    if renaming {
        return (key == Key::Named(Named::Escape)).then_some(Message::CancelRename);
    }
    // while the address field is focused the keystrokes belong to it;
    // only Escape (discard) is handled globally
    if address_editing {
        return (key == Key::Named(Named::Escape)).then_some(Message::CancelAddressEdit);
    }
    // while the search field is focused the keystrokes belong to it;
    // only Escape (leave edit mode) is handled globally
    if search_editing {
        return (key == Key::Named(Named::Escape)).then_some(Message::CancelSearch);
    }
    // the Menu key (near AltGr) opens the context menu for the selection
    if key == Key::Named(Named::ContextMenu) {
        return Some(Message::MenuKey);
    }
    match key {
        Key::Named(Named::F2) => Some(Message::RenameSelected),
        Key::Named(Named::Delete) => Some(Message::DeleteSelection),
        Key::Named(Named::Backspace) => Some(Message::Up),
        Key::Named(Named::F5) => Some(Message::Refresh),
        Key::Named(Named::ArrowLeft) if modifiers.alt() => Some(Message::Back),
        Key::Named(Named::ArrowRight) if modifiers.alt() => Some(Message::Forward),
        // the idle selector (keyboard focus rectangle): arrows navigate
        // it in the folder view, Space selects/deselects its item
        Key::Named(Named::ArrowUp) => Some(Message::FocusMove(FocusDir::Up)),
        Key::Named(Named::ArrowDown) => Some(Message::FocusMove(FocusDir::Down)),
        Key::Named(Named::ArrowLeft) => Some(Message::FocusMove(FocusDir::Left)),
        Key::Named(Named::ArrowRight) => Some(Message::FocusMove(FocusDir::Right)),
        Key::Named(Named::Space) => Some(Message::FocusSpace),
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

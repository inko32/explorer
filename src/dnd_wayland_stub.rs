//! Non-Unix stand-in for the Wayland side-channel (`dnd_wayland.rs`,
//! unix only). The real side-channel never exists off-Wayland: the
//! window's raw handles never arrive (`window::raw_handle` yields
//! `None`), so `spawn` never runs and `Handle` is never constructed —
//! `dnd_wl` stays `None` and every method here is unreachable in
//! practice. This stub only keeps the GUI-side types and method calls
//! compiling; all of them are no-ops. `begin_drag` is the exception:
//! its `DndAction` argument is Wayland-only, so the single call site is
//! `#[cfg(unix)]`-gated instead of being mirrored here.

use crate::drag_paint::{DragVisual, ToplevelIcon};

pub type EventSender = iced::futures::channel::mpsc::UnboundedSender<Event>;
pub type EventReceiver = iced::futures::channel::mpsc::UnboundedReceiver<Event>;

/// Mirrors `dnd_wayland::Event` (kept in sync by hand — the enum is
/// plain data, no Wayland types).
#[derive(Debug, Clone)]
pub enum Event {
    Hover { mimes: Vec<String> },
    HoverLeft,
    Dropped { mime: String, data: Vec<u8> },
    DragFinished { performed: bool },
    PopupMotion { x: f32, y: f32 },
    PopupPress { x: f32, y: f32 },
    PopupDone,
    PopupKey { keysym: u32 },
}

/// Never constructed off-Wayland (see module docs): no-op methods only.
pub struct Handle;

impl Handle {
    pub fn update_visual(&self, _visual: DragVisual) {}

    pub fn open_popup(&self, _painted: f32, _system: f32, _visual: DragVisual) {}

    pub fn update_popup(&self, _visual: DragVisual) {}

    pub fn close_popup(&self) {}

    pub fn set_icon(&self, _icon: ToplevelIcon) {}
}

/// Pre-create the GUI-side event channel BEFORE the app starts (same
/// pattern as the unix impl, so `Subscription::run` finds a live stream).
pub fn init_channel() -> EventSender {
    let (ev_tx, ev_rx) = iced::futures::channel::mpsc::unbounded::<Event>();
    let _ = crate::DND_STREAM_RX.set(std::sync::Mutex::new(Some(ev_rx)));
    ev_tx
}

/// Off-Wayland there is no side-channel to spawn against.
pub fn spawn(_handles: (usize, usize, usize), _ev_tx: EventSender) -> Option<Handle> {
    None
}

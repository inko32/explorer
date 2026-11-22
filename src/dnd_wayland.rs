//! Wayland side-channel: one thread on winit's connection.
//!
//! iced/winit have no Wayland data-device (drag-out payloads, drag-in
//! offers, action negotiation), no off-window popups, and no toplevel
//! icon. This module fills the gaps without touching winit: a thread
//! owns a second event queue *on winit's Wayland connection* (same
//! client — the protocol requires it; see `raw_handle_wayland.rs`)
//! and drives `wl_data_device` from it.
//!
//! Module map (all on the same thread/connection):
//! - `raw_handle_wayland`: wrap the raw handles from the iced patch
//!   into typed Wayland objects
//! - `dnd_wayland` (this file): the thread, drag-out/drag-in, the drag
//!   image surface, the shared raster/text paint helpers
//! - `popup_wayland`: off-window menu popups (xdg_popup)
//! - `icon_wayland`: the compositor taskbar/window-switcher icon
//!   (xdg_toplevel_icon_v1)
//!
//! Design + protocol behavior were validated with the /tmp/dnd-spike
//! prototype on KWin:
//! - a second seat binding on the same client receives the full pointer
//!   stream (enter/button serials included) while the app's surface is
//!   focused — no winit serial plumbing needed;
//! - `start_drag` needs the real focused surface as origin (an unmapped
//!   placeholder is cancelled immediately);
//! - offering Copy|Move|Link together makes the *destination* pop the
//!   action-choice menu (matching our ambiguous-drop convention).
//!
//! Split of labor: the thread owns every Wayland object; the GUI thread
//! paints the drag image (icon theme + fonts live there — painters in
//! `drag_paint.rs`, shared with the in-window overlays) and sends the
//! pixels over; events flow back as [`Event`]s through the same
//! stream channel the fs worker uses.

use std::io::{Read as _, Write as _};
use std::os::unix::io::{AsFd as _, AsRawFd as _, BorrowedFd};

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::data_device_manager::data_device::{
    DataDevice, DataDeviceData, DataDeviceHandler,
};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler;
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{
    PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_registry, registry_handlers};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy as _, QueueHandle};

use crate::drag_paint::{swizzle, DragVisual};

// ---------------------------------------------------------------------------
// GUI-thread API
// ---------------------------------------------------------------------------

pub type EventSender = iced::futures::channel::mpsc::UnboundedSender<Event>;
pub type EventReceiver = iced::futures::channel::mpsc::UnboundedReceiver<Event>;

/// Commands GUI -> side-channel thread. DnD commands are handled here;
/// popup/icon commands are forwarded to their modules.
pub(crate) enum Cmd {
    /// Start a drag with the given file:// URIs. The serial is the one
    /// the thread captured from the press that armed the drag (the
    /// compositor validates it against the implicit grab).
    BeginDrag {
        uris: Vec<String>,
        actions: DndAction,
        /// wl buffer scale for the icon surface (ceil(output scale))
        scale: i32,
        visual: DragVisual,
    },
    /// Re-paint the drag image while a drag is live (caption changes
    /// when the modifier-derived action changes).
    UpdateVisual(DragVisual),
    /// see `popup_wayland`
    Popup(crate::popup_wayland::Cmd),
    /// see `icon_wayland`
    Icon(crate::icon_wayland::Cmd),
}

/// Side-channel events, delivered as `Message::Dnd`. The popup variants
/// are emitted by `popup_wayland`.
#[derive(Debug, Clone)]
pub enum Event {
    /// An external drag entered our window with these MIME types.
    Hover { mimes: Vec<String> },
    /// The external drag left our window.
    HoverLeft,
    /// An external (or our own) drag was dropped on our window; `data`
    /// is the payload of the negotiated MIME type.
    Dropped { mime: String, data: Vec<u8> },
    /// The drag we started ended: `performed` = a destination took the
    /// data; false = cancelled (Esc / no accepting target).
    DragFinished { performed: bool },
    /// Pointer moved over the menu popup, popup-local logical coords.
    PopupMotion { x: f32, y: f32 },
    /// Left press on the menu popup (popup-local logical coords).
    PopupPress { x: f32, y: f32 },
    /// The compositor dismissed the popup (outside click).
    PopupDone,
    /// A key pressed while the popup holds the grab: the raw xkb
    /// keysym (Esc = 0xff1b).
    PopupKey { keysym: u32 },
}

/// GUI-side handle to the side-channel thread.
pub struct Handle {
    pub(crate) tx: std::sync::mpsc::Sender<Cmd>,
    /// writing a byte wakes the thread's poll loop
    pub(crate) wake: std::fs::File,
}

impl Handle {
    pub fn begin_drag(&self, uris: Vec<String>, actions: DndAction, scale: i32, visual: DragVisual) {
        self.send(Cmd::BeginDrag {
            uris,
            actions,
            scale,
            visual,
        });
    }

    pub fn update_visual(&self, visual: DragVisual) {
        self.send(Cmd::UpdateVisual(visual));
    }

    /// the popup/icon commands (`open_popup`, `set_icon`, ...) are
    /// implemented next to their modules: `popup_wayland` and
    /// `icon_wayland` both `impl Handle`
    pub(crate) fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
        let _ = (&self.wake).write_all(&[1]);
    }
}

/// Pre-create the GUI-side event channel BEFORE the app starts, so the
/// `Subscription::run` stream picks up the real receiver on its first
/// poll (the fs worker uses the same pattern). The sender is stashed
/// for `spawn` to hand to the thread once the window exists.
pub fn init_channel() -> EventSender {
    let (ev_tx, ev_rx) = iced::futures::channel::mpsc::unbounded::<Event>();
    let _ = crate::DND_STREAM_RX.set(std::sync::Mutex::new(Some(ev_rx)));
    ev_tx
}

/// Wrap winit's Wayland objects (via `raw_handle_wayland`) and spawn
/// the side-channel thread. `handles` is `(wl_surface, wl_display,
/// xdg_toplevel)` from `iced::window::raw_handle`; `ev_tx` comes from
/// [`init_channel`]. `None` when the platform is not Wayland or the
/// wrap fails (the app then simply has no native DnD/popups/icon).
///
/// The thread must never panic: the release profile aborts the process
/// on any panic, so every fallible call in it is handled.
pub fn spawn(handles: (usize, usize, usize), ev_tx: EventSender) -> Option<Handle> {
    let (surface, display, toplevel) = handles;
    if surface == 0 || display == 0 || toplevel == 0 {
        return None;
    }
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Cmd>();
    let (wake_r, wake_w) = rustix::pipe::pipe().ok()?;

    std::thread::Builder::new()
        .name("explorer-wl".into())
        .spawn(move || thread_main(surface, display, toplevel, cmd_rx, ev_tx, wake_r))
        .ok()?;

    Some(Handle {
        tx: cmd_tx,
        wake: std::fs::File::from(wake_w),
    })
}

// ---------------------------------------------------------------------------
// The side-channel thread
// ---------------------------------------------------------------------------

/// Side-channel tracing, enabled with the `EXPLORER_DEBUG` env var
/// (the same switch the GUI debug output uses).
macro_rules! trace {
    ($($t:tt)*) => {
        if std::env::var_os("EXPLORER_DEBUG").is_some() {
            eprintln!($($t)*);
        }
    };
}


fn thread_main(
    surface_ptr: usize,
    display_ptr: usize,
    toplevel_ptr: usize,
    cmds: std::sync::mpsc::Receiver<Cmd>,
    ev_tx: EventSender,
    wake_r: std::os::unix::io::OwnedFd,
) {
    let result = run(surface_ptr, display_ptr, toplevel_ptr, &cmds, &ev_tx, &wake_r);
    if let Err(e) = result {
        eprintln!("wayland side-channel stopped: {e}");
    }
    // keep the sender alive-owner informed: a silent stop means no more
    // drag-out; drag-in events just stop arriving
    drop(ev_tx);
}

fn run(
    surface_ptr: usize,
    display_ptr: usize,
    toplevel_ptr: usize,
    cmds: &std::sync::mpsc::Receiver<Cmd>,
    ev_tx: &EventSender,
    wake_r: &std::os::unix::io::OwnedFd,
) -> Result<(), String> {
    let raw = crate::raw_handle_wayland::wrap((surface_ptr, display_ptr, toplevel_ptr))?;
    let crate::raw_handle_wayland::RawHandles {
        conn,
        origin,
        parent,
        toplevel,
    } = raw;

    // Our own registry + queue on winit's connection.
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    // taskbar/switcher icon support — absent on compositors without the
    // staging global (e.g. mutter); SetIcon then stays a no-op
    let icon_mgr = globals
        .bind::<smithay_client_toolkit::reexports::protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1, _, _>(&qh, 1..=1, ())
        .ok();

    let shm = Shm::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let compositor = CompositorState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let ddm = DataDeviceManagerState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let xdg = smithay_client_toolkit::shell::xdg::XdgShell::bind(&globals, &qh)
        .map_err(|e| e.to_string())?;
    // both or neither: fractional-scale compositors always pair them
    // (wp_fractional_scale_v1 requires wp_viewporter)
    let fractional = match (
        globals.bind(&qh, 1..=1, ()),
        globals.bind(&qh, 1..=1, ()),
    ) {
        (Ok(fsm), Ok(vp)) => Some((fsm, vp)),
        _ => None,
    };
    let icon_pool = SlotPool::new(256 * 256 * 4, &shm).map_err(|e| e.to_string())?;
    // the toplevel icon needs its own pool: its buffers must stay alive
    // AND immutable for the icon's lifetime (protocol no_buffer error),
    // so they may never recycle through the drag-icon pool
    let toplevel_pool = SlotPool::new(192 * 1024, &shm).map_err(|e| e.to_string())?;
    let popup_pool = SlotPool::new(512 * 1024, &shm).map_err(|e| e.to_string())?;
    let mut state = State {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        shm,
        ddm,
        xdg,
        parent,
        toplevel,
        icon_mgr,
        toplevel_icon: None,
        toplevel_pool,
        popup: None,
        popup_pool,
        fractional,
        device: None,
        origin,
        icon_pool,
        icon_surface: None,
        icon_buffer: None,
        icon_scale: 1,
        source: None,
        payload: Vec::new(),
        last_serial: 0,
        ptr_pos: (0.0, 0.0),
        hover_mimes: Vec::new(),
        pending: Vec::new(),
        ev_tx: ev_tx.clone(),
    };

    // learn the seat + pointer capabilities before accepting commands
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;

    loop {
        // 1. drain commands
        while let Ok(cmd) = cmds.try_recv() {
            state.apply_cmd(cmd, &qh);
        }
        // 2. dispatch everything queued
        let _ = queue.dispatch_pending(&mut state);
        let _ = conn.flush();
        // 3. wait for compositor input, a command wake, or drop data.
        // The poll fd set borrows everything; extract plain flags and
        // act afterwards (no borrows held across mutations).
        let (conn_in, wake_in, recv_mask, timed_out) = {
            let conn_b = conn.as_fd();
            // SAFETY: `wake_r` outlives the loop
            let wake_b = unsafe { BorrowedFd::borrow_raw(wake_r.as_raw_fd()) };
            let mut fds = vec![
                rustix::event::PollFd::new(&conn_b, rustix::event::PollFlags::IN),
                rustix::event::PollFd::new(&wake_b, rustix::event::PollFlags::IN),
            ];
            // SAFETY: each pipe lives in `state.pending`, which is not
            // mutated while `fds` is alive
            let recvs: Vec<BorrowedFd<'_>> = state
                .pending
                .iter()
                .map(|p| unsafe { BorrowedFd::borrow_raw(p.pipe.as_raw_fd()) })
                .collect();
            for r in &recvs {
                fds.push(rustix::event::PollFd::new(r, rustix::event::PollFlags::IN));
            }
            // while data is flowing in a pending read, wake periodically
            // to enforce the EOF-free grace completion
            let timeout = if state.pending.iter().any(|p| !p.buf.is_empty()) {
                Some(&rustix::event::Timespec { tv_sec: 0, tv_nsec: 50_000_000 })
            } else {
                None
            };
            let n = rustix::event::poll(&mut fds, timeout).map_err(|e| e.to_string())?;
            let mut mask = 0u32;
            for (i, f) in fds.iter().enumerate().skip(2) {
                if f.revents().contains(rustix::event::PollFlags::IN) {
                    mask |= 1 << i;
                }
            }
            (
                fds[0].revents().contains(rustix::event::PollFlags::IN),
                fds[1].revents().contains(rustix::event::PollFlags::IN),
                mask,
                n == 0,
            )
        };
        if conn_in
            && let Some(guard) = conn.prepare_read()
        {
            // actually pull the events into the queues
            let _ = guard.read();
        }
        if wake_in {
            let mut buf = [0u8; 64];
            let _ = rustix::io::read(wake_r, &mut buf);
        }
        // readable pipes first (index i in `fds` == pending[i-2])
        for i in 2.. {
            if recv_mask & (1 << i) == 0 {
                break;
            }
            state.read_pending(i - 2);
        }
        // grace completion: a pending read with data that has been idle
        // is complete even without EOF (the source may hold the pipe)
        let now = std::time::Instant::now();
        let mut i = 0;
        while i < state.pending.len() {
            let p = &state.pending[i];
            if !p.buf.is_empty() && now.duration_since(p.last_data).as_millis() > 200 {
                trace!("[dnd] grace-complete, {} bytes -> Dropped", p.buf.len());
                let p = state.pending.remove(i);
                let _ = state.ev_tx.unbounded_send(Event::Dropped {
                    mime: p.mime,
                    data: p.buf,
                });
            } else {
                i += 1;
            }
        }
        let _ = timed_out;
    }
}

pub(crate) struct State {
    pub(crate) registry_state: RegistryState,
    pub(crate) seat_state: SeatState,
    pub(crate) output_state: OutputState,
    pub(crate) compositor: CompositorState,
    pub(crate) shm: Shm,
    pub(crate) ddm: DataDeviceManagerState,
    pub(crate) device: Option<DataDevice>,
    /// the explorer window surface (wrapped foreign object) — drag origin
    pub(crate) origin: WlSurface,
    pub(crate) xdg: smithay_client_toolkit::shell::xdg::XdgShell,
    /// the window's xdg_toplevel wrapper — the popup parent chain
    pub(crate) parent: smithay_client_toolkit::shell::xdg::window::Window,
    /// the raw xdg_toplevel proxy — target for xdg_toplevel_icon_v1
    pub(crate) toplevel: smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::XdgToplevel,
    /// `xdg_toplevel_icon_manager_v1`, when the compositor offers it
    pub(crate) icon_mgr: Option<
        smithay_client_toolkit::reexports::protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1,
    >,
    /// the icon currently assigned to the toplevel (kept alive: its
    /// buffers must not be destroyed or modified while assigned)
    pub(crate) toplevel_icon: Option<crate::icon_wayland::ToplevelIconState>,
    pub(crate) toplevel_pool: SlotPool,
    /// the open off-window menu popup, if any
    pub(crate) popup: Option<crate::popup_wayland::PopupState>,
    pub(crate) popup_pool: SlotPool,
    /// fractional-scale globals, when the compositor offers them — the
    /// same condition winit uses for the parent window's fractional
    /// path; the popup then maps its buffer to model-logical units 1:1
    /// via wp_viewport (no downscale on fractional outputs)
    pub(crate) fractional: Option<(WpFractionalScaleManagerV1, WpViewporter)>,
    icon_pool: SlotPool,
    icon_surface: Option<WlSurface>,
    icon_buffer: Option<Buffer>,
    /// wl buffer scale of the icon surface (output-space rendering)
    icon_scale: i32,
    /// live drag: dropping it cancels the drag (SCTK destroys the source)
    source: Option<smithay_client_toolkit::data_device_manager::data_source::DragSource>,
    /// uri-list bytes served to the destination on request
    payload: Vec<u8>,
    /// serial of the last left-button press (grab serial for start_drag)
    last_serial: u32,
    /// last pointer position over the window, in PARENT SURFACE-LOCAL
    /// coordinates — the only exact anchor for popups (model-logical
    /// units differ from surface-local under DPI)
    pub(crate) ptr_pos: (f64, f64),
    /// MIMEs of the drag currently hovering our window
    hover_mimes: Vec<String>,
    /// accepted drops being read from their pipes
    pending: Vec<PendingRead>,
    pub(crate) ev_tx: EventSender,
}

struct PendingRead {
    mime: String,
    pipe: smithay_client_toolkit::data_device_manager::ReadPipe,
    buf: Vec<u8>,
    /// last time payload bytes arrived — EOF-free completion falls back
    /// to a data-idle grace period (some sources hold the pipe open, e.g.
    /// KIO's action menu, even though the uri-list is already complete)
    last_data: std::time::Instant,
}

impl State {
    fn apply_cmd(&mut self, cmd: Cmd, qh: &QueueHandle<State>) {
        match cmd {
            Cmd::BeginDrag {
                uris,
                actions,
                scale,
                visual,
            } => {
                if self.device.is_none() {
                    return;
                }
                // a stale source can only be a drag whose terminal event
                // we have not dispatched yet — cancel it and take over
                self.source.take();
                // the drag icon renders in OUTPUT space (compositor
                // shows it at the cursor), so it needs an explicit
                // buffer scale — without it a 1.5x output enlarges the
                // icon 1.5x. Recreate the surface when the scale
                // changes (buffer scale is fixed per surface).
                if self.icon_scale != scale {
                    self.icon_surface = None;
                    self.icon_scale = scale;
                }
                let source = self.ddm.create_drag_and_drop_source(
                    qh,
                    ["text/uri-list", "text/plain;charset=utf-8"],
                    actions,
                );
                let icon = self.ensure_icon(qh, &visual);
                source.start_drag(self.device.as_ref().unwrap(), &self.origin, icon.as_ref(), self.last_serial);
                // RFC 2483 §5: one URI per line, each line CRLF-
                // terminated (a bare \r between URIs breaks parsers —
                // Dolphin then sees one mangled "path" and reports the
                // files as missing)
                let mut payload = String::new();
                for uri in &uris {
                    payload.push_str(uri);
                    payload.push_str("\r\n");
                }
                self.payload = payload.into_bytes();
                self.source = Some(source);
            }
            Cmd::UpdateVisual(visual) => {
                if self.source.is_some() {
                    self.ensure_icon(qh, &visual);
                }
            }
            Cmd::Popup(c) => crate::popup_wayland::apply(self, c, qh),
            Cmd::Icon(c) => crate::icon_wayland::apply(self, c, qh),
        }
    }

    /// (re)allocate the drag-icon surface/buffer for `visual` and copy
    /// the pixels in (BGRA swizzle: wayland shm is BGRA on LE, we paint
    /// RGBA like tiny-skia).
    /// Copy `visual` into a fresh shm buffer on the (persistent) icon
    /// surface. A NEW buffer every paint: the compositor holds the
    /// previously attached one (the surface stays mapped between
    /// drags), so reusing it would find its slot busy and silently
    /// drop the drag image. Released slots recycle through the pool.
    fn ensure_icon(&mut self, qh: &QueueHandle<State>, visual: &DragVisual) -> Option<WlSurface> {
        let (w, h) = (visual.width.max(1), visual.height.max(1));
        let (buffer, canvas) = self
            .icon_pool
            .create_buffer(w as i32, h as i32, w as i32 * 4, wayland_client::protocol::wl_shm::Format::Argb8888)
            .ok()?;
        if self.icon_surface.is_none() {
            let surface = self.compositor.create_surface(qh);
            surface.set_buffer_scale(self.icon_scale.max(1));
            self.icon_surface = Some(surface);
        }
        let src = swizzle(&visual.pixels);
        // the pool may pad the slot past w*h*4
        let dst = canvas.get_mut(..src.len())?;
        dst.copy_from_slice(&src);
        let surface = self.icon_surface.as_ref()?;
        surface.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surface).ok()?;
        surface.commit();
        self.icon_buffer = Some(buffer);
        Some(surface.clone())
    }

    fn read_pending(&mut self, index: usize) {
        let Some(p) = self.pending.get_mut(index) else { return };
        let mut chunk = [0u8; 8192];
        match p.pipe.read(&mut chunk) {
            Ok(0) => {
                trace!("[dnd] pipe EOF, {} bytes -> Dropped", p.buf.len());
                let p = self.pending.remove(index);
                let _ = self.ev_tx.unbounded_send(Event::Dropped {
                    mime: p.mime,
                    data: p.buf,
                });
            }
            Ok(n) => {
                trace!("[dnd] pipe read {n} bytes (total {})", p.buf.len() + n);
                p.buf.extend_from_slice(&chunk[..n]);
                p.last_data = std::time::Instant::now();
            }
            Err(e) => {
                trace!("[dnd] pipe read error: {e}");
                self.pending.remove(index);
            }
        }
    }
}

// --- SCTK handlers ---------------------------------------------------------

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: wayland_client::protocol::wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
}

fn return_btn(b: u32) -> &'static str {
    match b {
        0x110 => "press-left",
        0x111 => "press-right",
        _ => "press-other",
    }
}

fn return_rel(b: u32) -> &'static str {
    match b {
        0x110 => "rel-left",
        0x111 => "rel-right",
        _ => "rel-other",
    }
}

impl smithay_client_toolkit::shell::xdg::window::WindowHandler for State {
    fn request_close(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::shell::xdg::window::Window,
    ) {
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::shell::xdg::window::Window,
        _: smithay_client_toolkit::shell::xdg::window::WindowConfigure,
        _: u32,
    ) {
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.device.is_none() {
            // our own pointer binding: KWin (and wlroots) deliver pointer
            // events to every binding of the focused surface's client,
            // so this stream carries the grab serials winit never exposes
            let _pointer = self.seat_state.get_pointer(qh, &seat);
            self.device = Some(self.ddm.get_data_device(qh, &seat));
        }
        if capability == Capability::Keyboard {
            // menu popup keys: Esc dismissal (and later, arrow-key nav)
            let _keyboard = self.seat_state.get_keyboard(qh, &seat, None);
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            self.device = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, events: &[PointerEvent]) {
        use PointerEventKind::*;
        for e in events {
            if std::env::var_os("EXPLORER_DEBUG").is_some() {
                let kind = match e.kind {
                    Motion { .. } => "motion",
                    Press { button, .. } => return_btn(button),
                    Release { button, .. } => return_rel(button),
                    Enter { .. } => "enter",
                    Leave { .. } => "leave",
                    _ => "other",
                };
                let on_popup = self
                    .popup
                    .as_ref()
                    .map(|p| &e.surface == p.popup.wl_surface())
                    .unwrap_or(false);
                eprintln!(
                    "[ptr] {kind} surface={} popup={on_popup} pos=({:.0},{:.0})",
                    if e.surface == self.origin { "origin".into() } else { format!("{:?}", e.surface.id()) },
                    e.position.0,
                    e.position.1
                );
            }
            // menu popup input (the popup is our client's surface, so
            // our pointer binding receives its events while the grab
            // lasts; the main window never sees them)
            if crate::popup_wayland::on_pointer(self, e) {
                continue;
            }
            match e.kind {
                // track the pointer over the window in surface-local
                // coordinates (the exact popup anchor) — motion AND
                // presses: a right-click that teleports without a motion
                // event would otherwise leave the anchor stale at (0,0)
                Motion { .. } | Press { .. } | Enter { .. } => {
                    self.ptr_pos = e.position;
                    if let Press { button, serial, .. } = e.kind
                        && button == BTN_LEFT
                    {
                        // a press on the main window while the popup is
                        // open is an outside click (popup_done is not
                        // guaranteed on all compositors)
                        crate::popup_wayland::dismiss(self);
                        self.last_serial = serial;
                    }
                }
                _ => {}
            }
        }
    }
}

impl smithay_client_toolkit::seat::keyboard::KeyboardHandler for State {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: &WlSurface,
        _: u32,
        _: &[u32],
        _: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: &WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        crate::popup_wayland::on_key(self, event.keysym.raw());
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::Modifiers,
        _: smithay_client_toolkit::seat::keyboard::RawModifiers,
        _: u32,
    ) {
    }
}

impl DataSourceHandler for State {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, mime: String, mut fd: WritePipe) {
        if mime == "text/uri-list" {
            let _ = fd.write_all(&self.payload);
        } else {
            // text/plain: serve the first URI (plain-text targets)
            let text = String::from_utf8_lossy(&self.payload);
            let line = text.lines().next().unwrap_or("").as_bytes().to_vec();
            let _ = fd.write_all(&line);
        }
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        self.source.take();
        self.payload.clear();
        let _ = self.ev_tx.unbounded_send(Event::DragFinished { performed: false });
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        self.source.take();
        self.payload.clear();
        let _ = self.ev_tx.unbounded_send(Event::DragFinished { performed: true });
    }
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl DataDeviceHandler for State {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice, x: f64, y: f64, surface: &WlSurface) {
        let same = surface == &self.origin;
        trace!("[dnd] ENTER same_surface={same} x={x:.0} y={y:.0}");
        if !same {
            return;
        }
        if let Some(offer) = device.data::<DataDeviceData>().unwrap().drag_offer() {
            let mimes: Vec<String> = offer.with_mime_types(|m| m.to_vec());
            // external drops into the explorer always copy (Windows
            // semantics; offering Move would make KIO pop its
            // action-choice menu and hold the drop open)
            offer.accept_mime_type(offer.serial, Some("text/uri-list".into()));
            offer.set_actions(DndAction::Copy, DndAction::Copy);
            self.hover_mimes = mimes.clone();
            let _ = self.ev_tx.unbounded_send(Event::Hover { mimes });
        } else {
            trace!("[dnd] ENTER (internal drag, no offer)");
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        trace!("[dnd] LEAVE hover={}", !self.hover_mimes.is_empty());
        if !self.hover_mimes.is_empty() {
            self.hover_mimes.clear();
            let _ = self.ev_tx.unbounded_send(Event::HoverLeft);
        }
    }
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        trace!("[dnd] DROP performed");
        let Some(offer) = device.data::<DataDeviceData>().unwrap().drag_offer() else {
            trace!("[dnd] DROP but no drag_offer!");
            return;
        };
        // prefer uri-list, then any plain text, then the first offered type
        let pick = |mimes: &[String]| -> Option<String> {
            for pref in ["text/uri-list", "text/plain;charset=utf-8", "text/plain", "UTF8_STRING"] {
                if let Some(m) = mimes.iter().find(|m| m.as_str() == pref) {
                    return Some(m.clone());
                }
            }
            mimes.first().cloned()
        };
        let mime = pick(&offer.with_mime_types(|m| m.to_vec()));
        let Some(mime) = mime else { return };
        offer.accept_mime_type(offer.serial, Some(mime.clone()));
        match offer.receive(mime.clone()) {
            Ok(pipe) => {
                trace!("[dnd] receive ok, mime={mime}");
                // finish + destroy IMMEDIATELY: the drop is committed from
                // the protocol's point of view — KWin then tells the
                // source the drag is done and its fd closes. Waiting for
                // EOF first deadlocks: a Copy|Move offer makes KIO hold
                // the pipe open until its action-choice menu is picked.
                // The received fd stays valid after offer.destroy().
                offer.finish();
                offer.destroy();
                self.pending.push(PendingRead {
                    mime,
                    pipe,
                    buf: Vec::new(),
                    last_data: std::time::Instant::now(),
                });
            }
            Err(e) => {
                trace!("[dnd] receive FAILED: {e}");
                offer.finish();
                offer.destroy();
            }
        }
    }
}

impl DataOfferHandler for State {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(State);
wayland_client::delegate_noop!(State: WpFractionalScaleManagerV1);
wayland_client::delegate_noop!(State: WpViewporter);
// wp_viewport has no events; the dispatch2 blanket (below) needs this
// for the () user data attached at get_viewport
impl smithay_client_toolkit::dispatch2::Dispatch2<smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport, State> for () {
    fn event(
        &self,
        _: &mut State,
        _: &smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport,
        _: <smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport as wayland_client::Proxy>::Event,
        _: &wayland_client::Connection,
        _: &wayland_client::QueueHandle<State>,
    ) {
    }
}
impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
smithay_client_toolkit::delegate_dispatch2!(State);


//! Wayland drag & drop side-channel.
//!
//! iced/winit have no Wayland data-device (drag-out payloads, drag-in
//! offers, action negotiation). This module fills the gap without
//! touching winit: a thread owns a second event queue *on winit's
//! Wayland connection* (same client — the protocol requires it; the
//! compositor routes drop events only to the surface's client and
//! validates the drag origin against the focused surface).
//!
//! The two raw pointers come from the tiny local iced patch
//! (`deps/iced-raw-handle.patch`, `iced::window::raw_handle`); see the
//! Makefile "local iced overlay" section. Design + protocol behavior
//! were validated with the /tmp/dnd-spike prototype on KWin:
//! - a second seat binding on the same client receives the full pointer
//!   stream (enter/button serials included) while the app's surface is
//!   focused — no winit serial plumbing needed;
//! - `start_drag` needs the real focused surface as origin (an unmapped
//!   placeholder is cancelled immediately);
//! - offering Copy|Move|Link together makes the *destination* pop the
//!   action-choice menu (matching our ambiguous-drop convention).
//!
//! Split of labor: the thread owns every Wayland object; the GUI thread
//! paints the drag image (icon theme + fonts live there) and sends the
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
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_registry, registry_handlers};
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy as _, QueueHandle};

// ---------------------------------------------------------------------------
// GUI-thread API
// ---------------------------------------------------------------------------

pub type EventSender = iced::futures::channel::mpsc::UnboundedSender<Event>;
pub type EventReceiver = iced::futures::channel::mpsc::UnboundedReceiver<Event>;

/// Commands GUI -> side-channel thread.
enum Cmd {
    /// Start a drag with the given file:// URIs. The serial is the one
    /// the thread captured from the press that armed the drag (the
    /// compositor validates it against the implicit grab).
    BeginDrag {
        uris: Vec<String>,
        actions: DndAction,
        visual: DragVisual,
    },
    /// Re-paint the drag image while a drag is live (caption changes
    /// when the modifier-derived action changes).
    UpdateVisual(DragVisual),
}

/// A fully painted drag image: premultiplied RGBA pixels. The hotspot
/// is (0,0); the painter pre-offsets content so the compositor-placed
/// image lands exactly where the in-window overlays used to.
#[derive(Clone)]
pub struct DragVisual {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Side-channel events, delivered as `Message::Dnd`.
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
}

/// GUI-side handle to the side-channel thread.
pub struct Handle {
    tx: std::sync::mpsc::Sender<Cmd>,
    /// writing a byte wakes the thread's poll loop
    wake: std::fs::File,
}

impl Handle {
    pub fn begin_drag(&self, uris: Vec<String>, actions: DndAction, visual: DragVisual) {
        self.send(Cmd::BeginDrag {
            uris,
            actions,
            visual,
        });
    }

    pub fn update_visual(&self, visual: DragVisual) {
        self.send(Cmd::UpdateVisual(visual));
    }

    fn send(&self, cmd: Cmd) {
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

/// Wrap winit's Wayland objects and spawn the side-channel thread.
/// `handles` is `(wl_surface, wl_display)` from
/// `iced::window::raw_handle`; `ev_tx` comes from [`init_channel`].
/// `None` when the platform is not Wayland or the wrap fails (the app
/// then simply has no native DnD).
///
/// The thread must never panic: the release profile aborts the process
/// on any panic, so every fallible call in it is handled.
pub fn spawn(handles: (usize, usize), ev_tx: EventSender) -> Option<Handle> {
    let (surface, display) = handles;
    if surface == 0 || display == 0 {
        return None;
    }
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Cmd>();
    let (wake_r, wake_w) = rustix::pipe::pipe().ok()?;

    std::thread::Builder::new()
        .name("explorer-dnd".into())
        .spawn(move || thread_main(surface, display, cmd_rx, ev_tx, wake_r))
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
    cmds: std::sync::mpsc::Receiver<Cmd>,
    ev_tx: EventSender,
    wake_r: std::os::unix::io::OwnedFd,
) {
    let result = run(surface_ptr, display_ptr, &cmds, &ev_tx, &wake_r);
    if let Err(e) = result {
        eprintln!("dnd-wayland: side-channel stopped: {e}");
    }
    // keep the sender alive-owner informed: a silent stop means no more
    // drag-out; drag-in events just stop arriving
    drop(ev_tx);
}

fn run(
    surface_ptr: usize,
    display_ptr: usize,
    cmds: &std::sync::mpsc::Receiver<Cmd>,
    ev_tx: &EventSender,
    wake_r: &std::os::unix::io::OwnedFd,
) -> Result<(), String> {
    // SAFETY: both objects belong to this process (winit owns them);
    // wrapping does not take ownership — winit keeps driving the
    // connection's main queue while we only use OUR event queue on it.
    // opaque pointer: the exact foreign type is named by wayland-sys,
    // which we don't depend on — `.cast()` infers it
    let backend = unsafe {
        Backend::from_foreign_display((display_ptr as *mut std::ffi::c_void).cast())
    };
    let conn = Connection::from_backend(backend);
    // SAFETY: the surface exists and belongs to the same client.
    // SAFETY: the surface exists and belongs to the same client.
    let origin_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface_ptr as *mut _) }
        .map_err(|e| e.to_string())?;
    let origin = WlSurface::from_id(&conn, origin_id).map_err(|e| e.to_string())?;

    // Our own registry + queue on winit's connection.
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();

    let shm = Shm::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let compositor = CompositorState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let ddm = DataDeviceManagerState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let icon_pool = SlotPool::new(256 * 256 * 4, &shm).map_err(|e| e.to_string())?;
    let mut state = State {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        shm,
        ddm,
        device: None,
        origin,
        icon_pool,
        icon_surface: None,
        icon_buffer: None,
        source: None,
        payload: Vec::new(),
        last_serial: 0,
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

struct State {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    shm: Shm,
    ddm: DataDeviceManagerState,
    device: Option<DataDevice>,
    /// the explorer window surface (wrapped foreign object) — drag origin
    origin: WlSurface,
    icon_pool: SlotPool,
    icon_surface: Option<WlSurface>,
    icon_buffer: Option<Buffer>,
    /// live drag: dropping it cancels the drag (SCTK destroys the source)
    source: Option<smithay_client_toolkit::data_device_manager::data_source::DragSource>,
    /// uri-list bytes served to the destination on request
    payload: Vec<u8>,
    /// serial of the last left-button press (grab serial for start_drag)
    last_serial: u32,
    /// MIMEs of the drag currently hovering our window
    hover_mimes: Vec<String>,
    /// accepted drops being read from their pipes
    pending: Vec<PendingRead>,
    ev_tx: EventSender,
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
                visual,
            } => {
                if self.device.is_none() {
                    return;
                }
                // a stale source can only be a drag whose terminal event
                // we have not dispatched yet — cancel it and take over
                self.source.take();
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
            self.icon_surface = Some(self.compositor.create_surface(qh));
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

/// RGBA -> BGRA (in place copy).
fn swizzle(rgba: &[u8]) -> Vec<u8> {
    let (chunks, _) = rgba.as_chunks::<4>();
    let mut out = Vec::with_capacity(rgba.len());
    for px in chunks {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}

// --- SCTK handlers ---------------------------------------------------------

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: wayland_client::protocol::wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
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
            match e.kind {
                Press { button, serial, .. } if button == BTN_LEFT => self.last_serial = serial,
                _ => {}
            }
        }
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
impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
smithay_client_toolkit::delegate_dispatch2!(State);

// ---------------------------------------------------------------------------
// Drag-image painting (GUI thread)
// ---------------------------------------------------------------------------

/// The visual the compositor shows at the cursor during a drag: the
/// faded icon-stack badge plus the caption plate — the two tooltips the
/// in-window overlay (`ui::drag_caption` + `ui::drag_icon_badge`) shows,
/// composed into one surface (Wayland has a single drag icon).
///
/// Layout matches the overlay exactly (ui.rs): the caption plate sits at
/// (cursor + 16, cursor + 20); the badge's top-left is
/// (cursor + 16 - 50 - 19, cursor + 20 - 50 - 19). The surface hotspot is
/// (0,0), so content is drawn at (0,0) = badge top-left and the caption
/// plate at (69, 69).
pub fn paint_drag_visual(
    items: &[crate::dnd::DraggedItem],
    top: Option<&str>,
    total: usize,
    caption: &crate::dnd::DragCaption,
    skin: &crate::theme::Palette,
    scale: f32,
) -> DragVisual {
    const BADGE: f32 = 50.0;
    const PAD: f32 = 3.0;
    const AREA: f32 = BADGE - 2.0 * PAD; // 44
    const STEP: f32 = 3.0;
    const MAX: usize = 5;
    const COUNT: f32 = 18.0;
    const GAP: f32 = 19.0; // badge <-> caption
    const FADE: u16 = 153; // 0.6

    let scale = scale.max(1.0);
    // caption plate extent (logical)
    let plate = match caption {
        crate::dnd::DragCaption::Hidden => None,
        crate::dnd::DragCaption::Banned => Some((20.0, 20.0, None)),
        crate::dnd::DragCaption::Undecided => plate_size("…", scale),
        crate::dnd::DragCaption::Action { label, .. } => plate_size(label, scale),
    };
    let (pw, ph) = plate
        .as_ref()
        .map(|(w, h, _)| (*w, *h))
        .unwrap_or((0.0, 0.0));
    // surface covers badge + plate; the plate's top-left is at
    // (DX + BADGE + GAP - (DX - BADGE - GAP)) = 2*(BADGE+GAP) = 138? no:
    // caption_tl - badge_tl = ((DX) - (DX - BADGE - GAP), (DY) - (DY - BADGE - GAP))
    //                      = (BADGE + GAP, BADGE + GAP) = (69, 69)
    let plate_at = BADGE + GAP; // 69
    let w = ((plate_at + pw).max(BADGE) * scale).ceil() as u32;
    let h = ((plate_at + ph).max(BADGE) * scale).ceil() as u32;
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut cv = Canvas { px: &mut px, w, h };

    // --- the badge square (accent @ 0.6) ---
    let accent = skin.accent;
    cv.fill_rect(scale, 0.0, 0.0, BADGE, BADGE, accent, FADE);

    // --- the icon stack, deepest first ---
    let top = top.map(str::to_string);
    let mut shown: Vec<&crate::dnd::DraggedItem> = items
        .iter()
        .filter(|i| Some(i.uri.as_str()) != top.as_deref())
        .take(MAX - 1)
        .collect();
    if let Some(t) = items.iter().find(|i| Some(i.uri.as_str()) == top.as_deref()) {
        shown.insert(0, t);
    } else {
        shown = items.iter().take(MAX).collect();
    }
    let n = shown.len().max(1);
    let icon_size = AREA - (n as f32 - 1.0) * STEP;
    for (j, it) in shown.iter().enumerate().rev() {
        // deepest icon first: index 0 is the top item, drawn last at the
        // largest offset — matches the in-window stack
        let off = (n - 1 - j) as f32 * STEP;
        let name = crate::dnd::uri_to_path(&it.uri)
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_else(|| it.uri.clone());
        let icon = icon_pix(skin, it.kind == crate::dnd::ItemKind::Directory, &name, icon_size * scale);
        cv.blend_icon(scale, PAD + off, PAD + off, icon_size, &icon, FADE);
    }

    // --- the count plate (solid accent, bottom-left) ---
    if total > 1 {
        cv.fill_rect(scale, BADGE - COUNT, BADGE - COUNT, COUNT, COUNT, accent, 255);
        let label = total.to_string();
        let (tw, th) = text_size(&label, 11.0 * scale);
        draw_text(
            &mut cv,
            (BADGE - COUNT + (COUNT - tw / scale) / 2.0) * scale,
            (BADGE - COUNT + (COUNT - th / scale) / 2.0) * scale,
            11.0 * scale,
            &label,
            [255, 255, 255, 255],
        );
    }

    // --- the caption plate ---
    if let Some((pw, ph, text)) = plate {
        let x = plate_at * scale;
        let y = plate_at * scale;
        let pw = pw * scale;
        let ph = ph * scale;
        cv.fill_rect(1.0, x, y, pw, ph, skin.menu_bg, 255);
        // 1px border (physical)
        cv.stroke_rect(x, y, pw, ph, skin.menu_border);
        match (text, caption) {
            (Some(text), _) => {
                let size = 13.0 * scale;
                let (tw, th) = text_size(&text, size);
                draw_text(
                    &mut cv,
                    x + (pw - tw) / 2.0,
                    y + (ph - th) / 2.0,
                    size,
                    &text,
                    rgba255(skin.text),
                );
            }
            // banned glyph: red circle + slash, vector-drawn via resvg
            _ if matches!(caption, crate::dnd::DragCaption::Banned) => {
                let icon = raster_svg_str(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" fill="none" stroke="#E81123" stroke-width="2.2"/><line x1="5.6" y1="14.4" x2="14.4" y2="5.6" stroke="#E81123" stroke-width="2.2"/></svg>"##,
                    20.0 * scale,
                );
                cv.blend_icon(1.0, x, y, 20.0 * scale, &icon, 255);
            }
            _ => {}
        }
    }

    DragVisual {
        width: w,
        height: h,
        pixels: px,
    }
}

fn icon_pix(skin: &crate::theme::Palette, is_dir: bool, name: &str, size: f32) -> crate::icon::IconPix {
    crate::icon::raster_for(is_dir, name, size.round().max(1.0) as u32, skin)
}

fn raster_svg_str(svg: &str, _size: f32) -> crate::icon::IconPix {
    // reuse the icon module's rasterizer through a throwaway builtin name?
    // simplest: inline parse via resvg here
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &Default::default());
    match tree {
        Ok(tree) => {
            let size = tree.size();
            let (w, h) = (size.width().ceil().max(1.0) as u32, size.height().ceil().max(1.0) as u32);
            let mut pm = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
            resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pm.as_mut());
            crate::icon::IconPix {
                width: w,
                height: h,
                pixels: pm.data().to_vec(),
            }
        }
        Err(_) => crate::icon::IconPix {
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        },
    }
}

// --- tiny raster helpers (all coordinates physical px unless *scale) ---

fn rgba255(c: iced::Color) -> [u8; 4] {
    [
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    ]
}

struct Canvas<'a> {
    px: &'a mut [u8],
    w: u32,
    h: u32,
}

impl Canvas<'_> {
    #[allow(clippy::too_many_arguments)]
    fn fill_rect(&mut self, scale: f32, x: f32, y: f32, rw: f32, rh: f32, c: iced::Color, alpha: u16) {
        let c = rgba255(c);
        let x0 = (x * scale) as i32;
        let y0 = (y * scale) as i32;
        let x1 = ((x + rw) * scale).ceil() as i32;
        let y1 = ((y + rh) * scale).ceil() as i32;
        for yy in y0.max(0)..y1.min(self.h as i32) {
            for xx in x0.max(0)..x1.min(self.w as i32) {
                self.blend(xx, yy, c, alpha as u8);
            }
        }
    }

    fn stroke_rect(&mut self, x: f32, y: f32, rw: f32, rh: f32, c: iced::Color) {
        let c = rgba255(c);
        let x1 = (x + rw - 1.0) as i32;
        let y1 = (y + rh - 1.0) as i32;
        for xx in x as i32..=x1 {
            self.blend(xx, y as i32, c, 255);
            self.blend(xx, y1, c, 255);
        }
        for yy in y as i32..=y1 {
            self.blend(x as i32, yy, c, 255);
            self.blend(x1, yy, c, 255);
        }
    }

    fn blend_icon(&mut self, scale: f32, x: f32, y: f32, size: f32, icon: &crate::icon::IconPix, fade: u16) {
        // fit the icon into size*size, centered, blended at `fade`/255
        let (iw, ih) = (icon.width as f32, icon.height as f32);
        if iw <= 0.0 || ih <= 0.0 {
            return;
        }
        let fit = (size * scale) / iw.max(ih);
        let dw = (iw * fit) as i32;
        let dh = (ih * fit) as i32;
        let ox = (x * scale) as i32 + ((size * scale) as i32 - dw) / 2;
        let oy = (y * scale) as i32 + ((size * scale) as i32 - dh) / 2;
        for j in 0..dh {
            for i in 0..dw {
                let sx = (i as f32 / fit) as u32;
                let sy = (j as f32 / fit) as u32;
                if sx >= icon.width || sy >= icon.height {
                    continue;
                }
                let o = ((sy * icon.width + sx) * 4) as usize;
                let a = icon.pixels[o + 3] as u16 * fade / 255;
                self.blend(
                    ox + i,
                    oy + j,
                    [icon.pixels[o], icon.pixels[o + 1], icon.pixels[o + 2], 255],
                    a as u8,
                );
            }
        }
    }

    /// Source-over blend of `color` (premultiplied by mask) at (x, y).
    fn blend(&mut self, x: i32, y: i32, color: [u8; 4], mask: u8) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return;
        }
        let a = color[3] as u16 * mask as u16 / 255;
        let inv = 255 - a;
        let p = &mut self.px[((y as usize * self.w as usize) + x as usize) * 4..][..4];
        for c in 0..3 {
            let src = color[c] as u16 * a / 255;
            p[c] = (src + p[c] as u16 * inv / 255) as u8;
        }
        p[3] = (a + p[3] as u16 * inv / 255) as u8;
    }
}

// --- text (cosmic-text, cached font system) ---

fn text_system() -> &'static std::sync::Mutex<(cosmic_text::FontSystem, cosmic_text::SwashCache)> {
    static TS: std::sync::OnceLock<std::sync::Mutex<(cosmic_text::FontSystem, cosmic_text::SwashCache)>> =
        std::sync::OnceLock::new();
    TS.get_or_init(|| {
        std::sync::Mutex::new((cosmic_text::FontSystem::new(), cosmic_text::SwashCache::new()))
    })
}

fn plate_size(text: &str, scale: f32) -> Option<(f32, f32, Option<String>)> {
    let size = 13.0 * scale;
    let (w, h) = text_size(text, size);
    // padding [2, 7] logical + border, like boxed_caption
    Some((w / scale + 14.0 + 2.0, h / scale + 4.0 + 2.0, Some(text.to_string())))
}

fn text_size(text: &str, size: f32) -> (f32, f32) {
    let mut ts = text_system().lock().unwrap();
    let (fs, _) = &mut *ts;
    let mut buffer = cosmic_text::Buffer::new(fs, cosmic_text::Metrics::new(size, size * 1.25));
    buffer.set_text(text, &cosmic_text::Attrs::new(), cosmic_text::Shaping::Basic, None);
    buffer.shape_until_scroll(fs, false);
    let mut w = 0.0f32;
    let mut h = 0.0f32;
    for run in buffer.layout_runs() {
        w = w.max(run.line_w);
        h = h.max(run.line_top + run.line_height);
    }
    (w, h)
}

fn draw_text(cv: &mut Canvas, x: f32, y: f32, size: f32, text: &str, color: [u8; 4]) {
    let mut ts = text_system().lock().unwrap();
    let (fs, cache) = &mut *ts;
    let mut buffer = cosmic_text::Buffer::new(fs, cosmic_text::Metrics::new(size, size * 1.25));
    buffer.set_text(text, &cosmic_text::Attrs::new(), cosmic_text::Shaping::Basic, None);
    buffer.shape_until_scroll(fs, false);
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical = glyph.physical((x, y + run.line_y), 1.0);
            let Some(img) = cache.get_image(fs, physical.cache_key).clone() else {
                continue;
            };
            let iw = img.placement.width as i32;
            let ih = img.placement.height as i32;
            let ox = physical.x + img.placement.left;
            let oy = physical.y + img.placement.top;
            for j in 0..ih {
                for i in 0..iw {
                    let m = img.data[(j * iw + i) as usize];
                    cv.blend(ox + i, oy + j, color, m);
                }
            }
        }
    }
}

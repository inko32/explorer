//! Wayland drag-in/out demo: a window with a file icon you can drag out
//! (the compositor shows the icon at the cursor, even outside the window),
//! and a drop zone that accepts external file drops.
//!
//! All DnD logic lives in `dnd.rs` behind a small facade — that module is
//! the prototype for the explorer integration. Everything here is just
//! window + input + painting glue.

mod dnd;
mod draw;
mod text;

use std::num::NonZeroU32;
use std::path::Path;
use std::time::Duration;

use dnd::{Dnd, DragOut};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::data_device_manager::data_device::{
    DataDeviceData, DataDeviceHandler,
};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler;
use smithay_client_toolkit::data_device_manager::WritePipe;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopHandle, PostAction};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::seat::pointer::{
    CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shell::xdg::{XdgShell, XdgSurface};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_registry, registry_handlers};
use std::io::Read as _;

use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, Proxy as _, QueueHandle};

const DRAG_PATH: &str = "/tmp/a.txt";
const DEFAULT_W: u32 = 560;
const DEFAULT_H: u32 = 400;

fn main() {
    let conn = Connection::connect_to_env().expect("connect to the compositor (WAYLAND_DISPLAY)");
    let (globals, event_queue) = registry_queue_init(&conn).expect("registry");
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<App> = EventLoop::try_new().expect("event loop");
    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .expect("wayland source");

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let xdg_shell = XdgShell::bind(&globals, &qh).expect("xdg_wm_base");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let dnd = Dnd::new(&globals, &qh).expect("wl_data_device_manager");

    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_title("Wayland DnD demo — /tmp/a.txt");
    window.set_app_id("dnd-demo");
    window.set_min_size(Some((400, 320)));
    window.commit();

    let cursor_surface = compositor.create_surface(&qh);
    let pool = SlotPool::new(DEFAULT_W as usize * DEFAULT_H as usize * 4, &shm).expect("slot pool");

    let mut app = App {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        shm,
        dnd,
        window,
        width: DEFAULT_W,
        height: DEFAULT_H,
        scale: 1,
        buffer: None,
        pool,
        cursor_surface,
        pointer: None,
        ptr_serial: 0,
        press: None,
        drag_icon: None,
        pending_drop: None,
        text: text::Text::new(),
        received: None,
        outcome: None,
        exit: false,
        dirty: true,
        loop_handle: event_loop.handle(),
        _conn: conn,
    };

    while !app.exit {
        event_loop.dispatch(Duration::from_millis(16), &mut app).expect("dispatch");
        if app.dirty {
            app.dirty = false;
            let _ = app.redraw();
        }
    }
}

struct DragIcon {
    surface: wl_surface::WlSurface,
    _pool: SlotPool,
    _buffer: Buffer,
}

struct PendingDrop {
    offer: DragOffer,
    mime: String,
    data: Vec<u8>,
}

struct App {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    shm: Shm,
    dnd: Dnd,
    window: Window,
    width: u32,
    height: u32,
    scale: i32,
    buffer: Option<Buffer>,
    pool: SlotPool,
    cursor_surface: wl_surface::WlSurface,
    pointer: Option<smithay_client_toolkit::seat::pointer::ThemedPointer>,
    ptr_serial: u32,
    /// pointer press position that may become a drag
    press: Option<(f64, f64)>,
    drag_icon: Option<DragIcon>,
    pending_drop: Option<PendingDrop>,
    text: text::Text,
    /// (mime, body) of the last external drop
    received: Option<(String, String)>,
    outcome: Option<DragOut>,
    exit: bool,
    dirty: bool,
    loop_handle: LoopHandle<'static, App>,
    _conn: Connection,
}

impl App {
    fn status_string(&self) -> String {
        if self.dnd.drag_out_active() {
            "dragging /tmp/a.txt — drop it on a file manager, the desktop, or back here".into()
        } else if self.dnd.external_hover() {
            format!("external drag hovering ({}) — release to accept", self.dnd.hover_mime().unwrap_or("?"))
        } else if let Some((mime, body)) = &self.received {
            format!("received drop: {mime} → {body}")
        } else if let Some(outcome) = &self.outcome {
            match outcome {
                DragOut::DroppedOutside => {
                    "dropped outside ✔ — the target app received file:///tmp/a.txt".into()
                }
                DragOut::Cancelled => "drag cancelled".into(),
                DragOut::Active => "dragging…".into(),
            }
        } else if Path::new(DRAG_PATH).exists() {
            format!("idle — {DRAG_PATH} exists, drag the icon out")
        } else {
            format!("idle — create {DRAG_PATH} first:  echo hi > {DRAG_PATH}")
        }
    }

    fn redraw(&mut self) -> Option<()> {
        // drain anything the dnd layer produced
        if let Some(o) = self.dnd.take_outcome() {
            self.outcome = Some(o);
        }
        if let Some(r) = self.dnd.take_received() {
            self.received = Some((r.mime, r.body));
        }

        let scale = self.scale.max(1);
        let pw = self.width * scale as u32;
        let ph = self.height * scale as u32;
        if self.buffer.is_none() {
            let (b, _) = self
                .pool
                .create_buffer(pw as i32, ph as i32, pw as i32 * 4, wl_shm::Format::Argb8888)
                .ok()?;
            self.buffer = Some(b);
        }
        let status = self.status_string();
        let scene = draw::Scene {
            width: self.width,
            height: self.height,
            dragging_out: self.dnd.drag_out_active(),
            hover: self.dnd.external_hover(),
            hover_mime: self.dnd.hover_mime(),
            received: self.received.as_ref(),
            status: &status,
        };

        let buffer = self.buffer.as_ref().unwrap();
        let canvas = self.pool.canvas(buffer)?; // compositor may still hold it
        draw::paint(canvas, pw, ph, scale as f32, &scene, &mut self.text);
        // tiny-skia is RGBA; wl_shm Argb8888 is BGRA on little-endian.
        // (Abgr8888 would avoid the swap but this compositor rejects it.)
        for px in canvas.chunks_exact_mut(4) {
            let b: &mut [u8; 4] = px.try_into().unwrap();
            b.swap(0, 2);
        }

        let surface = self.window.wl_surface();
        surface.damage_buffer(0, 0, pw as i32, ph as i32);
        buffer.attach_to(surface).expect("attach");
        self.window.commit();
        Some(())
    }

    /// Create (once) the drag-icon surface: the glyph centered so the
    /// cursor hotspot lands in the middle of the icon.
    fn ensure_drag_icon(&mut self, qh: &QueueHandle<App>) -> Option<wl_surface::WlSurface> {
        if let Some(icon) = &self.drag_icon {
            return Some(icon.surface.clone());
        }
        const GLYPH_W: f32 = 88.0;
        const GLYPH_H: f32 = 112.0;
        const PAD: f32 = 48.0; // on every side; cursor at the center
        let scale = self.scale.max(1);
        let pw = ((GLYPH_W + PAD * 2.0) * scale as f32) as u32;
        let ph = ((GLYPH_H + PAD * 2.0) * scale as f32) as u32;
        // keep the pool alive as long as the buffer (see DragIcon fields)
        let mut pool = SlotPool::new((pw * ph * 4) as usize, &self.shm).expect("icon pool");
        let (buffer, canvas) = match pool.create_buffer(pw as i32, ph as i32, pw as i32 * 4, wl_shm::Format::Argb8888) {
            Ok(v) => v,
            Err(_) => return None,
        };
        let mut pm = tiny_skia::PixmapMut::from_bytes(canvas, pw, ph).unwrap();
        let ts = tiny_skia::Transform::from_scale(scale as f32, scale as f32);
        draw::paint_icon(
            &mut pm,
            ts,
            draw::Rect { x: PAD, y: PAD, w: GLYPH_W, h: GLYPH_H },
            255,
            &mut self.text,
            scale as f32,
        );
        for px in canvas.chunks_exact_mut(4) {
            let b: &mut [u8; 4] = px.try_into().unwrap();
            b.swap(0, 2);
        }
        let surface = self.compositor.create_surface(qh);
        surface.set_buffer_scale(scale);
        surface.damage_buffer(0, 0, pw as i32, ph as i32);
        buffer.attach_to(&surface).expect("icon attach");
        surface.commit();
        self.drag_icon = Some(DragIcon { surface: surface.clone(), _pool: pool, _buffer: buffer });
        Some(surface)
    }
}

// --- SCTK plumbing: each handler forwards to `self.dnd` -------------------

impl DataSourceHandler for App {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource, _: Option<String>) {}

    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource, mime: String, fd: WritePipe) {
        self.dnd.fwd_source_send(&mime, fd);
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource) {
        self.dnd.fwd_source_cancelled();
        self.dirty = true;
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource) {
        self.dnd.fwd_source_dropped();
    }

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource) {
        self.dnd.fwd_source_finished();
        self.dirty = true;
    }

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_source::WlDataSource, _: smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction) {}
}

impl DataDeviceHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &wayland_client::protocol::wl_data_device::WlDataDevice, _: f64, _: f64, surface: &wl_surface::WlSurface) {
        if surface != self.window.wl_surface() {
            return;
        }
        if let Some(offer) = device.data::<DataDeviceData>().unwrap().drag_offer() {
            self.dnd.fwd_device_enter(&offer);
            self.dirty = true;
        }
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_device::WlDataDevice) {
        self.dnd.fwd_device_leave();
        self.dirty = true;
    }

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_device::WlDataDevice, _: f64, _: f64) {}

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_data_device::WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &wayland_client::protocol::wl_data_device::WlDataDevice) {
        let Some(offer) = device.data::<DataDeviceData>().unwrap().drag_offer() else {
            return;
        };
        let Some((offer, mime, pipe)) = self.dnd.fwd_drop_performed(&offer) else {
            return;
        };
        self.pending_drop = Some(PendingDrop { offer, mime, data: Vec::new() });
        let res = self.loop_handle.insert_source(pipe, |_, pipe, state: &mut App| {
            // SAFETY: the file is removed from the loop (PostAction::Remove)
            // before the source (and thus the fd) is dropped.
            let f: &mut std::fs::File = unsafe { pipe.get_mut() };
            let mut chunk = [0u8; 4096];
            match f.read(&mut chunk) {
                Ok(0) => {
                    if let Some(p) = state.pending_drop.take() {
                        state.dnd.complete_receive(&p.mime, &p.data);
                        p.offer.finish();
                        p.offer.destroy();
                        state.dirty = true;
                    }
                    PostAction::Remove
                }
                Ok(n) => {
                    if let Some(p) = &mut state.pending_drop {
                        p.data.extend_from_slice(&chunk[..n]);
                    }
                    PostAction::Continue
                }
                Err(_) => {
                    if let Some(p) = state.pending_drop.take() {
                        p.offer.finish();
                        p.offer.destroy();
                    }
                    PostAction::Remove
                }
            }
        });
        if res.is_err()
            && let Some(p) = self.pending_drop.take()
        {
            p.offer.finish();
            p.offer.destroy();
        }
    }
}

impl DataOfferHandler for App {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction) {}

    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction) {}
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, new_factor: i32) {
        if new_factor > 0 && new_factor != self.scale {
            self.scale = new_factor;
            self.window.wl_surface().set_buffer_scale(new_factor);
            self.buffer = None;
            self.dirty = true;
        }
    }

    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}

    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window, configure: WindowConfigure, serial: u32) {
        self.width = configure.new_size.0.map(NonZeroU32::get).unwrap_or(DEFAULT_W);
        self.height = configure.new_size.1.map(NonZeroU32::get).unwrap_or(DEFAULT_H);
        window.xdg_surface().ack_configure(serial);
        self.buffer = None;
        self.dirty = true;
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(pointer) = self.seat_state.get_pointer_with_theme::<App, ()>(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.cursor_surface.clone(),
                ThemeSpec::System,
            ) {
                self.pointer = Some(pointer);
            }
            self.dnd.add_seat(qh, &seat);
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, conn: &Connection, qh: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        use PointerEventKind::*;
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            match event.kind {
                Enter { serial } => {
                    self.ptr_serial = serial;
                    if let Some(p) = &self.pointer {
                        let _ = p.set_cursor(conn, CursorIcon::Default);
                    }
                }
                Leave { .. } => {
                    self.press = None;
                }
                Motion { .. } => {
                    if let Some((px, py)) = self.press {
                        let dx = event.position.0 - px;
                        let dy = event.position.1 - py;
                        if dx * dx + dy * dy > 100.0 {
                            self.press = None;
                            let icon = self.ensure_drag_icon(qh);
                            self.dnd.start_file_drag(
                                qh,
                                self.window.wl_surface(),
                                icon.as_ref(),
                                self.ptr_serial,
                                Path::new(DRAG_PATH),
                            );
                            self.dirty = true;
                        }
                    }
                }
                Press { button, serial, .. } if button == smithay_client_toolkit::seat::pointer::BTN_LEFT => {
                    self.ptr_serial = serial;
                    if !self.dnd.drag_out_active() && draw::ICON.contains(event.position.0, event.position.1) {
                        self.press = Some(event.position);
                    }
                }
                Release { .. } => {
                    self.press = None;
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(App);

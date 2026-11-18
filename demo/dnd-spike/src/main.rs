//! SPIKE: can a second event queue on the same Wayland client
//! (simulating the future side-channel next to winit/iced) do DnD against
//! a surface owned by the "main" queue?
//!
//! Queue 1 = pretend winit: owns window surface A, has a pointer binding.
//! Queue 2 = pretend dnd-wayland side channel: own seat binding (pointer +
//! data device), unmapped surface B as drag origin, drag icon surface.
//!
//! Hypotheses (KWin):
//!   H1: wl_pointer@q2 gets enter/motion/button over surface A.
//!   H2: start_drag(origin=B) with a q2 button serial works; drop onto
//!       another app transfers file:///tmp/a.txt.
//!   H3: wl_data_device@q2 gets offers/enter for drags over surface A.

use std::io::{Read as _, Write as _};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::data_device_manager::data_device::{DataDevice, DataDeviceData, DataDeviceHandler};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::{DataSourceHandler, DragSource};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::xdg::window::{Window, WindowConfigure, WindowDecorations, WindowHandler};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgSurface;
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::reexports::client::Proxy;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_registry, registry_handlers};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::{wl_data_device, wl_data_source, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, Proxy as _, QueueHandle};

fn main() {
    let conn = Connection::connect_to_env().unwrap();
    // queue 1 — pretend this is winit's queue
    let (globals, q1) = registry_queue_init::<Spike>(&conn).unwrap();
    let qh1 = q1.handle();
    let compositor = CompositorState::bind(&globals, &qh1).unwrap();
    let xdg_shell = XdgShell::bind(&globals, &qh1).unwrap();
    let shm = Shm::bind(&globals, &qh1).unwrap();

    let surface = compositor.create_surface(&qh1);
    let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &qh1);
    window.set_title("dnd-spike: pretend winit window (left-click = start drag)");
    window.commit();
    let pool = SlotPool::new(400 * 300 * 4, &shm).unwrap();

    // queue 2 — pretend this is the dnd side channel. EVERYTHING dnd
    // (seat, pointer, data device) is bound on THIS queue, like the real
    // side-channel thread will do on winit's connection.
    let mut q2 = conn.new_event_queue::<Spike>();
    let qh2 = q2.handle();
    let seat_state = SeatState::new(&globals, &qh2);
    let ddm = DataDeviceManagerState::bind(&globals, &qh2).unwrap();

    // drag origin: an UNMAPPED surface on q2 (same client as the window!)
    let origin = compositor.create_surface(&qh2);
    // drag icon: small red square, surface on q2
    let mut icon_pool = SlotPool::new(32 * 32 * 4, &shm).unwrap();
    let (icon_buffer, data) = icon_pool.create_buffer(32, 32, 32 * 4, wl_shm::Format::Argb8888).unwrap();
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 0, 0xff, 0xff]); // red (BGRA)
    }
    let icon = compositor.create_surface(&qh2);
    icon_buffer.attach_to(&icon).unwrap();
    icon.damage_buffer(0, 0, 32, 32);
    icon.commit();

    let mut spike = Spike {
        registry_state: RegistryState::new(&globals),
        seat_state,
        output_state: OutputState::new(&globals, &qh1),
        shm,
        window,
        pool,
        _icon_pool: icon_pool,
        buffer: None,
        configured: false,
        exit: false,
        // q2 bits
        ddm,
        device: None,
        q2_pointer: None,
        origin,
        icon,
        source: None,
    };

    // Two queues share one fd, so calloop (one loop) can't own both.
    // Manual pump: this mirrors the real side-channel thread design.
    let mut q1 = q1;
    let mut q2 = q2;
    let mut iters: u64 = 0;
    loop {
        q1.dispatch_pending(&mut spike).unwrap();
        q2.dispatch_pending(&mut spike).unwrap();
        conn.flush().unwrap();
        if spike.exit {
            break;
        }
        iters += 1;
        if iters % 100_000_000 == 0 {
            eprintln!("[loop] alive, {} iterations", iters);
        }
        // blocks until the compositor has events for either queue; the
        // guard (dropped at the end of the if) releases the read lock.
        match conn.prepare_read() {
            Some(guard) => {
                let mut fds = [rustix::event::PollFd::new(&conn, rustix::event::PollFlags::IN)];
                if rustix::event::poll(&mut fds, None).is_ok() {
                    // actually pull the events into the queues
                    let _ = guard.read();
                }
            }
            None => eprintln!("[loop] prepare_read returned None (spin)"),
        }
    }
}

struct Spike {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    window: Window,
    pool: SlotPool,
    _icon_pool: SlotPool,
    buffer: Option<Buffer>,
    configured: bool,
    exit: bool,
    // q2 side-channel bits
    ddm: DataDeviceManagerState,
    device: Option<DataDevice>,
    q2_pointer: Option<wl_pointer::WlPointer>,
    origin: wl_surface::WlSurface,
    icon: wl_surface::WlSurface,
    source: Option<DragSource>,
}

impl Spike {
    fn draw(&mut self) {
        if self.buffer.is_none() {
            let (b, c) = self.pool.create_buffer(400, 300, 400 * 4, wl_shm::Format::Argb8888).unwrap();
            let col: Vec<u8> = std::env::args()
                .skip(1)
                .take(3)
                .map(|a| u8::from_str_radix(&a, 16).unwrap_or(0x40))
                .collect();
            let (cr, cg, cb) = match col.len() {
                3 => (col[0], col[1], col[2]),
                _ => (0x40, 0x40, 0x60),
            };
            for px in c.chunks_exact_mut(4) {
                px.copy_from_slice(&[cb, cg, cr, 0xff]);
            }
            self.buffer = Some(b);
        }
        let b = self.buffer.as_ref().unwrap();
        self.window.wl_surface().damage_buffer(0, 0, 400, 300);
        b.attach_to(self.window.wl_surface()).unwrap();
        self.window.commit();
    }
}

impl CompositorHandler for Spike {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for Spike {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for Spike {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, w: &Window, c: WindowConfigure, serial: u32) {
        w.xdg_surface().ack_configure(serial);
        if !self.configured {
            self.configured = true;
            self.draw();
            eprintln!("[q1] window mapped");
        }
    }
}

impl SeatHandler for Spike {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.q2_pointer.is_none() {
            // NOTE: binding the pointer on **q2** — this is the whole point of
            // the spike: do we still get events while q1's surface is focused?
            let ptr = self.seat_state.get_pointer(qh, &seat).unwrap();
            self.q2_pointer = Some(ptr);
            self.device = Some(self.ddm.get_data_device(qh, &seat));
            eprintln!("[q2] seat bound: pointer + data device");
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            self.q2_pointer = None;
            self.device = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Spike {
    fn pointer_frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, pointer: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        use PointerEventKind::*;
        let is_q2 = self.q2_pointer.as_ref() == Some(pointer);
        for e in events {
            match e.kind {
                Enter { serial } if is_q2 => eprintln!("[q2] pointer ENTER serial={serial}"),
                Press { button, serial, .. } if is_q2 && button == BTN_LEFT => {
                    eprintln!("[q2] LEFT PRESS serial={serial} -> start_drag(origin=unmapped surface)");
                    let source = self.ddm.create_drag_and_drop_source(
                        qh,
                        ["text/uri-list", "text/plain;charset=utf-8"],
                        DndAction::Copy,
                    );
                    // KWin requires origin == the pointer-focused surface
                    source.start_drag(self.device.as_ref().unwrap(), self.window.wl_surface(), Some(&self.icon), serial);
                    self.source = Some(source);
                }
                Release { .. } if is_q2 && self.source.is_some() => eprintln!("[q2] release (drag still active until finish/cancel)"),
                _ => {}
            }
        }
    }
}

impl DataSourceHandler for Spike {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource, mime: Option<String>) {
        eprintln!("[q2] source accept: {mime:?}");
    }
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource, mime: String, mut fd: WritePipe) {
        eprintln!("[q2] SEND {mime}");
        fd.write_all(b"file:///tmp/a.txt\r\n").unwrap();
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource) {
        eprintln!("[q2] source CANCELLED");
        self.source = None;
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource) {
        eprintln!("[q2] source DROPPED (drop happened on a target)");
    }
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource) {
        eprintln!("[q2] source FINISHED — drop committed");
        self.source = None;
    }
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_source::WlDataSource, a: DndAction) {
        eprintln!("[q2] action selected: {a:?}");
    }
}

impl DataDeviceHandler for Spike {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, dev: &wl_data_device::WlDataDevice, x: f64, y: f64, surface: &wl_surface::WlSurface) {
        if let Some(offer) = dev.data::<DataDeviceData>().unwrap().drag_offer() {
            let mimes: Vec<String> = offer.with_mime_types(|m| m.to_vec());
            eprintln!("[q2] DnD ENTER x={x:.0} y={y:.0} mimes={mimes:?} (surface id {:?})", surface.id());
            offer.accept_mime_type(offer.serial, Some("text/uri-list".into()));
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        } else {
            eprintln!("[q2] DnD ENTER (internal drag, no offer)");
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_device::WlDataDevice) {
        eprintln!("[q2] DnD LEAVE");
    }
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_device::WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_data_device::WlDataDevice) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, dev: &wl_data_device::WlDataDevice) {
        eprintln!("[q2] DROP performed");
        if let Some(offer) = dev.data::<DataDeviceData>().unwrap().drag_offer() {
            // block on a thread so the dispatch loop stays responsive
            match offer.receive("text/uri-list".into()) {
                Ok(mut pipe) => std::thread::spawn(move || {
                    let mut s = String::new();
                    match pipe.read_to_string(&mut s) {
                        Ok(_) => eprintln!("[q2] RECEIVED: {s:?}"),
                        Err(e) => eprintln!("[q2] receive error: {e}"),
                    }
                }),
                Err(e) => {
                    eprintln!("[q2] receive setup error: {e}");
                    return;
                }
            };
            offer.finish();
            offer.destroy();
        }
    }
}

impl DataOfferHandler for Spike {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl ShmHandler for Spike {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(Spike);
impl ProvidesRegistryState for Spike {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
smithay_client_toolkit::delegate_dispatch2!(Spike);

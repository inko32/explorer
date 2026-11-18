//! Wayland drag & drop behind a small facade — this is the piece meant to be
//! ported back into the explorer.
//!
//! Port plan for the real app: winit (and therefore iced) has no Wayland
//! data-device support, so this module will move to a side-channel thread
//! owning its own `wayland_client::Connection` (wrapped around winit's
//! foreign `wl_display`). The UI talks to it over a channel with exactly
//! the calls/events mirrored here:
//!
//! UI -> dnd:  `start_file_drag`, `add_seat`
//! dnd -> UI:  `drag_out_active`, `take_outcome`, `external_hover`,
//!             `take_received`
//!
//! The `fwd_*` methods are the only glue that changes: here they are fed
//! from the SCTK trait impls in `main.rs`; in the explorer they are fed
//! from the side-channel thread's dispatch loop.

use std::io::Write;
use std::path::Path;

use smithay_client_toolkit::data_device_manager::data_device::DataDeviceData;
use smithay_client_toolkit::data_device_manager::data_source::{DataSourceData, DragSource};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, ReadPipe, WritePipe};
use smithay_client_toolkit::data_device_manager::data_device::DataDevice;
use smithay_client_toolkit::data_device_manager::data_offer::DragOffer;
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::reexports::client::globals::{BindError, GlobalList};
use smithay_client_toolkit::reexports::client::protocol::wl_data_device::WlDataDevice;
use smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::{
    DndAction, WlDataDeviceManager,
};
use smithay_client_toolkit::reexports::client::protocol::wl_data_source::WlDataSource;
use smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Dispatch, QueueHandle};

/// MIME types we serve/accept, most preferred first.
const MIME_TYPES: [&str; 2] = ["text/uri-list", "text/plain;charset=utf-8"];

/// Outcome of a drag the user started inside our window.
///
/// (`Active` is part of the API surface even though this demo never
/// constructs it: the explorer integration will forward live drag state.)
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragOut {
    /// Drag is live; the compositor is showing the drag icon at the cursor
    /// (also outside our window).
    Active,
    /// Released over a target that accepted the data.
    DroppedOutside,
    /// Cancelled (Esc pressed, or no target accepted).
    Cancelled,
}

/// Result of an external drop onto our window.
pub struct Received {
    pub mime: String,
    pub body: String,
}

pub struct Dnd {
    manager: DataDeviceManagerState,
    /// one data device per seat; the demo assumes a single seat
    device: Option<DataDevice>,
    /// live outgoing drag: dropping this struct cancels the drag (SCTK
    /// destroys the wl_data_source on Drop)
    out: Option<OutDrag>,
    outcome: Option<DragOut>,
    /// negotiated mime while an external drag hovers our window
    hover_mime: Option<String>,
    received: Option<Received>,
}

struct OutDrag {
    /// never read: keeping the DragSource alive IS the drag; dropping it
    /// cancels it
    _source: DragSource,
    /// bytes served to the target when it requests a mime (uri-list format)
    payload: Vec<u8>,
}

impl Dnd {
    pub fn new<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Result<Self, BindError>
    where
        D: Dispatch<WlDataSource, DataSourceData<()>>
            + Dispatch<WlDataDevice, DataDeviceData>
            + Dispatch<WlDataDeviceManager, GlobalData, D>
            + ProvidesRegistryState
            + 'static,
    {
        Ok(Self {
            manager: DataDeviceManagerState::bind(globals, qh)?,
            device: None,
            out: None,
            outcome: None,
            hover_mime: None,
            received: None,
        })
    }

    /// Bind the seat's data device (call once per seat that has a pointer).
    pub fn add_seat<D>(&mut self, qh: &QueueHandle<D>, seat: &WlSeat)
    where
        D: Dispatch<WlDataDevice, DataDeviceData> + 'static,
    {
        if self.device.is_none() {
            self.device = Some(self.manager.get_data_device(qh, seat));
        }
    }

    /// Start dragging `path` (advertised as a `file://` URI) out of `origin`.
    ///
    /// `icon` is shown by the compositor at the cursor — also outside our
    /// window — until the drag ends; its top-left is the hotspot. `serial`
    /// must be the pointer-grab serial of the press that started the drag.
    pub fn start_file_drag<D>(
        &mut self,
        qh: &QueueHandle<D>,
        origin: &WlSurface,
        icon: Option<&WlSurface>,
        serial: u32,
        path: &Path,
    ) where
        D: Dispatch<WlDataSource, DataSourceData<()>> + 'static,
    {
        if self.out.is_some() || self.device.is_none() {
            return;
        }
        let source = self.manager.create_drag_and_drop_source(
            qh,
            MIME_TYPES,
            DndAction::Copy | DndAction::Move,
        );
        source.start_drag(self.device.as_ref().unwrap(), origin, icon, serial);
        self.out = Some(OutDrag {
            _source: source,
            payload: format!("{}\r\n", file_uri(path)).into_bytes(),
        });
        self.outcome = None;
    }

    /// True while a drag started here is live (compositing owns the icon).
    pub fn drag_out_active(&self) -> bool {
        self.out.is_some()
    }

    /// Last finished drag outcome, if any (take clears it).
    pub fn take_outcome(&mut self) -> Option<DragOut> {
        self.outcome.take()
    }

    /// True while an external drag hovers our window and we accepted it.
    pub fn external_hover(&self) -> bool {
        self.hover_mime.is_some()
    }

    pub fn hover_mime(&self) -> Option<&str> {
        self.hover_mime.as_deref()
    }

    /// Last payload dropped onto us, if any (take clears it).
    pub fn take_received(&mut self) -> Option<Received> {
        self.received.take()
    }

    // --- forwards from the dispatch layer (see module doc) ---------------

    /// `wl_data_source.send`: the target requested the payload.
    pub fn fwd_source_send(&mut self, _mime: &str, mut fd: WritePipe) {
        if let Some(out) = &self.out {
            let _ = fd.write_all(&out.payload);
        }
        // dropping the fd closes it; that is the "eof" for the reader side
    }

    /// `wl_data_source.dnd_drop_performed`.
    pub fn fwd_source_dropped(&mut self) {}

    /// `wl_data_source.dnd_finished`: the drop completed, source is spent.
    pub fn fwd_source_finished(&mut self) {
        self.out.take();
        self.outcome = Some(DragOut::DroppedOutside);
    }

    /// `wl_data_source.cancelled`.
    pub fn fwd_source_cancelled(&mut self) {
        self.out.take();
        self.outcome = Some(DragOut::Cancelled);
    }

    /// `wl_data_device.enter`: an external drag entered our surface.
    pub fn fwd_device_enter(&mut self, offer: &DragOffer) {
        let mime = offer.with_mime_types(pick_mime);
        if let Some(mime) = mime {
            offer.accept_mime_type(offer.serial, Some(mime.clone()));
            offer.set_actions(DndAction::Copy | DndAction::Move, DndAction::Copy);
            self.hover_mime = Some(mime);
        }
    }

    /// `wl_data_device.leave`.
    pub fn fwd_device_leave(&mut self) {
        self.hover_mime = None;
    }

    /// `wl_data_device.drop`: open the read pipe for the negotiated mime.
    /// The caller pumps the pipe and feeds the bytes to `complete_receive`.
    pub fn fwd_drop_performed(&mut self, offer: &DragOffer) -> Option<(DragOffer, String, ReadPipe)> {
        let mime = self.hover_mime.clone().or_else(|| offer.with_mime_types(pick_mime))?;
        offer.accept_mime_type(offer.serial, Some(mime.clone()));
        offer.receive(mime.clone()).ok().map(|pipe| (offer.clone(), mime, pipe))
    }

    /// Called when the read pipe hits EOF; the offer is finished/destroyed
    /// by the caller (it owns the DragOffer clone).
    pub fn complete_receive(&mut self, mime: &str, bytes: &[u8]) {
        self.received = Some(Received {
            mime: mime.to_string(),
            body: String::from_utf8_lossy(bytes)
                .trim_end_matches(['\r', '\n'])
                .to_string(),
        });
        self.hover_mime = None;
    }
}

/// Pick the most useful mime from an offer, most preferred first.
fn pick_mime(mimes: &[String]) -> Option<String> {
    for pref in MIME_TYPES {
        if let Some(m) = mimes.iter().find(|m| m.as_str() == pref) {
            return Some(m.clone());
        }
    }
    // unknown content: take the first advertised type so the user still
    // sees *what* the object is
    mimes.first().cloned()
}

/// `/tmp/a.txt` -> `file:///tmp/a.txt`
pub fn file_uri(path: &Path) -> String {
    let mut s = String::from("file://");
    for c in path.as_os_str().to_string_lossy().chars() {
        // minimal percent-encoding for the characters that break URIs
        if c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~') {
            s.push(c);
        } else {
            s.push_str(&format!("%{:02X}", c as u32));
        }
    }
    s
}

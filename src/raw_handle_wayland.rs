//! Wrapping the raw Wayland handles from `iced::window::raw_handle`.
//!
//! `deps/iced-raw-handle.patch` (see `deps/iced-raw-handle.md`) adds
//! `iced::window::raw_handle`, which hands out three opaque pointers of
//! the explorer window: `(wl_surface, wl_display, xdg_toplevel)`. Every
//! side-channel duty — DnD (`dnd_wayland.rs`), off-window popups
//! (`popup_wayland.rs`), the toplevel icon (`icon_wayland.rs`) — has to
//! run on THAT connection: the compositor routes drop events/grabs and
//! popup/icon parents only between objects of the same client, so a
//! separate client connection cannot stand in.
//!
//! This module turns the pointers into typed proxies
//! (wayland-client's `from_foreign_display` / `from_ptr`). Wrapping
//! does not take ownership: winit keeps driving the connection's main
//! queue while the side-channel only uses its own event queue on it.

use smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy as _};

/// The explorer window's Wayland objects, wrapped for the
/// side-channel thread.
pub(crate) struct RawHandles {
    /// winit's connection — shared with the GUI thread's event loop
    pub conn: Connection,
    /// the explorer window surface — the drag origin (drop events
    /// route to its client); also the popup parent surface
    pub origin: WlSurface,
    /// the window's xdg_toplevel as an sctk window — the popup parent
    /// chain
    pub parent: smithay_client_toolkit::shell::xdg::window::Window,
    /// the raw xdg_toplevel proxy — target for xdg_toplevel_icon_v1
    pub toplevel: XdgToplevel,
}

/// Wrap `(wl_surface, wl_display, xdg_toplevel)` (pointers cast to
/// usize) into typed proxies. Fails when a wrap fails or the toplevel
/// is not an sctk-managed window (then there is no popup parent chain,
/// and the side-channel cannot start).
pub(crate) fn wrap(handles: (usize, usize, usize)) -> Result<RawHandles, String> {
    let (surface_ptr, display_ptr, toplevel_ptr) = handles;
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
    let origin_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface_ptr as *mut _) }
        .map_err(|e| e.to_string())?;
    let origin = WlSurface::from_id(&conn, origin_id).map_err(|e| e.to_string())?;
    // the window's xdg_toplevel, wrapped for the popup parent chain
    // SAFETY: same — winit owns it, same client, outlives the thread's
    // use of it (the window never goes away in this app)
    let toplevel_id =
        unsafe { ObjectId::from_ptr(XdgToplevel::interface(), toplevel_ptr as *mut _) }
            .map_err(|e| e.to_string())?;
    let toplevel = XdgToplevel::from_id(&conn, toplevel_id).map_err(|e| e.to_string())?;
    let parent = smithay_client_toolkit::shell::xdg::window::Window::from_xdg_toplevel(&toplevel)
        .ok_or("xdg_toplevel is not an sctk-managed window")?;
    Ok(RawHandles {
        conn,
        origin,
        parent,
        toplevel,
    })
}

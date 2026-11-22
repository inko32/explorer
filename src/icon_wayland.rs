//! Toplevel icon on Wayland (`xdg_toplevel_icon_v1`).
//!
//! winit/iced cannot set a taskbar/window-switcher icon on Wayland
//! (`set_window_icon` is a no-op there), so the side-channel thread
//! (`dnd_wayland.rs`) drives the staging protocol from the same
//! registry/queue; this module owns the command and the thread-side
//! apply. Compositors without the global (e.g. mutter) just never get
//! an icon pushed — everything stays a silent no-op.
//!
//! Protocol constraints: the `wl_buffer`s must be SQUARE
//! (`invalid_buffer`) and stay alive AND immutable for the icon's
//! lifetime (`no_buffer`), so each assignment gets a fresh icon object
//! backed by its own shm pool — its buffers may never recycle through
//! a pool that serves re-painted content.

use smithay_client_toolkit::reexports::protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use smithay_client_toolkit::reexports::protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_v1::XdgToplevelIconV1;
use smithay_client_toolkit::shm::slot::Buffer;
use wayland_client::{Connection, QueueHandle};

use crate::dnd_wayland::{Handle, State};
use crate::drag_paint::ToplevelIcon;

/// The icon assigned to our toplevel. The protocol requires the
/// wl_buffers to stay alive and unmodified until the icon object is
/// destroyed, so both are held here and only released together when a
/// new icon replaces this one.
pub(crate) struct ToplevelIconState {
    /// held (never read) so the icon object stays alive until replaced
    _icon: XdgToplevelIconV1,
    _buffers: Vec<Buffer>,
}

/// Icon commands GUI -> side-channel thread.
pub(crate) enum Cmd {
    /// Set the compositor's toplevel icon (taskbar/window-switcher),
    /// replacing any previous one. Ignored when the compositor has no
    /// `xdg_toplevel_icon_manager_v1`.
    Set(ToplevelIcon),
}

impl Handle {
    pub fn set_icon(&self, icon: ToplevelIcon) {
        self.send(crate::dnd_wayland::Cmd::Icon(Cmd::Set(icon)));
    }
}

/// Thread-side command handling (called from `State::apply_cmd`).
pub(crate) fn apply(state: &mut State, cmd: Cmd, qh: &QueueHandle<State>) {
    let Cmd::Set(payload) = cmd;
    let Some(mgr) = &state.icon_mgr else {
        return;
    };
    // buffers must be SQUARE (protocol invalid_buffer error);
    // the GUI guarantees width == height. A fresh icon object
    // per change: an icon is immutable once set on a toplevel.
    let mut buffers = Vec::with_capacity(payload.sizes.len());
    let icon = mgr.create_icon(qh, ());
    for size in &payload.sizes {
        let (w, h) = (size.visual.width.max(1), size.visual.height.max(1));
        let Ok((buffer, canvas)) = state.toplevel_pool.create_buffer(
            w as i32,
            h as i32,
            w as i32 * 4,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        ) else {
            icon.destroy();
            return;
        };
        let src = crate::drag_paint::swizzle(&size.visual.pixels);
        let Some(dst) = canvas.get_mut(..src.len()) else {
            icon.destroy();
            return;
        };
        dst.copy_from_slice(&src);
        icon.add_buffer(buffer.wl_buffer(), size.scale.max(1));
        buffers.push(buffer);
    }
    mgr.set_icon(&state.toplevel, Some(&icon));
    // applied on the toplevel's next commit (winit redraws);
    // only now may the replaced icon (and its buffers) die
    state.toplevel_icon = Some(ToplevelIconState {
        _icon: icon,
        _buffers: buffers,
    });
}

// the icon manager's icon_size/done hints are optional preferences —
// we ship fixed 64/128px sizes, so the events are parsed and dropped
// (delegate_noop! is unusable here: it panics on interfaces WITH events)
impl wayland_client::Dispatch<XdgToplevelIconManagerV1, ()> for State {
    fn event(
        _: &mut State,
        _: &XdgToplevelIconManagerV1,
        _: <XdgToplevelIconManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}
wayland_client::delegate_noop!(State: XdgToplevelIconV1);


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

use crate::dnd_wayland::{DragVisual, Handle, State};

/// Pixels for one toplevel-icon assignment: one square buffer per
/// wl scale, premultiplied RGBA (like [`DragVisual`]).
pub struct ToplevelIcon {
    pub sizes: Vec<IconSize>,
}

pub struct IconSize {
    /// wl buffer scale the pixels were rendered for (e.g. 1, 2)
    pub scale: i32,
    /// square `width == height`
    pub visual: DragVisual,
}

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
        let src = crate::dnd_wayland::swizzle(&size.visual.pixels);
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

// ---------------------------------------------------------------------------
// Icon painting (GUI thread)
// ---------------------------------------------------------------------------

/// Paint the toplevel (taskbar/window-switcher) icon: the location icon
/// centered on a transparent SQUARE canvas at each wl scale — the
/// protocol only accepts square shm buffers. Two sizes cover common
/// taskbars (16-32px at scale 1, hidpi at scale 2).
pub fn paint_toplevel_icon(name: &str, skin: &crate::theme::Palette) -> ToplevelIcon {
    let sizes = [(1, 64), (2, 128)]
        .into_iter()
        .map(|(scale, px)| {
            let icon = crate::icon::raster(name, px, skin);
            let mut visual = DragVisual {
                width: px,
                height: px,
                pixels: vec![0; (px * px * 4) as usize],
            };
            let mut cv = crate::dnd_wayland::Canvas {
                px: &mut visual.pixels,
                w: px,
                h: px,
            };
            cv.blend_icon(1.0, 0.0, 0.0, px as f32, &icon, 255);
            IconSize { scale, visual }
        })
        .collect();
    ToplevelIcon { sizes }
}

//! Off-window menu popups on Wayland (`xdg_popup`).
//!
//! The app paints the menu into pixels (`menu.rs::paint_popup`, GUI
//! thread); this module hosts them in a real `xdg_popup` on the
//! side-channel thread (`dnd_wayland.rs`). The anchor is the thread's
//! own tracked pointer position (parent surface-local — exact, no unit
//! conversion). The popup surface lives in the COMPOSITOR's coordinate
//! space (1 unit = 1/system physical px), so its on-screen size and
//! crispness follow the raw winit scale, not the painted one — with
//! `--dpi` the two differ (see `Cmd::Open` below).
//!
//! Input: while the grab lasts, the thread's own pointer/keyboard
//! bindings receive the popup's events (it is our client's surface);
//! the main window never sees them. `on_pointer`/`on_key` are fed
//! from the thread's sctk handlers and translate to [`Event`]s.

use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, BTN_LEFT};
use smithay_client_toolkit::shm::slot::SlotPool;
use wayland_client::{Connection, QueueHandle};

use crate::dnd_wayland::{Event, Handle, State};
use crate::drag_paint::DragVisual;

/// Popup commands GUI -> side-channel thread.
pub(crate) enum Cmd {
    /// Open an off-window menu popup, showing the painted pixels.
    /// `painted` is the scale the visual's pixels were painted at
    /// (scale_dpi); `system` is the raw winit scale of the parent
    /// output. The popup surface lives in the COMPOSITOR's coordinate
    /// space (1 unit = 1/system physical px), so its on-screen size
    /// and crispness follow `system`, not `painted` — with `--dpi`
    /// the two differ.
    Open {
        painted: f32,
        system: f32,
        visual: DragVisual,
    },
    /// Re-paint an open popup (hover changes).
    Update(DragVisual),
    /// Close the popup.
    Close,
}

impl Handle {
    pub fn open_popup(&self, painted: f32, system: f32, visual: DragVisual) {
        self.send(crate::dnd_wayland::Cmd::Popup(Cmd::Open {
            painted,
            system,
            visual,
        }));
    }

    pub fn update_popup(&self, visual: DragVisual) {
        self.send(crate::dnd_wayland::Cmd::Popup(Cmd::Update(visual)));
    }

    pub fn close_popup(&self) {
        self.send(crate::dnd_wayland::Cmd::Popup(Cmd::Close));
    }
}

/// Thread-side command handling (called from `State::apply_cmd`).
pub(crate) fn apply(state: &mut State, cmd: Cmd, qh: &QueueHandle<State>) {
    match cmd {
        Cmd::Open {
            painted,
            system,
            visual,
        } => open(state, painted, system, visual, qh),
        Cmd::Update(visual) => {
            if let Some(p) = &mut state.popup {
                commit_popup_pixels(&mut state.popup_pool, p, visual);
            }
        }
        Cmd::Close => close(state),
    }
}

/// Close an open popup without an event (GUI-initiated close: the
/// caller already knows).
pub(crate) fn close(state: &mut State) {
    state.popup = None;
}

/// Compositor/GUI-external dismissal (outside click on the main
/// window, or `popup_done`): clear and notify the GUI.
pub(crate) fn dismiss(state: &mut State) {
    if state.popup.take().is_some() {
        let _ = state.ev_tx.unbounded_send(Event::PopupDone);
    }
}

/// Feed a pointer event from the thread's `PointerHandler`; returns
/// true when it hit the popup (the caller must then skip its own
/// handling).
pub(crate) fn on_pointer(state: &mut State, e: &PointerEvent) -> bool {
    let Some(p) = &state.popup else { return false };
    if &e.surface != p.popup.wl_surface() {
        return false;
    }
    // popup-local -> model-logical for hit-testing
    let f = p.to_model;
    let (x, y) = (e.position.0 as f32 * f, e.position.1 as f32 * f);
    match e.kind {
        PointerEventKind::Motion { .. } => {
            let _ = state.ev_tx.unbounded_send(Event::PopupMotion { x, y });
        }
        PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
            let _ = state.ev_tx.unbounded_send(Event::PopupPress { x, y });
        }
        _ => {}
    }
    true
}

/// Feed a pressed keysym from the thread's `KeyboardHandler` (the
/// popup holds the grab while open; Esc dismissal arrives here).
pub(crate) fn on_key(state: &mut State, keysym: u32) {
    if state.popup.is_some() {
        let _ = state.ev_tx.unbounded_send(Event::PopupKey { keysym });
    }
}

/// An open xdg_popup hosting an off-window menu.
pub(crate) struct PopupState {
    pub(crate) popup: smithay_client_toolkit::shell::xdg::popup::Popup,
    /// pixels waiting for the first configure before they can attach
    pending: Option<DragVisual>,
    /// popup-local -> model-logical factor (hover hit-testing)
    pub(crate) to_model: f32,
    /// 1:1 buffer mapping on fractional-capable compositors
    viewport: Option<WpViewport>,
}

impl Drop for PopupState {
    fn drop(&mut self) {
        if let Some(vp) = &self.viewport {
            vp.destroy();
        }
    }
}

/// xdg_positioner userdata: the positioner has no events, but
/// wayland-client still wants a Dispatch impl — a local type keeps it
/// off the delegate_dispatch2 blanket (and the orphan rule blocks
/// implementing sctk's Dispatch2 for sctk's GlobalData).
struct PosData;

impl<D> smithay_client_toolkit::dispatch2::Dispatch2<
    smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::XdgPositioner,
    D,
> for PosData
{
    fn event(
        &self,
        _: &mut D,
        _: &smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::XdgPositioner,
        _: smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::Event,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
    }
}

/// Open (or replace) the menu popup at the tracked pointer position.
fn open(state: &mut State, painted: f32, system: f32, visual: DragVisual, qh: &QueueHandle<State>) {
    // replace any open popup
    state.popup = None;
    let anchor = (state.ptr_pos.0.round() as i32, state.ptr_pos.1.round() as i32);
    let pos = state.xdg.xdg_wm_base().create_positioner(qh, PosData);
    let (w, h) = (visual.width.max(1) as i32, visual.height.max(1) as i32);
    // logical size/anchor: the parent surface-local space,
    // i.e. the COMPOSITOR's coordinate system (1 unit =
    // 1/system physical px). The buffer holds `painted` px
    // per model unit, so it maps 1 physical px per buffer
    // px (crisp, same on-screen size as the window
    // content) only when the unit is `system`:
    //     surface-local = buffer / system
    //     to_model      = system / painted
    // Without `--dpi` painted == system and this reduces to
    // the model size. On integer-only compositors (no
    // fractional globals) fall back to the plain
    // buffer-scale chain, which matches the parent's
    // truncated integer scale.
    let painted = painted.max(1.0);
    let system = system.max(1.0);
    let fractional = state.fractional.is_some();
    let (ls, unit) = if fractional {
        (system.ceil().max(1.0) as i32, system)
    } else {
        let ls = painted.ceil().max(1.0) as i32;
        (ls, ls as f32)
    };
    let (pw, ph) = (
        (w as f32 / unit).round().max(1.0) as i32,
        (h as f32 / unit).round().max(1.0) as i32,
    );
    pos.set_size(pw, ph);
    pos.set_anchor_rect(anchor.0, anchor.1, 1, 1);
    use smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::{
        Anchor, ConstraintAdjustment, Gravity,
    };
    pos.set_anchor(Anchor::TopLeft);
    // the surface is placed TOWARDS the gravity: bottom_right
    // puts the menu's top-left at the cursor (extends
    // down-right); the constraint adjustment flips it at
    // screen edges
    pos.set_gravity(Gravity::BottomRight);
    // let the compositor flip/slide the menu to stay on screen
    pos.set_constraint_adjustment(
        ConstraintAdjustment::FlipX
            | ConstraintAdjustment::FlipY
            | ConstraintAdjustment::SlideX
            | ConstraintAdjustment::SlideY,
    );
    use smithay_client_toolkit::shell::xdg::XdgSurface as _;
    if let Ok(popup) = smithay_client_toolkit::shell::xdg::popup::Popup::new(
        state.parent.xdg_surface(),
        &pos,
        qh,
        &state.compositor,
        &state.xdg,
    ) {
        popup.wl_surface().set_buffer_scale(ls);
        let viewport = fractional.then(|| {
            let vp = state
                .fractional
                .as_ref()
                .expect("fractional globals")
                .1
                .get_viewport(popup.wl_surface(), qh, ());
            vp.set_destination(pw.max(1), ph.max(1));
            vp
        });
        let to_model = unit / painted;
        state.popup = Some(PopupState {
            popup,
            pending: Some(visual),
            to_model,
            viewport,
        });
    }
    pos.destroy();
}

/// Copy menu pixels into a fresh buffer on the popup surface (new
/// slot every paint; the compositor may hold the previous one).
fn commit_popup_pixels(pool: &mut SlotPool, popup: &mut PopupState, visual: DragVisual) {
    let (w, h) = (visual.width.max(1), visual.height.max(1));
    let Ok((buffer, canvas)) = pool.create_buffer(
        w as i32,
        h as i32,
        w as i32 * 4,
        wayland_client::protocol::wl_shm::Format::Argb8888,
    ) else {
        return;
    };
    let src = crate::drag_paint::swizzle(&visual.pixels);
    let Some(dst) = canvas.get_mut(..src.len()) else {
        return;
    };
    dst.copy_from_slice(&src);
    let surface = popup.popup.wl_surface();
    surface.damage_buffer(0, 0, w as i32, h as i32);
    if buffer.attach_to(surface).is_ok() {
        surface.commit();
    }
}

impl smithay_client_toolkit::shell::xdg::popup::PopupHandler for State {
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::shell::xdg::popup::Popup,
        _: smithay_client_toolkit::shell::xdg::popup::PopupConfigure,
    ) {
        // first configure: attach the waiting pixels
        if let Some(p) = &mut self.popup
            && let Some(visual) = p.pending.take()
        {
            commit_popup_pixels(&mut self.popup_pool, p, visual);
        }
    }

    fn done(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::shell::xdg::popup::Popup,
    ) {
        dismiss(self);
    }
}

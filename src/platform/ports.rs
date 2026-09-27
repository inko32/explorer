//! Window control seam. Border module talks to the window only through this
//! trait, so the whole app can run headless (NullPort) for snapshots.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResizeDir {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub trait WindowPort {
    fn drag_move(&self);
    fn drag_resize(&self, dir: ResizeDir);
    fn minimize(&self);
    fn toggle_maximize(&self);
    fn is_maximized(&self) -> bool;
}

/// Headless stand-in for verification renders.
pub struct NullPort;

impl WindowPort for NullPort {
    fn drag_move(&self) {}
    fn drag_resize(&self, _dir: ResizeDir) {}
    fn minimize(&self) {}
    fn toggle_maximize(&self) {}
    fn is_maximized(&self) -> bool {
        false
    }
}

/// Real window backed by winit (Wayland compositor requests).
pub struct WinitPort {
    pub window: std::sync::Arc<winit::window::Window>,
}

impl WindowPort for WinitPort {
    fn drag_move(&self) {
        let _ = self.window.drag_window();
    }
    fn drag_resize(&self, dir: ResizeDir) {
        let d = match dir {
            ResizeDir::Left => winit::window::ResizeDirection::West,
            ResizeDir::Right => winit::window::ResizeDirection::East,
            ResizeDir::Top => winit::window::ResizeDirection::North,
            ResizeDir::Bottom => winit::window::ResizeDirection::South,
            ResizeDir::TopLeft => winit::window::ResizeDirection::NorthWest,
            ResizeDir::TopRight => winit::window::ResizeDirection::NorthEast,
            ResizeDir::BottomLeft => winit::window::ResizeDirection::SouthWest,
            ResizeDir::BottomRight => winit::window::ResizeDirection::SouthEast,
        };
        let _ = self.window.drag_resize_window(d);
    }
    fn minimize(&self) {
        self.window.set_minimized(true);
    }
    fn toggle_maximize(&self) {
        let max = self.window.is_maximized();
        self.window.set_maximized(!max);
    }
    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }
}

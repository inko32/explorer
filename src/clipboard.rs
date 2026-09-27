//! Internal file clipboard (D6: UI never touches fs directly).
//!
//! System clipboard integration (text + file URIs for KDE/GTK targets)
//! replaces/extends this later without touching UI modules.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug)]
pub struct Clipboard {
    pub mode: ClipMode,
    pub paths: Vec<std::path::PathBuf>,
}

impl Clipboard {
    pub fn new_copy(paths: Vec<std::path::PathBuf>) -> Self {
        Self { mode: ClipMode::Copy, paths }
    }
    pub fn new_cut(paths: Vec<std::path::PathBuf>) -> Self {
        Self { mode: ClipMode::Cut, paths }
    }
}

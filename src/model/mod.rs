//! Application state (MVU model) — pure logic, no widgets.
//!
//! Split by concern:
//! - [`message`]: the `Message` enum + the small UI-state enums
//! - [`explorer`]: the `Explorer` state, the `update` loop and the
//!   `handle` dispatcher (window chrome, layout, fs events)
//! - [`folder`]: folder-view logic (view steps, selection, marquee,
//!   item geometry, column drags)
//! - [`nav`]: the navigation tree cache (`NavTree`) + reveal/scroll
//! - [`menu`]: context menus
//! - [`dnd`]: the drag & drop lifecycle
//! - [`ops`]: file operations (clipboard, delete, rename)
//! - [`ribbon`]: the ribbon commanding layer (context, shell commands)
//! - [`format`]: date/size/type label formatting helpers
//! - [`tests`]: end-to-end drag & drop tests through the MVU loop

mod dnd;
mod explorer;
mod folder;
mod format;
mod menu;
mod message;
mod nav;
mod ops;
mod ribbon;
#[cfg(test)]
mod tests;

pub use explorer::Explorer;
pub use format::{format_date, format_size, type_label};
pub use message::{FocusDir, FrameStyle, Message, NavSection, Pane, SortBy};

//! The navigation tree cache: expanded nodes + folder/archive children
//! per listed path.


use super::explorer::Explorer;
use super::message::Message;

use std::path::{Path, PathBuf};

use crate::fs::{self, Job, Scheduler};

/// Navigation tree state: which nodes are open and the cached
/// folder/archive children of every listed node.
pub struct NavTree {
    pub favorites_open: bool,
    pub network_open: bool,
    /// Expanded folder/archive paths.
    pub expanded: std::collections::BTreeSet<PathBuf>,
    /// path -> its folder/archive children (filled from every List
    /// result; freed bottom-up when a subtree collapses).
    pub children: std::collections::HashMap<PathBuf, Vec<fs::Entry>>,
    /// Scroll offset + viewport height of the tree scrollable.
    pub scroll_y: f32,
    pub viewport_h: f32,
    /// Sub-row wheel remainder: the tree scrolls whole lines, so small
    /// (touchpad) deltas accumulate here until they fill a row.
    pub scroll_rem: f32,
}

impl NavTree {
    pub(super) fn new() -> Self {
        Self {
            favorites_open: true,
            network_open: false,
            expanded: Default::default(),
            children: Default::default(),
            scroll_y: 0.0,
            viewport_h: 0.0,
            scroll_rem: 0.0,
        }
    }

    /// Keep only folders and archives, hidden entries out unless requested,
    /// sorted folders-first by name (Windows tree order).
    pub(super) fn store(
        &mut self,
        path: &Path,
        entries: &[fs::Entry],
        show_hidden: bool,
        sched: &Scheduler,
    ) {
        let mut v: Vec<fs::Entry> = entries
            .iter()
            .filter(|e| {
                (show_hidden || !e.is_hidden)
                    && (e.is_dir() || fs::archive::is_archive_path(&e.path))
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        // an expanded child whose entry `drop_subtree` freed must be
        // re-listed, otherwise it renders empty forever (nothing else
        // re-lists a node that is already in `expanded`)
        for k in &v {
            if self.expanded.contains(&k.path) && !self.children.contains_key(&k.path) {
                sched.send(Job::List(k.path.clone()));
            }
        }
        self.children.insert(path.to_path_buf(), v);
    }

    /// Free the listings of `path` and its whole subtree. Called when
    /// the node collapses: the invisible subtree's cache goes back to
    /// the allocator, bounding the cache by the visible tree. Expansion
    /// state survives in `expanded`; `store` re-lists an expanded child
    /// whose entry is missing, so the subtree restores on re-expansion.
    /// Each path leaves the map at most once, so symlink cycles only
    /// cost duplicate frontier walks.
    pub(super) fn drop_subtree(&mut self, path: &Path) {
        let Some(kids) = self.children.remove(path) else {
            return;
        };
        let mut frontier: Vec<PathBuf> = kids.into_iter().map(|k| k.path).collect();
        while let Some(p) = frontier.pop() {
            if let Some(kids) = self.children.remove(&p) {
                frontier.extend(kids.into_iter().map(|k| k.path));
            }
        }
    }

    /// Request a listing for `path` when it is not cached yet.
    pub(super) fn ensure_listed(&self, sched: &Scheduler, path: &Path) {
        if !self.children.contains_key(path) {
            sched.send(Job::List(path.to_path_buf()));
        }
    }
}

impl Explorer {
    /// Expand the cwd's ancestors in the nav tree so the current location
    /// stays visible (Win7 "expand to current folder" behavior), then
    /// scroll the tree to the current row. Volumes sit directly under
    /// Computer, so inside a mounted volume the chain stops at the mount
    /// point (the tree does not re-descend through /media/... to reach
    /// the drive it already shows). Listings that are still in flight
    /// defer the reveal via `request_reveal`.
    pub(crate) fn nav_reveal_cwd(&mut self) -> iced::Task<Message> {
        let boundary = self
            .volumes
            .iter()
            .filter(|v| v.mount != Path::new("/") && self.cwd.starts_with(&v.mount))
            .map(|v| v.mount.as_path())
            .max_by_key(|m| m.components().count());
        let mut targets: Vec<PathBuf> = Vec::new();
        let mut p: &Path = &self.cwd;
        loop {
            targets.push(p.to_path_buf());
            if Some(p) == boundary {
                break;
            }
            match p.parent() {
                Some(parent) => p = parent,
                None => break,
            }
        }
        self.request_reveal = targets.iter().any(|t| !self.nav.children.contains_key(t));
        for t in targets {
            self.nav.expanded.insert(t.clone());
            self.nav.ensure_listed(&self.sched, &t);
        }
        if self.request_reveal {
            iced::Task::none()
        } else {
            self.nav_scroll_to_cwd()
        }
    }

    /// Scroll the nav tree so the current-folder row is visible. Only
    /// correct once every reveal target is cached (the row offsets are
    /// computed from the tree contents).
    pub(crate) fn nav_scroll_to_cwd(&self) -> iced::Task<Message> {
        let Some(idx) = crate::nav_pane::selected_row_index(self) else {
            return iced::Task::none();
        };
        let (_, item_h, ..) = self.ribbon.metrics();
        // favorites rows precede the tree; reveal targets sit below them
        let favs = if self.nav.favorites_open {
            crate::nav_pane::favorites_len() + 1
        } else {
            1
        };
        let row = (favs + idx) as f32;
        let padding = 8.0; // column padding(4) top + bottom
        let viewport = self.nav.viewport_h;
        if viewport <= 0.0 {
            return iced::Task::none();
        }
        let content_h = padding + (crate::nav_pane::tree_row_count(self) as f32) * item_h;
        let max_scroll = (content_h - viewport).max(0.0);
        let row_top = 4.0 + row * item_h;
        let row_bottom = row_top + item_h;
        let target = if row_top < self.nav.scroll_y {
            // navigating up: leave a couple of rows of context above the
            // row (ancestors/siblings) instead of pinning it flush to
            // the top — mirrors the down case which shows ancestors
            // above the row pinned at the bottom.
            (row_top - 3.0 * item_h).max(0.0)
        } else if row_bottom > self.nav.scroll_y + viewport {
            row_bottom - viewport
        } else {
            return iced::Task::none(); // already visible
        };
        // whole-line scrolling: keep the top row complete (a clipped
        // row at the bottom edge is fine)
        let target = (target / item_h).round() * item_h;
        let y = (target.clamp(0.0, max_scroll) / max_scroll.max(f32::EPSILON)).clamp(0.0, 1.0);
        iced::widget::operation::scrollable::snap_to(
            crate::nav_pane::nav_scrollable_id(),
            iced::widget::operation::scrollable::RelativeOffset {
                x: None,
                y: Some(y),
            },
            iced::widget::operation::Animation::Instant,
        )
    }
}


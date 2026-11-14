//! Folder-view update logic: view-step switching with scroll anchoring,
//! selection (click/Ctrl/Shift/rubber-band), item geometry and the
//! Details column drags. The `handle` match delegates here.

use iced::Task;

use crate::fs;
use crate::view_step as vs;

use super::explorer::Explorer;
use super::message::{FocusDir, Message};

/// Grid adjacency shared by the Tiles / SmallIcon / IconTop families:
/// left/right stay in the row, up/down keep the column (no wrap-around).
fn grid_neighbor(i: usize, n: usize, cols: usize, dir: FocusDir) -> Option<usize> {
    // then(), not then_some(): the subtraction arguments must not be
    // evaluated when the guard fails (debug builds overflow-check)
    match dir {
        FocusDir::Left => (!i.is_multiple_of(cols)).then(|| i - 1),
        FocusDir::Right => (i % cols + 1 < cols && i + 1 < n).then(|| i + 1),
        FocusDir::Up => (i >= cols).then(|| i - cols),
        FocusDir::Down => (i + cols < n).then(|| i + cols),
    }
}

impl Explorer {
    /// §10 per-folder persistence: restore the stored view index for the
    /// current folder (snapped to the enabled mask); unknown folders use
    /// the default.
    pub(crate) fn restore_folder_view(&mut self) {
        let stored = self.folder_views.get(&self.cwd).copied();
        self.view_index = self
            .view_cfg
            .nearest_enabled(stored.unwrap_or(self.view_cfg.presets.details));
        self.refresh_it_label_lines();
        self.zoom_acc = 0.0;
        self.zoom_step_at = None;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
    }

    /// Switch the view step index (§5 preset pick / §7 wheel step),
    /// snapping to the enabled mask, keeping the first visible item
    /// anchored in the viewport (§9) and storing the choice per folder
    /// (§10). Selection and hover stay untouched.
    pub(crate) fn set_view_index(&mut self, index: u8) -> iced::Task<Message> {
        let index = self.view_cfg.nearest_enabled(index);
        if index == self.view_index {
            return iced::Task::none();
        }
        let old = self.view_index;
        let old_compact = self.details_compact;
        self.view_index = index;
        self.refresh_it_label_lines();
        self.folder_views.insert(self.cwd.clone(), index);
        self.anchor_view(old, old_compact)
    }

    /// §9 scroll anchor: keep the first visible item (in the OLD layout)
    /// at the same viewport position in the current layout.
    pub(crate) fn anchor_view(&mut self, old_index: u8, old_compact: bool) -> iced::Task<Message> {
        let cfg = &self.view_cfg;
        let pane_w = self.file_pane_width();
        let longest = self
            .entries
            .iter()
            .map(|e| e.name.chars().count())
            .max()
            .unwrap_or(0);
        let n = self.entries.len();
        let old_offset = if cfg.family(old_index) == crate::view_step::Family::List {
            self.scroll_x
        } else {
            self.scroll_y
        };
        // §8.3 icon-top row heights are dynamic: the OLD layout's label
        // line counts are recomputed on the fly (the cache matches the
        // current index only)
        let old_lines = self.it_lines_for(old_index);
        let item = cfg.first_visible(
            old_index,
            old_offset,
            pane_w,
            self.viewport_h,
            n,
            longest,
            &old_lines,
            old_compact,
        );
        let within = old_offset
            - cfg.item_offset(
                old_index,
                item,
                pane_w,
                self.viewport_h,
                longest,
                &old_lines,
                old_compact,
            );
        let index = self.view_index;
        let offset = (cfg.item_offset(
            index,
            item,
            pane_w,
            self.viewport_h,
            longest,
            &self.it_label_lines,
            self.details_compact,
        ) + within)
            .max(0.0);
        // the scrollable clamps its offset to (content - viewport).max(0)
        // at render; an instant scroll_to stores the offset UNCLAMPED and
        // iced publishes no on_scroll for it (none at all when the content
        // fits the viewport), so an anchor past the range would leave
        // scroll_y ahead of the real viewport and the rubber band one
        // header-height off. Mirror the clamp here (x for List, y else)
        let extent = cfg.content_extent(
            index,
            n,
            pane_w,
            self.viewport_h,
            longest,
            &self.it_label_lines,
            self.details_compact,
        );
        let offset = if cfg.family(index) == crate::view_step::Family::List {
            offset.min((extent - pane_w).max(0.0))
        } else {
            offset.min((extent - self.viewport_h).max(0.0))
        };
        let (x, y) = if cfg.family(index) == crate::view_step::Family::List {
            self.scroll_x = offset;
            self.scroll_y = 0.0;
            (Some(offset), Some(0.0))
        } else {
            self.scroll_y = offset;
            self.scroll_x = 0.0;
            (Some(0.0), Some(offset))
        };
        iced::widget::operation::scrollable::scroll_to(
            crate::folder::file_scrollable_id(),
            iced::widget::operation::scrollable::AbsoluteOffset { x, y },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// §7 Ctrl+wheel input handling: accumulate the delta and change one
    /// step per ±notch_delta; at most one step per step_interval_ms —
    /// extra delta is dropped, not queued. Wheel up = next enabled index
    /// above (larger icons; ScrollDirection default).
    pub(crate) fn wheel_zoom(&mut self, delta: f32) -> iced::Task<Message> {
        let input = self.view_cfg.input;
        let now = std::time::Instant::now();
        if self
            .zoom_step_at
            .is_some_and(|t| now.duration_since(t).as_millis() < input.step_interval_ms as u128)
        {
            return iced::Task::none();
        }
        self.zoom_acc += delta;
        if self.zoom_acc.abs() < input.notch_delta {
            return iced::Task::none();
        }
        let up = (self.zoom_acc > 0.0) != input.invert_direction;
        self.zoom_acc = 0.0;
        self.zoom_step_at = Some(now);
        let next = self.view_cfg.next_enabled(self.view_index, up);
        if next == self.view_index {
            return iced::Task::none(); // §7 bounds: stop at the first/last index
        }
        self.set_view_index(next)
    }

    /// File-view row press: the Ctrl/Shift/plain selection gestures.
    /// Plain presses arm a potential drag (or a multi-selection
    /// collapse on release) — see PressArm in model/dnd.rs.
    pub(crate) fn select_press(&mut self, i: usize) -> Task<Message> {
        self.active_pane = crate::model::Pane::Folder;
        if i >= self.entries.len() {
            return Task::none();
        }
        if self.ctrl_held {
            // Ctrl+click toggles the item, keeps the rest
            if !self.selected.remove(&i) {
                self.selected.insert(i);
            }
            self.focus_item = Some(i);
            self.sel_anchor = Some(i);
            self.last_click = None;
            self.press = None; // selection gesture, not a drag
            return Task::none();
        }
        if self.shift_held {
            // Shift+click: the anchor..=item range
            let anchor = self.sel_anchor.unwrap_or(i);
            self.selected = (anchor.min(i)..=anchor.max(i)).collect();
            self.focus_item = Some(i);
            self.last_click = None;
            self.press = None; // selection gesture, not a drag
            return Task::none();
        }
        // Pressing a row that is part of a multi-selection must
        // NOT collapse the selection yet: a drag takes all of it,
        // a plain release collapses it (see `collapse`).
        if self.selected.contains(&i) && self.selected.len() > 1 {
            self.focus_item = Some(i);
            self.press = Some(super::dnd::PressArm {
                i,
                at: self.cursor,
                collapse: true,
                right: false,
            });
            return Task::none();
        }
        self.press = Some(super::dnd::PressArm {
            i,
            at: self.cursor,
            collapse: false,
            right: false,
        });
        self.select_single(i)
    }

    /// The idle-selected (keyboard focus) item: the tracked one when
    /// still valid, else the first entry (the default, per Windows).
    pub(crate) fn focus_or_default(&self) -> usize {
        self.focus_item
            .filter(|&i| i < self.entries.len())
            .unwrap_or(0)
    }

    /// Arrow-key navigation of the idle selector (model/message.rs):
    /// the rectangle follows the layout geometry in every view. Plain
    /// arrows move the selection along, Ctrl+arrow moves the rectangle
    /// only (then Space accumulates a multiple selection), Shift+arrow
    /// extends the range from the anchor. The scroll axis follows an
    /// off-screen rectangle.
    pub(crate) fn focus_move(&mut self, dir: FocusDir) -> Task<Message> {
        if self.entries.is_empty() {
            return Task::none();
        }
        self.active_pane = crate::model::Pane::Folder;
        let next = self.neighbor(self.focus_or_default(), dir).unwrap_or_else(|| self.focus_or_default());
        self.focus_item = Some(next);
        self.focus_shown = true;
        if self.shift_held {
            let anchor = self.sel_anchor.unwrap_or(next);
            self.selected = (anchor.min(next)..=anchor.max(next)).collect();
        } else if !self.ctrl_held {
            // a plain arrow moves the selection with the rectangle
            self.selected.clear();
            self.selected.insert(next);
            self.sel_anchor = Some(next);
        }
        self.scroll_item_into_view(next)
    }

    /// Space toggles the idle-selected item (the Windows checkbox
    /// gesture): an unselected item joins the selection (Ctrl held or
    /// not — that is the multiple-selection accumulate), a selected
    /// one leaves it.
    pub(crate) fn focus_space(&mut self) -> Task<Message> {
        if self.entries.is_empty() {
            return Task::none();
        }
        self.active_pane = crate::model::Pane::Folder;
        let i = self.focus_or_default();
        self.focus_item = Some(i);
        self.focus_shown = true;
        if !self.selected.remove(&i) {
            self.selected.insert(i);
        }
        self.sel_anchor = Some(i);
        Task::none()
    }

    /// The adjacent item in `dir` for the current layout: a single
    /// column in Content/Details, rows-per-column in List, grid
    /// columns in Tiles/SmallIcon/IconTop. None at the edge.
    fn neighbor(&self, i: usize, dir: FocusDir) -> Option<usize> {
        let n = self.entries.len();
        let cfg = &self.view_cfg;
        let w = self.file_pane_width();
        match cfg.family(self.view_index) {
            vs::Family::Content | vs::Family::Details => match dir {
                FocusDir::Up => i.checked_sub(1),
                FocusDir::Down => (i + 1 < n).then(|| i + 1),
                FocusDir::Left | FocusDir::Right => None,
            },
            vs::Family::List => {
                let rows = ((self.viewport_h / cfg.layouts.list.row_h).floor() as usize).max(1);
                match dir {
                    FocusDir::Up => (!i.is_multiple_of(rows)).then(|| i - 1),
                    FocusDir::Down => (i % rows < rows - 1 && i + 1 < n).then(|| i + 1),
                    FocusDir::Left => (i >= rows).then(|| i - rows),
                    FocusDir::Right => (i + rows < n).then(|| i + rows),
                }
            }
            vs::Family::Tiles => {
                let t = &cfg.layouts.tiles;
                grid_neighbor(i, n, vs::grid_cols(t.cell_w, 0.0, w), dir)
            }
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(cfg.icon_px(self.view_index));
                grid_neighbor(i, n, vs::grid_cols(m.cell_w, m.gap, w), dir)
            }
            vs::Family::IconTop => {
                let m = cfg.icon_top(cfg.icon_px(self.view_index));
                grid_neighbor(i, n, vs::grid_cols(m.cell_w, m.gap, w), dir)
            }
        }
    }

    /// Scroll the scroll axis so item `i` is fully visible (a keyboard
    /// focus move keeps the rectangle inside the viewport; List scrolls
    /// x, every other family y). Mirrors the anchor_view clamp: an
    /// instant scroll_to stores the offset unclamped, which would leave
    /// scroll_x/scroll_y ahead of the real viewport.
    fn scroll_item_into_view(&mut self, i: usize) -> Task<Message> {
        let cfg = &self.view_cfg;
        let index = self.view_index;
        let longest = self
            .entries
            .iter()
            .map(|e| e.name.chars().count())
            .max()
            .unwrap_or(0);
        let pane_w = self.file_pane_width();
        let offset = cfg.item_offset(
            index,
            i,
            pane_w,
            self.viewport_h,
            longest,
            &self.it_label_lines,
            self.details_compact,
        );
        // the item's extent along the scroll axis
        let len = match cfg.family(index) {
            vs::Family::Content => cfg.layouts.content.row_h,
            vs::Family::Details => cfg.details_pitch(self.details_compact),
            vs::Family::List => cfg.list_col_w(longest),
            vs::Family::Tiles => cfg.layouts.tiles.cell_h,
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(cfg.icon_px(index));
                m.cell_h + m.gap
            }
            vs::Family::IconTop => {
                let m = cfg.icon_top(cfg.icon_px(index));
                let own = self.it_label_lines.get(i).copied().unwrap_or(1).max(1) as f32;
                m.pad + cfg.icon_px(index) + m.gap + own * cfg.icon_top.label_line_h + m.pad
            }
        };
        let extent = cfg.content_extent(
            index,
            self.entries.len(),
            pane_w,
            self.viewport_h,
            longest,
            &self.it_label_lines,
            self.details_compact,
        );
        let is_x = cfg.family(index) == vs::Family::List;
        let (scroll, viewport) = if is_x {
            (self.scroll_x, pane_w)
        } else {
            (self.scroll_y, self.viewport_h)
        };
        let mut target = scroll;
        if offset < scroll {
            target = offset;
        } else if offset + len > scroll + viewport {
            target = offset + len - viewport;
        }
        target = target.clamp(0.0, (extent - viewport).max(0.0));
        if (target - scroll).abs() < 0.5 {
            return Task::none();
        }
        let (x, y) = if is_x {
            self.scroll_x = target;
            (Some(target), None)
        } else {
            self.scroll_y = target;
            (None, Some(target))
        };
        iced::widget::operation::scrollable::scroll_to(
            crate::folder::file_scrollable_id(),
            iced::widget::operation::scrollable::AbsoluteOffset { x, y },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// Right-pressed a file-view row: unlike the selection gestures
    /// this always arms a potential drag — a plain release opens the
    /// row's context menu; moving past the threshold starts a
    /// right-button drag whose drop opens the action-choice menu
    /// (Windows right-drag semantics) instead of committing directly.
    pub(crate) fn right_press(&mut self, i: usize) -> Task<Message> {
        self.active_pane = crate::model::Pane::Folder;
        if i >= self.entries.len() {
            return Task::none();
        }
        self.press = Some(super::dnd::PressArm {
            i,
            at: self.cursor,
            collapse: false,
            right: true,
        });
        Task::none()
    }

    /// Press on a Details column right-edge resizer: start a column
    /// drag (the resizer resizes only its own column).
    pub(crate) fn column_split_begin(&mut self, col: usize) -> Task<Message> {
        if col < self.details_cols.len() {
            self.col_drag = Some(ColDrag {
                col,
                start_x: None,
                start_w: self.details_cols[col],
            });
        }
        Task::none()
    }

    /// Cursor moved during a Details column drag (window-logical x).
    pub(crate) fn column_split_move(&mut self, x: f32) -> Task<Message> {
        if let Some(drag) = &mut self.col_drag {
            // first move after the press anchors the drag origin
            let start_x = *drag.start_x.get_or_insert(x);
            // the resizer sits at the column's right edge: dragging
            // right widens the column
            let d = &self.detail_cfg;
            self.details_cols[drag.col] =
                (drag.start_w + (x - start_x)).clamp(d.col_min, d.col_max);
        }
        Task::none()
    }

    /// Left-click a Details column header: sort by the column's key;
    /// clicking the already-sorted column toggles ascending/descending
    /// (SPEC/detail-view.txt §9).
    pub(crate) fn column_sort_click(&mut self, col: usize) -> Task<Message> {
        let Some(key) = self.detail_cfg.columns.get(col).and_then(|c| c.sort) else {
            return Task::none();
        };
        if self.sort == key {
            self.sort_asc = !self.sort_asc;
        } else {
            self.sort = key;
            self.sort_asc = true;
        }
        // the 75% blue tint shows while the header is held pressed
        self.col_pressed = Some(col);
        self.apply_sort();
        Task::none()
    }

    /// Plain (no-modifier) selection of one row: collapse the selection
    /// and run the double-click emulation (two presses on the same row
    /// within 400ms navigate into a folder/archive). Called at press
    /// time for unselected rows and at release time for the collapse of
    /// a multi-selection press.
    pub(crate) fn select_single(&mut self, i: usize) -> iced::Task<Message> {
        self.selected.clear();
        self.selected.insert(i);
        self.focus_item = Some(i);
        self.sel_anchor = Some(i);
        let now = std::time::Instant::now();
        if let Some((last_i, t)) = self.last_click
            && last_i == i
            && now.duration_since(t).as_millis()
                < u128::from(crate::theme::double_click_ms())
        {
            self.last_click = None;
            if let Some(e) = self.entries.get(i)
                && (e.kind == fs::EntryKind::Dir || fs::archive::is_archive_path(&e.path))
            {
                let p = e.path.clone();
                return self.navigate(p);
            }
            return iced::Task::none();
        }
        self.last_click = Some((i, now));
        iced::Task::none()
    }

    /// Item rectangle in file-view CONTENT coordinates (before scroll),
    /// shared by the Menu-key anchor and the rubber-band hit test.
    pub(crate) fn item_rect(&self, i: usize, pane_w: f32) -> (f32, f32, f32, f32) {
        use crate::view_step as vs;
        let cfg = &self.view_cfg;
        let s = cfg.icon_px(self.view_index);
        match cfg.family(self.view_index) {
            vs::Family::Content => {
                let row_h = cfg.layouts.content.row_h;
                (0.0, i as f32 * row_h, pane_w, row_h)
            }
            vs::Family::Details => {
                let row_h = cfg.details_row_h(self.details_compact);
                let pitch = cfg.details_pitch(self.details_compact);
                (
                    0.0,
                    cfg.layouts.details.header_h
                        + cfg.layouts.details.files_gap
                        + i as f32 * pitch,
                    pane_w,
                    row_h,
                )
            }
            vs::Family::List => {
                let l = &cfg.layouts.list;
                let longest = self
                    .entries
                    .iter()
                    .map(|e| e.name.chars().count())
                    .max()
                    .unwrap_or(0);
                let col_w = cfg.list_col_w(longest);
                let rows = ((self.viewport_h / l.row_h).floor() as usize).max(1);
                (
                    (i / rows) as f32 * col_w,
                    (i % rows) as f32 * l.row_h,
                    col_w,
                    l.row_h,
                )
            }
            vs::Family::Tiles => {
                let t = &cfg.layouts.tiles;
                let cols = vs::grid_cols(t.cell_w, 0.0, pane_w);
                (
                    (i % cols) as f32 * t.cell_w,
                    (i / cols) as f32 * t.cell_h,
                    t.cell_w,
                    t.cell_h,
                )
            }
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(s);
                let cols = vs::grid_cols(m.cell_w, m.gap, pane_w);
                (
                    (i % cols) as f32 * (m.cell_w + m.gap),
                    (i / cols) as f32 * (m.cell_h + m.gap),
                    m.cell_w,
                    m.cell_h,
                )
            }
            vs::Family::IconTop => {
                // §8.3 dynamic row heights: y comes from the per-row
                // prefix sums; the height is this item's own chrome
                let m = cfg.icon_top(s);
                let rows = cfg.icon_top_rows(
                    self.view_index,
                    self.entries.len(),
                    pane_w,
                    &self.it_label_lines,
                );
                let col = i % rows.cols;
                let row = i / rows.cols;
                (
                    col as f32 * (m.cell_w + m.gap),
                    rows.starts[row],
                    m.cell_w,
                    rows.item_h.get(i).copied().unwrap_or(m.cell_h),
                )
            }
        }
    }

    /// Rubber-band: select every item whose cell intersects the marquee.
    pub(crate) fn marquee_update(&mut self) {
        use crate::view_step as vs;
        let Some(m) = &self.marquee else { return };
        let (x0, x1) = (m.x0.min(m.x1), m.x0.max(m.x1));
        let (y0, y1) = (m.y0.min(m.y1), m.y0.max(m.y1));
        let pane_w = self.file_pane_width();
        let mut hits = m.base.clone();
        // a plain click on empty space is a zero-size marquee: it only
        // releases the selection (a degenerate rect would pass the
        // strict bounds below because the rows span the full pane width)
        let dragged = x1 - x0 > 1.0 && y1 - y0 > 1.0;
        if !dragged {
            if hits != self.selected {
                self.selected = hits;
            }
            return;
        }
        if self.view_cfg.family(self.view_index) == vs::Family::IconTop {
            // §8.3 dynamic row heights: one geometry pass, then direct
            // per-item math (item_rect would rebuild the prefix sums
            // for every item)
            let s = self.view_cfg.icon_px(self.view_index);
            let m = self.view_cfg.icon_top(s);
            let rows = self.view_cfg.icon_top_rows(
                self.view_index,
                self.entries.len(),
                pane_w,
                &self.it_label_lines,
            );
            for i in 0..self.entries.len() {
                let ix = (i % rows.cols) as f32 * (m.cell_w + m.gap);
                let iy = rows.starts[i / rows.cols];
                let iw = m.cell_w;
                let ih = rows.item_h.get(i).copied().unwrap_or(m.cell_h);
                if ix < x1 && ix + iw > x0 && iy < y1 && iy + ih > y0 {
                    hits.insert(i);
                }
            }
        } else {
            for i in 0..self.entries.len() {
                let (ix, iy, iw, ih) = self.item_rect(i, pane_w);
                if ix < x1 && ix + iw > x0 && iy < y1 && iy + ih > y0 {
                    hits.insert(i);
                }
            }
        }
        if hits != self.selected {
            self.selected = hits;
        }
    }

    /// Left-bottom corner of file-view item `i` in window coordinates
    /// (Menu-key anchor). Best-effort: recomputed from the same layout
    /// math as ui.rs + folder.rs.
    pub(crate) fn item_anchor(&self, i: usize) -> (f32, f32) {
        let (ft, fr, _, fl) = crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
        let title_h = if self.self_border {
            crate::border::titlebar_h(self.frame_style, self.maximized)
        } else {
            0.0
        };
        let content_w = self.win_w - fl - fr;
        let mult = self.dpi_multiplier();
        let (min_ribbon_w, _, _, _) = self.ribbon.sacrifice();
        let ribbon_h = if content_w >= min_ribbon_w * mult {
            crate::ribbon::widget::layout::ribbon_height(&self.ribbon.framework().view())
        } else {
            0.0
        };
        let band_h = self.ribbon.band_height();
        let nav_w = self.layout_widths().0;
        // the nav_gap spacer is part of the folder view (always shown)
        let pane_x = fl + nav_w + self.ribbon.nav_gap() + if nav_w > 0.0 { 4.0 } else { 0.0 };
        let pane_y = ft + title_h + ribbon_h + band_h;
        let (x, y, _, h) = self.item_rect(i, self.file_pane_width());
        (pane_x + x - self.scroll_x, pane_y + y + h - self.scroll_y)
    }
}

/// Details column drag: which column (index into `detail_cfg.columns`),
/// the cursor x at drag start (learned on the first move) and the
/// column width at drag start.
pub(crate) struct ColDrag {
    col: usize,
    start_x: Option<f32>,
    start_w: f32,
}

#[cfg(test)]
mod focus_tests {
    //! The idle selector (keyboard focus rectangle): layout-aware
    //! arrow navigation in every family, Space toggle, and the
    //! free-space click that releases the selection but shows the
    //! rectangle on the first released item.
    use super::*;
    use std::path::PathBuf;

    fn entry(name: &str) -> fs::Entry {
        fs::Entry {
            name: name.into(),
            path: PathBuf::from("/tmp").join(name),
            kind: fs::EntryKind::File,
            size: 0,
            modified: None,
            is_hidden: false,
            is_symlink: false,
            is_executable: false,
            symlink_target: None,
        }
    }

    fn explorer(n: usize) -> Explorer {
        let mut e = Explorer::new();
        e.entries = (0..n).map(|i| entry(&format!("item{i:02}"))).collect();
        e.viewport_h = 96.0;
        e.focus_item = Some(0);
        e.focus_shown = false;
        e
    }

    #[test]
    fn details_moves_vertically_only() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        assert_eq!(e.neighbor(0, FocusDir::Up), None);
        assert_eq!(e.neighbor(0, FocusDir::Down), Some(1));
        assert_eq!(e.neighbor(4, FocusDir::Down), None);
        assert_eq!(e.neighbor(2, FocusDir::Left), None);
        assert_eq!(e.neighbor(2, FocusDir::Right), None);
    }

    #[test]
    fn grid_follows_columns_and_rows() {
        let mut e = explorer(0);
        e.view_index = e.view_cfg.presets.tiles;
        let t = &e.view_cfg.layouts.tiles;
        let cols = vs::grid_cols(t.cell_w, 0.0, e.file_pane_width());
        // enough items for two full rows plus one partial
        let n = cols * 2 + 1;
        e.entries = (0..n).map(|i| entry(&format!("item{i:02}"))).collect();
        assert_eq!(e.neighbor(0, FocusDir::Left), None);
        assert_eq!(e.neighbor(0, FocusDir::Right), Some(1));
        assert_eq!(e.neighbor(cols - 1, FocusDir::Right), None);
        assert_eq!(e.neighbor(0, FocusDir::Down), Some(cols));
        assert_eq!(e.neighbor(cols, FocusDir::Up), Some(0));
        // the last (partial) row: below the first row, none further
        assert_eq!(e.neighbor(cols, FocusDir::Down), Some(2 * cols));
        assert_eq!(e.neighbor(2 * cols, FocusDir::Down), None);
    }

    #[test]
    fn list_follows_rows_per_column() {
        let mut e = explorer(10);
        e.view_index = e.view_cfg.presets.list;
        // 96px viewport / 24px rows = 4 rows per column
        assert_eq!(e.neighbor(0, FocusDir::Down), Some(1));
        assert_eq!(e.neighbor(3, FocusDir::Down), None);
        // index 4 tops the second column: nothing above, one left
        assert_eq!(e.neighbor(4, FocusDir::Up), None);
        assert_eq!(e.neighbor(5, FocusDir::Up), Some(4));
        assert_eq!(e.neighbor(4, FocusDir::Left), Some(0));
        assert_eq!(e.neighbor(4, FocusDir::Right), Some(8));
        assert_eq!(e.neighbor(6, FocusDir::Right), None);
    }

    #[test]
    fn plain_arrow_moves_selection_ctrl_moves_rectangle_only() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        let _ = e.focus_move(FocusDir::Down);
        assert_eq!(e.focus_item, Some(1));
        assert!(e.focus_shown);
        assert_eq!(e.selected.iter().copied().collect::<Vec<_>>(), vec![1]);
        // Ctrl+arrow: the rectangle walks, the selection stays
        e.ctrl_held = true;
        let _ = e.focus_move(FocusDir::Down);
        assert_eq!(e.focus_item, Some(2));
        assert_eq!(e.selected.iter().copied().collect::<Vec<_>>(), vec![1]);
        e.ctrl_held = false;
    }

    #[test]
    fn space_toggles_the_idle_item() {
        let mut e = explorer(3);
        e.view_index = e.view_cfg.presets.details;
        // rectangle on the first item (the default), invisible
        assert!(!e.focus_shown);
        let _ = e.focus_space();
        assert!(e.focus_shown);
        assert!(e.selected.contains(&0));
        // Ctrl+Space accumulate: walk to item 2, add it
        e.ctrl_held = true;
        let _ = e.focus_move(FocusDir::Down);
        let _ = e.focus_move(FocusDir::Down);
        let _ = e.focus_space();
        assert_eq!(e.selected.iter().copied().collect::<Vec<_>>(), vec![0, 2]);
        e.ctrl_held = false;
        // Space on a selected item releases it
        let _ = e.focus_space();
        assert_eq!(e.selected.iter().copied().collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn free_space_click_shows_rectangle_on_first_released() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        e.selected.insert(2);
        e.selected.insert(4);
        e.focus_shown = false;
        let _ = e.update(Message::MarqueeBegin { x: 5.0, y: 5.0 });
        let _ = e.update(Message::MarqueeEnd);
        // the selection is released…
        assert!(e.selected.is_empty());
        // …and the idle selector borders the first released item
        assert_eq!(e.focus_item, Some(2));
        assert!(e.focus_shown);
    }
}

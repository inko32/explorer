//! Folder-view update logic: view-step switching with scroll anchoring,
//! selection (click/Ctrl/Shift/rubber-band), item geometry and the
//! Details column drags. The `handle` match delegates here.

use iced::Task;

use crate::fs;

use super::explorer::{DOUBLE_CLICK_MS, Explorer};
use super::message::Message;

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
            self.sel_anchor = Some(i);
            self.last_click = None;
            self.press = None; // selection gesture, not a drag
            return Task::none();
        }
        if self.shift_held {
            // Shift+click: the anchor..=item range
            let anchor = self.sel_anchor.unwrap_or(i);
            self.selected = (anchor.min(i)..=anchor.max(i)).collect();
            self.last_click = None;
            self.press = None; // selection gesture, not a drag
            return Task::none();
        }
        // Pressing a row that is part of a multi-selection must
        // NOT collapse the selection yet: a drag takes all of it,
        // a plain release collapses it (see `collapse`).
        if self.selected.contains(&i) && self.selected.len() > 1 {
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

    /// Press on a Details column split handle: start a column drag.
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
            // the handle sits at the column's left edge: dragging
            // left widens the column
            let d = &self.view_cfg.layouts.details;
            self.details_cols[drag.col] =
                (drag.start_w + (start_x - x)).clamp(d.col_min, d.col_max);
        }
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
        self.sel_anchor = Some(i);
        let now = std::time::Instant::now();
        if let Some((last_i, t)) = self.last_click
            && last_i == i
            && now.duration_since(t).as_millis() < DOUBLE_CLICK_MS
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
                    cfg.layouts.details.header_h + i as f32 * pitch,
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
            crate::border::titlebar_h(self.frame_style)
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

/// Details column drag: which fixed column (0=Date, 1=Type, 2=Size),
/// the cursor x at drag start (learned on the first move) and the
/// column width at drag start.
pub(crate) struct ColDrag {
    col: usize,
    start_x: Option<f32>,
    start_w: f32,
}

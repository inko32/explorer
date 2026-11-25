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
        // the data-column highlight is Details-only state
        self.detail_col_focus = None;
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
    /// (§10). Selection and hover stay untouched. A committed switch
    /// also discards an in-flight gallery hover preview.
    pub(crate) fn set_view_index(&mut self, index: u8) -> iced::Task<Message> {
        self.preview_view = None;
        // the data-column highlight is Details-only state
        self.detail_col_focus = None;
        let index = self.view_cfg.nearest_enabled(index);
        // persist even when the preview already shows this step
        self.folder_views.insert(self.cwd.clone(), index);
        self.apply_view_index(index)
    }

    /// Gallery hover preview: temporarily switch the view step without
    /// persisting it; the committed step is backed up on the first
    /// preview and restored by [`Self::end_view_preview`].
    pub(crate) fn preview_view_index(&mut self, index: u8) -> iced::Task<Message> {
        let index = self.view_cfg.nearest_enabled(index);
        if self.preview_view.is_none() {
            self.preview_view = Some(self.view_index);
        }
        self.apply_view_index(index)
    }

    /// Revert a gallery hover preview to the committed view step.
    pub(crate) fn end_view_preview(&mut self) -> iced::Task<Message> {
        match self.preview_view.take() {
            Some(backup) => self.apply_view_index(backup),
            None => iced::Task::none(),
        }
    }

    fn apply_view_index(&mut self, index: u8) -> iced::Task<Message> {
        if index == self.view_index {
            return iced::Task::none();
        }
        let old = self.view_index;
        let old_compact = self.details_compact;
        self.view_index = index;
        self.refresh_it_label_lines();
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
        // a manual click releases the data-column highlight (keyboard
        // releases go through the arrow keys)
        self.detail_col_focus = None;
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
        // Details: while the idle selector is shown (or a selection
        // exists), Left/Right walk the data-column highlight across
        // the focused row — a no-function selector that lets keyboard
        // users scroll wide columns into view. It never touches the
        // idle selector's visibility or the selection. Up/Down keep
        // the highlighted column and move normally: the highlight
        // follows the focus to the next line, like a click on another
        // row (only a mouse press or the Menu key releases it).
        let details = self.view_cfg.family(self.view_index) == vs::Family::Details;
        if !details {
            self.detail_col_focus = None;
        }
        if details && matches!(dir, FocusDir::Left | FocusDir::Right) {
            if self.focus_shown || !self.selected.is_empty() {
                return self.detail_col_step(dir == FocusDir::Right);
            }
            // nothing shown yet: reveal the idle selector only (a
            // rectangle-only move) — Left/Right must never select
            self.focus_shown = true;
            return Task::none();
        }
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

    /// Left/Right step of the Details data-column highlight: engage
    /// past the leftmost visible column, then walk right/left, clamped
    /// at both ends. The column strip scrolls so the highlighted
    /// column stays fully visible (the point of the selector).
    /// Position 0 (the leftmost visible column, whatever the header
    /// order) is never drawn: the whole idle selector covers it.
    fn detail_col_step(&mut self, right: bool) -> Task<Message> {
        let vis: Vec<usize> = self
            .details_col_order
            .iter()
            .copied()
            .filter(|&g| self.details_col_on.get(g).copied().unwrap_or(true))
            .collect();
        if vis.is_empty() {
            return Task::none();
        }
        let cur = self.detail_col_focus.unwrap_or(0).min(vis.len() - 1);
        let next = if self.detail_col_focus.is_none() {
            // engage: Right jumps past the leftmost column — position
            // 0 IS the whole idle selector, already on screen (Left
            // engages there by stepping down to it)
            (if right { 1 } else { 0 }).min(vis.len() - 1)
        } else if right {
            (cur + 1).min(vis.len() - 1)
        } else {
            cur.saturating_sub(1)
        };
        self.detail_col_focus = Some(next);
        self.scroll_detail_col_into_view(vis[next])
    }

    /// Scroll the outer Details column strip so column `col` (index
    /// into `detail_cfg.columns`) is fully visible horizontally.
    /// Mirrors `scroll_item_into_view`'s clamp: an instant scroll_to
    /// stores the offset unclamped, which would leave scroll_x ahead
    /// of the real viewport.
    fn scroll_detail_col_into_view(&mut self, col: usize) -> Task<Message> {
        // the column's content-x range: the files margin, then every
        // visible column before it plus its resizer (§13/§15)
        let mut x = self.detail_cfg.files_left;
        for &g in &self.details_col_order {
            if !self.details_col_on.get(g).copied().unwrap_or(true) {
                continue;
            }
            if g == col {
                break;
            }
            x += self.details_col_w(g) + crate::folder::COL_RESIZER_W;
        }
        let w = self.details_col_w(col);
        let pane_w = self.file_pane_width();
        let max = (self.details_natural_w() - pane_w).max(0.0);
        let mut target = self.scroll_x;
        if x < self.scroll_x {
            target = x;
        } else if x + w > self.scroll_x + pane_w {
            target = x + w - pane_w;
        }
        target = target.clamp(0.0, max);
        if (target - self.scroll_x).abs() < 0.5 {
            return Task::none();
        }
        self.scroll_x = target;
        iced::widget::operation::scrollable::scroll_to(
            crate::folder::details_h_scrollable_id(),
            iced::widget::operation::scrollable::AbsoluteOffset {
                x: Some(target),
                y: None,
            },
            iced::widget::operation::Animation::Instant,
        )
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
                let rows = cfg.list_rows_per_col(self.viewport_h);
                match dir {
                    FocusDir::Up => (!i.is_multiple_of(rows)).then(|| i - 1),
                    FocusDir::Down => (i % rows < rows - 1 && i + 1 < n).then(|| i + 1),
                    FocusDir::Left => (i >= rows).then(|| i - rows),
                    FocusDir::Right => (i + rows < n).then(|| i + rows),
                }
            }
            vs::Family::Tiles => {
                let t = &cfg.layouts.tiles;
                grid_neighbor(i, n, cfg.grid_cols_at(self.view_index, t.cell_w, 0.0, w), dir)
            }
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(cfg.icon_px(self.view_index));
                grid_neighbor(i, n, cfg.grid_cols_at(self.view_index, m.cell_w, m.gap, w), dir)
            }
            vs::Family::IconTop => {
                let m = cfg.icon_top(cfg.icon_px(self.view_index));
                grid_neighbor(i, n, cfg.grid_cols_at(self.view_index, m.cell_w, m.gap, w), dir)
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
                let block =
                    crate::text_metrics::line_block(cfg.style.font_size, crate::fonts::primary());
                m.pad + cfg.icon_px(index) + vs::ICON_LABEL_GAP + vs::label_box_h(own, block)
                    + vs::LABEL_PAD_BOTTOM
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
    /// Press on a Details column title: arm a potential reorder drag —
    /// the 75% press tint shows while held; travel past the drag
    /// threshold becomes the header reorder drag, a plain release sorts
    /// (Windows header semantics).
    pub(crate) fn column_header_press(&mut self, col: usize) -> Task<Message> {
        self.col_pressed = Some(col);
        if self.col_arm.is_none() {
            self.col_arm = Some((col, self.cursor));
        }
        Task::none()
    }

    /// Release of a header press that never crossed the drag threshold:
    /// sort by the column's key, toggling ascending/descending on the
    /// already-sorted column.
    pub(crate) fn column_sort_release(&mut self, col: usize) -> Task<Message> {
        self.col_pressed = None;
        let Some(key) = self.detail_cfg.columns.get(col).and_then(|c| c.sort) else {
            return Task::none();
        };
        if self.sort == key {
            self.sort_asc = !self.sort_asc;
        } else {
            self.sort = key;
            self.sort_asc = true;
        }
        self.apply_sort();
        Task::none()
    }

    /// A header press travelled past the drag threshold: start the
    /// column reorder drag.
    pub(crate) fn column_reorder_begin(&mut self, col: usize, at: iced::Point) {
        self.col_arm = None;
        self.col_pressed = None;
        // the drag starts from the column's own slot; the walk in
        // `col_reorder_over` keeps it there until a midpoint is crossed
        let over = self.col_reorder_over(col, at.x, at.x);
        self.col_reorder = Some(ColReorder {
            col,
            start_x: at.x,
            cur_x: at.x,
            over,
            from_over: over,
            anim_t: 1.0,
        });
    }

    /// Cursor moved during a header reorder drag: the dragged header
    /// follows the cursor; crossing a target header's midpoint re-targets
    /// the reserved slot and restarts the shift animation.
    pub(crate) fn column_reorder_move(&mut self, x: f32) -> Task<Message> {
        let Some((col, cur_x, start_x, over_now)) = self.col_reorder.as_mut().map(|d| {
            d.cur_x = x;
            (d.col, d.cur_x, d.start_x, d.over)
        }) else {
            return Task::none();
        };
        let over = self.col_reorder_over(col, cur_x, start_x);
        if over != over_now
            && let Some(d) = &mut self.col_reorder
        {
            d.from_over = d.over;
            d.over = over;
            d.anim_t = 0.0;
        }
        Task::none()
    }

    /// Release of a header reorder drag: move the dragged column next
    /// to the slot the drag reserved; only now do the rows follow.
    pub(crate) fn column_reorder_end(&mut self) -> Task<Message> {
        let Some(drag) = self.col_reorder.take() else {
            return Task::none();
        };
        let base = self.details_base(drag.col);
        let mut order = self.details_col_order.clone();
        order.retain(|&g| g != drag.col);
        if let Some(&next) = base.get(drag.over) {
            let at = order
                .iter()
                .position(|&g| g == next)
                .unwrap_or(order.len());
            order.insert(at, drag.col);
        } else {
            order.push(drag.col);
        }
        self.details_col_order = order;
        Task::none()
    }

    /// The shift-animation tick: advance toward the settled layout.
    pub(crate) fn column_reorder_tick(&mut self) -> Task<Message> {
        if let Some(d) = &mut self.col_reorder {
            d.anim_t = (d.anim_t + REORDER_TICK_MS / REORDER_SHIFT_MS).min(1.0);
        }
        Task::none()
    }

    /// The width of a Details column (the header resizer adjusts it).
    fn details_col_w(&self, col: usize) -> f32 {
        self.details_cols
            .get(col)
            .copied()
            .unwrap_or(self.detail_cfg.columns[col].width)
    }

    /// The reorder drag's "base" list: the visible columns in display
    /// order, minus the dragged one.
    pub(crate) fn details_base(&self, col: usize) -> Vec<usize> {
        self.details_col_order
            .iter()
            .copied()
            .filter(|&g| g != col && self.details_col_on.get(g).copied().unwrap_or(true))
            .collect()
    }

    /// Header-local x of a visible column in the committed layout.
    pub(crate) fn col_committed_x(&self, col: usize) -> f32 {
        let mut x = self.detail_cfg.files_left;
        for &g in self.details_col_order.iter() {
            if !self.details_col_on.get(g).copied().unwrap_or(true) {
                continue;
            }
            if g == col {
                return x;
            }
            x += self.details_col_w(g) + crate::folder::COL_RESIZER_W;
        }
        x
    }

    /// Header-local x of every configured column when the dragged
    /// header reserves `slot`: the base (visible, minus the dragged)
    /// headers compressed, the dragged width inserted at the slot (4px
    /// resizer gaps, §15). Only base columns get entries; the result is
    /// indexed by column index. Shared by the slot walk and the header
    /// rendering.
    pub(crate) fn col_slot_positions(&self, col: usize, slot: usize) -> Vec<f32> {
        let base = self.details_base(col);
        let mut pos = vec![0.0f32; self.detail_cfg.columns.len()];
        let mut x = self.detail_cfg.files_left;
        for (n, &g) in base.iter().enumerate() {
            if n == slot {
                x += self.details_col_w(col) + crate::folder::COL_RESIZER_W;
            }
            pos[g] = x;
            x += self.details_col_w(g) + crate::folder::COL_RESIZER_W;
        }
        pos
    }

    /// The slot the dragged header currently reserves, walking one
    /// header at a time from the column's own slot: each step compares
    /// the dragged center with the neighbor's center in the CURRENT
    /// reserved layout, so one crossing shifts exactly one header (no
    /// cascade) and nothing moves before the first threshold.
    pub(crate) fn col_reorder_over(&self, col: usize, cur_x: f32, start_x: f32) -> usize {
        let base = self.details_base(col);
        let mut over = 0;
        for &g in &self.details_col_order {
            if g == col {
                break;
            }
            if self.details_col_on.get(g).copied().unwrap_or(true) {
                over += 1;
            }
        }
        let cx = self.col_committed_x(col) + (cur_x - start_x) + self.details_col_w(col) / 2.0;
        loop {
            let pos = self.col_slot_positions(col, over);
            if over > 0 {
                let g = base[over - 1];
                if cx < pos[g] + self.details_col_w(g) / 2.0 {
                    over -= 1;
                    continue;
                }
            }
            if over < base.len() {
                let g = base[over];
                if cx > pos[g] + self.details_col_w(g) / 2.0 {
                    over += 1;
                    continue;
                }
            }
            return over;
        }
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

    /// Width of a Details row's chrome: the visible column block
    /// (widths + resizers), starting right of the files margin (the
    /// column header contains that margin; the item shifts right past
    /// it) and ending at the block's right edge (the 4px right reserve
    /// belongs to the header) — the same geometry `file_row` and the
    /// selection-border overlay use. The rubber band must hit this,
    /// not the full pane width: the background right of the column
    /// block is empty space. Allocation-free: `marquee_update` calls
    /// this through `item_rect` for every item.
    pub(crate) fn details_row_w(&self) -> f32 {
        let mut w = 0.0;
        let mut prev_visible = false;
        for &g in &self.details_col_order {
            if !self.details_col_on.get(g).copied().unwrap_or(true) {
                continue;
            }
            if prev_visible {
                w += crate::folder::COL_RESIZER_W;
            }
            w += self
                .details_cols
                .get(g)
                .copied()
                .unwrap_or(self.detail_cfg.columns[g].width);
            prev_visible = true;
        }
        w
    }

    /// Item rectangle in file-view CONTENT coordinates (before scroll),
    /// shared by the Menu-key anchor and the rubber-band hit test.
    pub(crate) fn item_rect(&self, i: usize, pane_w: f32) -> (f32, f32, f32, f32) {
        use crate::view_step as vs;
        let cfg = &self.view_cfg;
        let s = cfg.icon_px(self.view_index);
        // the files sit below the reserved files_top strip (the
        // IconTop branch reads it back from the row prefix sums)
        let top = cfg.files_top(self.view_index);
        // every layout reserves an empty strip left of the files (the
        // rubber band can reach or start there); the Details margin is
        // the column-header one
        let left = if cfg.family(self.view_index) == vs::Family::Details {
            self.detail_cfg.files_left
        } else {
            cfg.files_left(self.view_index)
        };
        match cfg.family(self.view_index) {
            vs::Family::Content => {
                // the item is row_h tall; the pitch overlaps adjacent
                // items by one line px (the shared divider line)
                let item_h = cfg.layouts.content.row_h;
                let pitch = cfg.content_pitch();
                (left, top + i as f32 * pitch, (pane_w - left).max(0.0), item_h)
            }
            vs::Family::Details => {
                let item_h = cfg.details_item_h();
                let pitch = cfg.details_pitch(self.details_compact);
                (
                    left,
                    cfg.layouts.details.header_h
                        + cfg.layouts.details.files_gap
                        + i as f32 * pitch,
                    // the rendered row spans only the column block;
                    // pane_w would rubber-band-select empty background
                    self.details_row_w(),
                    item_h,
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
                let rows = cfg.list_rows_per_col(self.viewport_h);
                (
                    left + (i / rows) as f32 * col_w,
                    top + (i % rows) as f32 * l.row_h,
                    col_w,
                    l.row_h,
                )
            }
            vs::Family::Tiles => {
                let t = &cfg.layouts.tiles;
                let cols = cfg.grid_cols_at(self.view_index, t.cell_w, 0.0, pane_w);
                (
                    left + (i % cols) as f32 * t.cell_w,
                    top + (i / cols) as f32 * t.cell_h,
                    t.cell_w,
                    t.cell_h,
                )
            }
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(s);
                let cols = cfg.grid_cols_at(self.view_index, m.cell_w, m.gap, pane_w);
                (
                    left + (i % cols) as f32 * (m.cell_w + m.gap),
                    top + (i / cols) as f32 * (m.cell_h + m.gap),
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
                    left + col as f32 * (m.cell_w + m.gap),
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
        let family = self.view_cfg.family(self.view_index);
        if family == vs::Family::List {
            // item_rect's List branch rescans every name for `longest`
            // on EACH call — O(n²) name scanning per mouse-move tick
            // (very slow in big folders). Hoist the column geometry
            // once, then direct per-item math (same pattern as the
            // IconTop branch below)
            let longest = self
                .entries
                .iter()
                .map(|e| e.name.chars().count())
                .max()
                .unwrap_or(0);
            let col_w = self.view_cfg.list_col_w(longest);
            let rows = self.view_cfg.list_rows_per_col(self.viewport_h);
            let top = self.view_cfg.files_top(self.view_index);
            let left = self.view_cfg.files_left(self.view_index);
            let row_h = self.view_cfg.layouts.list.row_h;
            for i in 0..self.entries.len() {
                let ix = left + (i / rows) as f32 * col_w;
                let iy = top + (i % rows) as f32 * row_h;
                if ix < x1 && ix + col_w > x0 && iy < y1 && iy + row_h > y0 {
                    hits.insert(i);
                }
            }
        } else if family == vs::Family::IconTop {
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
            let left = self.view_cfg.files_left(self.view_index);
            for i in 0..self.entries.len() {
                let ix = left + (i % rows.cols) as f32 * (m.cell_w + m.gap);
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

    /// Re-anchor the rubber band's far corner to the cursor after a
    /// scroll that came without a matching move (wheel roll, scrollbar
    /// step, edge auto-scroll): x1/y1 are content coordinates, so a
    /// scroll alone would leave the corner riding the items instead of
    /// the pointer (the anchor corner x0/y0 stays content-fixed on
    /// purpose — the band rides the content like a sheet on a canvas).
    pub(crate) fn marquee_follow_scroll(&mut self) {
        if let Some(m) = &mut self.marquee {
            m.x1 = m.px + self.scroll_x;
            m.y1 = m.py + self.scroll_y;
        }
        self.marquee_update();
    }

    /// Edge auto-scroll while a rubber-band drag is live (Windows
    /// drag-select behaviour): the cursor held inside the pane's edge
    /// margin scrolls the content toward that edge, accelerating the
    /// deeper it sits. Each tick also re-anchors the far corner, so the
    /// band keeps tracking the pointer while the anchor corner rides
    /// the content. List scrolls only horizontally, Details on both
    /// axes, every other family only vertically.
    pub(crate) fn marquee_auto_scroll(&mut self) -> Task<Message> {
        /// Edge margin (px) of the pane that triggers the auto-scroll.
        const MARGIN: f32 = 24.0;
        let Some(m) = &self.marquee else { return Task::none() };
        let (px, py) = (m.px, m.py);
        let cfg = &self.view_cfg;
        let family = cfg.family(self.view_index);
        let is_list = family == vs::Family::List;
        let is_details = family == vs::Family::Details;
        let pane_w = self.file_pane_width();
        let pane_h = self.viewport_h;
        let top = if is_details { cfg.layouts.details.header_h } else { 0.0 };
        // per-tick scroll step, accelerating into the margin
        let step = |gap: f32| 2.0 + (MARGIN - gap) * 0.75;
        let mut dx = 0.0f32;
        if is_list || is_details {
            if px < MARGIN {
                dx = -step(px.max(0.0));
            } else if pane_w - px < MARGIN {
                dx = step((pane_w - px).max(0.0));
            }
        }
        let mut dy = 0.0f32;
        if !is_list {
            if py < top + MARGIN {
                dy = -step((py - top).max(0.0));
            } else if pane_h - py < MARGIN {
                dy = step((pane_h - py).max(0.0));
            }
        }
        if dx == 0.0 && dy == 0.0 {
            return Task::none();
        }
        let n = self.entries.len();
        let longest = self
            .entries
            .iter()
            .map(|e| e.name.chars().count())
            .max()
            .unwrap_or(0);
        // clamp each axis into its own scroll range; a pinned axis is
        // dropped (nothing to scroll there)
        if dx != 0.0 {
            let natural = if is_details {
                // the outer column strip scrolls when the visible
                // columns are wider than the pane (same width the view
                // uses to decide the horizontal strip)
                self.details_natural_w()
            } else {
                cfg.content_extent(
                    self.view_index,
                    n,
                    pane_w,
                    pane_h,
                    longest,
                    &self.it_label_lines,
                    self.details_compact,
                )
            };
            let max = (natural - pane_w).max(0.0);
            let new = (self.scroll_x + dx).clamp(0.0, max);
            dx = new - self.scroll_x;
            self.scroll_x = new;
        }
        if dy != 0.0 {
            let extent = cfg.content_extent(
                self.view_index,
                n,
                pane_w,
                pane_h,
                longest,
                &self.it_label_lines,
                self.details_compact,
            );
            let max = (extent - pane_h).max(0.0);
            let new = (self.scroll_y + dy).clamp(0.0, max);
            dy = new - self.scroll_y;
            self.scroll_y = new;
        }
        if dx == 0.0 && dy == 0.0 {
            return Task::none(); // every edge axis already at its end
        }
        self.marquee_follow_scroll();
        // Details scrolls each axis on its own scrollable (the outer
        // horizontal strip for x, the row list for y)
        if is_details && dx != 0.0 {
            return iced::widget::operation::scrollable::scroll_to(
                crate::folder::details_h_scrollable_id(),
                iced::widget::operation::scrollable::AbsoluteOffset {
                    x: Some(self.scroll_x),
                    y: None,
                },
                iced::widget::operation::Animation::Instant,
            );
        }
        let (x, y) = if is_list {
            (Some(self.scroll_x), None)
        } else {
            (None, Some(self.scroll_y))
        };
        iced::widget::operation::scrollable::scroll_to(
            crate::folder::file_scrollable_id(),
            iced::widget::operation::scrollable::AbsoluteOffset { x, y },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// Width the Details strip needs (files margin + the visible
    /// column block + the right chrome pad): when this exceeds the
    /// pane the outer horizontal scrollable scrolls. Mirrors the view
    /// side (`folder.rs::details_cols_w`).
    fn details_natural_w(&self) -> f32 {
        let vis: Vec<usize> = self
            .details_col_order
            .iter()
            .copied()
            .filter(|&g| self.details_col_on.get(g).copied().unwrap_or(true))
            .collect();
        let cols_w = vis.iter().enumerate().fold(0.0, |acc, (n, &g)| {
            acc + self.details_col_w(g)
                + if n + 1 < vis.len() {
                    crate::folder::COL_RESIZER_W
                } else {
                    0.0
                }
        });
        self.detail_cfg.files_left + cols_w + self.detail_cfg.chrome_pad
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

/// Shift-animation frame step and total length (the base headers slide
/// aside when the dragged header crosses a midpoint).
const REORDER_TICK_MS: f32 = 16.0;
const REORDER_SHIFT_MS: f32 = 120.0;

/// Details column header reorder drag (the Windows header drag): the
/// dragged header floats with the cursor; the other headers shift
/// aside, animated, when its center crosses a target header's
/// midpoint; on release the column order commits and the rows follow.
pub(crate) struct ColReorder {
    /// dragged column (index into `detail_cfg.columns`)
    pub col: usize,
    /// cursor x at drag start (window-logical; deltas only — the
    /// header's window x is never needed)
    pub start_x: f32,
    /// cursor x now
    pub cur_x: f32,
    /// the insertion slot reserved for the dragged header (index into
    /// the visible list minus the dragged column)
    pub over: usize,
    /// the slot the shift animation interpolates from
    pub from_over: usize,
    /// shift animation progress 0..=1 (1 = settled)
    pub anim_t: f32,
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
        let cols = e
            .view_cfg
            .grid_cols_at(e.view_index, t.cell_w, 0.0, e.file_pane_width());
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
        // (96px viewport − 6px top strip) / 24px rows = 3 rows per column
        assert_eq!(e.neighbor(0, FocusDir::Down), Some(1));
        assert_eq!(e.neighbor(2, FocusDir::Down), None);
        // index 3 tops the second column: nothing above, one left
        assert_eq!(e.neighbor(3, FocusDir::Up), None);
        assert_eq!(e.neighbor(4, FocusDir::Up), Some(3));
        assert_eq!(e.neighbor(3, FocusDir::Left), Some(0));
        assert_eq!(e.neighbor(3, FocusDir::Right), Some(6));
        assert_eq!(e.neighbor(9, FocusDir::Right), None);
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

    #[test]
    fn double_free_space_click_hides_the_idle_selector() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        e.selected.insert(2);
        // first free-space click: selection released, rectangle shown
        let _ = e.update(Message::MarqueeBegin { x: 5.0, y: 5.0 });
        let _ = e.update(Message::MarqueeEnd);
        assert_eq!(e.focus_item, Some(2));
        assert!(e.focus_shown);
        // double-click on empty space: the rectangle hides where it
        // is, without moving back to the first item
        let _ = e.update(Message::MarqueeBegin { x: 6.0, y: 6.0 });
        let _ = e.update(Message::MarqueeEnd);
        assert_eq!(e.focus_item, Some(2));
        assert!(!e.focus_shown);
        // the next keyboard navigation shows it again
        let _ = e.update(Message::FocusMove(FocusDir::Down));
        assert_eq!(e.focus_item, Some(3));
        assert!(e.focus_shown);
    }

    #[test]
    fn marquee_edge_scrolls_toward_the_cursor() {
        let mut e = explorer(100);
        e.view_index = e.view_cfg.presets.details;
        // the cursor held at the pane's bottom edge: the content
        // scrolls down and the band's far corner tracks the pointer
        let bottom = e.viewport_h - 1.0;
        let _ = e.update(Message::MarqueeBegin { x: 10.0, y: 10.0 });
        let _ = e.update(Message::MarqueeMove { x: 10.0, y: bottom });
        let _ = e.update(Message::MarqueeAutoScroll);
        assert!(e.scroll_y > 0.0, "bottom edge must scroll down");
        let m = e.marquee.as_ref().expect("drag still live");
        assert!((m.y1 - (m.py + e.scroll_y)).abs() < 0.01);
        // the anchor corner stays content-fixed (it is NOT re-anchored)
        assert_eq!(m.y0, 10.0);
        // …and the anchor moved INTO the viewport as content scrolled:
        // drawn at y0 − scroll_y, it rides the content like a canvas
        assert!(m.y0 - e.scroll_y < 10.0);
        // the cursor parked mid-pane: no scrolling, no task. Pane-local
        // y is what matters: keep it clear of both 24px margins
        // (below the Details header + margin, above the bottom margin)
        let before = e.scroll_y;
        let _ = e.update(Message::MarqueeMove { x: 10.0, y: before + 60.0 });
        let _ = e.update(Message::MarqueeAutoScroll);
        assert_eq!(e.scroll_y, before);
        // the top edge scrolls back up (accelerating step)
        let _ = e.update(Message::MarqueeMove { x: 10.0, y: 2.0 });
        let _ = e.update(Message::MarqueeAutoScroll);
        assert!(e.scroll_y < before, "top edge must scroll up");
    }

    #[test]
    fn marquee_wheel_reanchors_the_far_corner() {
        let mut e = explorer(100);
        e.view_index = e.view_cfg.presets.details;
        let _ = e.update(Message::MarqueeBegin { x: 10.0, y: 10.0 });
        // park the corner just above row 1: only row 0 is inside the band
        let row1_y = {
            let (_, y, _, _) = e.item_rect(1, e.file_pane_width());
            y
        };
        let _ = e.update(Message::MarqueeMove { x: 60.0, y: row1_y - 2.0 });
        assert!(!e.selected.contains(&1));
        // a wheel roll scrolls under the stationary cursor: the far
        // corner must follow the pointer (content coord = pane + scroll)
        let _ = e.update(Message::FileScrollTo(30.0));
        let m = e.marquee.as_ref().expect("drag still live");
        assert!((m.y1 - (m.py + e.scroll_y)).abs() < 0.01);
        // row 1 sat below the old corner; it joins once the corner tracks
        assert!(e.selected.contains(&1));
    }

    #[test]
    fn details_right_engages_the_column_highlight_without_touching_selection() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        // no idle selector shown, no selection: Right reveals the idle
        // selector only — it must NOT select the item, and the
        // highlight is not engaged yet
        assert!(!e.focus_shown);
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert!(e.focus_shown);
        assert_eq!(e.detail_col_focus, None);
        assert!(e.selected.is_empty());
        // now engaged: Right jumps straight to the first highlight
        // past the leftmost column (position 0 IS the whole idle
        // selector — already on screen), leaving the idle selector
        // and the selection alone
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(1));
        assert!(e.selected.is_empty());
        assert!(e.focus_shown);
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(2));
        // …and Left walks it back; reaching the leftmost column puts
        // the whole idle selector back (position 0, never drawn)
        let _ = e.update(Message::FocusMove(FocusDir::Left));
        assert_eq!(e.detail_col_focus, Some(1));
        let _ = e.update(Message::FocusMove(FocusDir::Left));
        assert_eq!(e.detail_col_focus, Some(0));
        let _ = e.update(Message::FocusMove(FocusDir::Left));
        assert_eq!(e.detail_col_focus, Some(0), "clamped at the first column");
    }

    #[test]
    fn details_column_highlight_releases_on_a_mouse_press() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        e.focus_shown = true;
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(1));
        // a manual click on another row releases the highlight (the
        // plain click also selects the pressed row as usual)
        let _ = e.update(Message::Select(3));
        assert_eq!(e.detail_col_focus, None);
        assert_eq!(e.focus_item, Some(3));
        // …and so does a free-space (marquee) press
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(1));
        let _ = e.update(Message::MarqueeBegin { x: 5.0, y: 5.0 });
        assert_eq!(e.detail_col_focus, None);
    }

    #[test]
    fn details_column_highlight_walks_to_the_last_column_and_clamps() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        e.focus_shown = true;
        let vis = e
            .details_col_order
            .iter()
            .filter(|&&g| e.details_col_on.get(g).copied().unwrap_or(true))
            .count();
        assert!(vis >= 2);
        for _ in 0..vis + 2 {
            let _ = e.update(Message::FocusMove(FocusDir::Right));
        }
        assert_eq!(e.detail_col_focus, Some(vis - 1), "clamped at the last column");
    }

    #[test]
    fn details_column_highlight_follows_vertical_moves() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        e.focus_shown = true;
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(1));
        // Down moves like a plain idle move (the selection follows)
        // and KEEPS the column: the highlight follows to the next line
        let _ = e.update(Message::FocusMove(FocusDir::Down));
        assert_eq!(e.detail_col_focus, Some(1));
        assert_eq!(e.focus_item, Some(1));
        assert!(e.selected.contains(&1));
        // Ctrl+Down likewise keeps the column (rectangle-only move)
        e.ctrl_held = true;
        let _ = e.update(Message::FocusMove(FocusDir::Down));
        assert_eq!(e.detail_col_focus, Some(1));
        assert_eq!(e.focus_item, Some(2));
        assert!(e.selected.contains(&1), "Ctrl+Down moves the rectangle only");
        e.ctrl_held = false;
        // the highlight now sits on line two's same column; Up walks
        // back the same way
        let _ = e.update(Message::FocusMove(FocusDir::Up));
        assert_eq!(e.detail_col_focus, Some(1));
        assert_eq!(e.focus_item, Some(1));
    }

    #[test]
    fn details_column_highlight_engages_over_a_selection() {
        let mut e = explorer(5);
        e.view_index = e.view_cfg.presets.details;
        // a selection without a shown idle selector still engages,
        // focused on the tracked item
        e.selected.insert(3);
        e.focus_item = Some(3);
        let _ = e.update(Message::FocusMove(FocusDir::Right));
        assert_eq!(e.detail_col_focus, Some(1));
        assert_eq!(e.focus_item, Some(3));
        assert_eq!(e.selected.iter().copied().collect::<Vec<_>>(), vec![3]);
    }
}

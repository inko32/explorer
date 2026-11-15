//! Context-menu update logic: right-click selection, menu construction
//! and command dispatch. The `handle` match delegates here.


use crate::fs;

use super::explorer::Explorer;
use super::message::Message;

impl Explorer {
    /// Right-click a file-view item: Windows selects it first when it is
    /// not part of the selection, then opens the menu at the cursor.
    pub(crate) fn context_select(&mut self, i: usize) {
        let (x, y) = (self.cursor.x, self.cursor.y);
        self.context_select_at(i, x, y);
    }

    /// Selection menu at an explicit anchor (right-click: the cursor;
    /// Menu key: the element's left-bottom corner).
    pub(crate) fn context_select_at(&mut self, i: usize, x: f32, y: f32) {
        if !self.selected.contains(&i) {
            self.selected.clear();
            self.selected.insert(i);
        }
        let n = self.selected.len();
        let (mut has_files, mut has_dirs) = (false, false);
        let mut files = Vec::new();
        for &i in &self.selected {
            if let Some(e) = self.entries.get(i) {
                if e.is_dir() {
                    has_dirs = true;
                } else {
                    has_files = true;
                    files.push(e.path.clone());
                }
            }
        }
        let entries = if fs::archive::resolve(&self.cwd).is_some() {
            crate::menu::build_archive(&self.menu_cfg, n, has_files, has_dirs)
        } else {
            let groups = crate::menu::groups_for(&self.menu_cfg, &files);
            crate::menu::build_selection(&self.menu_cfg, n, has_files, has_dirs, &groups)
        };
        // the menu shields the view behind it: drop the live hover and
        // the nav context mark
        self.hovered = None;
        self.nav_hover = None;
        self.nav_selected = None;
        self.menu = Some(crate::menu::Open {
            entries,
            x,
            y,
            hover: None,
            nav_target: None,
            bar: None,
        });
        self.menu_popup_open();
    }

    /// Right-click a Details column header: the column menu — one
    /// checkable entry per available column (SPEC/detail-view.txt §9).
    pub(crate) fn column_settings_menu(&mut self) {
        let entries = self
            .detail_cfg
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let on = self.details_col_on.get(i).copied().unwrap_or(true);
                crate::menu::Entry::Item(crate::menu::Item::plain(
                    &format!("col_toggle:{}", c.id),
                    if on { format!("✓ {}", c.label) } else { c.label.clone() },
                ))
            })
            .collect();
        self.open_column_menu(entries);
    }

    /// The Details column dropdown button: its filter menu (§9). The
    /// sort entries come first; real filtering is not implemented yet.
    pub(crate) fn column_filter_menu(&mut self, col: usize) {
        let Some(c) = self.detail_cfg.columns.get(col) else { return };
        let mut entries = Vec::new();
        if c.sort.is_some() {
            entries.push(crate::menu::Entry::Item(crate::menu::Item::plain(
                &format!("col_sort_asc:{}", c.id),
                "Sort ascending".into(),
            )));
            entries.push(crate::menu::Entry::Item(crate::menu::Item::plain(
                &format!("col_sort_desc:{}", c.id),
                "Sort descending".into(),
            )));
            entries.push(crate::menu::Entry::Sep);
        }
        entries.push(crate::menu::Entry::Item(crate::menu::Item::disabled(
            "col_filter",
            "Filter…".into(),
        )));
        self.open_column_menu(entries);
    }

    /// Open a Details column menu at the cursor (like a context menu).
    fn open_column_menu(&mut self, entries: Vec<crate::menu::Entry>) {
        // the menu shields the view behind it: drop the live hover
        self.hovered = None;
        self.menu = Some(crate::menu::Open {
            entries,
            x: self.cursor.x,
            y: self.cursor.y,
            hover: None,
            nav_target: None,
            bar: None,
        });
    }

    /// Host `self.menu` off-window: paint it and open the xdg_popup at
    /// its anchor. No-op (in-window fallback) without the side channel
    /// or for address-bar dropdowns (`bar: Some`, anchored widgets).
    /// Call right after assigning `self.menu`.
    pub(crate) fn menu_popup_open(&mut self) {
        let Some(wl) = &self.dnd_wl else { return };
        let Some(open) = &self.menu else { return };
        if open.bar.is_some() {
            return;
        }
        let skin = if self.dark {
            crate::ui_common::skin::dark()
        } else {
            crate::ui_common::skin::light()
        };
        let painted = self.scale_dpi();
        let Some(visual) = crate::menu::paint_popup(open, &self.menu_cfg, skin, painted)
        else {
            return;
        };
        // unit chain: the popup buffer is painted at `painted`
        // (= scale_dpi), and its wl buffer scale equals the PARENT
        // surface's — winit uses ceil(effective scale), and the
        // effective scale IS scale_dpi (system scale x the --dpi
        // multiplier). On fractional-capable compositors the side
        // channel maps buffer/painted into the parent's surface-local
        // space 1:1 with a wp_viewport; on integer-only compositors it
        // falls back to the buffer/scale chain. Input arrives in
        // popup-local units and is converted to model-logical inside
        // the side channel.
        let popup_scale = painted.ceil().max(1.0) as i32;
        wl.open_popup(painted, popup_scale, visual);
        self.menu_popup = true;
    }

    /// Re-paint an open popup (hover changes).
    pub(crate) fn repaint_menu_popup(&mut self) {
        if !self.menu_popup {
            return;
        }
        let (Some(wl), Some(open)) = (&self.dnd_wl, &self.menu) else {
            return;
        };
        let skin = if self.dark {
            crate::ui_common::skin::dark()
        } else {
            crate::ui_common::skin::light()
        };
        if let Some(visual) =
            crate::menu::paint_popup(open, &self.menu_cfg, skin, self.scale_dpi())
        {
            wl.update_popup(visual);
        }
    }

    /// Close the menu wherever it is hosted (popup or in-window).
    pub(crate) fn menu_close(&mut self) {
        self.menu = None;
        if self.menu_popup {
            self.menu_popup = false;
            if let Some(wl) = &self.dnd_wl {
                wl.close_popup();
            }
        }
    }

    /// Menu item activated: close the menu, then dispatch (§8: every
    /// command re-validates at execution — the file ops report errors
    /// via the status bar).
    pub(crate) fn menu_dispatch(&mut self, idx: usize) -> iced::Task<Message> {
        if self.menu_popup
            && let Some(wl) = &self.dnd_wl
        {
            wl.close_popup();
            self.menu_popup = false;
        }
        let Some(open) = self.menu.take() else {
            return iced::Task::none();
        };
        // any menu pick ends the pending-drop lifecycle (the drop ids
        // below consume it; everything else must leave no stale state)
        let pending = self.pending_drop.take();
        // the menu is closing: the nav context mark goes with it
        self.nav_selected = None;
        // address-bar dropdowns (history / subfolders / selector): the
        // pick goes straight to the entry paths — no yaml command lookup
        if let Some(bar) = &open.bar {
            let item = matches!(open.entries.get(idx), Some(crate::menu::Entry::Item(_)));
            let path = bar
                .paths
                .get(idx)
                .cloned()
                .filter(|p| !p.as_os_str().is_empty());
            // a bar-menu pick ends address edit mode (clicks on the
            // dropdown entries give up the modifications)
            self.address_edit = None;
            if let (true, Some(path)) = (item, path) {
                return match bar.kind {
                    crate::menu::BarMenuKind::Subdirs => self.navigate(path),
                    crate::menu::BarMenuKind::History | crate::menu::BarMenuKind::Selector => {
                        self.history_nav(path)
                    }
                };
            }
            return iced::Task::none();
        }
        let Some(crate::menu::Entry::Item(item)) = open.entries.get(idx).cloned() else {
            return iced::Task::none();
        };
        match item.id.as_str() {
            // drag drop-behavior menu (PendingDrop was stashed when the
            // ambiguous drop opened it): the pick becomes the drop action
            "drop_move" | "drop_copy" | "drop_link" | "drop_run" => {
                let Some(pending) = pending else {
                    return iced::Task::none();
                };
                let action = match item.id.as_str() {
                    "drop_move" => crate::dnd::DropAction::Move,
                    "drop_copy" => crate::dnd::DropAction::Copy,
                    "drop_link" => crate::dnd::DropAction::Link,
                    _ => crate::dnd::DropAction::Run,
                };
                self.perform_drop(pending.sources, pending.target, action)
            }
            _ if item.id == "drop_cancel" => iced::Task::none(),
            // Details column menu picks (built in code above, not from
            // menu.yaml): toggle a column, sort by a column, placeholder
            _ if item.id.starts_with("col_toggle:") => {
                let id = &item.id["col_toggle:".len()..];
                if let Some(i) = self.detail_cfg.columns.iter().position(|c| c.id == id)
                    && let Some(on) = self.details_col_on.get_mut(i)
                {
                    *on = !*on;
                }
                iced::Task::none()
            }
            _ if item.id.starts_with("col_sort_asc:")
                || item.id.starts_with("col_sort_desc:") =>
            {
                let asc = item.id.starts_with("col_sort_asc:");
                let id = item.id.rsplit(':').next().unwrap_or("");
                if let Some(i) = self.detail_cfg.columns.iter().position(|c| c.id == id)
                    && let Some(key) = self.detail_cfg.columns[i].sort
                {
                    self.sort = key;
                    self.sort_asc = asc;
                    self.apply_sort();
                }
                iced::Task::none()
            }
            "col_filter" => {
                self.status = "Column filtering is not implemented yet".into();
                iced::Task::none()
            }
            "open" => {
                if let Some(p) = open.nav_target {
                    return self.update(Message::Navigate(p));
                }
                // directories navigate (first one); files go to the §5 backend
                let mut nav = None;
                let mut files = Vec::new();
                for &i in &self.selected {
                    if let Some(e) = self.entries.get(i) {
                        if (e.is_dir() || fs::archive::is_archive_path(&e.path)) && nav.is_none() {
                            nav = Some(e.path.clone());
                        } else if !e.is_dir() {
                            files.push(e.path.clone());
                        }
                    }
                }
                if !files.is_empty() {
                    if fs::archive::resolve(&self.cwd).is_some() {
                        self.status = "opening inside archives is not supported yet".into();
                    } else if let Err(e) = crate::menu::open_files(&self.menu_cfg, &files) {
                        self.status = format!("Open: {e}");
                    }
                }
                if let Some(p) = nav {
                    return self.navigate(p);
                }
                iced::Task::none()
            }
            "cut" => self.update(Message::CutSelection),
            "copy" => self.update(Message::CopySelection),
            "paste" => self.update(Message::Paste),
            "delete" => self.update(Message::DeleteSelection),
            "rename" => self.update(Message::RenameSelected),
            "select_all" => self.update(Message::SelectAll),
            "refresh" => self.update(Message::Refresh),
            "new_folder" => self.update(Message::NewFolder),
            "properties" => {
                self.status = "Properties dialog is not implemented yet".into();
                iced::Task::none()
            }
            // §6 placeholder (status notice until the dialog lands)
            "open_with" => {
                self.status = "Open with: application selection is not implemented yet".into();
                iced::Task::none()
            }
            "print" => {
                self.status = "Printing is not configured yet".into();
                iced::Task::none()
            }
            // type-group marker (menu.yaml imagefiles): proves the
            // menu distinguishes file types; the feature itself is reserved
            "preview" => {
                self.status = "Preview is not implemented yet".into();
                iced::Task::none()
            }
            "extract_here" => {
                self.status = "Extraction is not supported yet".into();
                iced::Task::none()
            }
            "open_new_window" => {
                // nav menu: the row's path; selection menu: the (single)
                // selected folder; else the current folder
                let target = open
                    .nav_target
                    .or_else(|| {
                        self.selected
                            .iter()
                            .next()
                            .and_then(|&i| self.entries.get(i))
                            .map(|e| e.path.clone())
                    })
                    .unwrap_or_else(|| self.cwd.clone());
                match std::env::current_exe() {
                    Ok(exe) => {
                        if let Err(e) = std::process::Command::new(exe).arg(&target).spawn() {
                            self.status = format!("New window: {e}");
                        }
                    }
                    Err(e) => self.status = format!("New window: {e}"),
                }
                iced::Task::none()
            }
            _ => {
                // YAML-defined command (§2 dispatcher): argv + paths
                let paths = open
                    .nav_target
                    .map(|p| vec![p])
                    .unwrap_or_else(|| self.selected_paths());
                if let Some(argv) = item.run {
                    if let Err(e) = crate::menu::run_custom(&argv, &paths) {
                        self.status = e;
                    }
                } else {
                    self.status = format!("unknown command {:?}", item.id);
                }
                iced::Task::none()
            }
        }
    }
}

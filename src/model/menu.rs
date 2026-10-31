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
        });
    }

    /// Menu item activated: close the menu, then dispatch (§8: every
    /// command re-validates at execution — the file ops report errors
    /// via the status bar).
    pub(crate) fn menu_dispatch(&mut self, idx: usize) -> iced::Task<Message> {
        let Some(open) = self.menu.take() else {
            return iced::Task::none();
        };
        // any menu pick ends the pending-drop lifecycle (the drop ids
        // below consume it; everything else must leave no stale state)
        let pending = self.pending_drop.take();
        // the menu is closing: the nav context mark goes with it
        self.nav_selected = None;
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

//! Drag & drop update logic (SPEC/DRAGGING.md): drag start from a
//! press past the threshold, and the drop commit that dispatches the
//! file operation to the fs worker. The `handle` match delegates here.

use std::path::{Path, PathBuf};


use crate::fs::Job;

use super::explorer::Explorer;
use super::message::Message;

impl Explorer {
    /// §5.1 drop-target feedback: `path` is the innermost target under
    /// the pointer while a drag is active.
    pub(crate) fn drop_targeted(&self, path: &Path) -> bool {
        self.dnd.active() && self.dnd.current().and_then(|t| t.path()) == Some(path)
    }

    /// §5 drag start: the press moved past the threshold on a file-view
    /// row. Drags the whole selection when the row is part of it,
    /// otherwise the row alone (which becomes the selection).
    pub(crate) fn begin_drag(&mut self, i: usize, right: bool) {
        if i >= self.entries.len() {
            return;
        }
        let paths = if self.selected.contains(&i) {
            self.selected_paths()
        } else {
            self.selected.clear();
            self.selected.insert(i);
            vec![self.entries[i].path.clone()]
        };
        if paths.is_empty() {
            return;
        }
        let items = paths
            .iter()
            .map(|p| {
                let e = self.entries.iter().find(|e| &e.path == p);
                let kind = match e {
                    Some(e) if e.is_symlink => crate::dnd::ItemKind::Symlink,
                    Some(e) if e.is_dir() => crate::dnd::ItemKind::Directory,
                    Some(_) => crate::dnd::ItemKind::File,
                    None => crate::dnd::ItemKind::Other,
                };
                crate::dnd::DraggedItem {
                    uri: crate::dnd::file_uri(p),
                    kind,
                }
            })
            .collect();
        let n = paths.len();
        let top = Some(crate::dnd::file_uri(&self.entries[i].path));
        self.dnd.start(crate::dnd::DragSession {
            payload: crate::dnd::DragPayload::Files(items),
            source: crate::dnd::DragSource::FileView,
            right,
            top,
        });
        // The cursor was already inside the file view when the drag
        // began, so the empty area's zone fired its enter before the
        // session existed and the event was dropped: seed it (dropping
        // back onto empty space targets the current folder).
        let seed = crate::dnd::DropTarget::EmptyDirectory(self.cwd.clone());
        self.dnd
            .apply(crate::dnd::DragEvent::Enter { target: seed });
        self.dnd
            .set_modifiers(self.ctrl_held, self.shift_held, self.alt_held);
        self.status = format!("Dragging {n} item{}", if n == 1 { "" } else { "s" });
    }

    /// §5.4 drop: revalidate the target and dispatch the file operation
    /// to the fs worker (§10 — the UI never copies files itself). An
    /// action-ambiguous drop (Shift / right-button drag) opens the
    /// action-choice menu instead of committing.
    pub(crate) fn commit_drag(&mut self) -> iced::Task<Message> {
        let target = self.dnd.current().cloned();
        let ambiguous = self.dnd.ambiguous();
        let Some(session) = self.dnd.take() else {
            return iced::Task::none();
        };
        let Some(target) = target else {
            // released over no target: nothing happens (§5.5 semantics)
            self.status = "Drop cancelled".into();
            return iced::Task::none();
        };
        let sources: Vec<PathBuf> = match &session.payload {
            crate::dnd::DragPayload::Files(items) => items
                .iter()
                .filter_map(|i| crate::dnd::uri_to_path(&i.uri))
                .collect(),
            _ => {
                self.status = "this data cannot be dropped here".into();
                return iced::Task::none();
            }
        };
        if sources.is_empty() {
            self.status = "only local files can be dropped here".into();
            return iced::Task::none();
        }
        let permitted = crate::dnd::permitted_actions(&session.payload, &target);
        if ambiguous {
            if permitted.is_empty() {
                self.status = "Cannot drop here".into();
            } else {
                self.open_drop_menu(sources, target, &permitted);
            }
            return iced::Task::none();
        }
        let action =
            crate::dnd::resolve_action(self.ctrl_held, self.shift_held, self.alt_held, &permitted);
        self.perform_drop(sources, target, action)
    }

    /// The action-choice menu for an ambiguous drop (§5.4): every
    /// permitted action, the non-ambiguous default in bold (Windows
    /// convention), plus Cancel. The pick executes via `perform_drop`.
    fn open_drop_menu(
        &mut self,
        sources: Vec<PathBuf>,
        target: crate::dnd::DropTarget,
        permitted: &[crate::dnd::DropAction],
    ) {
        use crate::dnd::DropAction as A;
        // like the context menus: the menu shields the view behind it
        self.hovered = None;
        self.nav_hover = None;
        self.nav_selected = None;
        self.pending_drop = Some(super::dnd::PendingDrop {
            sources,
            target: target.clone(),
        });
        // deterministic order, not the policy vec's
        let offered = [A::Move, A::Copy, A::Link, A::Run];
        let default = crate::dnd::resolve_action(false, false, false, permitted);
        let name = target
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        let mut entries: Vec<crate::menu::Entry> = Vec::new();
        for action in offered.iter().filter(|a| permitted.contains(a)) {
            let (id, label) = match action {
                A::Move => ("drop_move", "Move here".to_string()),
                A::Copy => ("drop_copy", "Copy here".to_string()),
                A::Link => ("drop_link", "Create link here".to_string()),
                A::Run => (
                    "drop_run",
                    match &name {
                        Some(n) => format!("Run with \"{n}\""),
                        None => "Run here".to_string(),
                    },
                ),
                _ => continue,
            };
            let mut item = crate::menu::Item::plain(id, label);
            item.bold = *action == default;
            entries.push(crate::menu::Entry::Item(item));
        }
        entries.push(crate::menu::Entry::Sep);
        entries.push(crate::menu::Entry::Item(crate::menu::Item::plain(
            "drop_cancel",
            "Cancel".to_string(),
        )));
        self.menu = Some(crate::menu::Open {
            entries,
            x: self.cursor.x,
            y: self.cursor.y,
            hover: None,
            nav_target: None,
        });
    }

    /// §5.4/§12: the single place a drop becomes a file operation.
    /// Revalidates against the live filesystem, then dispatches the
    /// job to the fs worker (the UI never copies files itself).
    pub(crate) fn perform_drop(
        &mut self,
        sources: Vec<PathBuf>,
        target: crate::dnd::DropTarget,
        action: crate::dnd::DropAction,
    ) -> iced::Task<Message> {
        if let Err(reason) = crate::dnd::validate_commit(&sources, &target, action) {
            self.status = reason;
            return iced::Task::none();
        }
        let Some(dest) = target.path().map(Path::to_path_buf) else {
            self.status = "this target cannot accept the drop".into();
            return iced::Task::none();
        };
        match action {
            crate::dnd::DropAction::Copy => {
                self.sched.send(Job::Copy {
                    sources,
                    dest_dir: dest,
                    cut: false,
                });
                self.status = "Copying…".into();
            }
            crate::dnd::DropAction::Move => {
                self.sched.send(Job::Copy {
                    sources,
                    dest_dir: dest,
                    cut: true,
                });
                self.status = "Moving…".into();
            }
            crate::dnd::DropAction::Link => {
                self.sched.send(Job::Link {
                    sources,
                    dest_dir: dest,
                });
                self.status = "Creating links…".into();
            }
            other => {
                self.status = format!("{other:?} drops are not implemented");
            }
        }
        iced::Task::none()
    }
}

/// A file-view press that may still become a drag (SPEC/DRAGGING.md §5):
/// the row, the cursor at press, and whether the release must collapse
/// a multi-selection down to that row (press-without-drag on a selected
/// row of a multi-selection). `right` marks a right-button drag: its
/// drop opens the action-choice menu instead of committing.

#[derive(Clone, Copy)]
pub(crate) struct PressArm {
    pub(crate) i: usize,
    pub(crate) at: iced::Point,
    pub(crate) collapse: bool,
    pub(crate) right: bool,
}

/// A drop released while the drag was action-ambiguous (Shift+drag /
/// right-button drag): the choice menu is open, waiting for the pick
/// that executes the file operation.
pub(crate) struct PendingDrop {
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) target: crate::dnd::DropTarget,
}

//! Drag & drop update logic: drag start from a
//! press past the threshold, and the drop commit that dispatches the
//! file operation to the fs worker. The `handle` match delegates here.

use std::path::{Path, PathBuf};


use crate::fs::Job;

use super::explorer::Explorer;
use super::message::Message;

impl Explorer {
    /// An external (or our own) drag was dropped on the window with the
    /// given MIME payload. URI lists become a file drop into the current
    /// folder — the same commit path as every other drop (§5.4/§12).
    pub(crate) fn wayland_drop_received(&mut self, mime: String, data: Vec<u8>) {
        if mime == "text/uri-list" {
            let text = String::from_utf8_lossy(&data);
            let sources: Vec<PathBuf> = text
                .lines()
                .flat_map(crate::dnd::uri_to_path)
                .collect();
            if sources.is_empty() {
                self.status = "only local files can be dropped here".into();
                return;
            }
            let target = crate::dnd::DropTarget::EmptyDirectory(self.cwd.clone());
            match crate::dnd::validate_commit(&sources, &target, crate::dnd::DropAction::Copy) {
                Ok(()) => {
                    let _ = self.perform_drop(sources, target, crate::dnd::DropAction::Copy);
                }
                Err(reason) => self.status = reason,
            }
        } else if let Ok(text) = String::from_utf8(data) {
            // non-file drops have no importer yet (§9) — show what came in
            let shown: String = text.chars().take(80).collect();
            self.status = format!("received {mime}: {shown}");
        }
    }

    /// §5.1 drop-target feedback: `path` is the innermost target under
    /// the pointer while a drag is active.
    pub(crate) fn drop_targeted(&self, path: &Path) -> bool {
        self.dnd.active() && self.dnd.current().and_then(|t| t.path()) == Some(path)
    }

    /// §5 drag start: the press moved past the threshold on a file-view
    /// row. Drags the whole selection when the row is part of it,
    /// otherwise the row alone (which becomes the selection).
    pub(crate) fn begin_drag(&mut self, i: usize, right: bool) {
        // a fresh press implies no native drag is live — reset any flag
        // a never-finished previous drag may have left behind
        self.dnd_wl_drag = false;
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
        self.focus_item = Some(i);
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
        // The drag starts as the in-window simulation; it hands off to
        // the native Wayland drag only if the pointer leaves the window
        // (see `wayland_handoff`).
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

    /// The drag image renders in OUTPUT space (the compositor shows it
    /// at the cursor over any surface), where surface-local units map to
    /// physical pixels via `output_scale / buffer_scale`. With integer
    /// buffer scale `ls = ceil(output_scale)` and a paint factor of
    /// `scale_dpi * ls / output_scale`, the icon lands at exactly
    /// `scale_dpi` device px per logical unit at any --dpi (without this,
    /// a 1.5x output enlarges a scale-1 icon 1.5x).
    fn wl_paint_scale(&self) -> (f32, i32) {
        let out = self.system_scale.max(0.01);
        let ls = out.ceil().max(1.0) as i32;
        (self.scale_dpi() * ls as f32 / out, ls)
    }

    /// The simulated drag crossed the window edge: hand it to the
    /// compositor. The implicit grab keeps the press serial valid, so
    /// `start_drag` succeeds even though the press happened earlier.
    /// The drag image starts with a target-less caption ("Move",
    /// "Copy", "Link" or "…") — the protocol cannot name foreign
    /// targets; modifier changes still update it via
    /// `refresh_wayland_visual`.
    pub(crate) fn wayland_handoff(&mut self) {
        let (Some(wl), Some(crate::dnd::DragPayload::Files(items))) =
            (&self.dnd_wl, self.dnd.payload())
        else {
            return;
        };
        let caption = self.wayland_caption();
        let skin = if self.dark {
            crate::ui_common::skin::dark()
        } else {
            crate::ui_common::skin::light()
        };
        let (paint, ls) = self.wl_paint_scale();
        let visual = crate::dnd_wayland::paint_drag_visual(
            items,
            self.dnd.top(),
            items.len(),
            &caption,
            &skin,
            paint,
        );
        let uris: Vec<String> = items.iter().map(|i| i.uri.clone()).collect();
        // Copy|Move advertised together (the core protocol has no link
        // action): the destination (or the compositor) resolves, like
        // our ambiguous-drop menu.
        use smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction as Wl;
        wl.begin_drag(uris, Wl::Copy | Wl::Move, ls, visual);
        self.dnd_wl_drag = true;
    }

    /// The target-less caption for the compositor drag image: the
    /// negotiated action name ("Move"/"Copy"/"Link") or "…" while the
    /// drop is action-ambiguous. In-window zone captions must not be
    /// used — the zones freeze at handoff and would go stale.
    fn wayland_caption(&self) -> crate::dnd::DragCaption {
        let all = [
            crate::dnd::DropAction::Copy,
            crate::dnd::DropAction::Move,
            crate::dnd::DropAction::Link,
        ];
        if self.dnd.ambiguous() {
            crate::dnd::DragCaption::Undecided
        } else {
            let action =
                crate::dnd::resolve_action(self.ctrl_held, self.shift_held, self.alt_held, &all);
            crate::dnd::DragCaption::Action {
                action,
                label: format!("{action:?}"),
            }
        }
    }

    /// Push the current drag caption onto the compositor drag image
    /// (called when the modifier-derived caption changes mid-drag).
    pub(crate) fn refresh_wayland_visual(&mut self) {
        if !self.dnd_wl_drag {
            return;
        }
        let (Some(wl), Some(crate::dnd::DragPayload::Files(items))) =
            (&self.dnd_wl, self.dnd.payload())
        else {
            return;
        };
        let skin = if self.dark {
            crate::ui_common::skin::dark()
        } else {
            crate::ui_common::skin::light()
        };
        let caption = self.wayland_caption();
        let (paint, _) = self.wl_paint_scale();
        let visual = crate::dnd_wayland::paint_drag_visual(
            items,
            self.dnd.top(),
            items.len(),
            &caption,
            &skin,
            paint,
        );
        wl.update_visual(visual);
    }

    /// §5.4 drop: revalidate the target and dispatch the file operation
    /// to the fs worker (§10 — the UI never copies files itself). An
    /// action-ambiguous drop (Shift / right-button drag) opens the
    /// action-choice menu instead of committing.
    pub(crate) fn commit_drag(&mut self) -> iced::Task<Message> {
        // Wayland: the compositor owns the drop — the outcome arrives
        // as `Event::DragFinished` (drop onto our own window also comes
        // back as `Event::Dropped` with the URI list). Just clear the
        // local session.
        if self.dnd_wl_drag {
            self.dnd.take();
            return iced::Task::none();
        }
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
            bar: None,
        });
        self.menu_popup_open();
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

/// A file-view press that may still become a drag:
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

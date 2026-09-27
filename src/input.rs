//! Keyboard bindings — the single table for the whole app (add rebindable
//! config later without touching call sites).

use crate::app::Cmd;
use crate::ui::file_view::ViewMode;

pub fn gather(ctx: &egui::Context, pending: &mut Vec<Cmd>) {
    // While a TextEdit (inline rename, search) has focus, only Escape leaves.
    if ctx.egui_wants_keyboard_input() {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            pending.push(Cmd::CancelRename);
        }
        return;
    }

    let key = |k: egui::Key| ctx.input(move |i| i.key_pressed(k));
    let mods = ctx.input(|i| i.modifiers);

    if key(egui::Key::F2) {
        pending.push(Cmd::RenameSelected);
    }
    if key(egui::Key::Delete) {
        pending.push(Cmd::DeleteSelection);
    }
    if key(egui::Key::Backspace) {
        pending.push(Cmd::Up);
    }
    if mods.alt && key(egui::Key::ArrowLeft) {
        pending.push(Cmd::Back);
    }
    if mods.alt && key(egui::Key::ArrowRight) {
        pending.push(Cmd::Forward);
    }
    if key(egui::Key::F5) || (mods.ctrl && key(egui::Key::R)) {
        pending.push(Cmd::Refresh);
    }
    if mods.ctrl && !mods.shift && key(egui::Key::C) {
        pending.push(Cmd::CopySelection);
    }
    if mods.ctrl && !mods.shift && key(egui::Key::X) {
        pending.push(Cmd::CutSelection);
    }
    if mods.ctrl && !mods.shift && key(egui::Key::V) {
        pending.push(Cmd::Paste);
    }
    if mods.ctrl && key(egui::Key::A) {
        pending.push(Cmd::SelectAll);
    }
    if mods.ctrl && mods.shift && key(egui::Key::N) {
        pending.push(Cmd::NewFolder);
    }
    if mods.ctrl && key(egui::Key::L) {
        pending.push(Cmd::Status("Address bar focus: coming soon".into()));
    }
    let _ = ViewMode::Details; // silence unused until bindings grow
}

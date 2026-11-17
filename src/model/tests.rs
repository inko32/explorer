//! End-to-end drag & drop through the MVU loop:
//! multi-selection + folder drags with the default Move action, and
//! the Ctrl override to Copy.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::dnd::{DragCaption, DropAction, DropTarget};
use crate::fs::{self, FsEvent};
use crate::model::{Explorer, Message};

fn entry(name: &str, path: PathBuf, dir: bool) -> fs::Entry {
    fs::Entry {
        name: name.into(),
        path,
        kind: if dir {
            fs::EntryKind::Dir
        } else {
            fs::EntryKind::File
        },
        size: 3,
        modified: None,
        is_hidden: false,
        is_symlink: false,
        is_executable: false,
        symlink_target: None,
    }
}

fn cursor(x: f32, y: f32) -> Message {
    Message::Event(iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position: iced::Point::new(x, y),
    }))
}

fn release() -> Message {
    Message::Event(iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
        iced::mouse::Button::Left,
    )))
}

fn right_release() -> Message {
    Message::Event(iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
        iced::mouse::Button::Right,
    )))
}

fn wait_for(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

fn fixture(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root.join("a_dir")).unwrap();
    std::fs::write(root.join("b_file"), b"xyz").unwrap();
    std::fs::create_dir_all(root.join("z_dest")).unwrap();
}

/// Drive a real Explorer: select a folder + a file (multi), press the
/// folder, drag past the threshold, drop onto z_dest. The default
/// action must be Move: both items land in z_dest and vanish from
/// the root. Then repeat with Ctrl held: Copy, originals stay.
#[test]
fn dnd_multi_and_folder_default_move() {
    let root = std::env::temp_dir().join("explorer_dnd_mv_test");
    fixture(&root);

    let mut ex = Explorer::new();
    ex.cwd = root.clone();
    let list = |entries: Vec<fs::Entry>| {
        Message::FsEvent(FsEvent::Listed {
            path: root.clone(),
            result: Ok(entries),
        })
    };
    ex.update(list(vec![
        entry("b_file", root.join("b_file"), false),
        entry("a_dir", root.join("a_dir"), true),
        entry("z_dest", root.join("z_dest"), true),
    ]));
    // sorted dirs-first: [a_dir, z_dest, b_file]
    assert_eq!(ex.entries[0].name, "a_dir");
    assert_eq!(ex.entries[1].name, "z_dest");
    assert_eq!(ex.entries[2].name, "b_file");

    // --- multi-selection drag of folder + file, default action ---
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(0)); // a_dir
    ex.ctrl_held = true;
    ex.update(Message::Select(2)); // + b_file
    ex.ctrl_held = false;
    assert_eq!(ex.selected.len(), 2);
    // press the already-selected folder: selection must survive
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(0));
    assert_eq!(
        ex.selected.len(),
        2,
        "press must not collapse the selection"
    );
    // drag past the threshold
    ex.update(cursor(60.0, 60.0));
    let payload = ex.dnd.payload().expect("drag active").clone();
    assert_eq!(payload.len(), 2, "both selected items drag");
    // drop onto z_dest
    ex.update(Message::DragTargetEnter(crate::dnd::DropTarget::Directory(
        root.join("z_dest"),
    )));
    ex.update(release());
    assert_eq!(ex.status, "Moving…", "default action is Move");
    assert!(wait_for(|| {
        root.join("z_dest/a_dir").is_dir() && root.join("z_dest/b_file").is_file()
    }));
    assert!(!root.join("a_dir").exists());
    assert!(!root.join("b_file").exists());

    // --- Ctrl override: Copy ---
    ex.update(list(vec![
        entry("b_file", root.join("b_file"), false),
        entry("c_file", root.join("c_file"), false),
        entry("z_dest", root.join("z_dest"), true),
    ]));
    std::fs::write(root.join("b_file"), b"xyz").unwrap();
    std::fs::write(root.join("c_file"), b"xyz").unwrap();
    // sorted: dirs first -> [z_dest, b_file, c_file]
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(1)); // b_file
    ex.ctrl_held = true;
    ex.update(Message::Select(2)); // + c_file
    ex.ctrl_held = false;
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(1)); // press selected row of the multi
    assert_eq!(ex.selected.len(), 2);
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(crate::dnd::DropTarget::Directory(
        root.join("z_dest"),
    )));
    ex.ctrl_held = true; // Ctrl at release = Copy
    ex.update(release());
    assert_eq!(ex.status, "Copying…", "Ctrl overrides the action to Copy");
    assert!(wait_for(|| root.join("z_dest/b_file").is_file()));
    assert!(root.join("b_file").is_file(), "copy keeps the original");

    let _ = std::fs::remove_dir_all(&root);
}

/// A plain release on a selected row of a multi-selection collapses
/// the selection to that row (no drag, no modifier).
#[test]
fn dnd_press_without_drag_collapses() {
    let root = std::env::temp_dir().join("explorer_dnd_collapse_test");
    fixture(&root);
    let mut ex = Explorer::new();
    ex.cwd = root.clone();
    ex.update(Message::FsEvent(FsEvent::Listed {
        path: root.clone(),
        result: Ok(vec![
            entry("a_dir", root.join("a_dir"), true),
            entry("z_dest", root.join("z_dest"), true),
            entry("b_file", root.join("b_file"), false),
        ]),
    }));
    ex.update(cursor(5.0, 5.0));
    ex.update(Message::Select(0));
    ex.ctrl_held = true;
    ex.update(Message::Select(2));
    ex.ctrl_held = false;
    assert_eq!(ex.selected.len(), 2);
    ex.update(cursor(5.0, 5.0));
    ex.update(Message::Select(0)); // press, no move
    ex.update(release()); // plain release: collapse
    assert_eq!(ex.selected.iter().copied().collect::<Vec<_>>(), vec![0]);
    assert!(!ex.dnd.active());
    let _ = std::fs::remove_dir_all(&root);
}

fn menu_ids(ex: &Explorer) -> Vec<String> {
    ex.menu
        .as_ref()
        .expect("menu open")
        .entries
        .iter()
        .map(|e| match e {
            crate::menu::Entry::Item(i) => i.id.clone(),
            crate::menu::Entry::Sep => "-".into(),
        })
        .collect()
}

/// The cursor caption follows the hovered target: "Move to" for a
/// directory, the banned glyph for a plain file, "Run with" for an
/// executable, "…" for Shift/right drags. An ambiguous drop opens the
/// action-choice menu instead of committing; the pick runs the op.
#[test]
fn dnd_caption_and_choice_menu() {
    let root = std::env::temp_dir().join("explorer_dnd_caption_test");
    fixture(&root);
    let exe = root.join("run_me");
    std::fs::write(&exe, b"#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut ex = Explorer::new();
    ex.cwd = root.clone();
    ex.update(Message::FsEvent(FsEvent::Listed {
        path: root.clone(),
        result: Ok(vec![
            entry("a_dir", root.join("a_dir"), true),
            entry("z_dest", root.join("z_dest"), true),
            entry("b_file", root.join("b_file"), false),
        ]),
    }));
    let dest = || DropTarget::Directory(root.join("z_dest"));

    // plain drag onto a directory: the "Move to" caption
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(2)); // b_file
    ex.update(cursor(60.0, 60.0)); // past the threshold
    assert!(ex.dnd.active());
    ex.update(Message::DragTargetEnter(dest()));
    assert_eq!(
        ex.dnd.caption(),
        Some(&DragCaption::Action {
            action: DropAction::Move,
            label: "Move to \"z_dest\"".into(),
        })
    );
    ex.update(Message::DragCancel);
    assert!(!ex.dnd.active());
    assert_eq!(ex.dnd.caption(), None);

    // hovering another plain file (the exe fixture, not dragged): banned
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(2));
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(DropTarget::File(exe.clone())));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Banned));
    ex.update(Message::DragCancel);

    // hovering an executable: "Run with"
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(2));
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(DropTarget::Executable(exe.clone())));
    assert_eq!(
        ex.dnd.caption(),
        Some(&DragCaption::Action {
            action: DropAction::Run,
            label: "Run with \"run_me\"".into(),
        })
    );
    ex.update(Message::DragCancel);

    // Shift mid-drag: the "…" caption, the drop opens the choice menu
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(2));
    ex.shift_held = true; // pressed after the press armed the drag
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(dest()));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Undecided));
    ex.update(release());
    assert!(!ex.dnd.active(), "the ambiguous drop must not commit");
    assert_eq!(
        menu_ids(&ex),
        ["drop_move", "drop_copy", "drop_link", "-", "drop_cancel"]
    );
    // the default action (Move) is bold, per the Windows convention
    match &ex.menu.as_ref().unwrap().entries[0] {
        crate::menu::Entry::Item(i) => assert!(i.bold, "the default action is bold"),
        _ => panic!("expected the Move item"),
    }
    // pick Copy: b_file lands in z_dest, the original stays
    ex.update(Message::MenuPick(1));
    assert_eq!(ex.status, "Copying…");
    assert!(wait_for(|| root.join("z_dest/b_file").is_file()));
    assert!(root.join("b_file").is_file(), "copy keeps the original");

    // right-drag: also "…", releasing the button drops via the menu;
    // picking Cancel commits nothing
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::RightPress(2));
    ex.update(cursor(60.0, 60.0));
    assert!(ex.dnd.active() && ex.dnd.right());
    ex.update(Message::DragTargetEnter(dest()));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Undecided));
    ex.update(right_release());
    assert_eq!(
        menu_ids(&ex),
        ["drop_move", "drop_copy", "drop_link", "-", "drop_cancel"]
    );
    ex.update(Message::MenuPick(4)); // Cancel
    assert!(!root.join("z_dest/a_dir").exists());

    // a plain right release (no drag) opens the row's context menu
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::RightPress(2));
    ex.update(right_release());
    assert!(ex.menu.is_some());
    assert!(!ex.dnd.active());
    ex.update(Message::CloseMenu);

    let _ = std::fs::remove_dir_all(&root);
}

/// An open menu is modal for hover only: the items under it must not
/// highlight, but clicks/right-clicks still reach them (hover messages
/// flow again once the menu closes).
#[test]
fn hover_frozen_while_menu_open() {
    let mut ex = Explorer::new();
    ex.update(Message::Hover(Some(3)));
    assert_eq!(ex.hovered, Some(3));
    ex.update(Message::ContextBackground);
    assert!(ex.menu.is_some());
    assert_eq!(ex.hovered, None, "opening a menu drops the live hover");
    // hover events pass through the menu to the items behind: ignored
    ex.update(Message::Hover(Some(5)));
    assert_eq!(ex.hovered, None, "file hover frozen while the menu is open");
    ex.update(Message::NavHover(Some((PathBuf::from("/x"), false))));
    assert_eq!(ex.nav_hover, None, "nav hover frozen too");
    ex.update(Message::CloseMenu);
    ex.update(Message::Hover(Some(7)));
    assert_eq!(ex.hovered, Some(7), "hover flows again after the menu closes");
}

/// Right-clicking a nav item: the nav pane takes focus, the row shows
/// the selection mark while its menu is open, and the file-view
/// selection is left untouched.
#[test]
fn nav_context_menu_focus_and_mark() {
    let mut ex = Explorer::new();
    ex.selected.insert(0);
    ex.update(Message::ContextNav(PathBuf::from("/data/dir")));
    assert!(matches!(ex.active_pane, crate::model::Pane::Nav));
    assert_eq!(ex.nav_selected, Some(PathBuf::from("/data/dir")));
    assert!(ex.menu.is_some());
    assert_eq!(ex.selected.len(), 1, "file selection untouched");
    ex.update(Message::CloseMenu);
    assert_eq!(ex.nav_selected, None, "the mark goes with the menu");
}

/// Hovering the dragged item itself or its current parent folder shows
/// nothing at all (a no-op drop, not a rejection); a read-only
/// destination shows the banned glyph — and releasing there commits
/// nothing.
#[test]
fn dnd_caption_banned_for_self_and_readonly() {
    let root = std::env::temp_dir().join("explorer_dnd_ban_test");
    fixture(&root);
    let ro = root.join("ro_dest");
    std::fs::create_dir_all(&ro).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    }

    let mut ex = Explorer::new();
    ex.cwd = root.clone();
    ex.update(Message::FsEvent(FsEvent::Listed {
        path: root.clone(),
        result: Ok(vec![
            entry("a_dir", root.join("a_dir"), true),
            entry("ro_dest", ro.clone(), true),
            entry("z_dest", root.join("z_dest"), true),
            entry("b_file", root.join("b_file"), false),
        ]),
    }));
    // sorted dirs-first: [a_dir, ro_dest, z_dest, b_file]

    // b_file dragged over its own parent folder: nothing is shown
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(3)); // b_file
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(DropTarget::Directory(root.clone())));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Hidden));
    // over a genuine folder: the Move caption is back
    ex.update(Message::DragTargetEnter(DropTarget::Directory(root.join("a_dir"))));
    assert!(matches!(
        ex.dnd.caption(),
        Some(DragCaption::Action { action: DropAction::Move, .. })
    ));
    ex.update(Message::DragCancel);

    // a_dir dragged over itself: nothing is shown
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(0)); // a_dir
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(DropTarget::Directory(root.join("a_dir"))));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Hidden));
    ex.update(Message::DragCancel);

    // the read-only destination: banned, and the drop is rejected
    ex.update(cursor(10.0, 10.0));
    ex.update(Message::Select(3)); // b_file
    ex.update(cursor(60.0, 60.0));
    ex.update(Message::DragTargetEnter(DropTarget::Directory(ro.clone())));
    assert_eq!(ex.dnd.caption(), Some(&DragCaption::Banned));
    ex.update(release());
    assert_eq!(ex.status, "the drop was rejected");
    assert!(!root.join("ro_dest/b_file").exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Collapsing a nav node frees its whole subtree's cached listings,
/// but the descendants keep their expansion state so re-expanding
/// restores the tree as it was.
#[test]
fn nav_collapse_frees_subtree() {
    let mut ex = Explorer::new();
    ex.cwd = PathBuf::from("/");
    let toggle = |ex: &mut Explorer, p: &str| {
        let _ = ex.update(Message::NavToggle(PathBuf::from(p)));
    };
    let list = |ex: &mut Explorer, p: &str, kids: Vec<fs::Entry>| {
        let _ = ex.update(Message::FsEvent(FsEvent::Listed {
            path: PathBuf::from(p),
            result: Ok(kids),
        }));
    };

    toggle(&mut ex, "/");
    list(&mut ex, "/", vec![
        entry("a", PathBuf::from("/a"), true),
        entry("b", PathBuf::from("/b"), true),
    ]);
    toggle(&mut ex, "/a");
    list(&mut ex, "/a", vec![entry("x", PathBuf::from("/a/x"), true)]);
    toggle(&mut ex, "/a/x");
    list(&mut ex, "/a/x", vec![]);
    toggle(&mut ex, "/b");
    list(&mut ex, "/b", vec![]);

    assert_eq!(ex.nav.children.len(), 4);

    // collapsing "/": the whole subtree's listings are freed
    toggle(&mut ex, "/");
    assert!(ex.nav.children.is_empty());

    // ... but expansion state survives, for the restore on re-expansion
    assert!(ex.nav.expanded.contains(Path::new("/a")));
    assert!(ex.nav.expanded.contains(Path::new("/a/x")));

    // re-expanding re-lists "/" and the cache fills again
    toggle(&mut ex, "/");
    list(&mut ex, "/", vec![entry("a", PathBuf::from("/a"), true)]);
    assert!(ex.nav.children.contains_key(Path::new("/")));
}

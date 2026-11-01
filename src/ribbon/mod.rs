//! Ribbon subsystem — a faithful Rust/iced re-implementation of the
//! Windows Ribbon Framework API (`UIRibbon.dll`), developed for the
//! explorer, plus the explorer's commanding backend.
//!
//! Framework modules (the public API mirrors the COM interfaces of the
//! original):
//!
//! | `UIRibbon.h`                | this module                        |
//! |-----------------------------|------------------------------------|
//! | `IUIFramework`              | [`RibbonFramework`]                |
//! | `IUIRibbon`                 | [`RibbonView`]                     |
//! | `IUIApplication`            | [`UiApplication`]                  |
//! | `IUICommandHandler`         | [`CommandHandler`]                 |
//! | `IUISimplePropertySet`      | [`PropertySet`] / [`SimplePropertySet`] |
//! | `IUICollection`             | [`UiCollection`]                   |
//! | `IUICollectionChangedEvent` | [`OnCollectionChanged`]            |
//! | `IUIContextualUI`           | [`ContextualUi`]                   |
//! | `IUIImage`                  | [`UiImage`]                        |
//! | `UI_PKEY_*`                 | [`PropertyKey`] + [`PropValue`]    |
//! | `UI_EXECUTIONVERB` etc.     | [`enums`]                          |
//!
//! Instead of compiled ribbon markup (`.ribbon` XML resources), the ribbon
//! is declared in YAML (`assets/ribbon.yaml`) and loaded with
//! [`RibbonFramework::load_ui`]. The [`widget`] module provides the iced
//! widget. The commanding backend (`cmd`, `Snapshot`, `Ribbon`, `view`)
//! maps framework executions onto explorer [`Message`]s.

// Framework modules mirror the COM API surface of UIRibbon.dll even where
// the explorer does not use every entry point yet — allow the unused
// parts without dead-code noise (bin crates lint unused `pub` items).
#[allow(dead_code)]
pub mod collection;
#[allow(dead_code)]
pub mod enums;
#[allow(dead_code)]
pub mod framework;
#[allow(dead_code)]
pub mod icons;
#[allow(dead_code)]
pub mod image;
#[allow(dead_code)]
pub mod prop;
#[allow(dead_code)]
pub mod spec;
#[allow(dead_code)]
pub mod theme;
#[allow(dead_code)]
pub mod widget;

#[cfg(test)]
mod framework_tests;

#[allow(unused_imports)]
pub use collection::{OnCollectionChanged, UiCollection};
#[allow(unused_imports)]
pub use enums::*;
#[allow(unused_imports)]
pub use framework::{
    CommandHandler, ContextualUi, RibbonError, RibbonEvent, RibbonFramework,
    RibbonView, UiApplication,
};
#[allow(unused_imports)]
pub use image::UiImage;
#[allow(unused_imports)]
pub use prop::{FontProps, PropertyKey, PropertySet, PropValue, SimplePropertySet};
#[allow(unused_imports)]
pub use spec::{
    AppMenuSpec, ControlSpec, CtrlSize, GroupSpec, MenuItemSpec, QatSpec,
    RibbonSpec, TabSpec,
};
#[allow(unused_imports)]
pub use theme::{hsb_to_rgb, ContextualColor, Theme};

pub type Result<T> = std::result::Result<T, RibbonError>;

// ---------------------------------------------------------------------------
// explorer commanding backend

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use iced::Widget as _;
use iced::{Color, Element};

use crate::model::{Explorer, Message, SortBy};
use crate::ribbon::widget::Ribbon as RibbonWidget;

// ---------------------------------------------------------------------------
// command ids (mirror assets/ribbon.yaml `commands:`)

// Command id manifest mirroring assets/ribbon.yaml `commands:`. Most
// ids are referenced only from the yaml (resolved by name), so unused
// constants here are expected — they document the numeric ids.
#[allow(dead_code)]
pub mod cmd {
    pub const NEW_WINDOW: u32 = 2;
    pub const OPEN_TERMINAL: u32 = 3;
    pub const EXIT: u32 = 9;

    pub const UNDO: u32 = 18;
    pub const REDO: u32 = 19;
    pub const PASTE: u32 = 20;
    pub const PASTE_SPECIAL: u32 = 21;
    pub const CUT: u32 = 23;
    pub const COPY: u32 = 24;

    pub const MOVE_TO: u32 = 25;
    pub const MOVE_TO_HOME: u32 = 26;
    pub const MOVE_TO_DESKTOP: u32 = 27;
    pub const COPY_TO: u32 = 28;
    pub const COPY_TO_HOME: u32 = 29;
    pub const COPY_TO_DESKTOP: u32 = 30;
    pub const DELETE: u32 = 31;
    pub const DELETE_PERMANENT: u32 = 32;
    pub const RENAME: u32 = 33;
    pub const NEW_FOLDER: u32 = 34;
    pub const NEW_ITEM: u32 = 35;
    pub const PROPERTIES: u32 = 36;
    pub const OPEN: u32 = 37;
    pub const EDIT: u32 = 38;
    pub const HISTORY: u32 = 39;

    pub const SELECT_ALL: u32 = 40;
    pub const SELECT_NONE: u32 = 41;
    pub const INVERT: u32 = 42;

    pub const SHARE_NOBODY: u32 = 50;
    pub const SHARE_EMAIL: u32 = 51;

    pub const NAV_PANE: u32 = 60;
    pub const PREVIEW_PANE: u32 = 61;
    pub const DETAILS_PANE: u32 = 62;
    pub const VIEW_DETAILS: u32 = 63;
    pub const VIEW_LARGE: u32 = 64;
    /// Reserved view option: compact Details rows (24px, the default).
    pub const COMPACT_DETAILS: u32 = 78;
    pub const SORT_BY: u32 = 65;
    pub const SORT_NAME: u32 = 66;
    pub const SORT_SIZE: u32 = 67;
    pub const SORT_MODIFIED: u32 = 68;
    pub const GROUP_BY: u32 = 69;
    pub const SIZE_COLUMNS: u32 = 70;
    pub const TOGGLE_CHECKBOXES: u32 = 71;
    pub const TOGGLE_EXTENSIONS: u32 = 72;
    pub const TOGGLE_HIDDEN: u32 = 73;
    pub const TOGGLE_DARK: u32 = 74;
    pub const TOGGLE_BORDER: u32 = 75;
    pub const TOGGLE_FRAME: u32 = 76;
    pub const TOGGLE_GRAY_DISABLED: u32 = 307;
    /// View → Options: use the system Breeze icon theme instead of the
    /// embedded explorer pack.
    pub const TOGGLE_ICON_PACK: u32 = 308;
    /// TEMP: rainbow gradient on the folder-view backdrop rectangle.
    pub const TOGGLE_RAINBOW: u32 = 79;
    pub const FOLDER_OPTIONS: u32 = 77;

    pub const CTX_MENU: u32 = 140;

    pub const HELP: u32 = 99;

    pub const CTX_DRIVE: u32 = 100;
    pub const DRIVE_PROPERTIES: u32 = 101;
    pub const CTX_PICTURE: u32 = 110;
    pub const OPEN_PICTURE: u32 = 111;
    pub const SET_WALLPAPER: u32 = 112;
    pub const CTX_ARCHIVE: u32 = 120;
    pub const EXTRACT_HERE: u32 = 121;

    // ---- File menu / Home ----
    pub const OPEN_TERMINAL_ADMIN: u32 = 200;
    pub const ABOUT: u32 = 201;
    pub const PIN: u32 = 202;
    pub const COPY_PATH: u32 = 203;
    pub const PASTE_SHORTCUT: u32 = 204;
    pub const PASTE_SYMLINK: u32 = 205;
    pub const DELETE_SUDO: u32 = 206;
    pub const SHOW_RECYCLE_CONFIRM: u32 = 207;
    pub const NEW_SHORTCUT: u32 = 208;
    pub const NEW_SYMLINK: u32 = 209;
    pub const NEW_TEXT_FILE: u32 = 210;
    pub const EASY_ACCESS: u32 = 211;
    pub const MAP_DRIVE: u32 = 212;
    pub const SHARE_ACCESS: u32 = 213;
    pub const REMOVE_PROPERTIES: u32 = 214;
    pub const PERMISSION: u32 = 215;
    pub const MAKE_EXECUTABLE: u32 = 216;
    pub const GRANT_PERMISSION: u32 = 217;
    pub const OPEN_WITH: u32 = 218;
    pub const OPEN_NOTEPAD: u32 = 219;
    pub const OPEN_EXCEL: u32 = 220;
    pub const OPEN_WORD: u32 = 221;
    pub const CHOOSE_APP: u32 = 222;
    pub const FILTERS: u32 = 223;
    pub const FILTER_ALL: u32 = 224;
    pub const FILTER_DOCS: u32 = 225;
    pub const FILTER_SHEETS: u32 = 226;
    pub const FILTER_SLIDES: u32 = 227;
    pub const FILTER_IMAGES: u32 = 228;
    pub const FILTER_VIDEOS: u32 = 229;
    pub const FILTER_PDF: u32 = 230;

    // ---- Share ----
    pub const SHARE: u32 = 231;
    pub const BURN: u32 = 232;
    pub const PRINT: u32 = 233;
    pub const FAX: u32 = 234;
    pub const USER_LIST: u32 = 235;
    pub const REMOVE_ACCESS: u32 = 236;
    pub const ADVANCED_SECURITY: u32 = 237;
    pub const COMPRESS: u32 = 238;
    pub const ZIP: u32 = 239;
    pub const SEVEN_Z: u32 = 240;
    pub const TAR_XZ: u32 = 241;
    pub const READ_ONLY: u32 = 242;

    // ---- View ----
    pub const SORT_DATE: u32 = 245;
    pub const SORT_TYPE: u32 = 246;
    pub const SORT_TAGS: u32 = 247;
    pub const SORT_CREATED: u32 = 248;
    pub const SORT_TAKEN: u32 = 249;
    pub const SORT_DIMENSIONS: u32 = 250;
    pub const SORT_RATING: u32 = 251;
    pub const SORT_ASC: u32 = 252;
    pub const SORT_DESC: u32 = 253;
    pub const CHOOSE_COLUMNS: u32 = 254;
    pub const GROUP_NONE: u32 = 255;
    pub const GROUP_NAME: u32 = 256;
    pub const GROUP_DATE: u32 = 257;
    pub const GROUP_TYPE: u32 = 258;
    pub const GROUP_SIZE: u32 = 259;
    pub const GROUP_TAGS: u32 = 260;
    pub const GROUP_RATING: u32 = 261;
    pub const GROUP_ASC: u32 = 262;
    pub const GROUP_DESC: u32 = 263;
    pub const ADD_COLUMNS: u32 = 264;
    pub const SHOW_TREE_EXPANDER: u32 = 265;
    pub const HIDE_SELECTED: u32 = 266;
    pub const CHANGE_FOLDER_OPTIONS: u32 = 267;
    pub const EXPAND_TO_FOLDER: u32 = 268;
    pub const SHOW_ALL_FOLDERS: u32 = 269;

    // ---- Drive Tools ----
    pub const BITLOCKER: u32 = 270;
    pub const BITLOCKER_ON: u32 = 271;
    pub const BITLOCKER_MANAGE: u32 = 272;
    pub const BITLOCKER_PIN: u32 = 273;
    pub const OPTIMIZE: u32 = 274;
    pub const CLEANUP: u32 = 275;
    pub const FORMAT: u32 = 276;
    pub const AUTOPLAY: u32 = 277;
    pub const EJECT: u32 = 278;
    pub const FINISH_BURNING: u32 = 279;
    pub const ERASE_DISC: u32 = 280;

    // ---- Picture Tools ----
    pub const ROTATE_LEFT: u32 = 281;
    pub const ROTATE_RIGHT: u32 = 282;
    pub const SLIDE_SHOW: u32 = 283;

    // ---- Compressed Folder Tools / Archive ----
    pub const EXTRACT_ALL: u32 = 284;
    pub const CTX_COMPRESSED: u32 = 285;

    // ---- Search Tools ----
    pub const CTX_SEARCH: u32 = 286;
    pub const THIS_PC: u32 = 287;
    pub const CURRENT_FOLDER: u32 = 288;
    pub const ALL_SUBFOLDERS: u32 = 289;
    pub const SEARCH_AGAIN_IN: u32 = 290;
    pub const SEARCH_PLACES: u32 = 291;
    pub const SEARCH_INTERNET: u32 = 292;
    pub const DATE_MODIFIED: u32 = 293;
    pub const KIND: u32 = 294;
    pub const SIZE_FILTER: u32 = 295;
    pub const OTHER_PROPERTIES: u32 = 296;
    pub const RECENT_SEARCHES: u32 = 297;
    pub const CLEAR_SEARCH_HISTORY: u32 = 298;
    pub const ADVANCED_OPTIONS: u32 = 299;
    pub const CHANGE_INDEXED: u32 = 300;
    pub const INDEX_FILE_CONTENTS: u32 = 301;
    pub const INDEX_SYSTEM_FILES: u32 = 302;
    pub const INDEX_COMPRESSED: u32 = 303;
    pub const SAVE_SEARCH: u32 = 304;
    pub const OPEN_FILE_LOCATION: u32 = 305;
    pub const CLOSE_SEARCH: u32 = 306;

    // ---- Video / Music Tools ----
    pub const CTX_VIDEO: u32 = 310;
    pub const CTX_MUSIC: u32 = 311;
    pub const PLAY: u32 = 312;
    pub const PLAY_ALL: u32 = 313;
    pub const ADD_TO_PLAYLIST: u32 = 314;

    // ---- Application Tools ----
    pub const CTX_APPLICATION: u32 = 315;
    pub const PIN_TASKBAR: u32 = 316;
    pub const RUN_AS_ADMIN: u32 = 317;
    pub const RUN_AS_USER: u32 = 318;
    pub const TROUBLESHOOT: u32 = 319;

    // ---- Disc Image Tools ----
    pub const CTX_DISC_IMAGE: u32 = 320;
    pub const MOUNT: u32 = 321;

    // ---- Network ----
    pub const CTX_NETWORK: u32 = 322;
    pub const NET_PROPERTIES: u32 = 323;
    pub const NET_OPEN: u32 = 324;
    pub const REMOTE_DESKTOP: u32 = 325;
    pub const ADD_DEVICES: u32 = 326;
    pub const VIEW_PRINTERS: u32 = 327;
    pub const DEVICE_WEBPAGE: u32 = 328;
    pub const SEARCH_AD: u32 = 329;
    pub const NET_SHARING_CENTER: u32 = 330;

    // ---- Computer ----
    pub const CTX_COMPUTER: u32 = 331;
    pub const COMP_PROPERTIES: u32 = 332;
    pub const COMP_OPEN: u32 = 333;
    pub const COMP_RENAME: u32 = 334;
    pub const ACCESS_MEDIA: u32 = 335;
    pub const CONNECT_MEDIA: u32 = 336;
    pub const DISCONNECT_MEDIA: u32 = 337;
    pub const MAP_NETWORK_DRIVE: u32 = 338;
    pub const DISCONNECT_DRIVE: u32 = 339;
    pub const ADD_NETWORK_LOCATION: u32 = 340;
    pub const OPEN_SETTINGS: u32 = 341;
    pub const UNINSTALL_PROGRAM: u32 = 342;
    pub const SYSTEM_PROPERTIES: u32 = 343;
    pub const MANAGE: u32 = 344;
}

/// Commands that exist in the markup but have no implementation yet.
/// Kept enabled=false via [`ExplorerHandler::update_property`].
const ALWAYS_DISABLED: &[u32] = &[
    cmd::EDIT,
    cmd::HISTORY,
    cmd::SHARE_NOBODY,
    cmd::SHARE_EMAIL,
    cmd::SIZE_COLUMNS,
    cmd::TOGGLE_CHECKBOXES,
    cmd::TOGGLE_EXTENSIONS,
    cmd::FOLDER_OPTIONS,
    cmd::DRIVE_PROPERTIES,
    cmd::SET_WALLPAPER,
    cmd::EXTRACT_HERE,
];

// ---------------------------------------------------------------------------
// context + `when` expressions (contextual tab visibility rules)

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    pub is_drive: bool,
    pub inside_archive: bool,
    pub has_archive: bool,
    pub has_image: bool,
    pub has_selection: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_yaml_loads() {
        let spec = crate::ribbon::spec::parse(include_str!("../../assets/ribbon.yaml"))
            .expect("embedded ribbon.yaml is valid");
        assert!(!spec.tabs.is_empty());
    }

    #[test]
    fn embedded_yaml_context_commands_are_declared() {
        // Every tab `context_command` must name a DECLARED command — an
        // undeclared name silently resolves to a hashed stable id whose
        // ContextAvailable falls into the handler's `_ => true` arm,
        // making the contextual tab always visible (regression: the
        // Archive Tools tab showed in every folder because the yaml
        // lacked `cmdCtxArchive`).
        use crate::ribbon::spec::{self, CommandRef};
        let text = include_str!("../../assets/ribbon.yaml");
        let mut spec = spec::parse(text).expect("embedded ribbon.yaml is valid");
        let declared: std::collections::BTreeSet<u32> = spec::collect_commands(&mut spec)
            .unwrap()
            .iter()
            .map(|c| c.0)
            .collect();
        for tab in &spec.tabs {
            if let Some(CommandRef::Id(id)) = tab.context_command {
                assert!(
                    declared.contains(&id) && id < 1000,
                    "tab {:?} context_command resolved to undeclared id {id}",
                    tab.id
                );
            }
        }
        // the archive tab must resolve to cmd::CTX_ARCHIVE specifically,
        // whose handler arm drives it (inside archive / archive selected)
        let archive = spec.tabs.iter().find(|t| t.id == "archive_tools").unwrap();
        assert!(matches!(
            archive.context_command,
            Some(CommandRef::Id(cmd::CTX_ARCHIVE))
        ));
    }

    #[test]
    fn embedded_yaml_commands_resolve_in_place() {
        // collect_commands must write resolved numeric ids back into EVERY
        // CommandRef slot — menus/popups render via `CommandRef::Id` only
        // (regression: popups did not show when menu refs stayed Names).
        use crate::ribbon::spec::{self, CommandRef, ControlSpec};
        let mut spec =
            spec::parse(include_str!("../../assets/ribbon.yaml")).unwrap();
        spec::collect_commands(&mut spec).unwrap();

        let popup = spec.context_popups.first().expect("context popup");
        assert!(matches!(popup.command, Some(CommandRef::Id(_))));
        for item in &popup.items {
            assert!(matches!(item.command, Some(CommandRef::Id(_))));
        }
        assert!(matches!(spec.help_button, Some(CommandRef::Id(_))));
        let menu = spec.application_menu.as_ref().unwrap();
        assert!(matches!(menu.command, Some(CommandRef::Id(_))));
        for item in menu
            .items
            .iter()
            .filter(|i| i.command.is_some())
        {
            assert!(matches!(item.command, Some(CommandRef::Id(_))));
        }
        let home = spec.tabs.iter().find(|t| t.id == "home").unwrap();
        // find the Paste split button wherever it sits in the Clipboard group
        let paste = home.groups[0]
            .controls
            .iter()
            .find_map(|c| match c {
                ControlSpec::SplitButton(s)
                    if matches!(s.command, Some(CommandRef::Id(id)) if id == cmd::PASTE) =>
                {
                    Some(s)
                }
                _ => None,
            })
            .expect("home clipboard group has a Paste split button");
        for item in &paste.items {
            assert!(matches!(item.command, Some(CommandRef::Id(_))));
        }
    }
}

// ---------------------------------------------------------------------------
// snapshot: everything the commanding layer needs from the Explorer model

#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    pub ctx: Context,
    pub has_clipboard: bool,
    pub has_items: bool,
    /// Current view step index (SPEC/FOLDER-SPEC.MD); the view commands
    /// light up only when it equals their preset (§5 bullet rule).
    pub view_index: u8,
    /// The side pane shown right of the folder view (Preview XOR Details).
    pub side_pane: crate::pane::Mode,
    /// The Details / Large-icons preset indices (folder-view.yaml §5).
    pub preset_details: u8,
    pub preset_large: u8,
    pub show_hidden: bool,
    pub details_compact: bool,
    pub dark: bool,
    pub self_border: bool,
    pub win8_frame: bool,
    /// TEMP: rainbow backdrop toggle state.
    pub rainbow: bool,
    /// Disabled buttons/icons get the gray-scaled-faded style.
    pub disabled_gray: bool,
    /// System icon theme (Breeze) instead of the embedded explorer pack.
    pub breeze_icons: bool,
    pub cwd: PathBuf,
    pub selected_paths: Vec<PathBuf>,
    pub first_selected: Option<usize>,
}

impl Explorer {
    /// Capture the model state the ribbon handler queries (enablement,
    /// toggle values, contextual availability, command payloads).
    pub fn ribbon_snapshot(&self) -> Snapshot {
        Snapshot {
            ctx: self.context(),
            has_clipboard: self.clipboard.is_some(),
            has_items: !self.entries.is_empty(),
            view_index: self.view_index,
            side_pane: self.pane,
            preset_details: self.view_cfg.presets.details,
            preset_large: self.view_cfg.presets.large,
            show_hidden: self.show_hidden,
            details_compact: self.details_compact,
            dark: self.dark,
            self_border: self.self_border,
            win8_frame: self.frame_style == crate::model::FrameStyle::Win8,
            rainbow: self.rainbow,
            disabled_gray: self.disabled_gray,
            breeze_icons: self.breeze_icons,
            cwd: self.cwd.clone(),
            selected_paths: self.selected_paths(),
            first_selected: self.selected.iter().next().copied(),
        }
    }
}

// ---------------------------------------------------------------------------
// shared state between the framework handlers and the backend

struct Shared {
    snap: Snapshot,
    /// Messages produced by `Execute` — drained by the model after dispatch.
    out: Vec<Message>,
}

// ---------------------------------------------------------------------------
// IUICommandHandler

struct ExplorerHandler {
    shared: Rc<RefCell<Shared>>,
}

impl CommandHandler for ExplorerHandler {
    fn execute(
        &mut self,
        command: u32,
        verb: ExecutionVerb,
        _key: Option<PropertyKey>,
        _value: Option<PropValue>,
        _props: Option<std::sync::Arc<dyn crate::ribbon::PropertySet + Send>>,
    ) -> Result<()> {
        // gallery/combo previews are not used by the explorer ribbon
        if !matches!(verb, ExecutionVerb::Execute) {
            return Ok(());
        }
        let snap = self.shared.borrow().snap.clone();
        let mut out = Vec::new();
        match command {
            cmd::EXIT => out.push(Message::Close),
            cmd::NEW_WINDOW => match std::env::current_exe() {
                Ok(exe) => {
                    let cwd = snap.cwd.clone();
                    match std::process::Command::new(exe).arg(&cwd).spawn() {
                        Ok(_) => out.push(Message::Status(format!(
                            "Opened new window at {}",
                            cwd.display()
                        ))),
                        Err(e) => out.push(Message::Status(format!("New window: {e}"))),
                    }
                }
                Err(e) => out.push(Message::Status(format!("New window: {e}"))),
            },
            cmd::OPEN_TERMINAL => {
                out.push(Message::RunShell("konsole --workdir {cwd}".into()))
            }
            cmd::UNDO | cmd::REDO => {
                out.push(Message::Status("Undo/redo history is not implemented yet".into()))
            }
            cmd::PASTE | cmd::PASTE_SPECIAL => out.push(Message::Paste),
            cmd::CUT => out.push(Message::CutSelection),
            cmd::COPY => out.push(Message::CopySelection),
            cmd::MOVE_TO | cmd::COPY_TO => {
                out.push(Message::Status("Pick a destination from the menu".into()))
            }
            cmd::MOVE_TO_HOME | cmd::MOVE_TO_DESKTOP | cmd::COPY_TO_HOME
            | cmd::COPY_TO_DESKTOP => {
                let cut = matches!(command, cmd::MOVE_TO_HOME | cmd::MOVE_TO_DESKTOP);
                let leaf = if matches!(command, cmd::MOVE_TO_HOME | cmd::COPY_TO_HOME)
                {
                    ""
                } else {
                    "Desktop"
                };
                let dest = home_path(leaf);
                out.push(Message::TransferTo {
                    paths: snap.selected_paths.clone(),
                    dest,
                    cut,
                });
            }
            cmd::DELETE => out.push(Message::DeleteSelection),
            cmd::DELETE_PERMANENT => out.push(Message::DeletePermanent),
            cmd::RENAME => out.push(Message::RenameSelected),
            cmd::NEW_FOLDER => out.push(Message::NewFolder),
            cmd::OPEN | cmd::OPEN_PICTURE => {
                if let Some(i) = snap.first_selected {
                    out.push(Message::Open(i));
                }
            }
            cmd::SELECT_ALL => out.push(Message::SelectAll),
            cmd::SELECT_NONE => out.push(Message::SelectNone),
            cmd::INVERT => out.push(Message::InvertSelection),
            cmd::VIEW_DETAILS => out.push(Message::SetView(snap.preset_details)),
            cmd::VIEW_LARGE => out.push(Message::SetView(snap.preset_large)),
            cmd::COMPACT_DETAILS => out.push(Message::ToggleDetailsCompact),
            cmd::PREVIEW_PANE => out.push(Message::ToggleSidePane(crate::pane::Mode::Preview)),
            cmd::DETAILS_PANE => out.push(Message::ToggleSidePane(crate::pane::Mode::Details)),
            cmd::SORT_BY => {
                out.push(Message::Status("Pick a sort key from the menu".into()))
            }
            cmd::SORT_NAME => out.push(Message::SetSort(SortBy::Name)),
            cmd::SORT_SIZE => out.push(Message::SetSort(SortBy::Size)),
            cmd::SORT_MODIFIED => out.push(Message::SetSort(SortBy::Modified)),
            cmd::TOGGLE_HIDDEN => out.push(Message::ToggleHidden),
            cmd::TOGGLE_DARK => out.push(Message::ToggleDark),
            cmd::TOGGLE_BORDER => out.push(Message::ToggleSelfBorder),
            cmd::TOGGLE_FRAME => out.push(Message::ToggleFrameStyle),
            cmd::TOGGLE_GRAY_DISABLED => out.push(Message::ToggleGrayDisabled),
            cmd::TOGGLE_ICON_PACK => out.push(Message::ToggleIconPack),
            cmd::TOGGLE_RAINBOW => out.push(Message::ToggleRainbow),
            cmd::CTX_MENU => out.push(Message::Status("Context menu".into())),
            cmd::HELP => out.push(Message::Status("Help is not implemented yet".into())),
            other => out.push(Message::Status(format!(
                "Command #{other} is not implemented yet"
            ))),
        }
        self.shared.borrow_mut().out.extend(out);
        Ok(())
    }

    fn update_property(
        &mut self,
        command: u32,
        key: PropertyKey,
        _current: Option<PropValue>,
    ) -> Result<Option<PropValue>> {
        let shared = self.shared.borrow();
        let snap = &shared.snap;
        Ok(match key {
            PropertyKey::Enabled => {
                let enabled = if ALWAYS_DISABLED.contains(&command) {
                    false
                } else {
                    match command {
                        cmd::CUT
                        | cmd::COPY
                        | cmd::DELETE
                        | cmd::DELETE_PERMANENT
                        | cmd::RENAME
                        | cmd::OPEN
                        | cmd::OPEN_PICTURE
                        | cmd::MOVE_TO
                        | cmd::COPY_TO
                        | cmd::MOVE_TO_HOME
                        | cmd::MOVE_TO_DESKTOP
                        | cmd::COPY_TO_HOME
                        | cmd::COPY_TO_DESKTOP => snap.ctx.has_selection,
                        cmd::PASTE | cmd::PASTE_SPECIAL => snap.has_clipboard,
                        cmd::SELECT_ALL | cmd::INVERT => snap.has_items,
                        cmd::SELECT_NONE => snap.ctx.has_selection,
                        _ => true,
                    }
                };
                Some(PropValue::Bool(enabled))
            }
            PropertyKey::BooleanValue => {
                let on = match command {
                    cmd::VIEW_DETAILS => snap.view_index == snap.preset_details,
                    cmd::VIEW_LARGE => snap.view_index == snap.preset_large,
                    cmd::COMPACT_DETAILS => snap.details_compact,
                    cmd::PREVIEW_PANE => snap.side_pane == crate::pane::Mode::Preview,
                    cmd::DETAILS_PANE => snap.side_pane == crate::pane::Mode::Details,
                    cmd::TOGGLE_HIDDEN => snap.show_hidden,
                    cmd::TOGGLE_DARK => snap.dark,
                    cmd::TOGGLE_BORDER => snap.self_border,
                    cmd::TOGGLE_FRAME => snap.win8_frame,
                    cmd::TOGGLE_GRAY_DISABLED => snap.disabled_gray,
                    cmd::TOGGLE_ICON_PACK => snap.breeze_icons,
                    cmd::TOGGLE_RAINBOW => snap.rainbow,
                    _ => false,
                };
                Some(PropValue::Bool(on))
            }
            PropertyKey::ContextAvailable => {
                let available = match command {
                    cmd::CTX_DRIVE => snap.ctx.is_drive,
                    cmd::CTX_PICTURE => snap.ctx.has_image,
                    cmd::CTX_ARCHIVE => snap.ctx.inside_archive || snap.ctx.has_archive,
                    // contextual tabs not yet driven by the explorer
                    // context stay hidden until their detector is wired
                    cmd::CTX_SEARCH | cmd::CTX_VIDEO | cmd::CTX_MUSIC
                    | cmd::CTX_COMPRESSED | cmd::CTX_APPLICATION
                    | cmd::CTX_DISC_IMAGE | cmd::CTX_NETWORK
                    | cmd::CTX_COMPUTER => false,
                    _ => true,
                };
                Some(PropValue::UInt(
                    if available {
                        ContextAvailability::Available
                    } else {
                        ContextAvailability::NotAvailable
                    } as u32,
                ))
            }
            PropertyKey::TooltipTitle => Some(PropValue::Str(command_name(command))),
            PropertyKey::TooltipDescription if command == cmd::HELP => {
                Some(PropValue::Str("Show help (F1)".into()))
            }
            _ => None,
        })
    }
}

/// `~/` expansion for command targets.
fn home_path(leaf: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match (home, leaf.is_empty()) {
        (Some(h), true) => h,
        (Some(h), false) => h.join(leaf),
        (None, _) => PathBuf::from(leaf),
    }
}

/// Display name for tooltips/statuses.
fn command_name(command: u32) -> String {
    let name = match command {
        cmd::NEW_WINDOW => "New window",
        cmd::OPEN_TERMINAL => "Open terminal here",
        cmd::EXIT => "Exit",
        cmd::UNDO => "Undo",
        cmd::REDO => "Redo",
        cmd::PASTE => "Paste",
        cmd::PASTE_SPECIAL => "Paste special",
        cmd::CUT => "Cut",
        cmd::COPY => "Copy",
        cmd::MOVE_TO => "Move to",
        cmd::COPY_TO => "Copy to",
        cmd::DELETE => "Delete",
        cmd::DELETE_PERMANENT => "Delete permanently",
        cmd::RENAME => "Rename",
        cmd::NEW_FOLDER => "New folder",
        cmd::PROPERTIES => "Properties",
        cmd::OPEN => "Open",
        cmd::SELECT_ALL => "Select all",
        cmd::SELECT_NONE => "Select none",
        cmd::INVERT => "Invert selection",
        cmd::VIEW_DETAILS => "Details view",
        cmd::VIEW_LARGE => "Large icons view",
        cmd::COMPACT_DETAILS => "Compact details view",
        cmd::PREVIEW_PANE => "Preview pane",
        cmd::DETAILS_PANE => "Details pane",
        cmd::SORT_BY => "Sort by",
        cmd::TOGGLE_HIDDEN => "Hidden items",
        cmd::TOGGLE_DARK => "Dark mode",
        cmd::TOGGLE_BORDER => "Custom window border",
        cmd::TOGGLE_FRAME => "Windows 8 frame style",
        cmd::TOGGLE_GRAY_DISABLED => "Gray out unavailable buttons",
        cmd::TOGGLE_ICON_PACK => "Breeze icon theme",
        cmd::TOGGLE_RAINBOW => "Rainbow pane backdrop",
        cmd::HELP => "Help",
        _ => return format!("Command #{command}"),
    };
    name.into()
}

// ---------------------------------------------------------------------------
// IUIApplication

struct ExplorerApp {
    shared: Rc<RefCell<Shared>>,
}

impl crate::ribbon::UiApplication for ExplorerApp {
    fn on_create_ui_command(
        &mut self,
        _command: u32,
        _type_id: CommandType,
    ) -> Rc<RefCell<dyn CommandHandler>> {
        Rc::new(RefCell::new(ExplorerHandler {
            shared: self.shared.clone(),
        }))
    }
}

// ---------------------------------------------------------------------------
// backend

/// Which contextual set a tab belongs to (for visibility/auto-activation).
#[derive(Clone, Copy, PartialEq, Eq)]
enum CtxTab {
    Main,
    Drive,
    Picture,
    Archive,
}

struct TabInfo {
    id: String,
    ctx: CtxTab,
}

/// The ribbon backend: owns the framework and the commanding state.
pub struct Ribbon {
    framework: RibbonFramework,
    shared: Rc<RefCell<Shared>>,
    tabs: Vec<TabInfo>,
}

impl Ribbon {
    pub fn new() -> Self {
        let shared = Rc::new(RefCell::new(Shared {
            snap: Snapshot::default(),
            out: Vec::new(),
        }));
        let mut framework = RibbonFramework::new();
        framework.initialize(ExplorerApp {
            shared: shared.clone(),
        });
        framework
            .load_ui(&load_yaml())
            .expect("assets/ribbon.yaml must be valid rs-ribbon markup");
        framework.set_modes(1).expect("modes");
        // debug hook: EXPLORER_DEBUG_POPUP=x,y opens the context popup at
        // startup (same overlay pipeline as the ribbon dropdowns)
        if let Ok(pos) = std::env::var("EXPLORER_DEBUG_POPUP") {
            let (x, y) = pos
                .split_once(',')
                .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
                .unwrap_or((400, 300));
            framework
                .contextual_ui(cmd::CTX_MENU)
                .show_at_location(x, y);
        }
        let tabs = framework
            .spec()
            .expect("loaded")
            .tabs
            .iter()
            .map(|t| {
                let ctx = match t.context_command {
                    Some(crate::ribbon::spec::CommandRef::Id(cmd::CTX_DRIVE)) => CtxTab::Drive,
                    Some(crate::ribbon::spec::CommandRef::Id(cmd::CTX_PICTURE)) => CtxTab::Picture,
                    Some(crate::ribbon::spec::CommandRef::Id(cmd::CTX_ARCHIVE)) => CtxTab::Archive,
                    _ => CtxTab::Main,
                };
                TabInfo {
                    id: t.id.clone(),
                    ctx,
                }
            })
            .collect();
        Self {
            framework,
            shared,
            tabs,
        }
    }

    pub fn framework(&self) -> &RibbonFramework {
        &self.framework
    }

    /// Route a ribbon event through the framework
    /// (`IUICommandHandler::Execute` + invalidation flush).
    pub fn dispatch(&mut self, event: &RibbonEvent) {
        let _ = self.framework.dispatch(event);
    }

    /// Messages produced by the most recent `dispatch` (Execute calls).
    pub fn take_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.shared.borrow_mut().out)
    }

    /// Publish the model snapshot to the framework and refresh every
    /// command (enablement / toggle values / contextual availability).
    pub fn sync(&mut self, snap: Snapshot) {
        self.shared.borrow_mut().snap = snap;
        let _ = self.framework.invalidate_ui_command(
            UI_ALL_COMMANDS,
            crate::ribbon::Invalidations::STATE | crate::ribbon::Invalidations::VALUE,
            None,
        );
        self.framework.flush_pending_invalidations();
    }

    /// Sync only when the snapshot actually changed since the last one
    /// (update() runs per input event — mouse moves must not re-query
    /// every command). Returns whether it changed.
    pub fn sync_changed(&mut self, snap: &Snapshot) -> bool {
        if self.shared.borrow().snap == *snap {
            return false;
        }
        self.sync(snap.clone());
        true
    }

    /// Win8-style contextual-tab auto-activation: when a context appears
    /// (drive root, image selected, archive entered), jump to its tab;
    /// when the active contextual tab's context disappears, fall back to
    /// Home. NOT applied for ribbon-originated events (the user just
    /// clicked a tab — don't yank the selection away).
    pub fn auto_activate(&mut self) {
        let ctx = self.shared.borrow().snap.ctx;
        let visible = |k: CtxTab| match k {
            CtxTab::Main => true,
            CtxTab::Drive => ctx.is_drive,
            CtxTab::Picture => ctx.has_image,
            CtxTab::Archive => ctx.inside_archive || ctx.has_archive,
        };
        let activates = |k: CtxTab| match k {
            CtxTab::Drive => ctx.is_drive,
            CtxTab::Picture => ctx.has_image,
            CtxTab::Archive => ctx.inside_archive,
            CtxTab::Main => false,
        };
        let _ = activates;
        let view = self.framework.view();
        let selected = view.selected_tab();
        // never jump to a contextual tab on selection changes; only fall
        // back to Home when the active tab's context disappears
        if let Some(kind) = self.tabs.get(selected).map(|t| t.ctx)
            && !visible(kind)
            && let Some(home) = self.tabs.iter().position(|t| t.id == "home")
        {
            view.select_tab(home);
        }
    }
}

/// A contextual tab-set block for the window title bar (Win8 chrome):
/// the colored block sits exactly above its tabs on the ribbon strip.
pub struct ContextualBlock {
    pub label: String,
    pub color: iced::Color,
    /// x/width in ribbon-widget coordinates (= title-bar content coords).
    pub x: f32,
    pub w: f32,
    pub first_tab: usize,
    pub last_tab: usize,
}

impl Ribbon {
    /// Visible contextual sets with their on-strip geometry (empty when
    /// no contextual tab is available).
    pub fn contextual_blocks(&self) -> Vec<ContextualBlock> {
        let Ok(spec) = self.framework.spec() else {
            return Vec::new();
        };
        let rects = self.framework.view().tab_rects();
        let mut out: Vec<ContextualBlock> = Vec::new();
        for (i, tab) in spec.tabs.iter().enumerate() {
            let Some(name) = &tab.contextual else { continue };
            // hidden tabs have no rect in the last layout
            let Some((_, x, w)) = rects.iter().find(|(idx, _, _)| idx == &i)
            else {
                continue;
            };
            let color = tab
                .contextual
                .as_deref()
                .and_then(ContextualColor::from_name)
                .or_else(|| {
                    tab.context_color
                        .as_deref()
                        .and_then(ContextualColor::from_name)
                })
                .unwrap_or(ContextualColor::Blue)
                .accent();
            if let Some(last) = out.last_mut()
                && last.label == *name
                && last.last_tab + 1 == i
            {
                last.w = x + w - last.x;
                last.last_tab = i;
                continue;
            }
            out.push(ContextualBlock {
                label: name.clone(),
                color,
                x: *x,
                w: *w,
                first_tab: i,
                last_tab: i,
            });
        }
        out
    }
}

impl Ribbon {
    /// QAT docked into the window title bar ("show above the ribbon")?
    pub fn qat_docked_top(&self) -> bool {
        self.framework.view().qat_dock() == crate::ribbon::enums::ControlDock::Top
    }

    /// QAT buttons for the title bar: (command, icon name, enabled).
    pub fn qat_buttons(&self) -> Vec<(u32, String, bool)> {
        self.framework
            .view()
            .qat_commands()
            .iter()
            .map(|&cmd| {
                let icon = match self.framework.property_or(
                    cmd,
                    PropertyKey::SmallImage,
                    PropValue::Image(UiImage::Icon("blank")),
                ) {
                    PropValue::Image(UiImage::Named(n)) => n,
                    PropValue::Image(UiImage::Icon(n)) => n.to_string(),
                    _ => "blank".into(),
                };
                (cmd, icon, self.framework.command_enabled(cmd))
            })
            .collect()
    }
}

impl Ribbon {
    /// Window-size sacrifice thresholds (ribbon_below, nav_below,
    /// nav_resize_below), from the ribbon markup.
    pub fn sacrifice(&self) -> (f32, f32, f32, f32) {
        match self.framework.spec() {
            Ok(spec) => (
                spec.sacrifice.ribbon_below,
                spec.sacrifice.nav_below,
                spec.sacrifice.nav_resize_below,
                spec.sacrifice.search_below,
            ),
            Err(_) => (120.0, 600.0, 900.0, 400.0),
        }
    }
}

impl Ribbon {
    /// Window-size sacrifice threshold for the side (preview/details)
    /// pane: below this content width it is hidden.
    pub fn pane_below(&self) -> f32 {
        match self.framework.spec() {
            Ok(spec) => spec.sacrifice.pane_below,
            Err(_) => 800.0,
        }
    }

    /// Functionless spacer width between the nav pane (splitter) and
    /// the folder view, from the ribbon markup.
    pub fn nav_gap(&self) -> f32 {
        match self.framework.spec() {
            Ok(spec) => spec.metrics.nav_gap,
            Err(_) => 4.0,
        }
    }

    /// Height of the navigation band (nav buttons / address / search),
    /// from the ribbon markup.
    pub fn band_height(&self) -> f32 {
        match self.framework.spec() {
            Ok(spec) => spec.metrics.band_height,
            Err(_) => 20.0,
        }
    }

    /// Bar-button (nav circles / refresh / status view buttons) size as
    /// a multiple of the icon size (ribbon markup `metrics:
    /// bar_button_multiplier`).
    pub fn bar_button_multiplier(&self) -> f32 {
        match self.framework.spec() {
            Ok(spec) => spec.metrics.bar_button_multiplier,
            Err(_) => 2.2,
        }
    }

    /// UI metrics (band height, tree row height/indention/font scale),
    /// from the ribbon markup.
    pub fn metrics(&self) -> (f32, f32, f32, f32) {
        match self.framework.spec() {
            Ok(spec) => (
                spec.metrics.band_height,
                spec.metrics.tree_item_height,
                spec.metrics.tree_indention,
                spec.metrics.tree_font_scale,
            ),
            Err(_) => (36.0, 22.0, 14.0, 0.7),
        }
    }
}

/// Load the ribbon markup: a user file at `~/.config/explorer/ribbon.yaml`
/// overrides the embedded copy (same override path as before).
fn load_yaml() -> String {
    let user_path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/ribbon.yaml"));
    if let Some(p) = user_path
        && let Ok(text) = std::fs::read_to_string(&p)
    {
        return text;
    }
    include_str!("../../assets/ribbon.yaml").to_string()
}

// ---------------------------------------------------------------------------
// theme + view

/// rs-ribbon theme matching the app skin (light: Windows 8 ribbon look;
/// dark: recolored chrome). Every color comes from the theme file
/// (`crate::theme`): ribbon-specific tokens from the `ribbon` group,
/// everything else derived from the shared palette tokens.
/// `focused = false` (window in background, SPEC/Unfocused_Window.md):
/// no hover feedback at all and checked/pressed marks fade to the
/// inactive gray.
pub fn theme(dark: bool, gray_disabled: bool, scale_dpi: f32, focused: bool) -> Theme {
    use crate::ribbon::theme::tint;
    let p = if dark { crate::theme::dark() } else { crate::theme::light() };
    let r = &p.ribbon;
    // geometry + tab style come from the Windows 8 base; colors below
    let mut t = Theme::windows8();
    t.grayscale_disabled = gray_disabled;
    // borders keep a constant physical width unless the theme says they
    // follow the DPI (not_follow_dpi = 1 / scale)
    t.border_scale = if crate::theme::get().border_follow_dpi || scale_dpi <= 0.0 {
        1.0
    } else {
        1.0 / scale_dpi
    };
    t.accent = r.accent;
    t.orb = (r.accent, r.accent_dark);
    t.text = r.text;
    t.tab_text = r.tab_text;
    // hover is the folder-view hover color everywhere in the ribbon
    // (buttons, popup menu rows); buttons additionally get the
    // bluer hover border, in both themes. The *tabs* hover nearly
    // white (#FDFDFF), with a lighter connected line around the
    // hovered tab (theme `tab_hover_outline`).
    t.tab_hover = if dark { p.hover } else { Color::from_rgb8(0xFD, 0xFD, 0xFF) };
    t.tab_hover_line = Some(p.tab_hover_outline);
    t.hover_bg = p.hover;
    t.hover_border = Some(p.hover_btn_border);
    // pressed = the folder-view pressed blue (light theme included)
    t.pressed_bg = p.pressed;
    t.border = r.border;
    t.tab_line = r.tab_line;
    t.group_label = r.group_label;
    t.text_disabled = r.text_disabled;
    t.keytip_bg = r.keytip_bg;
    t.keytip_text = r.keytip_text;
    t.selection = r.accent;
    // structural pairs + dark chrome, derived from shared tokens
    let strip = if dark { p.ribbon_bg } else { p.window_bg };
    t.tab_strip = (strip, strip);
    t.qat_bg = (strip, strip);
    t.ribbon = (p.ribbon_bg, p.ribbon_bg);
    t.tab_active = p.ribbon_bg;
    t.group_separator = p.group_sep;
    let panel = if dark { p.menu_bg } else { p.window_bg };
    t.popup_bg = panel;
    t.tooltip_bg = panel;
    t.popup_border = if dark { p.menu_border } else { r.border };
    t.tooltip_border = if dark { p.menu_border } else { r.tooltip_border };
    // pressed reuses the hover border (light theme included); dark
    // chrome reuses the shared selection tokens for checked.
    t.pressed_border = dark.then_some(p.sel_btn_border);
    t.checked_border = dark.then_some(p.sel_btn_border);
    t.checked_bg = if dark { p.sel_btn } else { tint(r.accent, 0.80) };
    if !focused {
        // unfocused window: hover fills are baked into the scene at
        // layout time, so transparent hover colors guarantee no hover
        // paint even if stale hover state survives
        t.tab_hover = Color::TRANSPARENT;
        t.tab_hover_line = None;
        t.hover_bg = Color::TRANSPARENT;
        t.hover_border = None;
        t.pressed_bg = p.sel_inactive;
        t.pressed_border = Some(p.sel_inactive_border);
        t.checked_bg = p.sel_inactive;
        t.checked_border = Some(p.sel_inactive_border);
        t.selection = p.sel_inactive;
    }
    t
}

/// The ribbon element (iced widget bound to our framework).
pub fn view(state: &Explorer) -> Element<'_, Message> {
    crate::ribbon::image::set_dark(state.dark);
    let ignore_when_unfocused = state
        .ribbon
        .framework()
        .spec()
        .map(|s| s.ignore_input_when_unfocused)
        .unwrap_or(true);
    RibbonWidget::new(state.ribbon.framework())
        .theme(theme(
            state.dark,
            state.disabled_gray,
            state.scale_dpi(),
            state.focused,
        ))
        .fonts(state.fonts.clone())
        .responsive(state.focused || !ignore_when_unfocused)
        .on_event(Message::Ribbon)
        .boxed()
}

#[cfg(test)]
mod debug_ctx_tests {
    use super::*;

    #[test]
    fn contextual_tabs_hidden_by_default() {
        let mut r = Ribbon::new();
        r.sync(Snapshot::default());
        for id in [
            cmd::CTX_ARCHIVE,
            cmd::CTX_COMPRESSED,
            cmd::CTX_SEARCH,
            cmd::CTX_VIDEO,
            cmd::CTX_MUSIC,
            cmd::CTX_APPLICATION,
            cmd::CTX_DISC_IMAGE,
            cmd::CTX_NETWORK,
            cmd::CTX_COMPUTER,
        ] {
            assert_eq!(
                r.framework.context_availability(id),
                crate::ribbon::enums::ContextAvailability::NotAvailable,
                "ctx command {id} must be hidden with empty snapshot"
            );
        }
    }
}

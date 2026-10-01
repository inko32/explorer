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

use iced::Element;

use crate::model::{Explorer, Message, SortBy, ViewMode};
use crate::ribbon::widget::Ribbon as RibbonWidget;

// ---------------------------------------------------------------------------
// command ids (mirror assets/ribbon.yaml `commands:`)

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
}

/// Commands that exist in the markup but have no implementation yet.
/// Kept enabled=false via [`ExplorerHandler::update_property`].
const ALWAYS_DISABLED: &[u32] = &[
    cmd::NEW_ITEM,
    cmd::PROPERTIES,
    cmd::EDIT,
    cmd::HISTORY,
    cmd::SHARE_NOBODY,
    cmd::SHARE_EMAIL,
    cmd::NAV_PANE,
    cmd::PREVIEW_PANE,
    cmd::DETAILS_PANE,
    cmd::GROUP_BY,
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

#[derive(Clone, Copy, Default)]
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
        match &home.groups[0].controls[0] {
            ControlSpec::SplitButton(s) => {
                assert!(matches!(s.command, Some(CommandRef::Id(_))));
                for item in &s.items {
                    assert!(matches!(item.command, Some(CommandRef::Id(_))));
                }
            }
            other => panic!("expected split button, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// snapshot: everything the commanding layer needs from the Explorer model

#[derive(Clone, Default)]
pub struct Snapshot {
    pub ctx: Context,
    pub has_clipboard: bool,
    pub has_items: bool,
    pub view: Option<ViewMode>,
    pub show_hidden: bool,
    pub dark: bool,
    pub self_border: bool,
    pub win8_frame: bool,
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
            view: Some(self.view),
            show_hidden: self.show_hidden,
            dark: self.dark,
            self_border: self.self_border,
            win8_frame: self.frame_style == crate::model::FrameStyle::Win8,
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
            cmd::VIEW_DETAILS => out.push(Message::SetView(ViewMode::Details)),
            cmd::VIEW_LARGE => out.push(Message::SetView(ViewMode::Large)),
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
                    cmd::VIEW_DETAILS => snap.view == Some(ViewMode::Details),
                    cmd::VIEW_LARGE => snap.view == Some(ViewMode::Large),
                    cmd::TOGGLE_HIDDEN => snap.show_hidden,
                    cmd::TOGGLE_DARK => snap.dark,
                    cmd::TOGGLE_BORDER => snap.self_border,
                    cmd::TOGGLE_FRAME => snap.win8_frame,
                    _ => false,
                };
                Some(PropValue::Bool(on))
            }
            PropertyKey::ContextAvailable => {
                let available = match command {
                    cmd::CTX_DRIVE => snap.ctx.is_drive,
                    cmd::CTX_PICTURE => snap.ctx.has_image,
                    cmd::CTX_ARCHIVE => snap.ctx.inside_archive || snap.ctx.has_archive,
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
        cmd::SORT_BY => "Sort by",
        cmd::TOGGLE_HIDDEN => "Hidden items",
        cmd::TOGGLE_DARK => "Dark mode",
        cmd::TOGGLE_BORDER => "Custom window border",
        cmd::TOGGLE_FRAME => "Windows 8 frame style",
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
        if let Some(kind) = self.tabs.get(selected).map(|t| t.ctx) {
            if !visible(kind) {
                if let Some(home) = self.tabs.iter().position(|t| t.id == "home") {
                    view.select_tab(home);
                }
            }
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
            if let Some(last) = out.last_mut() {
                if last.label == *name && last.last_tab + 1 == i {
                    last.w = x + w - last.x;
                    last.last_tab = i;
                    continue;
                }
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
    /// Height of the navigation band (nav buttons / address / search),
    /// from the ribbon markup.
    pub fn band_height(&self) -> f32 {
        match self.framework.spec() {
            Ok(spec) => spec.metrics.band_height,
            Err(_) => 20.0,
        }
    }
}

/// Load the ribbon markup: a user file at `~/.config/explorer/ribbon.yaml`
/// overrides the embedded copy (same override path as before).
fn load_yaml() -> String {
    let user_path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".config/explorer/ribbon.yaml"));
    if let Some(p) = user_path {
        if let Ok(text) = std::fs::read_to_string(&p) {
            return text;
        }
    }
    include_str!("../../assets/ribbon.yaml").to_string()
}

// ---------------------------------------------------------------------------
// theme + view

/// rs-ribbon theme matching the app skin (light: Windows 8 ribbon look;
/// dark: recolored chrome).
pub fn theme(dark: bool) -> Theme {
    let mut t = Theme::windows8();
    if dark {
        use crate::ribbon::theme::rgb;
        t.tab_strip = (rgb(0x2D2D2D), rgb(0x2D2D2D));
        t.ribbon = (rgb(0x2D2D2D), rgb(0x2D2D2D));
        t.tab_active = rgb(0x2D2D2D);
        t.tab_hover = rgb(0x3A4046);
        t.qat_bg = (rgb(0x2D2D2D), rgb(0x2D2D2D));
        t.text = rgb(0xE8E8E8);
        t.tab_text = rgb(0xC8C8C8);
        t.group_label = rgb(0xA0A0A0);
        t.text_disabled = rgb(0x6E6E6E);
        t.border = rgb(0x555555);
        t.group_separator = rgb(0x505050);
        t.tab_line = rgb(0x505050);
        t.hover_bg = rgb(0x3A4046);
        t.hover_border = Some(rgb(0x4A90D9));
        t.pressed_bg = rgb(0x4A525A);
        t.pressed_border = Some(rgb(0x5A89C0));
        t.checked_bg = rgb(0x3D5A78);
        t.checked_border = Some(rgb(0x5A89C0));
        t.popup_bg = rgb(0x383838);
        t.popup_border = rgb(0x505050);
        t.tooltip_bg = rgb(0x383838);
        t.tooltip_border = rgb(0x505050);
        t.accent = rgb(0x4A90D9);
        t.selection = rgb(0x4A90D9);
        t.orb = (rgb(0x4A90D9), rgb(0x2C5F9E));
    }
    t
}

/// The ribbon element (iced widget bound to our framework).
pub fn view(state: &Explorer) -> Element<'_, Message> {
    crate::ribbon::image::set_dark(state.dark);
    RibbonWidget::new(state.ribbon.framework())
        .theme(theme(state.dark))
        .on_event(Message::Ribbon)
        .into()
}

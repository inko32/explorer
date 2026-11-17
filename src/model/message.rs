//! The `Message` enum (every UI/fs event that drives `Explorer::update`)
//! plus the small enums that name UI states.

use std::path::PathBuf;

use crate::fs::FsEvent;

/// Click-focus between the two selection panes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Folder,
    Nav,
}

/// Keyboard focus navigation direction (the idle-selector rectangle
/// follows the layout geometry: grid columns/rows in the icon views,
/// rows-per-column in List, a single column in Content/Details).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(iced::window::Id),
    /// System (compositor) scale factor, learned once at startup.
    SystemScale(f32),
    /// Press on the address/search splitter: start a drag.
    BeginBarSplit,
    /// Cursor moved during a splitter drag (window-logical x).
    BarSplitMove(f32),
    /// Press on the side-pane splitter: start a drag.
    BeginPaneSplit,
    /// Cursor moved during a side-pane splitter drag (window-logical x).
    PaneSplitMove(f32),
    /// Press on the navigation-pane splitter: start a drag.
    BeginNavSplit,
    /// Cursor moved during a nav-pane splitter drag (window-logical x).
    NavSplitMove(f32),
    /// Hover state of the address/search splitter (drives tint + cursor).
    SplitterHover(bool),
    /// Hover state of the back/forward history chevron (drives its
    /// glyph color, #808080 -> #50ACFF).
    BackListHover(bool),
    /// Hover state of the address bar's right-list button (shows its
    /// left divider line only while hovered).
    RightListHover(bool),
    /// Breadcrumb member hover: a text element and its arrow are
    /// tracked separately — hovering the arrow greys the text while
    /// hovering the text lights both blue. The
    /// exit clears only when that same member is still hovered
    /// (enter-before-exit event order between neighbors).
    CrumbHoverEnter {
        group: usize,
        arrow: bool,
    },
    CrumbHoverExit {
        group: usize,
        arrow: bool,
    },
    /// A breadcrumb dropdown arrow was clicked (segment index; 0 = the
    /// independent root arrow): list the subfolders on the fs worker
    /// and open the dropdown.
    CrumbArrow(usize),
    /// The back/forward history chevron was clicked: open the history
    /// dropdown.
    HistoryMenu,
    /// Clicked the free space of the address bar: enter address edit
    /// mode (the breadcrumbs become an editable full path).
    AddressEditStart,
    /// Address edit buffer change.
    AddressEditInput(String),
    /// Enter in the address edit field: go there.
    AddressEditSubmit,
    /// Escape / click-away: discard the address edits.
    CancelAddressEdit,
    /// The address bar right-list was clicked: open the location
    /// selector (the history, anchored below the address bar).
    SelectorMenu,
    /// Clicked the search box: make it editable (the hint text becomes
    /// a text input).
    SearchEditStart,
    /// Search box buffer change.
    SearchInput(String),
    /// Enter / the magnifier button in the search box: run the search
    /// (shows a placeholder text in the folder pane for now).
    SearchSubmit,
    /// Escape / click-away: leave search edit mode (the submitted
    /// query text stays in the box).
    CancelSearch,
    Navigate(PathBuf),
    /// Nav-pane row clicked: marks the nav pane as the active pane,
    /// then navigates.
    NavPress(PathBuf),
    Back,
    Forward,
    Up,
    Refresh,
    Select(usize),
    /// Arrow-key navigation of the idle selector (the focus
    /// rectangle): plain arrows move the selection along, Ctrl+arrow
    /// moves the rectangle only, Shift+arrow extends the range.
    FocusMove(FocusDir),
    /// Space toggles the idle-selected item (the Windows checkbox
    /// gesture; Ctrl+Space accumulates a multiple selection).
    FocusSpace,
    /// A row's hover ended; clears only if that row is still the
    /// hovered one (event order: moving up, the new row's enter
    /// arrives before the old row's exit).
    HoverExit(usize),
    /// Navigation tree row hover (same guarded-exit pattern); the bool
    /// tags a Favorites shortcut row (see nav_pane HOVER_TINTS_SAME_PATH).
    NavHover(Option<(PathBuf, bool)>),
    NavHoverExit(PathBuf),
    /// Rubber-band selection drag on empty folder-view space (content
    /// coordinates; the folder.rs MarqueeZone wrapper publishes these).
    MarqueeBegin {
        x: f32,
        y: f32,
    },
    MarqueeMove {
        x: f32,
        y: f32,
    },
    MarqueeEnd,
    /// Auto-scroll tick while a rubber-band drag is live: the content
    /// scrolls toward the cursor whenever it sits inside the pane's
    /// edge margin (Windows drag-select edge behaviour). Published by
    /// the subscription in main.rs, which is gated on an active
    /// marquee.
    MarqueeAutoScroll,
    Hover(Option<usize>),
    Open(usize),
    ToggleDark,
    ToggleGrayDisabled,
    ToggleIconPack,
    TitlebarPress,
    TitlebarDoubleClick,
    Minimize,
    Maximize,
    Close,
    // ribbon / keyboard commands
    CopySelection,
    CutSelection,
    Paste,
    DeleteSelection,
    RenameSelected,
    CommitRename(String),
    CancelRename,
    NewFolder,
    SelectAll,
    SelectNone,
    InvertSelection,
    /// Set the view to a step index (preset pick).
    SetView(u8),
    /// Gallery hover preview: temporarily switch the view step
    /// (`Some`) and revert to the committed step on leave (`None`).
    PreviewView(Option<u8>),
    /// Ctrl+scroll over the file view: accumulated wheel delta in §7
    /// units (Lines converted to notch deltas, Pixels as-is).
    WheelZoom(f32),
    /// Right-click a file-view item.
    ContextSelect(usize),
    /// Cursor position at a press, published by the `cursor_seed`
    /// wrapper (or `menu::on_right_free`) JUST BEFORE the press
    /// message — the bus is FIFO, so the model's cursor is fresh when
    /// the press message runs. Idle cursor moves no longer reach the
    /// model (main.rs drops them), so press-time readers (drag
    /// thresholds, menu anchors) get the position from the widget
    /// layer instead of a stale `Explorer::cursor`.
    CursorAt(iced::Point),
    /// Right-pressed a file-view row: arms a right-button drag — a
    /// plain release opens the row's context menu, a drag past the
    /// threshold drags with a drop-time action-choice menu.
    RightPress(usize),
    /// Right-click empty folder-view space.
    ContextBackground,
    /// Right-click a navigation pane element.
    ContextNav(PathBuf),
    /// A context menu item was activated (index into the open menu).
    MenuPick(usize),
    /// Context menu item hover (drives the highlight).
    MenuHover(Option<usize>),
    /// Close the context menu (Escape / click-away).
    CloseMenu,
    /// The keyboard Menu key (near AltGr): open the context menu for the
    /// selection, anchored at the first selected element's left-bottom
    /// corner.
    MenuKey,
    /// Windows scrollbar strip: absolute target offset for the file
    /// view (the x offset for List, the y offset otherwise).
    FileScrollTo(f32),
    /// Windows scrollbar strip under the Details view: absolute target
    /// x offset for the outer column strip (List reuses `FileScrollTo`
    /// for its x axis — its horizontal scrollable IS the file view).
    FileHScrollTo(f32),
    /// Same for the navigation pane (always vertical).
    NavScrollTo(f32),
    /// Toggle the side pane right of the folder view (mutually
    /// exclusive: Preview XOR Details XOR none).
    ToggleSidePane(crate::pane::Mode),
    /// Toggle the Details compact mode (24px rows instead of 32, §8.1).
    ToggleDetailsCompact,
    /// Press on a Details column split handle: start a column drag.
    ColumnSplitBegin(usize),
    /// Cursor moved during a Details column drag (window-logical x).
    ColumnSplitMove(f32),
    /// Pressed a Details column header title: arms a potential reorder
    /// drag; a plain release sorts, travel past the threshold drags
    /// (Windows header semantics).
    ColumnSortClick(usize),
    /// Clicked a Details column's dropdown button: open its filter
    /// menu.
    ColumnFilterMenu(usize),
    /// Right-clicked a Details column header: open the column menu.
    ColumnSettingsMenu,
    /// Details column header hover (drives the blue highlight; the
    /// exit clears only when that same column is still hovered).
    ColumnHoverEnter(usize),
    ColumnHoverExit(usize),
    /// A press on the Details header background (not a title, resizer
    /// or dropdown): capture only — the rubber band must never start
    /// on the title bar.
    ColumnHeaderPress,
    /// Animation tick for the Details column header reorder: eases the
    /// base headers into the newly reserved slot while a drag is live.
    ColumnReorderTick,
    SetSort(SortBy),
    /// A ribbon framework event (IUICommandHandler::Execute and friends).
    Ribbon(crate::ribbon::RibbonEvent),
    /// Expand/collapse a folder or archive node in the navigation tree.
    NavToggle(PathBuf),
    /// Expand/collapse a navigation pane section (Favorites/Computer/Network).
    NavSectionToggle(NavSection),
    /// Scroll position of the navigation pane tree.
    NavScrolled {
        y: f32,
        height: f32,
    },
    /// Move/copy the selection to a destination directory (the ribbon
    /// "Move to" / "Copy to" dropdowns).
    TransferTo {
        paths: Vec<PathBuf>,
        dest: PathBuf,
        cut: bool,
    },
    /// Run a shell script via `sh -c` (`{cwd}`/`{selection}` placeholders).
    RunShell(String),
    /// Permanently delete the selection (no trash).
    DeletePermanent,
    ToggleHidden,
    RenameInput(String),
    Status(String),
    Event(iced::Event),
    FsEvent(FsEvent),
    /// Wayland side-channel event (drag-in hover/drop, drag-out
    /// finish, menu-popup input) — see `dnd_wayland` + `popup_wayland`.
    Dnd(crate::dnd_wayland::Event),
    /// Raw Wayland window handles `(wl_surface, wl_display,
    /// xdg_toplevel)` delivered by `iced::window::raw_handle` once the
    /// window exists; spawns the Wayland side-channel (DnD + popups +
    /// icon) via `raw_handle_wayland::wrap`.
    WlHandles(Option<(usize, usize, usize)>),
    /// Scroll offset + viewport height from the file list scrollable.
    /// A scrollable reports only its own axis (`None` = unchanged):
    /// Details nests the vertical list inside a horizontal scrollable
    /// and a zero from the other axis would clobber the live offset.
    Scrolled {
        x: Option<f32>,
        y: Option<f32>,
        height: Option<f32>,
    },
    /// The file-view scrollable area changed size (Sensor) — the
    /// accurate viewport height (on_scroll stays silent when the
    /// content fits, window resize carries the WINDOW height).
    FileAreaResized(f32, f32),
    /// Same for the navigation pane area.
    NavAreaResized(f32, f32),
    /// Window resized (for list virtualization viewport + title-bar layout).
    WindowResized(f32, f32),
    /// Switch between the self-rendered border and native decorations.
    ToggleSelfBorder,
    /// Pressed a contextual title-bar block: activate its tab AND let the
    /// band drag the window (a plain click without movement still switches).
    BlockPress(usize),
    /// Begin a compositor-driven edge resize (custom border mode).
    StartResize(crate::border::ResizeDir),
    /// Caption-button hover tracking: bit 0 = minimize, 1 = maximize.
    /// The Win8.1 min/max glyphs swap black/white on hover, which the
    /// button style can't do to an svg.
    CaptionHover(u8, bool),
    /// The real maximized state reported by the windowing system.
    /// Resizes can come from a compositor-side maximize/unmaximize the
    /// app never commanded, so the local toggle flag is synced from
    /// this instead of trusted blindly.
    MaximizedState(bool),
    /// Cycle the window frame style (Win8 → Win10 → Win8.1).
    ToggleFrameStyle,
    /// View → Options toggle: switch directly to the Win8.1 frame (and
    /// back to the Win8 frame when pressed again).
    ToggleFrame81,
    /// TEMP: paint the folder-view backdrop rectangle with a rainbow
    /// gradient (View-tab ribbon toggle).
    ToggleRainbow,
    /// Toggle the QAT between the title bar ("above") and below the ribbon.
    ToggleQatDock,
    // ---- drag & drop ----
    /// The pointer entered a registered drop target while dragging.
    DragTargetEnter(crate::dnd::DropTarget),
    /// The pointer left a registered drop target.
    DragTargetLeave(crate::dnd::DropTarget),
    /// Cancel the active drag (Escape / window focus loss).
    DragCancel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    Name,
    Size,
    Type,
    Modified,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameStyle {
    /// Win8: 3px accent outer border + 1px darker inner border
    Win8,
    /// Win10: single 1px border
    Win10,
    /// Win8.1: 1px outer outline + 6px border + 1px inner outline
    /// (the 1px outlines are never DPI-scaled)
    Win81,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipMode {
    Copy,
    Cut,
}

/// Navigation pane sections (foldable group headers without a backing
/// location; "Computer" is the "/" tree node itself, not a section flag).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavSection {
    Favorites,
    Network,
}

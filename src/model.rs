//! Application state (MVU model) — pure logic, no widgets.

use std::path::{Path, PathBuf};
use crate::fs::{self, FsEvent, Job};

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
    Navigate(PathBuf),
    Back,
    Forward,
    Up,
    Refresh,
    Select(usize),
    /// A row's hover ended; clears only if that row is still the
    /// hovered one (event order: moving up, the new row's enter
    /// arrives before the old row's exit).
    HoverExit(usize),
    /// Navigation tree row hover (same guarded-exit pattern).
    NavHover(Option<PathBuf>),
    NavHoverExit(PathBuf),
    /// Rubber-band selection drag on empty folder-view space (content
    /// coordinates; the folder.rs MarqueeZone wrapper publishes these).
    MarqueeBegin { x: f32, y: f32 },
    MarqueeMove { x: f32, y: f32 },
    MarqueeEnd,
    Hover(Option<usize>),
    Open(usize),
    ToggleDark,
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
    /// Set the view to a step index (SPEC/FOLDER-SPEC.MD §5 preset pick).
    SetView(u8),
    /// Ctrl+scroll over the file view: accumulated wheel delta in §7
    /// units (Lines converted to notch deltas, Pixels as-is).
    WheelZoom(f32),
    /// Right-click a file-view item (SPEC/MENU_SPEC.MD).
    ContextSelect(usize),
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
    SetSort(SortBy),
    /// A ribbon framework event (IUICommandHandler::Execute and friends).
    Ribbon(crate::ribbon::RibbonEvent),
    /// Expand/collapse a folder or archive node in the navigation tree.
    NavToggle(PathBuf),
    /// Expand/collapse a navigation pane section (Favorites/Computer/Network).
    NavSectionToggle(NavSection),
    /// Scroll position of the navigation pane tree.
    NavScrolled { y: f32, height: f32 },
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
    /// Scroll offset + viewport height from the file list scrollable.
    Scrolled { x: f32, y: f32, height: f32 },
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
    /// Switch between the Win8 (double) and Win10 (1px) window borders.
    ToggleFrameStyle,
    /// Toggle the QAT between the title bar ("above") and below the ribbon.
    ToggleQatDock,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameStyle {
    /// Win8: 3px accent outer border + 1px darker inner border
    Win8,
    /// Win10: single 1px border
    Win10,
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

/// Navigation tree state: which nodes are open and the cached
/// folder/archive children of every listed node.
pub struct NavTree {
    pub favorites_open: bool,
    pub network_open: bool,
    /// Expanded folder/archive paths.
    pub expanded: std::collections::BTreeSet<PathBuf>,
    /// path -> its folder/archive children (filled from every List result).
    pub children: std::collections::HashMap<PathBuf, Vec<fs::Entry>>,
    /// Scroll offset + viewport height of the tree scrollable.
    pub scroll_y: f32,
    pub viewport_h: f32,
}

impl NavTree {
    fn new() -> Self {
        Self {
            favorites_open: true,
            network_open: false,
            expanded: Default::default(),
            children: Default::default(),
            scroll_y: 0.0,
            viewport_h: 0.0,
        }
    }

    /// Keep only folders and archives, hidden entries out unless requested,
    /// sorted folders-first by name (Windows tree order).
    fn store(&mut self, path: &Path, entries: &[fs::Entry], show_hidden: bool) {
        let mut v: Vec<fs::Entry> = entries
            .iter()
            .filter(|e| (show_hidden || !e.is_hidden)
                && (e.is_dir() || fs::archive::is_archive_path(&e.path)))
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            b.is_dir().cmp(&a.is_dir()).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.children.insert(path.to_path_buf(), v);
    }

    /// Request a listing for `path` when it is not cached yet.
    fn ensure_listed(&self, sched: &fs::Scheduler, path: &Path) {
        if !self.children.contains_key(path) {
            sched.send(Job::List(path.to_path_buf()));
        }
    }
}

/// Details column drag: which fixed column (0=Date, 1=Type, 2=Size),
/// the cursor x at drag start (learned on the first move) and the
/// column width at drag start.
struct ColDrag {
    col: usize,
    start_x: Option<f32>,
    start_w: f32,
}

/// Rubber-band selection rectangle (file-view content coordinates).
pub struct Marquee {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// Selection snapshot at drag start (Ctrl+drag adds to it).
    base: std::collections::BTreeSet<usize>,
}

pub struct Explorer {
    pub window: Option<iced::window::Id>,
    pub cwd: PathBuf,
    pub entries: Vec<fs::Entry>,
    pub selected: std::collections::BTreeSet<usize>,
    pub hovered: Option<usize>,
    /// Hovered navigation tree row.
    pub nav_hover: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    pub dark: bool,
    /// `--ui-debug`: outline every element (colored 1px borders).
    pub ui_debug: bool,
    /// `--dpi=N`: absolute UI scale factor (percent of 96 dpi, or direct).
    /// None = follow the system scale.
    pub dpi_target: Option<f32>,
    /// Compositor scale factor, learned via `window::scale_factor` after open.
    pub system_scale: f32,
    /// Search-box width as a share of the content width (the scaling factor
    /// for the address/search split; polished later).
    pub search_share: f32,
    /// Splitter drag in progress.
    pub bar_split: bool,
    /// Pointer over the address/search splitter.
    pub splitter_hover: bool,
    pub maximized: bool,
    /// Ctrl held (drives Ctrl+scroll view zoom over the file view).
    pub ctrl_held: bool,
    /// Shift held (Shift+click range selection).
    pub shift_held: bool,
    /// Range-selection anchor (last plain/Ctrl click).
    sel_anchor: Option<usize>,
    /// Rubber-band selection in progress (folder view, all layouts).
    pub marquee: Option<Marquee>,
    /// Last title-bar press (double-click detection: the 2nd press must not
    /// start a window drag, or the in-flight move scrambles the placement of
    /// the subsequent maximize).
    last_title_press: Option<std::time::Instant>,
    /// Self-rendered window border/title bar (off = native decorations).
    pub self_border: bool,
    /// Win8 double border vs Win10 1px border.
    pub frame_style: FrameStyle,
    pub clipboard: Option<(ClipMode, Vec<PathBuf>)>,
    /// Folder view configuration (`assets/folder-view.yaml`,
    /// SPEC/FOLDER-SPEC.MD): step sizes, layout metrics, enabled mask.
    pub view_cfg: crate::view_step::ViewCfg,
    /// View step index (SPEC/FOLDER-SPEC.MD §3).
    pub view_index: u8,
    /// §6 enabled mask (resolved from folder-view.yaml at startup).
    #[allow(dead_code)] // the config is read via view_cfg.mask(); this
                        // cache is for the future per-folder masks
    view_mask: u64,
    /// §7 wheel-delta accumulator (reset when Ctrl is released).
    zoom_acc: f32,
    /// §7 rate limit: time of the last zoom step.
    zoom_step_at: Option<std::time::Instant>,
    /// §10 per-folder stored view index (session persistence).
    folder_views: std::collections::HashMap<PathBuf, u8>,
    /// §8.1 Details compact mode (24px rows); compact is the default.
    pub details_compact: bool,
    /// Details column widths (Date modified / Type / Size); the Name
    /// column takes the remaining width. Adjustable via header handles.
    pub details_cols: [f32; 3],
    /// Details column drag in progress.
    col_drag: Option<ColDrag>,
    pub sort: SortBy,
    pub show_hidden: bool,
    /// Font sizes (assets/fonts.yaml).
    pub fonts: crate::fonts::FontSizes,
    /// The ribbon (rs-ribbon framework + explorer commanding).
    pub ribbon: crate::ribbon::Ribbon,
    /// File-view scroll offsets (List scrolls horizontally, §8.1).
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub viewport_h: f32,
    /// Measured window width (resize events) for title-bar layout.
    pub win_w: f32,
    /// Inline rename: (row index, text buffer).
    pub renaming: Option<(usize, String)>,
    /// Context menu configuration (assets/menu.yaml) + the open menu.
    pub menu_cfg: crate::menu::MenuCfg,
    pub menu: Option<crate::menu::Open>,
    /// Last known cursor position (window-logical; menu placement).
    cursor: iced::Point,
    /// The side pane (Preview / Details / none, right of the folder
    /// view) and its cached content.
    pub pane: crate::pane::Mode,
    /// Side pane width (splitter-dragged; `pane::clamped` at use).
    pub pane_width: f32,
    /// Side-pane splitter drag in progress.
    pub pane_split: bool,
    /// Navigation pane width (splitter-dragged; the yaml sacrifice rules
    /// shrink/hide it when the window is narrow).
    pub nav_width: f32,
    /// Nav-pane splitter drag in progress.
    pub nav_split: bool,
    pane_key: Option<PathBuf>,
    pub pane_content: crate::pane::Content,
    pub status: String,
    pub last_click: Option<(usize, std::time::Instant)>,
    pub loading: bool,
    pub volumes: Vec<fs::Volume>,
    /// Navigation pane tree (expanded nodes + cached children).
    pub nav: NavTree,
    /// The cwd changed but volumes/listings were not known yet: re-run
    /// the nav-tree reveal when the data arrives.
    request_reveal: bool,
    sched: fs::Scheduler,
    pending_rename: Option<String>,
}

/// Recursive copy (files + dirs); true on success.
fn copy_rec(src: &Path, dst: &Path) -> bool {
    if src.is_dir() {
        if std::fs::create_dir_all(dst).is_err() {
            return false;
        }
        let Ok(rd) = std::fs::read_dir(src) else { return false };
        for e in rd.flatten() {
            if !copy_rec(&e.path(), &dst.join(e.file_name())) {
                return false;
            }
        }
        true
    } else {
        std::fs::copy(src, dst).is_ok()
    }
}

impl Explorer {
    pub fn new() -> Self {
        let (sched, ev_rx) = fs::spawn();
        let _ = crate::FS_STREAM_RX.set(std::sync::Mutex::new(Some(ev_rx)));
        // Initial path: first positional arg (flags and their values are
        // skipped — see main.rs for the flag set).
        let mut cwd = None;
        let mut skip_next = false;
        for arg in std::env::args().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if arg == "--render" {
                skip_next = true;
                continue;
            }
            if arg.starts_with('-') {
                continue;
            }
            cwd = Some(PathBuf::from(arg));
            break;
        }
        let cwd = cwd
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/"));
        let view_cfg = crate::view_step::ViewCfg::load();
        let view_index = view_cfg.presets.details;
        let view_mask = view_cfg.mask();
        let details_cols = view_cfg.details_col_widths();
        let mut s = Self {
            window: None,
            cwd,
            entries: Vec::new(),
            selected: Default::default(),
            hovered: None,
            nav_hover: None,
            back: Vec::new(),
            forward: Vec::new(),
            dark: false,
            ui_debug: std::env::var_os("EXPLORER_UI_DEBUG").is_some(),
            dpi_target: std::env::var("EXPLORER_DPI")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v: &f32| *v > 0.0),
            system_scale: 0.0,
            search_share: 0.15625, // 180 / 1152
            bar_split: false,
            splitter_hover: false,
            maximized: false,
            ctrl_held: false,
            shift_held: false,
            sel_anchor: None,
            marquee: None,
            last_title_press: None,
            self_border: true,
            frame_style: FrameStyle::Win8,
            clipboard: None,
            view_cfg,
            view_index,
            view_mask,
            zoom_acc: 0.0,
            zoom_step_at: None,
            folder_views: Default::default(),
            details_compact: true,
            details_cols,
            col_drag: None,
            sort: SortBy::Name,
            show_hidden: false,
            fonts: crate::fonts::FontSizes::load(),
            ribbon: crate::ribbon::Ribbon::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            viewport_h: 600.0,
            win_w: 1152.0,
            renaming: None,
            menu_cfg: crate::menu::MenuCfg::load(),
            menu: None,
            cursor: iced::Point::ORIGIN,
            pane: crate::pane::Mode::None,
            pane_width: crate::pane::DEFAULT_WIDTH,
            pane_split: false,
            nav_width: crate::nav_pane::NAV_W,
            nav_split: false,
            pane_key: None,
            pane_content: crate::pane::Content::None,
            status: String::new(),
            last_click: None,
            loading: false,
            volumes: Vec::new(),
            nav: NavTree::new(),
            request_reveal: false,
            sched,
            pending_rename: None,
        };
        s.request_listing();
        s.sched.send(Job::Volumes);
        // Startup runs before volumes/listings arrive, so the reveal defers
        // via request_reveal; the returned scroll Task has no tree to act on
        // yet and is intentionally dropped.
        let _ = s.nav_reveal_cwd();
        let snap = s.ribbon_snapshot();
        s.ribbon.sync(snap);
        s.ribbon.auto_activate();
        s
    }

    /// Scale-factor multiplier for iced (total = system x multiplier, polled
    /// every frame). Returns 1.0 until the system scale is learned.
    pub fn dpi_multiplier(&self) -> f32 {
        match (self.dpi_target, self.system_scale) {
            (Some(target), s) if s > 0.0 => target / s,
            _ => 1.0,
        }
    }

    pub fn title(&self) -> String {
        if self.cwd == Path::new("/") {
            "Computer".into()
        } else {
            self.cwd
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.cwd.display().to_string())
        }
    }

    /// Window-icon glyph for the current location (title bar, 16px):
    /// Computer / home / a drive root / inside an archive / folder.
    pub fn location_icon(&self) -> &'static str {
        if self.cwd == Path::new("/") {
            "computer"
        } else if fs::archive::resolve(&self.cwd).is_some() {
            "archive"
        } else if self.volumes.iter().any(|v| v.mount == self.cwd) {
            "drive"
        } else if std::env::var_os("HOME").map(PathBuf::from).as_deref() == Some(self.cwd.as_path()) {
            "home"
        } else {
            "folder"
        }
    }

    pub fn back_is_empty(&self) -> bool {
        self.back.is_empty()
    }

    pub fn forward_is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    pub fn theme(&self) -> iced::Theme {
        if self.dark {
            iced::Theme::custom("win8-dark", crate::ui_common::skin::DARK.palette())
        } else {
            iced::Theme::custom("win8-light", crate::ui_common::skin::LIGHT.palette())
        }
    }

    fn request_listing(&mut self) {
        self.sched.send(Job::List(self.cwd.clone()));
        self.loading = true;
    }

    fn navigate(&mut self, path: PathBuf) -> iced::Task<Message> {
        if path == self.cwd {
            return iced::Task::none();
        }
        self.back.push(self.cwd.clone());
        self.forward.clear();
        self.cwd = path;
        self.selected.clear();
        self.renaming = None;
        self.restore_folder_view();
        self.request_listing();
        self.nav_reveal_cwd()
    }

    /// §10 per-folder persistence: restore the stored view index for the
    /// current folder (snapped to the enabled mask); unknown folders use
    /// the default.
    fn restore_folder_view(&mut self) {
        let stored = self.folder_views.get(&self.cwd).copied();
        self.view_index = self
            .view_cfg
            .nearest_enabled(stored.unwrap_or(self.view_cfg.presets.details));
        self.zoom_acc = 0.0;
        self.zoom_step_at = None;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
    }

    /// (nav pane width, side pane width) actually shown at the current
    /// window size — the yaml sacrifice rules: the side pane hides below
    /// `pane_below` (before the folder view gets squeezed), the nav pane
    /// shrinks below `nav_resize_below` and hides below `nav_below`.
    /// ui.rs lays out with exactly these widths.
    pub(crate) fn layout_widths(&self) -> (f32, f32) {
        let (_, fr, _, fl) =
            crate::border::frame_insets_pub(self.self_border, self.frame_style);
        let content_w = self.win_w - fl - fr;
        let mult = self.dpi_multiplier();
        let (_, min_nav_w, nav_resize_below, _) = self.ribbon.sacrifice();
        let nav_w = if content_w >= min_nav_w * mult {
            if content_w >= nav_resize_below * mult {
                self.nav_width
            } else {
                (content_w * self.nav_width / (nav_resize_below * mult)).max(110.0)
            }
        } else {
            0.0
        };
        let side_w = if self.pane != crate::pane::Mode::None
            && content_w >= self.ribbon.pane_below() * mult
        {
            crate::pane::clamped(self.pane_width, content_w)
        } else {
            0.0
        };
        (nav_w, side_w)
    }

    /// Width of the file-view pane (window minus frame, nav pane, side
    /// pane and scrollbar strip) — the same math ui.rs uses to lay out
    /// the main row. The model needs it for the §9 scroll anchor (grid
    /// columns depend on the width).
    fn file_pane_width(&self) -> f32 {
        let (_, fr, _, fl) =
            crate::border::frame_insets_pub(self.self_border, self.frame_style);
        let content_w = self.win_w - fl - fr;
        let (nav_w, side_w) = self.layout_widths();
        // the nav_gap spacer is part of the folder view (always shown);
        // the splitters (4px) and the pane separator (1px) only with
        // their panes
        let chrome = self.ribbon.nav_gap()
            + if nav_w > 0.0 { 4.0 } else { 0.0 }
            + if side_w > 0.0 { 4.0 + 1.0 } else { 0.0 };
        let pane_w = content_w - nav_w - side_w - chrome;
        // the scrollbar strip collapses to 0px when the content fits
        // (same rule as folder::file_view), widening the pane
        let cfg = &self.view_cfg;
        let longest = self
            .entries
            .iter()
            .map(|e| e.name.chars().count())
            .max()
            .unwrap_or(0);
        let with_strip = (pane_w - crate::scrollbar::THICKNESS).max(0.0);
        let extent = cfg.content_extent(
            self.view_index,
            self.entries.len(),
            with_strip,
            self.viewport_h,
            longest,
            self.details_compact,
        );
        let viewport_axis = if cfg.family(self.view_index) == crate::view_step::Family::List {
            pane_w
        } else {
            self.viewport_h
        };
        if extent > viewport_axis {
            with_strip
        } else {
            pane_w.max(0.0)
        }
    }

    /// Switch the view step index (§5 preset pick / §7 wheel step),
    /// snapping to the enabled mask, keeping the first visible item
    /// anchored in the viewport (§9) and storing the choice per folder
    /// (§10). Selection and hover stay untouched.
    fn set_view_index(&mut self, index: u8) -> iced::Task<Message> {
        let index = self.view_cfg.nearest_enabled(index);
        if index == self.view_index {
            return iced::Task::none();
        }
        let old = self.view_index;
        let old_compact = self.details_compact;
        self.view_index = index;
        self.folder_views.insert(self.cwd.clone(), index);
        self.anchor_view(old, old_compact)
    }

    /// §9 scroll anchor: keep the first visible item (in the OLD layout)
    /// at the same viewport position in the current layout.
    fn anchor_view(&mut self, old_index: u8, old_compact: bool) -> iced::Task<Message> {
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
        let item = cfg.first_visible(
            old_index,
            old_offset,
            pane_w,
            self.viewport_h,
            n,
            longest,
            old_compact,
        );
        let within = old_offset
            - cfg.item_offset(old_index, item, pane_w, self.viewport_h, longest, old_compact);
        let index = self.view_index;
        let offset = (cfg.item_offset(
            index,
            item,
            pane_w,
            self.viewport_h,
            longest,
            self.details_compact,
        ) + within)
            .max(0.0);
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
    fn wheel_zoom(&mut self, delta: f32) -> iced::Task<Message> {
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

    /// Expand the cwd's ancestors in the nav tree so the current location
    /// stays visible (Win7 "expand to current folder" behavior), then
    /// scroll the tree to the current row. Volumes sit directly under
    /// Computer, so inside a mounted volume the chain stops at the mount
    /// point (the tree does not re-descend through /media/... to reach
    /// the drive it already shows). Listings that are still in flight
    /// defer the reveal via `request_reveal`.
    fn nav_reveal_cwd(&mut self) -> iced::Task<Message> {
        let boundary = self
            .volumes
            .iter()
            .filter(|v| v.mount != Path::new("/") && self.cwd.starts_with(&v.mount))
            .map(|v| v.mount.as_path())
            .max_by_key(|m| m.components().count());
        let mut targets: Vec<PathBuf> = Vec::new();
        let mut p: &Path = &self.cwd;
        loop {
            targets.push(p.to_path_buf());
            if Some(p) == boundary {
                break;
            }
            match p.parent() {
                Some(parent) => p = parent,
                None => break,
            }
        }
        self.request_reveal = targets.iter().any(|t| !self.nav.children.contains_key(t));
        for t in targets {
            self.nav.expanded.insert(t.clone());
            self.nav.ensure_listed(&self.sched, &t);
        }
        if self.request_reveal {
            iced::Task::none()
        } else {
            self.nav_scroll_to_cwd()
        }
    }

    /// Scroll the nav tree so the current-folder row is visible. Only
    /// correct once every reveal target is cached (the row offsets are
    /// computed from the tree contents).
    fn nav_scroll_to_cwd(&self) -> iced::Task<Message> {
        let Some(idx) = crate::nav_pane::selected_row_index(self) else {
            return iced::Task::none();
        };
        let (_, item_h, ..) = self.ribbon.metrics();
        // favorites rows precede the tree; reveal targets sit below them
        let favs = if self.nav.favorites_open {
            crate::nav_pane::favorites_len() + 1
        } else {
            1
        };
        let row = (favs + idx) as f32;
        let padding = 8.0; // column padding(4) top + bottom
        let viewport = self.nav.viewport_h;
        if viewport <= 0.0 {
            return iced::Task::none();
        }
        let content_h = padding + (crate::nav_pane::tree_row_count(self) as f32) * item_h;
        let max_scroll = (content_h - viewport).max(0.0);
        let row_top = 4.0 + row * item_h;
        let row_bottom = row_top + item_h;
        let target = if row_top < self.nav.scroll_y {
            // navigating up: leave a couple of rows of context above the
            // row (ancestors/siblings) instead of pinning it flush to
            // the top — mirrors the down case which shows ancestors
            // above the row pinned at the bottom.
            (row_top - 3.0 * item_h).max(0.0)
        } else if row_bottom > self.nav.scroll_y + viewport {
            row_bottom - viewport
        } else {
            return iced::Task::none(); // already visible
        };
        let y = (target.clamp(0.0, max_scroll) / max_scroll.max(f32::EPSILON)).clamp(0.0, 1.0);
        iced::widget::operation::scrollable::snap_to(
            crate::nav_pane::nav_scrollable_id(),
            iced::widget::operation::scrollable::RelativeOffset { x: None, y: Some(y) },
        iced::widget::operation::Animation::Instant,
        )
    }

    /// Sort entries: directories first, then by the active key.
    pub fn apply_sort(&mut self) {
        let sort = self.sort;
        self.entries.sort_by(|a, b| {
            let ad = a.kind == fs::EntryKind::Dir;
            let bd = b.kind == fs::EntryKind::Dir;
            if ad != bd {
                return bd.cmp(&ad);
            }
            match sort {
                SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Modified => a.modified.cmp(&b.modified),
            }
        });
    }

    pub(crate) fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected.iter().filter_map(|&i| self.entries.get(i)).map(|e| e.path.clone()).collect()
    }

    fn copy_or_cut(&mut self, mode: ClipMode) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            let n = paths.len();
            self.clipboard = Some((mode, paths));
            self.status = match mode {
                ClipMode::Copy => format!("Copied {n} item{}", if n == 1 { "" } else { "s" }),
                ClipMode::Cut => format!("Cut {n} item{}", if n == 1 { "" } else { "s" }),
            };
        }
    }

    fn paste(&mut self) {
        let Some((mode, sources)) = self.clipboard.clone() else { return };
        if sources.is_empty() {
            return;
        }
        let cut = mode == ClipMode::Cut;
        self.sched.send(Job::Copy { sources, dest_dir: self.cwd.clone(), cut });
        self.status = "Copying…".into();
        if cut {
            self.clipboard = None;
        }
    }

    fn delete_selection(&mut self) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        self.sched.send(Job::Trash(paths));
    }

    pub fn start_rename(&mut self) {
        if let Some(&i) = self.selected.iter().next() {
            if i < self.entries.len() {
                let name = self.entries[i].name.clone();
                self.renaming = Some((i, name));
            }
        }
    }

    fn commit_rename(&mut self, new_name: String) {
        let name = new_name.trim().to_string();
        if let Some((i, _)) = self.renaming.take() {
            if !name.is_empty() && i < self.entries.len() && name != self.entries[i].name {
                let from = self.entries[i].path.clone();
                let to = from.with_file_name(&name);
                self.sched.send(Job::Rename(from, to));
            }
        }
    }

    fn handle_fs_event(&mut self, ev: FsEvent) {
        match ev {
            FsEvent::Listed { path, result } => {
                // every listing also feeds the nav tree cache (folders/archives only)
                if let Ok(entries) = &result {
                    self.nav.store(&path, entries, self.show_hidden);
                }
                if path == self.cwd {
                    self.loading = false;
                    match result {
                        Ok(entries) => {
                            let show_hidden = self.show_hidden;
                            self.entries =
                                entries.into_iter().filter(|e| show_hidden || !e.is_hidden).collect();
                            self.apply_sort();
                            self.selected.clear();
                            self.hovered = None;
                            self.renaming = None;
                            if let Ok(pat) = std::env::var("EXPLORER_DEBUG_SELECT") {
                                for (i, e) in self.entries.iter().enumerate() {
                                    if e.name.contains(&pat) {
                                        self.selected.insert(i);
                                    }
                                }
                            }
                            if let Some(name) = self.pending_rename.take() {
                                if let Some(i) = self.entries.iter().position(|e| e.name == name) {
                                    self.selected.insert(i);
                                    self.renaming = Some((i, name));
                                }
                            }
                        }
                        Err(e) => {
                            self.entries.clear();
                            self.status = e;
                        }
                    }
                }
            }
            FsEvent::Volumes(v) => self.volumes = v,
            FsEvent::DirCreated { parent, result } => {
                if parent == self.cwd {
                    if let Ok(name) = result {
                        self.pending_rename = Some(name);
                    }
                    self.request_listing();
                }
            }
            FsEvent::Renamed { result } => {
                if let Err(e) = result {
                    self.status = format!("Rename failed: {e}");
                }
                self.request_listing();
            }
            FsEvent::Trashed { result } => {
                if let Err(e) = result {
                    self.status = format!("Delete failed: {e}");
                }
                self.request_listing();
            }
            FsEvent::OpProgress { done_bytes, total_bytes, current } => {
                let pct = if total_bytes > 0 { done_bytes * 100 / total_bytes } else { 0 };
                self.status = format!(
                    "Copying {} — {}%",
                    current.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    pct
                );
            }
            FsEvent::OpDone { result } => {
                if let Err(e) = result {
                    self.status = format!("Operation failed: {e}");
                } else {
                    self.status.clear();
                }
                self.request_listing();
            }
        }
    }

    /// The context the ribbon commanding layer evaluates against:
    /// contextual-tab availability + enablement rules.
    pub fn context(&self) -> crate::ribbon::Context {
        let sel_has = |f: &dyn Fn(&fs::Entry) -> bool| {
            self.selected.iter().filter_map(|&i| self.entries.get(i)).any(f)
        };
        let has_image = sel_has(&|e: &fs::Entry| {
            e.name
                .rsplit_once('.')
                .map(|(_, x)| {
                    matches!(
                        x.to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg"
                    )
                })
                .unwrap_or(false)
        });
        crate::ribbon::Context {
            is_drive: self.cwd.parent().is_none() || self.volumes.iter().any(|v| v.mount == self.cwd),
            inside_archive: fs::archive::resolve(&self.cwd).is_some(),
            has_archive: sel_has(&|e: &fs::Entry| fs::archive::is_archive_path(&e.path)),
            has_image,
            has_selection: !self.selected.is_empty(),
        }
    }


    fn expand_placeholders(&self, script: &str) -> String {
        fn q(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let selection = self
            .selected
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .map(|e| q(&e.path.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ");
        script
            .replace("{cwd}", &q(&self.cwd.to_string_lossy()))
            .replace("{selection}", &selection)
            .replace("{selection_count}", &self.selected.len().to_string())
    }

    // ---- context menus (SPEC/MENU_SPEC.MD) ----

    /// Right-click a file-view item: Windows selects it first when it is
    /// not part of the selection, then opens the menu at the cursor.
    fn context_select(&mut self, i: usize) {
        let (x, y) = (self.cursor.x, self.cursor.y);
        self.context_select_at(i, x, y);
    }

    /// Selection menu at an explicit anchor (right-click: the cursor;
    /// Menu key: the element's left-bottom corner).
    fn context_select_at(&mut self, i: usize, x: f32, y: f32) {
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
        // the menu shields the view behind it: drop the live hover
        self.hovered = None;
        self.nav_hover = None;
        self.menu = Some(crate::menu::Open {
            entries,
            x,
            y,
            hover: None,
            nav_target: None,
        });
    }

    /// Item rectangle in file-view CONTENT coordinates (before scroll),
    /// shared by the Menu-key anchor and the rubber-band hit test.
    fn item_rect(&self, i: usize, pane_w: f32) -> (f32, f32, f32, f32) {
        use crate::view_step as vs;
        let cfg = &self.view_cfg;
        let s = cfg.icon_px(self.view_index);
        match cfg.family(self.view_index) {
            vs::Family::Content => {
                let row_h = cfg.layouts.content.row_h;
                (0.0, i as f32 * row_h, pane_w, row_h)
            }
            vs::Family::Details => {
                let row_h = cfg.details_row_h(self.details_compact);
                (0.0, cfg.layouts.details.header_h + i as f32 * row_h, pane_w, row_h)
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
                let rows = ((self.viewport_h / l.row_h).floor() as usize).max(1);
                ((i / rows) as f32 * col_w, (i % rows) as f32 * l.row_h, col_w, l.row_h)
            }
            vs::Family::Tiles => {
                let t = &cfg.layouts.tiles;
                let cols = vs::grid_cols(t.cell_w, 0.0, pane_w);
                ((i % cols) as f32 * t.cell_w, (i / cols) as f32 * t.cell_h, t.cell_w, t.cell_h)
            }
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(s);
                let cols = vs::grid_cols(m.cell_w, m.gap, pane_w);
                (
                    (i % cols) as f32 * (m.cell_w + m.gap),
                    (i / cols) as f32 * (m.cell_h + m.gap),
                    m.cell_w,
                    m.cell_h,
                )
            }
            vs::Family::IconTop => {
                let m = cfg.icon_top(s);
                let cols = vs::grid_cols(m.cell_w, m.gap, pane_w);
                (
                    (i % cols) as f32 * (m.cell_w + m.gap),
                    (i / cols) as f32 * (m.cell_h + m.gap),
                    m.cell_w,
                    m.cell_h,
                )
            }
        }
    }

    /// Rubber-band: select every item whose cell intersects the marquee.
    fn marquee_update(&mut self) {
        let Some(m) = &self.marquee else { return };
        let (x0, x1) = (m.x0.min(m.x1), m.x0.max(m.x1));
        let (y0, y1) = (m.y0.min(m.y1), m.y0.max(m.y1));
        let pane_w = self.file_pane_width();
        let mut hits = m.base.clone();
        for i in 0..self.entries.len() {
            let (ix, iy, iw, ih) = self.item_rect(i, pane_w);
            if ix < x1 && ix + iw > x0 && iy < y1 && iy + ih > y0 {
                hits.insert(i);
            }
        }
        if hits != self.selected {
            self.selected = hits;
        }
    }

    /// Left-bottom corner of file-view item `i` in window coordinates
    /// (Menu-key anchor). Best-effort: recomputed from the same layout
    /// math as ui.rs + folder.rs.
    fn item_anchor(&self, i: usize) -> (f32, f32) {
        let (ft, fr, _, fl) =
            crate::border::frame_insets_pub(self.self_border, self.frame_style);
        let title_h = if self.self_border { crate::border::TITLEBAR_H } else { 0.0 };
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
        let pane_x = fl + nav_w + self.ribbon.nav_gap()
            + if nav_w > 0.0 { 4.0 } else { 0.0 };
        let pane_y = ft + title_h + ribbon_h + band_h;
        let (x, y, _, h) = self.item_rect(i, self.file_pane_width());
        (pane_x + x - self.scroll_x, pane_y + y + h - self.scroll_y)
    }

    /// Menu item activated: close the menu, then dispatch (§8: every
    /// command re-validates at execution — the file ops report errors
    /// via the status bar).
    fn menu_dispatch(&mut self, idx: usize) -> iced::Task<Message> {
        let Some(open) = self.menu.take() else {
            return iced::Task::none();
        };
        let Some(crate::menu::Entry::Item(item)) = open.entries.get(idx).cloned() else {
            return iced::Task::none();
        };
        match item.id.as_str() {
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

    /// Keep the side-pane preview cache in sync with the selection
    /// (cheap key compare; reloads only when the selection changes).
    fn sync_pane(&mut self) {
        if self.pane == crate::pane::Mode::None {
            return;
        }
        let sel = self.selected.iter().next().and_then(|&i| self.entries.get(i));
        let key = sel.map(|e| e.path.clone());
        if key != self.pane_key {
            self.pane_key = key;
            self.pane_content = match sel {
                Some(e) => crate::pane::load(&e.path, e.is_dir()),
                None => crate::pane::Content::None,
            };
        }
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        // ribbon-originated events must not re-run contextual
        // auto-activation (the user just clicked a tab)
        let auto = !matches!(message, Message::Ribbon(_));
        let task = self.handle(message);
        self.sync_pane();
        // every model change refreshes the ribbon (enablement, toggle
        // values, contextual-tab availability) — but only when the
        // snapshot CHANGED: update() runs per input event, and a mouse
        // move must not invalidate + re-query every command
        let snap = self.ribbon_snapshot();
        if self.ribbon.sync_changed(&snap) && auto {
            self.ribbon.auto_activate();
        }
        task
    }

    fn handle(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window = Some(id);
                iced::window::scale_factor(id).map(Message::SystemScale)
            }
            Message::SystemScale(s) => {
                if std::env::var_os("EXPLORER_DEBUG").is_some() {
                    eprintln!("SystemScale: {s}");
                }
                self.system_scale = s;
                // iced applies the dpi multiplier to the viewport WITHOUT
                // re-emitting a Resized event, so once `--dpi` changes the
                // effective scale the cached logical win_w is stale (e.g.
                // --dpi=100 on a 150% system: win_w still 1152 while the
                // logical width becomes 1728) and the splitter math stops
                // tracking the cursor. Re-query the size under the new scale.
                if self.dpi_target.is_some() {
                    if let Some(id) = self.window {
                        return iced::window::size(id)
                            .map(|sz| Message::WindowResized(sz.width, sz.height));
                    }
                }
                iced::Task::none()
            }
            Message::Navigate(p) => self.navigate(p),
            Message::Back => {
                if let Some(p) = self.back.pop() {
                    self.forward.push(self.cwd.clone());
                    self.cwd = p;
                    self.selected.clear();
                    self.renaming = None;
                    self.restore_folder_view();
                    self.request_listing();
                }
                iced::Task::none()
            }
            Message::Forward => {
                if let Some(p) = self.forward.pop() {
                    self.back.push(self.cwd.clone());
                    self.cwd = p;
                    self.selected.clear();
                    self.renaming = None;
                    self.restore_folder_view();
                    self.request_listing();
                }
                iced::Task::none()
            }
            Message::Up => {
                match self.cwd.parent().map(Path::to_path_buf) {
                    Some(p) => self.navigate(p),
                    None => iced::Task::none(),
                }
            }
            Message::Refresh => {
                self.request_listing();
                // re-list every open nav node so the tree stays in sync
                let open: Vec<PathBuf> = self.nav.expanded.iter().cloned().collect();
                for p in open {
                    self.sched.send(Job::List(p));
                }
                iced::Task::none()
            }
            Message::NavToggle(path) => {
                if self.nav.expanded.contains(&path) {
                    self.nav.expanded.remove(&path);
                } else {
                    self.nav.expanded.insert(path.clone());
                    self.nav.ensure_listed(&self.sched, &path);
                }
                iced::Task::none()
            }
            Message::NavSectionToggle(section) => {
                match section {
                    NavSection::Favorites => self.nav.favorites_open = !self.nav.favorites_open,
                    NavSection::Network => self.nav.network_open = !self.nav.network_open,
                }
                iced::Task::none()
            }
            Message::Select(i) => {
                if i >= self.entries.len() {
                    return iced::Task::none();
                }
                if self.ctrl_held {
                    // Ctrl+click toggles the item, keeps the rest
                    if !self.selected.remove(&i) {
                        self.selected.insert(i);
                    }
                    self.sel_anchor = Some(i);
                    self.last_click = None;
                    return iced::Task::none();
                }
                if self.shift_held {
                    // Shift+click: the anchor..=item range
                    let anchor = self.sel_anchor.unwrap_or(i);
                    self.selected = (anchor.min(i)..=anchor.max(i)).collect();
                    self.last_click = None;
                    return iced::Task::none();
                }
                self.selected.clear();
                self.selected.insert(i);
                self.sel_anchor = Some(i);
                // double-click emulation: two presses on the same row within 400ms
                let now = std::time::Instant::now();
                if let Some((last_i, t)) = self.last_click {
                    if last_i == i && now.duration_since(t).as_millis() < 400 {
                        self.last_click = None;
                        if let Some(e) = self.entries.get(i) {
                            if e.kind == fs::EntryKind::Dir || fs::archive::is_archive_path(&e.path) {
                                let p = e.path.clone();
                                return self.navigate(p);
                            }
                        }
                        return iced::Task::none();
                    }
                }
                self.last_click = Some((i, now));
                iced::Task::none()
            }
            Message::MarqueeBegin { x, y } => {
                self.marquee = Some(Marquee {
                    x0: x,
                    y0: y,
                    x1: x,
                    y1: y,
                    // Ctrl+drag adds to the selection; plain drag replaces
                    base: if self.ctrl_held { self.selected.clone() } else { Default::default() },
                });
                // a plain click on empty space releases the selection
                // (the zero-size marquee intersects nothing)
                self.marquee_update();
                iced::Task::none()
            }
            Message::MarqueeMove { x, y } => {
                if let Some(m) = &mut self.marquee {
                    m.x1 = x;
                    m.y1 = y;
                }
                self.marquee_update();
                iced::Task::none()
            }
            Message::MarqueeEnd => {
                self.marquee_update();
                self.marquee = None;
                iced::Task::none()
            }
            Message::Hover(h) => {
                // TODO(perf): render_hz hover throttle (performance.yaml)
                // was here — disabled, see the input_hz note
                self.hovered = h;
                iced::Task::none()
            }
            Message::HoverExit(i) => {
                if self.hovered == Some(i) {
                    self.hovered = None;
                }
                iced::Task::none()
            }
            Message::NavHover(h) => {
                self.nav_hover = h;
                iced::Task::none()
            }
            Message::NavHoverExit(p) => {
                if self.nav_hover.as_ref() == Some(&p) {
                    self.nav_hover = None;
                }
                iced::Task::none()
            }
            Message::Open(i) => {
                if let Some(e) = self.entries.get(i) {
                    if e.kind == fs::EntryKind::Dir || fs::archive::is_archive_path(&e.path) {
                        let p = e.path.clone();
                        return self.navigate(p);
                    }
                    // regular file: the menu.yaml open backend (§5)
                    if fs::archive::resolve(&self.cwd).is_some() {
                        self.status = "opening inside archives is not supported yet".into();
                    } else if let Err(err) =
                        crate::menu::open_files(&self.menu_cfg, &[e.path.clone()])
                    {
                        self.status = format!("Open: {err}");
                    }
                }
                iced::Task::none()
            }
            Message::ToggleDark => {
                self.dark = !self.dark;
                iced::Task::none()
            }
            Message::TitlebarPress => {
                // Second press of a double-click: skip the drag; the
                // TitlebarDoubleClick event that follows toggles maximize,
                // exactly like the ^ caption button.
                let now = std::time::Instant::now();
                let is_second = self
                    .last_title_press
                    .map(|t| now.duration_since(t).as_millis() < 400)
                    .unwrap_or(false);
                self.last_title_press = Some(now);
                if is_second {
                    return iced::Task::none();
                }
                if let Some(id) = self.window {
                    return iced::window::drag(id);
                }
                iced::Task::none()
            }
            Message::TitlebarDoubleClick => {
                self.maximized = !self.maximized;
                if let Some(id) = self.window {
                    return iced::window::maximize(id, self.maximized);
                }
                iced::Task::none()
            }
            Message::Minimize => {
                if let Some(id) = self.window {
                    return iced::window::minimize(id, true);
                }
                iced::Task::none()
            }
            Message::Maximize => {
                self.maximized = !self.maximized;
                if let Some(id) = self.window {
                    return iced::window::maximize(id, self.maximized);
                }
                iced::Task::none()
            }
            Message::Close => iced::exit(),

            Message::CopySelection => {
                self.copy_or_cut(ClipMode::Copy);
                iced::Task::none()
            }
            Message::CutSelection => {
                self.copy_or_cut(ClipMode::Cut);
                iced::Task::none()
            }
            Message::Paste => {
                self.paste();
                iced::Task::none()
            }
            Message::DeleteSelection => {
                self.delete_selection();
                iced::Task::none()
            }
            Message::RenameSelected => {
                self.start_rename();
                iced::Task::none()
            }
            Message::CommitRename(name) => {
                self.commit_rename(name);
                iced::Task::none()
            }
            Message::CancelRename => {
                self.renaming = None;
                iced::Task::none()
            }
            Message::NewFolder => {
                self.sched.send(Job::Mkdir(self.cwd.clone()));
                iced::Task::none()
            }
            Message::SelectAll => {
                self.selected = (0..self.entries.len()).collect();
                self.status = format!("{} items selected", self.entries.len());
                iced::Task::none()
            }
            Message::SelectNone => {
                self.selected.clear();
                iced::Task::none()
            }
            Message::InvertSelection => {
                let n = self.entries.len();
                let old = std::mem::take(&mut self.selected);
                self.selected = (0..n).filter(|i| !old.contains(i)).collect();
                iced::Task::none()
            }
            Message::SetView(index) => self.set_view_index(index),
            Message::WheelZoom(delta) => self.wheel_zoom(delta),
            Message::ContextSelect(i) => {
                self.context_select(i);
                iced::Task::none()
            }
            Message::ContextBackground => {
                self.hovered = None;
                self.nav_hover = None;
                let entries = if fs::archive::resolve(&self.cwd).is_some() {
                    crate::menu::build_archive(&self.menu_cfg, 0, false, false)
                } else {
                    crate::menu::build_background(&self.menu_cfg, self.clipboard.is_some())
                };
                self.menu = Some(crate::menu::Open {
                    entries,
                    x: self.cursor.x,
                    y: self.cursor.y,
                    hover: None,
                    nav_target: None,
                });
                iced::Task::none()
            }
            Message::ContextNav(path) => {
                self.hovered = None;
                self.nav_hover = None;
                self.menu = Some(crate::menu::Open {
                    entries: crate::menu::build_nav(&self.menu_cfg),
                    x: self.cursor.x,
                    y: self.cursor.y,
                    hover: None,
                    nav_target: Some(path),
                });
                iced::Task::none()
            }
            Message::MenuPick(i) => self.menu_dispatch(i),
            Message::MenuHover(h) => {
                if let Some(m) = &mut self.menu {
                    m.hover = h;
                }
                iced::Task::none()
            }
            Message::CloseMenu => {
                self.menu = None;
                iced::Task::none()
            }
            Message::MenuKey => {
                // keyboard Menu key: anchor at the first selected
                // element's left-bottom corner; no selection = the
                // background menu at the cursor
                match self.selected.iter().next().copied() {
                    Some(i) if i < self.entries.len() => {
                        let (x, y) = self.item_anchor(i);
                        self.context_select_at(i, x, y);
                    }
                    _ => return self.update(Message::ContextBackground),
                }
                iced::Task::none()
            }
            Message::FileScrollTo(target) => {
                let (x, y) = if self.view_cfg.family(self.view_index)
                    == crate::view_step::Family::List
                {
                    self.scroll_x = target.max(0.0);
                    (Some(self.scroll_x), None)
                } else {
                    self.scroll_y = target.max(0.0);
                    (None, Some(self.scroll_y))
                };
                iced::widget::operation::scrollable::scroll_to(
                    crate::folder::file_scrollable_id(),
                    iced::widget::operation::scrollable::AbsoluteOffset { x, y },
                iced::widget::operation::Animation::Instant,
        )
            }
            Message::NavScrollTo(target) => {
                self.nav.scroll_y = target.max(0.0);
                iced::widget::operation::scrollable::scroll_to(
                    crate::nav_pane::nav_scrollable_id(),
                    iced::widget::operation::scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(self.nav.scroll_y),
                    },
                iced::widget::operation::Animation::Instant,
        )
            }
            Message::ToggleSidePane(m) => {
                self.pane = if self.pane == m {
                    crate::pane::Mode::None
                } else {
                    m
                };
                self.pane_key = None; // force a content reload
                iced::Task::none()
            }
            Message::ToggleDetailsCompact => {
                let old = self.details_compact;
                self.details_compact = !old;
                // row heights changed: re-anchor the first visible item
                if self.view_cfg.family(self.view_index) == crate::view_step::Family::Details {
                    return self.anchor_view(self.view_index, old);
                }
                iced::Task::none()
            }
            Message::ColumnSplitBegin(col) => {
                if col < self.details_cols.len() {
                    self.col_drag = Some(ColDrag {
                        col,
                        start_x: None,
                        start_w: self.details_cols[col],
                    });
                }
                iced::Task::none()
            }
            Message::ColumnSplitMove(x) => {
                if let Some(drag) = &mut self.col_drag {
                    // first move after the press anchors the drag origin
                    let start_x = *drag.start_x.get_or_insert(x);
                    // the handle sits at the column's left edge: dragging
                    // left widens the column
                    let d = &self.view_cfg.layouts.details;
                    self.details_cols[drag.col] =
                        (drag.start_w + (start_x - x)).clamp(d.col_min, d.col_max);
                }
                iced::Task::none()
            }
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.request_listing();
                iced::Task::none()
            }
            Message::Ribbon(ev) => {
                // IUICommandHandler::Execute round-trip: the handler queued
                // explorer messages; run them through the MVU loop.
                self.ribbon.dispatch(&ev);
                let msgs = self.ribbon.take_messages();
                let mut tasks = Vec::new();
                for m in msgs {
                    tasks.push(self.update(m));
                }
                iced::Task::batch(tasks)
            }
            Message::TransferTo { paths, dest, cut } => {
                if paths.is_empty() {
                    return iced::Task::none();
                }
                if fs::archive::resolve(&self.cwd).is_some() {
                    self.status = "move/copy inside archives is not supported yet".into();
                    return iced::Task::none();
                }
                let mut ok = 0usize;
                for p in &paths {
                    let Some(name) = p.file_name() else { continue };
                    let dst = dest.join(name);
                    let r = if cut {
                        std::fs::rename(p, &dst).is_ok()
                    } else {
                        copy_rec(p, &dst)
                    };
                    if r {
                        ok += 1;
                    }
                }
                self.status = format!(
                    "{ok} item(s) {} {}",
                    if cut { "moved to" } else { "copied to" },
                    dest.display()
                );
                self.request_listing();
                iced::Task::none()
            }
            Message::RunShell(script) => {
                let expanded = self.expand_placeholders(&script);
                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(expanded)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(_) => {}
                    Err(e) => self.status = format!("shell: {e}"),
                }
                iced::Task::none()
            }
            Message::BlockPress(tab) => {
                self.ribbon.framework().view().select_tab(tab);
                if let Some(id) = self.window {
                    return iced::window::drag(id);
                }
                iced::Task::none()
            }
            Message::StartResize(dir) => {
                if let Some(id) = self.window {
                    return dir.task(id);
                }
                iced::Task::none()
            }
            Message::SetSort(s) => {
                self.sort = s;
                self.apply_sort();
                iced::Task::none()
            }
            Message::DeletePermanent => {
                let paths = self.selected_paths();
                if fs::archive::resolve(&self.cwd).is_some() {
                    self.status = "delete inside archives is not supported yet".into();
                } else {
                    let mut ok = 0usize;
                    for p in &paths {
                        let r = if p.is_dir() {
                            std::fs::remove_dir_all(p)
                        } else {
                            std::fs::remove_file(p)
                        };
                        if r.is_ok() {
                            ok += 1;
                        }
                    }
                    self.status = format!("permanently deleted {ok} item(s)");
                    self.request_listing();
                }
                iced::Task::none()
            }
            Message::ToggleQatDock => {
                let view = self.ribbon.framework().view();
                use crate::ribbon::enums::ControlDock;
                let dock = match view.qat_dock() {
                    ControlDock::Top => ControlDock::Bottom,
                    ControlDock::Bottom => ControlDock::Top,
                };
                view.set_qat_dock(dock);
                iced::Task::none()
            }
            Message::ToggleFrameStyle => {
                self.frame_style = match self.frame_style {
                    FrameStyle::Win8 => FrameStyle::Win10,
                    FrameStyle::Win10 => FrameStyle::Win8,
                };
                iced::Task::none()
            }
            Message::Scrolled { x, y, height } => {
                self.scroll_x = x;
                self.scroll_y = y;
                self.viewport_h = height;
                iced::Task::none()
            }
            Message::WindowResized(w, h) => {
                self.win_w = w;
                if std::env::var_os("EXPLORER_DEBUG").is_some() {
                    eprintln!("WindowResized: logical {w}x{h}, system_scale={}, dpi={:?}",
                        self.system_scale, self.dpi_target);
                }
                iced::Task::none()
            }
            Message::ToggleSelfBorder => {
                self.self_border = !self.self_border;
                if let Some(id) = self.window {
                    return iced::window::toggle_decorations(id);
                }
                iced::Task::none()
            }
            Message::RenameInput(s) => {
                if let Some((_, buf)) = &mut self.renaming {
                    *buf = s;
                }
                iced::Task::none()
            }
            Message::Status(s) => {
                self.status = s;
                iced::Task::none()
            }
            Message::Event(e) => {
                // track the cursor (context-menu placement)
                // TODO(perf): assets/performance.yaml input_hz throttling
                // lived here — disabled (the high CPU on fast mouse moves
                // was not caused by this layer; re-evaluate with a profile)
                if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = &e {
                    self.cursor = *position;
                }
                // splitter drag: track the cursor, end on left release
                if self.bar_split {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            return self.update(Message::BarSplitMove(position.x));
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.bar_split = false;
                            return iced::Task::none();
                        }
                        _ => {}
                    }
                }
                // side-pane splitter drag, same pattern
                if self.pane_split {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            return self.update(Message::PaneSplitMove(position.x));
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.pane_split = false;
                            return iced::Task::none();
                        }
                        _ => {}
                    }
                }
                // nav-pane splitter drag, same pattern
                if self.nav_split {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            return self.update(Message::NavSplitMove(position.x));
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.nav_split = false;
                            return iced::Task::none();
                        }
                        _ => {}
                    }
                }
                // column drag: track the cursor, end on left release
                if self.col_drag.is_some() {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            return self.update(Message::ColumnSplitMove(position.x));
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.col_drag = None;
                            return iced::Task::none();
                        }
                        _ => {}
                    }
                }
                if let iced::Event::Keyboard(
                    iced::keyboard::Event::ModifiersChanged(m),
                ) = &e
                {
                    let ctrl = m.control();
                    self.shift_held = m.shift();
                    if !ctrl && self.ctrl_held {
                        // §7: Ctrl released — reset the delta accumulator
                        self.zoom_acc = 0.0;
                        self.zoom_step_at = None;
                    }
                    self.ctrl_held = ctrl;
                    return iced::Task::none();
                }
                if let Some(m) = crate::handle_key(e, self.renaming.is_some(), self.menu.is_some()) {
                    return self.update(m);
                }
                iced::Task::none()
            }
            Message::BeginBarSplit => {
                self.bar_split = true;
                iced::Task::none()
            }
            Message::BeginPaneSplit => {
                self.pane_split = true;
                iced::Task::none()
            }
            Message::BeginNavSplit => {
                self.nav_split = true;
                iced::Task::none()
            }
            Message::SplitterHover(h) => {
                self.splitter_hover = h;
                iced::Task::none()
            }
            Message::BarSplitMove(x) => {
                let (_, fr, _, fl) = crate::border::frame_insets_pub(self.self_border, self.frame_style);
                let content_w = (self.win_w - fl - fr).max(320.0);
                // search right edge sits at content_w - 6 (band padding); the
                // splitter center is 4px left of the search left edge (half
                // the 4px splitter + 2px row spacing) -> offset 6 + 4 = 10
                let search_w = (content_w - (x - fl) - 10.0).clamp(120.0, (content_w - 280.0).max(120.0));
                self.search_share = (search_w / content_w).clamp(0.08, 0.6);
                iced::Task::none()
            }
            Message::PaneSplitMove(x) => {
                // the pane's right edge is the content area's right edge
                let (_, fr, _, _) =
                    crate::border::frame_insets_pub(self.self_border, self.frame_style);
                let content_w = (self.win_w - fr).max(160.0);
                self.pane_width = crate::pane::clamped(content_w - x, content_w);
                iced::Task::none()
            }
            Message::NavSplitMove(x) => {
                let (_, fr, _, fl) =
                    crate::border::frame_insets_pub(self.self_border, self.frame_style);
                let content_w = self.win_w - fl - fr;
                self.nav_width = (x - fl).clamp(110.0, (content_w * 0.5).max(110.0));
                iced::Task::none()
            }
            Message::NavScrolled { y, height } => {
                self.nav.scroll_y = y;
                self.nav.viewport_h = height;
                iced::Task::none()
            }
            Message::FileAreaResized(_w, h) => {
                self.viewport_h = h;
                iced::Task::none()
            }
            Message::NavAreaResized(_w, h) => {
                self.nav.viewport_h = h;
                iced::Task::none()
            }
            Message::FsEvent(ev) => {
                // a nav listing finished (or volumes arrived) while the
                // tree reveal was waiting for data: re-run it now
                let pending = match &ev {
                    FsEvent::Listed { result, .. } => self.request_reveal && result.is_ok(),
                    FsEvent::Volumes(_) => self.request_reveal,
                    _ => false,
                };
                self.handle_fs_event(ev);
                if pending {
                    return self.nav_reveal_cwd();
                }
                iced::Task::none()
            }
        }
    }
}

/// "3/12/2012 10:20 PM" — Win8 en-US short format (hand-rolled, no chrono).
pub fn format_date(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86400);
    let secs = epoch_secs.rem_euclid(86400);
    let (y, mo, d) = civil_from_days(days);
    let h24 = secs / 3600;
    let mi = (secs % 3600) / 60;
    let (h12, ampm) = if h24 == 0 {
        (12, "AM")
    } else if h24 < 12 {
        (h24, "AM")
    } else if h24 == 12 {
        (12, "PM")
    } else {
        (h24 - 12, "PM")
    };
    format!("{}/{}/{} {}:{:02} {}", mo, d, y, h12, mi, ampm)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn format_size(bytes: u64) -> String {
    const K: u64 = 1024;
    fn dec(v: f64, unit: &str) -> String {
        if v >= 100.0 {
            format!("{:.0} {}", v, unit)
        } else if v >= 10.0 {
            format!("{:.1} {}", v, unit)
        } else {
            format!("{:.2} {}", v, unit)
        }
    }
    if bytes < K {
        format!("{} B", bytes)
    } else if bytes < K * K {
        dec(bytes as f64 / K as f64, "KB")
    } else if bytes < K * K * K {
        dec(bytes as f64 / (K * K) as f64, "MB")
    } else {
        dec(bytes as f64 / (K * K * K) as f64, "GB")
    }
}

pub fn type_label(e: &fs::Entry) -> &'static str {
    if e.is_symlink {
        return "Shortcut";
    }
    if e.is_dir() {
        return "File folder";
    }
    let ext = e.name.rsplit_once('.').map(|(_, x)| x.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "7z" => "7-Zip Archive",
        "zip" => "ZIP Archive",
        "rar" => "WinRAR Archive",
        "tar" => "TAR Archive",
        "gz" | "tgz" => "GZIP Archive",
        "bz2" => "Bzip2 Archive",
        "xz" => "XZ Archive",
        "zst" => "Zstandard Archive",
        "iso" => "ISO Image",
        "png" => "PNG File",
        "jpg" | "jpeg" => "JPEG Image",
        "gif" => "GIF Image",
        "txt" => "Text Document",
        "md" => "Markdown Document",
        "pdf" => "PDF Document",
        "rs" => "Rust Source File",
        "sh" => "Shell Script",
        "mp3" | "flac" | "ogg" => "Audio File",
        "mp4" | "mkv" | "mov" => "Video File",
        _ if fs::archive::is_archive_path(&e.path) => "Archive File",
        _ => "File",
    }
}

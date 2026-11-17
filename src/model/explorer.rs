//! The `Explorer` application state + the `update`/`handle` MVU loop.
//! Per-concern update logic lives in the sibling modules (`folder`,
//! `nav`, `menu`, `dnd`, `ops`, `ribbon`); `handle` below is the
//! dispatcher whose arms delegate to them.

use std::path::{Path, PathBuf};

use crate::fs::{self, FsEvent, Job};

use super::dnd::PressArm;
use super::folder::{ColDrag, ColReorder};
use super::format::type_label;
use super::message::{ClipMode, FrameStyle, Message, NavSection, Pane, SortBy};
use super::nav::NavTree;

/// Default address/search split: the search box takes 180 of the
/// 1152px reference content width.
const DEFAULT_SEARCH_SHARE: f32 = 180.0 / 1152.0;

/// Runtime cap of the back/forward history stacks (the address-bar
/// dropdown lists at most this many entries per side).
const HISTORY_MAX: usize = 20;

/// Widget id of the address edit field (focus/selection tasks).
const ADDRESS_INPUT_ID: &str = "address-input";

/// Widget id of the search edit field (focus task).
const SEARCH_INPUT_ID: &str = "search-input";

/// Push onto a history stack, dropping the oldest entry past
/// [`HISTORY_MAX`].
fn push_history(stack: &mut Vec<PathBuf>, p: PathBuf) {
    stack.push(p);
    if stack.len() > HISTORY_MAX {
        stack.remove(0);
    }
}

/// Narrowest the navigation pane can be dragged to (window-logical px).
const MIN_NAV_W: f32 = 110.0;

/// Rubber-band selection rectangle (file-view content coordinates).
pub struct Marquee {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// Selection snapshot at drag start (Ctrl+drag adds to it).
    pub(crate) base: std::collections::BTreeSet<usize>,
    /// Last cursor position in pane-local coordinates (clamped to the
    /// pane by the folder.rs MarqueeZone): recombined with the live
    /// scroll whenever the content scrolls without a matching move
    /// (wheel, edge auto-scroll), so the far corner keeps tracking the
    /// pointer while the anchor corner rides the content.
    pub(crate) px: f32,
    pub(crate) py: f32,
}

pub struct Explorer {
    pub window: Option<iced::window::Id>,
    pub cwd: PathBuf,
    pub entries: Vec<fs::Entry>,
    pub selected: std::collections::BTreeSet<usize>,
    /// Idle selector (the Windows keyboard-focus item): a border-only
    /// rectangle. Defaults to the first item after every listing, but
    /// stays invisible until an arrow key moves it or a plain
    /// free-space click releases the selection.
    pub(crate) focus_item: Option<usize>,
    /// Whether the idle-selector border is shown.
    pub(crate) focus_shown: bool,
    /// The Details data-column highlight (the keyboard Left/Right
    /// selector on the focused row): an index into the VISIBLE column
    /// list. Purely visual — no selection semantics; follows the focus
    /// on Up/Down, released by a mouse press and the Menu key.
    pub(crate) detail_col_focus: Option<usize>,
    pub hovered: Option<usize>,
    /// Hovered navigation tree row: the path plus a tag that is true
    /// when the hovered row is a Favorites shortcut (nav_pane uses it
    /// to tint only that row when HOVER_TINTS_SAME_PATH is off).
    pub nav_hover: Option<(PathBuf, bool)>,
    /// Nav row highlighted for the open context menu (right-click
    /// selection) — the file-view selection is left untouched.
    pub(crate) nav_selected: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    pub dark: bool,
    /// Disabled ribbon buttons/icons use the gray-scaled-faded style
    /// (see `disabled_style` in ribbon.yaml).
    pub disabled_gray: bool,
    /// Icons come from the system Breeze theme instead of the embedded
    /// explorer pack (View → Options “Breeze icons”).
    pub breeze_icons: bool,
    /// TEMP: rainbow gradient on the folder-view backdrop rectangle.
    pub rainbow: bool,
    /// Window has keyboard focus; when false, selections fade to gray
    /// and the ribbon goes inert.
    pub focused: bool,
    /// Pane that last received a click: its selection keeps the live
    /// colors, the other pane's fades.
    pub active_pane: Pane,
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
    /// Pointer over the back/forward history chevron.
    pub(crate) back_list_hover: bool,
    /// Pointer over the address bar's right-list button (its left
    /// divider line is visible only while hovered).
    pub(crate) right_list_hover: bool,
    /// Hovered breadcrumb member: `(segment index, is_arrow)`. The
    /// text and its arrow are tracked separately — arrow hover greys
    /// the text, text hover lights both.
    pub(crate) crumb_hover: Option<(usize, bool)>,
    /// Breadcrumb segment whose subfolder listing is in flight (the
    /// BarListed event with this path opens the dropdown).
    bar_listing: Option<PathBuf>,
    /// Address edit mode: `Some(buffer)` while the address bar shows an
    /// editable full path.
    pub(crate) address_edit: Option<String>,
    /// The search box is editable (clicked / focused).
    pub(crate) search_active: bool,
    /// Search box buffer (kept when edit mode ends, like Win8).
    pub(crate) search_text: String,
    /// The submitted query — drives the placeholder text in the folder
    /// pane until real search lands.
    pub(crate) search_query: Option<String>,
    pub maximized: bool,
    /// Caption-button hover bits (bit 0 = minimize, 1 = maximize); only
    /// the Win8.1 frame uses them (glyph color swap on hover).
    pub cap_hover: u8,
    /// Ctrl held (drives Ctrl+scroll view zoom over the file view).
    pub ctrl_held: bool,
    /// Shift held (Shift+click range selection).
    pub shift_held: bool,
    /// Range-selection anchor (last plain/Ctrl click).
    pub(crate) sel_anchor: Option<usize>,
    /// Rubber-band selection in progress (folder view, all layouts).
    pub marquee: Option<Marquee>,
    /// Last title-bar press (double-click detection: the 2nd press must not
    /// start a window drag, or the in-flight move scrambles the placement of
    /// the subsequent maximize).
    last_title_press: Option<std::time::Instant>,
    /// Contextual-block press pending click-vs-drag resolution: released
    /// in place activates the tab; moving past the threshold drags the
    /// window instead (without activating).
    block_press: Option<(usize, iced::Point)>,
    /// Self-rendered window border/title bar (off = native decorations).
    pub self_border: bool,
    /// Win8 double border vs Win10 1px border.
    pub frame_style: FrameStyle,
    pub clipboard: Option<(ClipMode, Vec<PathBuf>)>,
    /// Folder view configuration (`assets/folder-view.yaml`,
    /// step sizes, layout metrics, enabled mask.
    pub view_cfg: crate::view_step::ViewCfg,
    /// View step index.
    pub view_index: u8,
    /// Committed view step while a gallery hover preview temporarily
    /// overrides `view_index` (None = no preview active).
    pub preview_view: Option<u8>,
    /// §6 enabled mask (resolved from folder-view.yaml at startup).
    #[allow(dead_code)] // the config is read via view_cfg.mask(); this
    // cache is for the future per-folder masks
    view_mask: u64,
    /// §7 wheel-delta accumulator (reset when Ctrl is released).
    pub(crate) zoom_acc: f32,
    /// §7 rate limit: time of the last zoom step.
    pub(crate) zoom_step_at: Option<std::time::Instant>,
    /// §10 per-folder stored view index (session persistence).
    pub(crate) folder_views: std::collections::HashMap<PathBuf, u8>,
    /// §8.1 Details compact mode (24px rows); compact is the default.
    pub details_compact: bool,
    /// §8.3 icon-top: estimated wrapped label line count per entry
    /// (1..=cap), driving the dynamic row heights. Depends only on the
    /// names and the step's cell width, so it is refreshed when the
    /// entries (or their order) or the view step change — not on pane
    /// resizes.
    pub it_label_lines: Vec<u8>,
    /// Details-view column configuration (`assets/details-view.yaml`,
    /// the header metrics and the available
    /// columns.
    pub detail_cfg: crate::details_view::DetailsViewCfg,
    /// Per-column widths, parallel to `detail_cfg.columns`; adjusted by
    /// the header resizers (each resizer resizes its own column).
    pub details_cols: Vec<f32>,
    /// Per-column visibility, parallel to `detail_cfg.columns` (the
    /// column menu toggles it).
    pub details_col_on: Vec<bool>,
    /// Details column display order (a permutation of
    /// `detail_cfg.columns` indices; the column-menu visibility flags
    /// filter it). The header reorder drag rewrites it on release.
    pub details_col_order: Vec<usize>,
    /// Details column drag in progress.
    pub(crate) col_drag: Option<ColDrag>,
    /// Details column header reorder drag in progress.
    pub(crate) col_reorder: Option<ColReorder>,
    /// Armed Details header press: (column, cursor at press). Travel
    /// past the drag threshold becomes the reorder drag; a plain
    /// release sorts.
    pub(crate) col_arm: Option<(usize, iced::Point)>,
    /// Hovered Details column header (index into `detail_cfg.columns`).
    pub(crate) col_hovered: Option<usize>,
    /// Details column header held pressed (drives the 75% blue tint).
    pub(crate) col_pressed: Option<usize>,
    pub sort: SortBy,
    /// Sort direction (true = ascending).
    pub sort_asc: bool,
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
    /// Measured window height (resize events) for the frame strip
    /// layout (the Win8.1 frame tiles its bottom strips by y).
    pub win_h: f32,
    /// Inline rename: (row index, text buffer).
    pub renaming: Option<(usize, String)>,
    /// Context menu configuration (assets/menu.yaml) + the open menu.
    pub menu_cfg: crate::menu::MenuCfg,
    pub menu: Option<crate::menu::Open>,
    /// Last known cursor position (window-logical; menu placement).
    pub(crate) cursor: iced::Point,
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
    /// Last free-space (empty-area) press: a second press within
    /// theme.yaml `double_click_ms` is a double-click, which hides
    /// the idle selector where it is until the next keyboard
    /// navigation shows it again.
    pub(crate) last_free_click: Option<std::time::Instant>,
    pub loading: bool,
    pub volumes: Vec<fs::Volume>,
    /// Navigation pane tree (expanded nodes + cached children).
    pub nav: NavTree,
    /// The cwd changed but volumes/listings were not known yet: re-run
    /// the nav-tree reveal when the data arrives.
    pub(crate) request_reveal: bool,
    pub(crate) sched: fs::Scheduler,
    pending_rename: Option<String>,
    /// Left-press on a file-view row that may become a drag.
    pub(crate) press: Option<PressArm>,
    /// The active drag session manager.
    pub dnd: crate::dnd::Session,
    /// Wayland DnD side-channel (drag-out payloads + drag-in offers;
    /// `None` off-Wayland or before the window exists).
    pub dnd_wl: Option<crate::dnd_wayland::Handle>,
    /// Last `(location icon, dark, system icons)` pushed to the
    /// compositor's toplevel-icon protocol; None until the first push.
    wl_icon_key: Option<(&'static str, bool, bool)>,
    /// True while a native Wayland drag started here is live — the
    /// compositor shows the drag image, so the in-window overlays
    /// (icon badge + caption) must stay hidden.
    pub dnd_wl_drag: bool,
    /// An ambiguous drop (Shift / right-button drag) released and
    /// waiting for the choice-menu pick.
    pub(crate) pending_drop: Option<super::dnd::PendingDrop>,
    /// True while `self.menu` is hosted by the off-window Wayland popup
    /// (xdg_popup) instead of the in-window overlay.
    pub menu_popup: bool,
    /// Alt held (Alt-drop = create a link).
    pub(crate) alt_held: bool,
}

impl Explorer {
    pub fn new() -> Self {
        let (sched, ev_rx) = fs::spawn();
        let _ = crate::FS_STREAM_RX.set(std::sync::Mutex::new(Some(ev_rx)));
        // the DnD event channel must exist before the app starts so the
        // subscription stream picks it up; the thread connects later
        let dnd_tx = crate::dnd_wayland::init_channel();
        let _ = crate::DND_EVENT_TX.set(std::sync::Mutex::new(Some(dnd_tx)));
        // Initial path: first positional arg (flags and their values are
        // skipped — see main.rs for the flag set).
        let mut cwd = None;
        let mut skip_next = false;
        for arg in std::env::args().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if arg == "--render" || arg == "--dpi" {
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
        let detail_cfg = crate::details_view::DetailsViewCfg::load();
        let details_cols = detail_cfg.columns.iter().map(|c| c.width).collect();
        let details_col_on = detail_cfg.columns.iter().map(|c| c.on).collect();
        let details_col_order: Vec<usize> = (0..detail_cfg.columns.len()).collect();
        let mut s = Self {
            window: None,
            cwd,
            entries: Vec::new(),
            selected: Default::default(),
            focus_item: None,
            focus_shown: false,
            detail_col_focus: None,
            hovered: None,
            nav_hover: None,
            nav_selected: None,
            back: Vec::new(),
            forward: Vec::new(),
            dnd_wl: None,
            wl_icon_key: None,
            dnd_wl_drag: false,
            menu_popup: false,
            dark: false,
            // the ribbon.yaml `disabled_style` setting is read once the
            // ribbon framework exists (just below the Self literal)
            disabled_gray: false,
            // icons.yaml `theme:` may start on the system icon theme
            breeze_icons: crate::icon::system_active(),
            rainbow: false,
            focused: true,
            active_pane: Pane::Folder,
            dpi_target: std::env::var("EXPLORER_DPI")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v: &f32| *v > 0.0),
            system_scale: 0.0,
            search_share: DEFAULT_SEARCH_SHARE,
            bar_split: false,
            splitter_hover: false,
            back_list_hover: false,
            right_list_hover: false,
            crumb_hover: None,
            bar_listing: None,
            address_edit: None,
            search_active: false,
            search_text: String::new(),
            search_query: None,
            maximized: false,
            cap_hover: 0,
            ctrl_held: false,
            shift_held: false,
            sel_anchor: None,
            marquee: None,
            last_title_press: None,
            block_press: None,
            self_border: true,
            // the Win8.1 frame is the default (the View → Options
            // toggle still switches between it and the Win8 frame)
            frame_style: FrameStyle::Win81,
            clipboard: None,
            view_cfg,
            view_index,
            preview_view: None,
            view_mask,
            zoom_acc: 0.0,
            zoom_step_at: None,
            folder_views: Default::default(),
            details_compact: true,
            it_label_lines: Vec::new(),
            detail_cfg,
            details_cols,
            details_col_on,
            details_col_order,
            col_drag: None,
            col_reorder: None,
            col_arm: None,
            col_hovered: None,
            col_pressed: None,
            sort: SortBy::Name,
            sort_asc: true,
            show_hidden: false,
            fonts: crate::fonts::FontSizes::load(),
            ribbon: crate::ribbon::Ribbon::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            viewport_h: 600.0,
            win_w: 1152.0,
            win_h: 720.0,
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
            last_free_click: None,
            loading: false,
            volumes: Vec::new(),
            nav: NavTree::new(),
            request_reveal: false,
            sched,
            pending_rename: None,
            press: None,
            dnd: crate::dnd::Session::new(),
            pending_drop: None,
            alt_held: false,
        };
        // initial value comes from the `disabled_style` markup setting
        s.disabled_gray = matches!(
            s.ribbon.framework().spec(),
            Ok(spec)
                if spec.disabled_style
                    == crate::ribbon::spec::DisabledStyle::GrayScaledFaded
        );
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

    /// The absolute UI scale: how many device pixels one app unit
    /// occupies. With `--dpi` it is the target factor; without it the
    /// full system scale applies (iced total = system x multiplier and
    /// `dpi_multiplier()` is 1.0 then). Returns 1.0 before the system
    /// scale is learned. This is `scale_dpi` for the border-width
    /// factor.
    pub fn scale_dpi(&self) -> f32 {
        if let Some(t) = self.dpi_target.filter(|t| *t > 0.0) {
            t
        } else if self.system_scale > 0.0 {
            self.system_scale
        } else {
            1.0
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
        } else if std::env::var_os("HOME").map(PathBuf::from).as_deref() == Some(self.cwd.as_path())
        {
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
            iced::Theme::custom("win8-dark", crate::ui_common::skin::dark().palette())
        } else {
            iced::Theme::custom("win8-light", crate::ui_common::skin::light().palette())
        }
    }

    fn request_listing(&mut self) {
        self.sched.send(Job::List(self.cwd.clone()));
        self.loading = true;
    }

    pub(crate) fn navigate(&mut self, path: PathBuf) -> iced::Task<Message> {
        if path == self.cwd {
            return iced::Task::none();
        }
        push_history(&mut self.back, self.cwd.clone());
        self.forward.clear();
        self.cwd = path;
        self.selected.clear();
        self.renaming = None;
        self.press = None;
        self.dnd.take(); // a navigation cancels any active drag
        self.restore_folder_view();
        self.request_listing();
        self.nav_reveal_cwd()
    }

    /// Window-logical anchor for the address-bar dropdowns: the cursor
    /// x, just below the navigation band (the
    /// selector shows "just below the address bar, not like a menu").
    fn bar_anchor(&self) -> (f32, f32) {
        let (ft, fr, _, fl) =
            crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
        let title_h = if self.self_border {
            crate::border::titlebar_h(self.frame_style, self.maximized)
        } else {
            0.0
        };
        let content_w = self.win_w - fl - fr;
        let mult = self.dpi_multiplier();
        let (min_ribbon_w, _, _, _) = self.ribbon.sacrifice();
        let ribbon_h = if content_w >= min_ribbon_w * mult {
            crate::ribbon::widget::layout::ribbon_height(&self.ribbon.framework().view())
        } else {
            0.0
        };
        (self.cursor.x, ft + title_h + ribbon_h + self.ribbon.band_height())
    }

    /// The history dropdown (back_list chevron) and the location
    /// selector (right_list): the back stack (most recent first), a
    /// separator, then the forward stack (next first) — picks index
    /// into `BarMenu.paths`.
    fn history_menu(&mut self, kind: crate::menu::BarMenuKind) {
        if self.back.is_empty() && self.forward.is_empty() {
            self.status = "no history yet".into();
            return;
        }
        let mut entries = Vec::new();
        let mut paths = Vec::new();
        for p in self.back.iter().rev() {
            entries.push(crate::menu::Entry::Item(crate::menu::Item::plain(
                "bar",
                p.to_string_lossy().into_owned(),
            )));
            paths.push(p.clone());
        }
        // separator between the back and the forward sections (a dummy
        // path keeps `paths` parallel to `entries` for the dispatch)
        if !self.back.is_empty() && !self.forward.is_empty() {
            entries.push(crate::menu::Entry::Sep);
            paths.push(PathBuf::new());
        }
        for p in self.forward.iter().rev() {
            entries.push(crate::menu::Entry::Item(crate::menu::Item::plain(
                "bar",
                p.to_string_lossy().into_owned(),
            )));
            paths.push(p.clone());
        }
        let (x, y) = self.bar_anchor();
        self.menu = Some(crate::menu::Open {
            entries,
            x,
            y,
            hover: None,
            nav_target: None,
            bar: Some(crate::menu::BarMenu {
                kind,
                prefix: PathBuf::new(),
                paths,
            }),
            scroll: None,
        });
    }

    /// The subfolder dropdown for a breadcrumb segment: one entry per
    /// child directory (the listing ran off the UI thread in the
    /// CrumbArrow task).
    fn crumb_menu(&mut self, prefix: PathBuf, dirs: Vec<PathBuf>) {
        if dirs.is_empty() {
            self.status = format!("{} has no subfolders", prefix.display());
            return;
        }
        let entries = dirs
            .iter()
            .map(|p| {
                crate::menu::Entry::Item(crate::menu::Item::plain(
                    "bar",
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string()),
                ))
            })
            .collect();
        let (x, y) = self.bar_anchor();
        self.menu = Some(crate::menu::Open {
            entries,
            x,
            y,
            hover: None,
            nav_target: None,
            bar: Some(crate::menu::BarMenu {
                kind: crate::menu::BarMenuKind::Subdirs,
                prefix,
                paths: dirs,
            }),
            scroll: None,
        });
    }

    /// History-dropdown pick: jump to the entry while preserving the
    /// chronology around it (unlike a plain navigate, which truncates
    /// the forward stack). `navigate_to` only swaps `cwd` + reloads.
    pub(crate) fn history_nav(&mut self, target: PathBuf) -> iced::Task<Message> {
        if let Some(pos) = self.back.iter().rposition(|p| p == &target) {
            // rewind: the current folder and the entries newer than the
            // target become the forward stack, in chronology order —
            // existing forward below, then cwd, then the re-forwarded
            // entries on top (the forward pop order reads them first)
            self.forward.push(self.cwd.clone());
            while self.back.len() > pos + 1 {
                let p = self.back.pop().expect("len checked above");
                self.forward.push(p);
            }
            let p = self.back.pop().expect("pos checked above");
            return self.navigate_to(p);
        }
        if let Some(pos) = self.forward.iter().rposition(|p| p == &target) {
            // entries chronologically after the target are dropped;
            // the cursor and the in-between entries join the back stack
            self.back.push(self.cwd.clone());
            for j in (pos + 1..self.forward.len()).rev() {
                let p = self.forward.remove(j);
                self.back.push(p);
            }
            let p = self.forward.remove(pos);
            return self.navigate_to(p);
        }
        self.navigate(target)
    }

    /// `navigate` without touching the history stacks (the stacks were
    /// already rearranged by `history_nav`).
    fn navigate_to(&mut self, path: PathBuf) -> iced::Task<Message> {
        if path == self.cwd {
            return iced::Task::none();
        }
        self.cwd = path;
        self.selected.clear();
        self.renaming = None;
        self.press = None;
        self.dnd.take();
        self.restore_folder_view();
        self.request_listing();
        self.nav_reveal_cwd()
    }

    /// (nav pane width, side pane width) actually shown at the current
    /// window size — the yaml sacrifice rules: the side pane hides below
    /// `pane_below` (before the folder view gets squeezed), the nav pane
    /// shrinks below `nav_resize_below` and hides below `nav_below`.
    /// ui.rs lays out with exactly these widths.
    pub(crate) fn layout_widths(&self) -> (f32, f32) {
        let (_, fr, _, fl) = crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
        let content_w = self.win_w - fl - fr;
        let mult = self.dpi_multiplier();
        let (_, min_nav_w, nav_resize_below, _) = self.ribbon.sacrifice();
        let nav_w = if content_w >= min_nav_w * mult {
            if content_w >= nav_resize_below * mult {
                self.nav_width
            } else {
                (content_w * self.nav_width / (nav_resize_below * mult)).max(MIN_NAV_W)
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
    pub(crate) fn file_pane_width(&self) -> f32 {
        let (_, fr, _, fl) = crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
        let content_w = self.win_w - fl - fr;
        let (nav_w, side_w) = self.layout_widths();
        // nav visible: the 4px boundary strip (2px colorless, 1px
        // separator, 1px colorless) doubles as the splitter; nav
        // hidden: the standalone gap spacer; the pane splitter/separator
        // only with the side pane
        let chrome = if nav_w > 0.0 {
            4.0 * crate::theme::border_w(1.0)
        } else {
            self.ribbon.nav_gap()
        } + if side_w > 0.0 { 4.0 + 1.0 } else { 0.0 };
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
            &self.it_label_lines,
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

    /// Sort entries: directories first, then by the active key in the
    /// active direction (`sort_asc`).
    pub fn apply_sort(&mut self) {
        let sort = self.sort;
        let asc = self.sort_asc;
        self.entries.sort_by(|a, b| {
            let ad = a.kind == fs::EntryKind::Dir;
            let bd = b.kind == fs::EntryKind::Dir;
            if ad != bd {
                return bd.cmp(&ad);
            }
            let ord = match sort {
                SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Type => type_label(a).cmp(type_label(b)),
                SortBy::Modified => a.modified.cmp(&b.modified),
            };
            if asc { ord } else { ord.reverse() }
        });
        // the cache is indexed by position — reordering invalidates it
        self.refresh_it_label_lines();
    }

    /// (Re)compute the cached icon-top label line counts for the
    /// current entries and view step (empty for other families).
    pub(crate) fn refresh_it_label_lines(&mut self) {
        self.it_label_lines = self.estimate_it_label_lines(self.view_index);
    }

    /// Label line counts for a step index, uncached.
    fn estimate_it_label_lines(&self, index: u8) -> Vec<u8> {
        if self.view_cfg.family(index) != crate::view_step::Family::IconTop {
            return Vec::new();
        }
        let m = self.view_cfg.icon_top(self.view_cfg.icon_px(index));
        let label_w = (m.cell_w - self.view_cfg.icon_top.label_pad).max(1.0);
        let font = self.view_cfg.style.font_size;
        self.entries
            .iter()
            .map(|e| {
                crate::text_metrics::hard_wrap_count(
                    &e.name,
                    label_w,
                    m.lines as usize,
                    font,
                    crate::fonts::primary(),
                ) as u8
            })
            .collect()
    }

    /// Label line counts for any step index: the cache when it matches
    /// the current one, otherwise a fresh estimate (used for the OLD
    /// layout while §9 view-switch anchoring).
    pub(crate) fn it_lines_for(&self, index: u8) -> Vec<u8> {
        if index == self.view_index {
            self.it_label_lines.clone()
        } else {
            self.estimate_it_label_lines(index)
        }
    }

    pub(crate) fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .map(|e| e.path.clone())
            .collect()
    }

    fn handle_fs_event(&mut self, ev: FsEvent) {
        match ev {
            FsEvent::BarListed { path, result } => {
                // only the listing the bar is waiting for opens a menu
                if self.bar_listing.take().as_deref() != Some(path.as_path()) {
                    return;
                }
                if let Ok(entries) = &result {
                    self.nav.store(&path, entries, self.show_hidden, &self.sched);
                    let show_hidden = self.show_hidden;
                    let mut dirs: Vec<PathBuf> = entries
                        .iter()
                        .filter(|e| e.is_dir() && (show_hidden || !e.is_hidden))
                        .map(|e| e.path.clone())
                        .collect();
                    dirs.sort_by_key(|p| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            .unwrap_or_default()
                    });
                    self.crumb_menu(path, dirs);
                } else if let Err(e) = &result {
                    self.status = e.clone();
                }
            }
            FsEvent::Listed { path, result } => {
                // every listing also feeds the nav tree cache (folders/archives only)
                if let Ok(entries) = &result {
                    self.nav.store(&path, entries, self.show_hidden, &self.sched);
                }
                if path == self.cwd {
                    self.loading = false;
                    match result {
                        Ok(entries) => {
                            let show_hidden = self.show_hidden;
                            self.entries = entries
                                .into_iter()
                                .filter(|e| show_hidden || !e.is_hidden)
                                .collect();
                            self.apply_sort();
                            self.selected.clear();
                            // the idle selector restarts on the first
                            // item, invisible until keyboard navigation
                            self.focus_item = (!self.entries.is_empty()).then_some(0);
                            self.focus_shown = false;
                            self.hovered = None;
                            self.renaming = None;
                            if let Ok(pat) = std::env::var("EXPLORER_DEBUG_SELECT") {
                                for (i, e) in self.entries.iter().enumerate() {
                                    if e.name.contains(&pat) {
                                        self.selected.insert(i);
                                    }
                                }
                            }
                            if let Some(name) = self.pending_rename.take()
                                && let Some(i) = self.entries.iter().position(|e| e.name == name)
                            {
                                self.selected.insert(i);
                                self.focus_item = Some(i);
                                self.renaming = Some((i, name));
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
            FsEvent::Trashed { result } | FsEvent::Deleted { result } => {
                if let Err(e) = result {
                    self.status = format!("Delete failed: {e}");
                }
                self.request_listing();
            }
            FsEvent::Linked { result } => {
                match result {
                    Ok(n) => {
                        self.status = format!("Created {n} link{}", if n == 1 { "" } else { "s" })
                    }
                    Err(e) => self.status = format!("Link failed: {e}"),
                }
                self.request_listing();
            }
            FsEvent::OpProgress {
                done_bytes,
                total_bytes,
                current,
            } => {
                let pct = (done_bytes * 100).checked_div(total_bytes).unwrap_or(0);
                self.status = format!(
                    "Copying {} — {}%",
                    current
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
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

    /// Keep the side-pane preview cache in sync with the selection
    /// (cheap key compare; reloads only when the selection changes).
    fn sync_pane(&mut self) {
        if self.pane == crate::pane::Mode::None {
            return;
        }
        let sel = self
            .selected
            .iter()
            .next()
            .and_then(|&i| self.entries.get(i));
        let key = sel.map(|e| e.path.clone());
        if key != self.pane_key {
            self.pane_key = key;
            self.pane_content = match sel {
                Some(e) => crate::pane::load(&e.path, e.is_dir()),
                None => crate::pane::Content::None,
            };
        }
    }

    /// True while any pointer-drag interaction is live: the raw
    /// cursor stream must reach the model then (drag-initiation
    /// threshold, splitter drags, the Wayland DnD handoff). While idle,
    /// main.rs drops `CursorMoved` from the event subscription — every
    /// message would otherwise force a full UI rebuild + layout at
    /// pointer rate. Press-time positions arrive via `CursorAt`
    /// (the `cursor_seed` wrapper) instead.
    pub(crate) fn drag_live(&self) -> bool {
        self.press.is_some()
            || self.marquee.is_some()
            || self.bar_split
            || self.pane_split
            || self.nav_split
            || self.block_press.is_some()
            || self.dnd.active()
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        // ribbon-originated events must not re-run contextual
        // auto-activation (the user just clicked a tab)
        let auto = !matches!(message, Message::Ribbon(_));
        let task = self.handle(message);
        self.sync_pane();
        self.sync_wl_icon();
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

    /// Push the current location icon to the compositor (taskbar /
    /// window-switcher icon via xdg_toplevel_icon_v1) when it changed —
    /// on navigation, theme, or icon-pack switches. No-op off Wayland
    /// and on compositors without the global (the side-channel thread
    /// ignores the command; rasterizing two icons is only paid on an
    /// actual change, not per event).
    fn sync_wl_icon(&mut self) {
        let key = (
            self.location_icon(),
            self.dark,
            crate::icon::system_active(),
        );
        if self.wl_icon_key == Some(key) {
            return;
        }
        // only mark as sent AFTER the push: the side-channel spawns
        // asynchronously (Message::WlHandles), and an early update must
        // not consume the key without delivering it
        let Some(h) = &self.dnd_wl else {
            return;
        };
        self.wl_icon_key = Some(key);
        let skin = if self.dark {
            crate::ui_common::skin::dark()
        } else {
            crate::ui_common::skin::light()
        };
        h.set_icon(crate::icon_wayland::paint_toplevel_icon(key.0, &skin));
    }

    fn handle(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window = Some(id);
                // learn the scale and grab the raw Wayland handles; the
                // latter spawns the DnD side-channel (Message::WlHandles)
                iced::Task::batch([
                    iced::window::scale_factor(id).map(Message::SystemScale),
                    iced::window::raw_handle(id).map(Message::WlHandles),
                ])
            }
            Message::WlHandles(handles) => {
                let tx = crate::DND_EVENT_TX
                    .get()
                    .and_then(|m| m.lock().unwrap().take());
                if let (Some(h), Some(tx)) = (handles, tx) {
                    self.dnd_wl = crate::dnd_wayland::spawn(h, tx);
                }
                iced::Task::none()
            }
            Message::Dnd(ev) => {
                use crate::dnd_wayland::Event as E;
                match ev {
                    E::Hover { mimes } => {
                        if !self.dnd.active() {
                            self.status = if mimes.iter().any(|m| m == "text/uri-list") {
                                format!("Release to drop files into {}", self.cwd.display())
                            } else {
                                format!("Incoming drag: {}", mimes.join(", "))
                            };
                        }
                    }
                    E::HoverLeft => {
                        if !self.dnd.active() {
                            self.status.clear();
                        }
                    }
                    E::Dropped { mime, data } => self.wayland_drop_received(mime, data),
                    E::DragFinished { performed } => {
                        self.dnd_wl_drag = false;
                        self.dnd.take();
                        self.status = if performed {
                            "Drop completed".into()
                        } else {
                            "Drop cancelled".into()
                        };
                    }
                    // --- off-window menu popup input ---
                    E::PopupMotion { x, y } => {
                        if std::env::var_os("EXPLORER_DEBUG").is_some() {
                            eprintln!("[popup] motion {x:.0} {y:.0}");
                        }
                        if self.menu_popup
                            && let Some(open) = &mut self.menu
                        {
                            let hit =
                                crate::menu::hover_at(&open.entries, y, &self.menu_cfg.layout);
                            if hit != open.hover {
                                open.hover = hit;
                                self.repaint_menu_popup();
                            }
                        }
                    }
                    E::PopupPress { x, y } => {
                        if std::env::var_os("EXPLORER_DEBUG").is_some() {
                            eprintln!("[popup] press {x:.0} {y:.0}");
                        }
                        if self.menu_popup
                            && let Some(open) = &self.menu
                        {
                            let hit =
                                crate::menu::hover_at(&open.entries, y, &self.menu_cfg.layout);
                            let enabled = hit.is_some_and(|i| {
                                matches!(
                                    open.entries.get(i),
                                    Some(crate::menu::Entry::Item(it)) if it.enabled
                                )
                            });
                            if enabled
                                && let Some(i) = hit
                            {
                                // dispatch FIRST — it takes the menu
                                // (and closes the popup at its top);
                                // closing here would leave dispatch with
                                // self.menu == None
                                return self.menu_dispatch(i);
                            }
                        }
                    }
                    E::PopupDone => self.menu_close(),
                    E::PopupKey { keysym } => {
                        const ESCAPE: u32 = 0xff1b;
                        if keysym == ESCAPE {
                            self.menu_close();
                        }
                    }
                }
                iced::Task::none()
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
                if self.dpi_target.is_some()
                    && let Some(id) = self.window
                {
                    return iced::window::size(id)
                        .map(|sz| Message::WindowResized(sz.width, sz.height));
                }
                iced::Task::none()
            }
            Message::Navigate(p) => self.navigate(p),
            Message::Back => {
                if let Some(p) = self.back.pop() {
                    push_history(&mut self.forward, self.cwd.clone());
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
                    push_history(&mut self.back, self.cwd.clone());
                    self.cwd = p;
                    self.selected.clear();
                    self.renaming = None;
                    self.restore_folder_view();
                    self.request_listing();
                }
                iced::Task::none()
            }
            Message::Up => match self.cwd.parent().map(Path::to_path_buf) {
                Some(p) => self.navigate(p),
                None => iced::Task::none(),
            },
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
                    self.nav.drop_subtree(&path);
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
            // folder-view selection gesture (model/folder.rs)
            Message::Select(i) => self.select_press(i),
            Message::FocusMove(dir) => self.focus_move(dir),
            Message::FocusSpace => self.focus_space(),
            Message::MarqueeBegin { x, y } => {
                // a press on the pane's empty space focuses the folder
                // pane, same as pressing an item
                self.active_pane = Pane::Folder;
                // a plain click on free space releases the selection;
                // the idle selector then shows its border on the first
                // released item (Windows free-space click)
                let released = if self.ctrl_held {
                    None
                } else {
                    self.selected.iter().next().copied()
                };
                // a second free-space press within double_click_ms is a
                // double-click: it hides the idle selector instead of
                // showing the rectangle
                let now = std::time::Instant::now();
                let double_free = self.last_free_click.is_some_and(|t| {
                    now.duration_since(t).as_millis()
                        < u128::from(crate::theme::double_click_ms())
                });
                self.last_free_click = Some(now);
                // the drag owns the pointer until MarqueeEnd: drop the
                // hover chrome (folder items, details column headers)
                // and let the hover handlers below ignore updates, so
                // only the rubber band reacts to the cursor
                self.hovered = None;
                self.col_hovered = None;
                // a manual free-space press also releases the
                // data-column highlight (keyboard releases go through
                // the arrow keys)
                self.detail_col_focus = None;
                self.marquee = Some(Marquee {
                    x0: x,
                    y0: y,
                    x1: x,
                    y1: y,
                    // pane-local cursor = content coord − live scroll
                    // (both messages add the scroll at publish time)
                    px: x - self.scroll_x,
                    py: y - self.scroll_y,
                    // Ctrl+drag adds to the selection; plain drag replaces
                    base: if self.ctrl_held {
                        self.selected.clone()
                    } else {
                        Default::default()
                    },
                });
                // a plain click on empty space releases the selection
                // (the zero-size marquee intersects nothing)
                self.marquee_update();
                if double_free {
                    // the double-click hides the idle selector where it
                    // is (a temporary hider); the next keyboard
                    // navigation (arrow keys, Space) shows it again
                    self.focus_shown = false;
                    // previously the double-click released the idle
                    // selector back to its default: the rectangle on
                    // the first item, hidden
                    // self.focus_item = (!self.entries.is_empty()).then_some(0);
                    // self.focus_shown = false;
                } else if let Some(i) = released
                    && self.selected.is_empty()
                {
                    self.focus_item = Some(i);
                    self.focus_shown = true;
                }
                iced::Task::none()
            }
            Message::MarqueeMove { x, y } => {
                if let Some(m) = &mut self.marquee {
                    m.x1 = x;
                    m.y1 = y;
                    m.px = x - self.scroll_x;
                    m.py = y - self.scroll_y;
                }
                self.marquee_update();
                iced::Task::none()
            }
            Message::MarqueeEnd => {
                self.marquee_update();
                self.marquee = None;
                iced::Task::none()
            }
            Message::MarqueeAutoScroll => self.marquee_auto_scroll(),
            Message::Hover(h) => {
                // an open menu is modal for hover only: the items under
                // it must not highlight while it is hovered (clicks and
                // right-clicks still reach them as usual). The same
                // holds while a rubber-band drag owns the pointer.
                if self.menu.is_none() && self.marquee.is_none() {
                    // TODO(perf): render_hz hover throttle (performance.yaml)
                    // was here — disabled, see the input_hz note
                    self.hovered = h;
                }
                iced::Task::none()
            }
            Message::HoverExit(i) => {
                if self.menu.is_none() && self.marquee.is_none() && self.hovered == Some(i) {
                    self.hovered = None;
                }
                iced::Task::none()
            }
            Message::NavHover(h) => {
                if self.menu.is_none() && self.marquee.is_none() {
                    self.nav_hover = h;
                }
                iced::Task::none()
            }
            Message::NavHoverExit(p) => {
                if self.menu.is_none() && self.marquee.is_none()
                    && self.nav_hover.as_ref().is_some_and(|(h, _)| h == &p)
                {
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
                        crate::menu::open_files(&self.menu_cfg, std::slice::from_ref(&e.path))
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
            Message::ToggleGrayDisabled => {
                self.disabled_gray = !self.disabled_gray;
                iced::Task::none()
            }
            Message::ToggleIconPack => {
                self.breeze_icons = !self.breeze_icons;
                crate::icon::set_system(self.breeze_icons);
                iced::Task::none()
            }
            Message::TitlebarPress => {
                // Second press of a double-click: skip the drag; the
                // TitlebarDoubleClick event that follows toggles maximize,
                // exactly like the ^ caption button.
                let now = std::time::Instant::now();
                let is_second = self
                    .last_title_press
                    .map(|t| {
                        now.duration_since(t).as_millis()
                            < u128::from(crate::theme::double_click_ms())
                    })
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
            Message::PreviewView(index) => match index {
                Some(i) => self.preview_view_index(i),
                None => self.end_view_preview(),
            },
            Message::WheelZoom(delta) => self.wheel_zoom(delta),
            Message::ContextSelect(i) => {
                self.active_pane = Pane::Folder;
                self.context_select(i);
                iced::Task::none()
            }
            Message::RightPress(i) => self.right_press(i),
            Message::NavPress(p) => {
                self.active_pane = Pane::Nav;
                self.nav_selected = None;
                self.navigate(p)
            }
            Message::ContextBackground => {
                self.hovered = None;
                self.nav_hover = None;
                self.nav_selected = None;
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
                    bar: None,
                    scroll: None,
                });
                self.menu_popup_open();
                iced::Task::none()
            }
            Message::ContextNav(path) => {
                // the nav pane takes focus; the row shows the selection
                // colors while its menu is open (nothing else changes)
                self.active_pane = Pane::Nav;
                self.hovered = None;
                self.nav_hover = None;
                self.nav_selected = Some(path.clone());
                self.menu = Some(crate::menu::Open {
                    entries: crate::menu::build_nav(&self.menu_cfg),
                    x: self.cursor.x,
                    y: self.cursor.y,
                    hover: None,
                    nav_target: Some(path),
                    bar: None,
                    scroll: None,
                });
                self.menu_popup_open();
                iced::Task::none()
            }
            Message::ScrollbarMenu { pos, axis, targets, on_scroll } => {
                self.scrollbar_menu(pos, axis, targets, on_scroll);
                iced::Task::none()
            }
            Message::MenuPick(i) => self.menu_dispatch(i),
            Message::MenuHover(h) => {
                if self.marquee.is_none()
                    && let Some(m) = &mut self.menu
                    && m.hover != h
                {
                    m.hover = h;
                }
                iced::Task::none()
            }
            Message::CloseMenu => {
                self.pending_drop = None;
                self.menu_close();
                iced::Task::none()
            }
            Message::MenuKey => {
                // release the Details data-column highlight first:
                // the menu belongs to the plain selection
                self.detail_col_focus = None;
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
                let (x, y) =
                    if self.view_cfg.family(self.view_index) == crate::view_step::Family::List {
                        self.scroll_x = target.max(0.0);
                        (Some(self.scroll_x), None)
                    } else {
                        self.scroll_y = target.max(0.0);
                        (None, Some(self.scroll_y))
                    };
                // a wheel roll / scrollbar step during a rubber-band
                // drag scrolls under a stationary cursor: re-anchor the
                // far corner so the band keeps tracking the pointer
                self.marquee_follow_scroll();
                iced::widget::operation::scrollable::scroll_to(
                    crate::folder::file_scrollable_id(),
                    iced::widget::operation::scrollable::AbsoluteOffset { x, y },
                    iced::widget::operation::Animation::Instant,
                )
            }
            Message::FileHScrollTo(target) => {
                self.scroll_x = target.max(0.0);
                self.marquee_follow_scroll();
                iced::widget::operation::scrollable::scroll_to(
                    crate::folder::details_h_scrollable_id(),
                    iced::widget::operation::scrollable::AbsoluteOffset {
                        x: Some(self.scroll_x),
                        y: None,
                    },
                    iced::widget::operation::Animation::Instant,
                )
            }
            Message::NavScrollTo(target) => {
                // the tree scrolls line by line: snap to whole row
                // heights so the top row is always complete (a clipped
                // row at the bottom edge is fine). Sub-row deltas
                // accumulate so slow touchpad streams still move.
                let line = self.ribbon.metrics().1.max(1.0);
                let raw = target + self.nav.scroll_rem;
                let snapped = (raw / line).round() * line;
                self.nav.scroll_rem = raw - snapped;
                self.nav.scroll_y = snapped.max(0.0);
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
            // Details column drag (model/folder.rs)
            Message::ColumnSplitBegin(col) => self.column_split_begin(col),
            Message::ColumnSplitMove(x) => self.column_split_move(x),
            // Details column header (model/folder.rs / model/menu.rs)
            Message::ColumnSortClick(col) => self.column_header_press(col),
            Message::ColumnReorderTick => self.column_reorder_tick(),
            Message::ColumnFilterMenu(col) => {
                self.column_filter_menu(col);
                iced::Task::none()
            }
            Message::ColumnSettingsMenu => {
                self.column_settings_menu();
                iced::Task::none()
            }
            Message::ColumnHoverEnter(col) => {
                // no column hover while the rubber band owns the pointer
                if self.marquee.is_none() && self.col_hovered != Some(col) {
                    self.col_hovered = Some(col);
                }
                iced::Task::none()
            }
            Message::ColumnHoverExit(col) => {
                if self.marquee.is_none() && self.col_hovered == Some(col) {
                    self.col_hovered = None;
                }
                iced::Task::none()
            }
            // capture-only: the header background never starts the rubber band
            Message::ColumnHeaderPress => iced::Task::none(),
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.request_listing();
                iced::Task::none()
            }
            // IUICommandHandler::Execute round-trip (model/ribbon.rs)
            Message::Ribbon(ev) => self.on_ribbon(ev),
            Message::TransferTo { paths, dest, cut } => {
                if paths.is_empty() {
                    return iced::Task::none();
                }
                if fs::archive::resolve(&self.cwd).is_some() {
                    self.status = "move/copy inside archives is not supported yet".into();
                    return iced::Task::none();
                }
                // same-folder targets are a pointless "name (2)" no-op —
                // the same relationship paste and dnd reject
                let paths: Vec<PathBuf> = paths
                    .into_iter()
                    .filter(|p| crate::dnd::sources_fit(std::slice::from_ref(p), &dest))
                    .collect();
                if paths.is_empty() {
                    self.status = "The selection is already in that folder".into();
                    return iced::Task::none();
                }
                self.sched.send(Job::Copy {
                    sources: paths,
                    dest_dir: dest,
                    cut,
                });
                self.status = "Copying…".into();
                iced::Task::none()
            }
            // ribbon custom command (model/ribbon.rs)
            Message::RunShell(script) => self.run_shell(script),
            Message::BlockPress(tab) => {
                // defer: a release in place activates the tab, travel
                // past the threshold drags the window (resolved in the
                // Event handling below)
                self.block_press = Some((tab, self.cursor));
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
                    self.sched.send(Job::DeletePermanent(paths));
                    self.status = "Deleting…".into();
                }
                iced::Task::none()
            }
            Message::DragTargetEnter(t) => {
                if self.dnd.active() {
                    self.status = self.dnd.apply(crate::dnd::DragEvent::Enter { target: t });
                }
                iced::Task::none()
            }
            Message::DragTargetLeave(t) => {
                if self.dnd.active() {
                    self.status = self.dnd.apply(crate::dnd::DragEvent::Leave { target: t });
                }
                iced::Task::none()
            }
            Message::DragCancel => {
                if self.dnd.active() {
                    // §5.5: release all drag state, commit nothing
                    self.dnd.apply(crate::dnd::DragEvent::Cancel);
                    self.dnd.take();
                    self.press = None;
                    self.status = "Drop cancelled".into();
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
                    FrameStyle::Win10 => FrameStyle::Win81,
                    FrameStyle::Win81 => FrameStyle::Win8,
                };
                iced::Task::none()
            }
            Message::ToggleFrame81 => {
                self.frame_style = match self.frame_style {
                    FrameStyle::Win81 => FrameStyle::Win8,
                    _ => FrameStyle::Win81,
                };
                iced::Task::none()
            }
            Message::ToggleRainbow => {
                self.rainbow = !self.rainbow;
                iced::Task::none()
            }
            Message::Scrolled { x, y, height } => {
                // each scrollable reports only its own axis (Details
                // nests a horizontal one outside the vertical list)
                if let Some(x) = x {
                    self.scroll_x = x;
                }
                if let Some(y) = y {
                    self.scroll_y = y;
                }
                if let Some(height) = height {
                    self.viewport_h = height;
                }
                iced::Task::none()
            }
            Message::WindowResized(w, h) => {
                self.win_w = w;
                self.win_h = h;
                if std::env::var_os("EXPLORER_DEBUG").is_some() {
                    eprintln!(
                        "WindowResized: logical {w}x{h}, system_scale={}, dpi={:?}",
                        self.system_scale, self.dpi_target
                    );
                }
                // a resize can be the windowing system maximizing or
                // restoring the window (compositor keybind/snap) — sync
                // the real state instead of trusting the toggle flag
                if let Some(id) = self.window {
                    return iced::window::is_maximized(id).map(Message::MaximizedState);
                }
                iced::Task::none()
            }
            Message::MaximizedState(max) => {
                self.maximized = max;
                iced::Task::none()
            }
            Message::CaptionHover(bit, hovered) => {
                // no caption hover while the rubber band owns the pointer
                if self.marquee.is_none() {
                    let mask = 1u8 << bit;
                    self.cap_hover = if hovered {
                        self.cap_hover | mask
                    } else {
                        self.cap_hover & !mask
                    };
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
            // Press-time cursor refresh: the `cursor_seed` wrapper
            // publishes this just before the press message (FIFO bus),
            // so drag thresholds and menu anchors see the press
            // position even though idle cursor moves never reach the
            // model (main.rs drops them — each one would force a full
            // UI rebuild + layout at pointer rate).
            Message::CursorAt(p) => {
                self.cursor = p;
                iced::Task::none()
            }
            Message::Event(e) => {
                // Track the cursor during drags only: idle CursorMoved
                // events are filtered out of the subscription (see
                // `drag_live` in main.rs), so this arm only sees them
                // while a drag is live (plus every non-move event).
                if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = &e {
                    self.cursor = *position;
                    // Wayland: a simulated drag that leaves the window
                    // hands off to the compositor drag (grab serial stays
                    // valid for the whole implicit grab); drags that stay
                    // inside keep the full in-window simulation (zones,
                    // highlighting, action-choice menu)
                    if self.dnd.active()
                        && !self.dnd_wl_drag
                        && self.dnd_wl.is_some()
                        && (position.x < 0.0
                            || position.y < 0.0
                            || position.x > self.win_w
                            || position.y > self.win_h)
                    {
                        self.wayland_handoff();
                    }
                }
                // ---- drag & drop lifecycle ----
                use iced::keyboard::{Event as KeyEvent, Key, key::Named};
                match &e {
                    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                        // a press travelling past the threshold becomes a drag
                        if let Some(arm) = self.press {
                            let d = ((position.x - arm.at.x).powi(2)
                                + (position.y - arm.at.y).powi(2))
                            .sqrt();
                            if d >= crate::dnd::DRAG_THRESHOLD * self.dpi_multiplier() {
                                self.press = None;
                                self.begin_drag(arm.i, arm.right);
                            }
                        }
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                        iced::mouse::Button::Left,
                    )) => {
                        // a release without a drag collapses a pending
                        // multi-selection press (the drag case consumed
                        // the press when it passed the threshold)
                        if let Some(arm) = self.press.take()
                            && arm.collapse
                        {
                            return self.select_single(arm.i);
                        }
                        if self.dnd.active() && !self.dnd.right() {
                            return self.commit_drag();
                        }
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                        iced::mouse::Button::Right,
                    )) => {
                        // a right-press that never crossed the threshold
                        // is a plain context click (the drag case consumed
                        // the press); a right-drag release drops via the
                        // action-choice menu
                        if let Some(arm) = self.press.take()
                            && arm.right
                        {
                            return self.update(Message::ContextSelect(arm.i));
                        }
                        if self.dnd.active() && self.dnd.right() {
                            return self.commit_drag();
                        }
                    }
                    iced::Event::Keyboard(KeyEvent::KeyPressed {
                        key: Key::Named(Named::Escape),
                        ..
                    }) if self.dnd.active() => {
                        return self.update(Message::DragCancel);
                    }
                    // the compositor owns the pointer: focus loss means the
                    // button state is lost, so the drag cannot complete
                    iced::Event::Window(iced::window::Event::Focused) => {
                        self.focused = true;
                    }
                    iced::Event::Window(iced::window::Event::Unfocused) => {
                        self.focused = false;
                        if self.dnd.active() {
                            return self.update(Message::DragCancel);
                        }
                        // window focus loss dismisses an open popup
                        // menu just like an off-menu click (the shield
                        // only reacts to pointer events)
                        if self.menu.is_some() {
                            return self.update(Message::CloseMenu);
                        }
                    }
                    // external drag-in (§9): only delivered where winit
                    // supports file drops (see dnd::capabilities)
                    iced::Event::Window(iced::window::Event::FileHovered(p)) => {
                        if crate::dnd::capabilities().drag_in {
                            self.status = format!(
                                "Release to copy {} into {}",
                                p.display(),
                                self.cwd.display()
                            );
                        }
                    }
                    iced::Event::Window(iced::window::Event::FileDropped(p)) => {
                        if crate::dnd::capabilities().drag_in {
                            self.dnd.take();
                            if let Err(reason) = crate::dnd::validate_commit(
                                std::slice::from_ref(p),
                                &crate::dnd::DropTarget::EmptyDirectory(self.cwd.clone()),
                                crate::dnd::DropAction::Copy,
                            ) {
                                self.status = reason;
                            } else {
                                self.sched.send(Job::Copy {
                                    sources: vec![p.clone()],
                                    dest_dir: self.cwd.clone(),
                                    cut: false,
                                });
                                self.status = "Copying…".into();
                            }
                        }
                    }
                    iced::Event::Window(iced::window::Event::FilesHoveredLeft)
                        if !self.dnd.active() =>
                    {
                        self.status.clear();
                    }
                    _ => {}
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
                // contextual block press: release activates the tab,
                // travel past the threshold becomes a window drag
                if let Some((tab, at)) = self.block_press {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            let d = ((position.x - at.x).powi(2)
                                + (position.y - at.y).powi(2))
                            .sqrt();
                            if d >= crate::dnd::DRAG_THRESHOLD * self.dpi_multiplier() {
                                self.block_press = None;
                                if let Some(id) = self.window {
                                    return iced::window::drag(id);
                                }
                            }
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.block_press = None;
                            self.ribbon.framework().view().select_tab(tab);
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
                // armed header press: travel past the threshold becomes
                // a header reorder drag; a plain release sorts (Windows
                // header semantics)
                if let Some((col, at)) = self.col_arm {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            let d = ((position.x - at.x).powi(2)
                                + (position.y - at.y).powi(2))
                            .sqrt();
                            if d >= crate::dnd::DRAG_THRESHOLD * self.dpi_multiplier() {
                                self.column_reorder_begin(col, at);
                            }
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            self.col_arm = None;
                            return self.column_sort_release(col);
                        }
                        _ => {}
                    }
                }
                // column reorder drag: the dragged header follows the
                // cursor, the order commits on left release
                if self.col_reorder.is_some() {
                    use iced::mouse::{Button as MButton, Event as MEvent};
                    match &e {
                        iced::Event::Mouse(MEvent::CursorMoved { position }) => {
                            return self.column_reorder_move(position.x);
                        }
                        iced::Event::Mouse(MEvent::ButtonReleased(MButton::Left)) => {
                            return self.column_reorder_end();
                        }
                        _ => {}
                    }
                }
                // the column header's 75% press tint ends on release
                if self.col_pressed.is_some()
                    && matches!(
                        &e,
                        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                            iced::mouse::Button::Left
                        ))
                    )
                {
                    self.col_pressed = None;
                }
                if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) = &e {
                    let ctrl = m.control();
                    self.shift_held = m.shift();
                    self.alt_held = m.alt();
                    if !ctrl && self.ctrl_held {
                        // §7: Ctrl released — reset the delta accumulator
                        self.zoom_acc = 0.0;
                        self.zoom_step_at = None;
                    }
                    self.ctrl_held = ctrl;
                    if self.dnd.active() {
                        // modifier-driven action switch (Copy/Move/Link)
                        self.dnd.set_modifiers(ctrl, self.shift_held, self.alt_held);
                        self.status = self.dnd.feedback();
                        self.refresh_wayland_visual();
                    }
                    return iced::Task::none();
                }
                // address edit mode: a press outside the address bar
                // discards the edits (a press on the bar keeps them)
                if matches!(
                    e,
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Left
                    ))
                ) {
                    if self.address_edit.is_some()
                        && !crate::navigation::bar_rect(self).contains(self.cursor)
                    {
                        self.address_edit = None;
                    }
                    if self.search_active
                        && !crate::navigation::search_rect(self).contains(self.cursor)
                    {
                        self.search_active = false;
                    }
                }
                if let Some(m) = crate::handle_key(
                    e,
                    self.renaming.is_some(),
                    self.menu.is_some(),
                    self.address_edit.is_some(),
                    self.search_active,
                ) {
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
            // the rubber band owns the pointer during the drag: no
            // hover chrome on the splitter, nav lists, or breadcrumbs
            Message::SplitterHover(h) => {
                if self.marquee.is_none() {
                    self.splitter_hover = h;
                }
                iced::Task::none()
            }
            Message::BackListHover(h) => {
                if self.marquee.is_none() {
                    self.back_list_hover = h;
                }
                iced::Task::none()
            }
            Message::RightListHover(h) => {
                if self.marquee.is_none() {
                    self.right_list_hover = h;
                }
                iced::Task::none()
            }
            Message::CrumbHoverEnter { group, arrow } => {
                if self.marquee.is_none() && self.crumb_hover != Some((group, arrow)) {
                    self.crumb_hover = Some((group, arrow));
                }
                iced::Task::none()
            }
            Message::CrumbHoverExit { group, arrow } => {
                // moving into the neighboring member fires its enter
                // first; only clear when this member is still hovered
                if self.marquee.is_none() && self.crumb_hover == Some((group, arrow)) {
                    self.crumb_hover = None;
                }
                iced::Task::none()
            }
            Message::CrumbArrow(i) => {
                // list the segment's subfolders on the fs worker (VFS:
                // archives list like folders), then open the dropdown
                // (the BarListed event)
                let prefix = crate::navigation::prefix_of(&self.cwd, i);
                self.bar_listing = Some(prefix.clone());
                self.sched.send(Job::BarList(prefix));
                iced::Task::none()
            }
            Message::HistoryMenu => {
                self.history_menu(crate::menu::BarMenuKind::History);
                iced::Task::none()
            }
            Message::SelectorMenu => {
                // in edit mode the selector drops down while the edits
                // stay alive (the border blue goes with the open menu)
                self.history_menu(crate::menu::BarMenuKind::Selector);
                iced::Task::none()
            }
            Message::AddressEditStart => {
                if self.address_edit.is_none() {
                    self.address_edit = Some(self.cwd.to_string_lossy().into_owned());
                    let id = iced::widget::Id::new(ADDRESS_INPUT_ID);
                    return iced::Task::batch([
                        iced::widget::operation::focus(id.clone()),
                        iced::widget::operation::text_input::select_all(id),
                    ]);
                }
                iced::Task::none()
            }
            Message::AddressEditInput(s) => {
                if let Some(buf) = &mut self.address_edit {
                    *buf = s;
                }
                iced::Task::none()
            }
            Message::AddressEditSubmit => {
                if let Some(buf) = self.address_edit.take() {
                    let t = buf.trim();
                    if !t.is_empty() {
                        let p = PathBuf::from(t);
                        // relative input resolves against the cwd
                        let p = if p.is_absolute() { p } else { self.cwd.join(p) };
                        return self.navigate(p);
                    }
                }
                iced::Task::none()
            }
            Message::CancelAddressEdit => {
                self.address_edit = None;
                iced::Task::none()
            }
            Message::SearchEditStart => {
                if !self.search_active {
                    self.search_active = true;
                    let id = iced::widget::Id::new(SEARCH_INPUT_ID);
                    return iced::widget::operation::focus(id);
                }
                iced::Task::none()
            }
            Message::SearchInput(s) => {
                self.search_text = s;
                iced::Task::none()
            }
            Message::SearchSubmit => {
                let q = self.search_text.trim().to_string();
                if !q.is_empty() {
                    self.search_query = Some(q);
                }
                iced::Task::none()
            }
            Message::CancelSearch => {
                self.search_active = false;
                iced::Task::none()
            }
            Message::BarSplitMove(x) => {
                let (_, fr, _, fl) =
                    crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
                let content_w = (self.win_w - fl - fr).max(320.0);
                // search right edge sits at content_w - 12 (band padding);
                // the splitter (12px) hugs the search left edge, so its
                // center is 6px left of it -> offset 12 + 6 = 18
                let search_w =
                    (content_w - (x - fl) - 18.0).clamp(120.0, (content_w - 280.0).max(120.0));
                self.search_share = (search_w / content_w).clamp(0.08, 0.6);
                iced::Task::none()
            }
            Message::PaneSplitMove(x) => {
                // the pane's right edge is the content area's right edge
                let (_, fr, _, _) =
                    crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
                let content_w = (self.win_w - fr).max(160.0);
                self.pane_width = crate::pane::clamped(content_w - x, content_w);
                iced::Task::none()
            }
            Message::NavSplitMove(x) => {
                let (_, fr, _, fl) =
                    crate::border::frame_insets_pub(self.self_border, self.frame_style, self.maximized);
                let content_w = self.win_w - fl - fr;
                self.nav_width = (x - fl).clamp(MIN_NAV_W, (content_w * 0.5).max(MIN_NAV_W));
                iced::Task::none()
            }
            Message::NavScrolled { y, height } => {
                self.nav.viewport_h = height;
                // correct any offset that is not a whole row multiple
                // (e.g. the scrollable clamping after the tree shrank);
                // the correction never exceeds max_scroll, so it cannot
                // re-trigger a clamp
                let line = self.ribbon.metrics().1.max(1.0);
                let max_scroll = (crate::nav_pane::content_height(self) - height).max(0.0);
                let snapped =
                    ((y / line).round() * line).clamp(0.0, (max_scroll / line).floor() * line);
                if (snapped - y).abs() > 0.5 {
                    self.nav.scroll_y = snapped;
                    return iced::widget::operation::scrollable::scroll_to(
                        crate::nav_pane::nav_scrollable_id(),
                        iced::widget::operation::scrollable::AbsoluteOffset {
                            x: None,
                            y: Some(snapped),
                        },
                        iced::widget::operation::Animation::Instant,
                    );
                }
                self.nav.scroll_y = y;
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

#[cfg(test)]
mod history_tests {
    use super::*;

    /// Navigate without caring about the returned task (tests only
    /// assert the stack/cwd bookkeeping).
    fn nav(e: &mut Explorer, p: &str) {
        let _ = e.navigate(PathBuf::from(p));
    }

    #[test]
    fn history_pick_rewinds_and_replays_chronology() {
        let mut e = Explorer::new();
        e.cwd = PathBuf::from("/a");
        nav(&mut e, "/b");
        nav(&mut e, "/c");
        nav(&mut e, "/d");
        // back=[/a,/b,/c], cwd=/d, forward=[]
        let _ = e.history_nav(PathBuf::from("/b"));
        assert_eq!(e.cwd, PathBuf::from("/b"));
        assert_eq!(e.back, vec![PathBuf::from("/a")]);
        // forward: pop -> /c, then /d (chronology /b -> /c -> /d)
        assert_eq!(e.forward, vec![PathBuf::from("/d"), PathBuf::from("/c")]);

        // picking a forward entry replays through the in-between ones
        let _ = e.history_nav(PathBuf::from("/d"));
        assert_eq!(e.cwd, PathBuf::from("/d"));
        assert_eq!(
            e.back,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
        assert!(e.forward.is_empty());
    }

    #[test]
    fn history_pick_forward_keeps_later_entries() {
        let mut e = Explorer::new();
        e.cwd = PathBuf::from("/a");
        nav(&mut e, "/b");
        nav(&mut e, "/c");
        // chronology /a -> /b -> /c; go back once: cwd=/b, forward=[/c]
        let _ = e.update(Message::Back);
        // pick the next entry itself: no-op-ish, stacks unchanged
        let _ = e.history_nav(PathBuf::from("/c"));
        assert_eq!(e.cwd, PathBuf::from("/c"));
        assert_eq!(e.back, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(e.forward.is_empty());
    }

    #[test]
    fn history_stack_capped() {
        let mut stack = Vec::new();
        for i in 0..25 {
            push_history(&mut stack, PathBuf::from(format!("/p{i}")));
        }
        assert_eq!(stack.len(), HISTORY_MAX);
        // the oldest five fell off
        assert_eq!(stack.first(), Some(&PathBuf::from("/p5")));
        assert_eq!(stack.last(), Some(&PathBuf::from("/p24")));
    }
}

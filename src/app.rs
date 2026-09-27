//! Application core: composition root, command bus, Cx snapshot.
//!
//! Loose coupling (D6): UI modules receive a read-only `Cx` per frame and
//! emit `Cmd` values; App routes them to the fs worker / window port / module
//! states. Modules never call each other.

use crate::clipboard::{ClipMode, Clipboard};
use crate::fs::{self, FsEvent};
use crate::platform::{NullPort, WindowPort};
use crate::theme::Theme;
use crate::ui::address_bar::AddressBar;
use crate::ui::file_view::{FileView, ViewMode};
use crate::ui::nav_pane::NavPane;
use crate::ui::ribbon::{Ribbon, Tab};
use crate::ui::status_bar::StatusBar;
use crate::{border, input, Options};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

/// Commands UI modules emit; App routes them.
#[derive(Clone, Debug)]
#[allow(dead_code)] // reserved commands land in later stages
pub enum Cmd {
    // window
    DragMove,
    Minimize,
    ToggleMaximize,
    Exit,
    // navigation
    Navigate(PathBuf),
    Back,
    Forward,
    Up,
    Refresh,
    // file operations
    NewFolder,
    RenameSelected,
    CommitRename { index: usize, name: String },
    CancelRename,
    CopySelection,
    CutSelection,
    Paste,
    DeleteSelection,
    SelectAll,
    SelectNone,
    InvertSelection,
    // view
    SetView(ViewMode),
    ToggleHidden,
    ToggleNavPane,
    TogglePreviewPane,
    // ribbon chrome
    SetTab(Tab),
    ToggleCollapsed,
    ToggleBackstage,
    // misc
    SetDark(bool),
    Status(String),
}

/// Read-only snapshot handed to UI modules each frame.
#[allow(dead_code)] // summary fields are consumed as modules grow
pub struct Cx<'a> {
    pub theme: &'a Theme,
    pub cwd: &'a Path,
    /// Display name of cwd for the title bar and search placeholder.
    pub cwd_label: &'a str,
    pub focused: bool,
    pub maximized: bool,
    // navigation
    pub can_back: bool,
    pub can_forward: bool,
    pub can_up: bool,
    // file view summary
    pub item_count: usize,
    pub selected_count: usize,
    pub selected_bytes: u64,
    pub single_selected: Option<usize>, // index when exactly one selected
    pub view: ViewMode,
    pub show_hidden: bool,
    pub has_clipboard: bool,
    // chrome
    pub nav_visible: bool,
    pub clipboard_cut: bool,
    pub ops_line: String,
    pub status_line: String,
    pub port: &'a dyn WindowPort,
}

pub struct App {
    pub theme: Theme,
    last_theme_gen: u64,

    pub cwd: PathBuf,
    pub cwd_label: String,
    history_back: Vec<PathBuf>,
    history_forward: Vec<PathBuf>,

    pub nav: NavPane,
    pub fv: FileView,
    pub addr: AddressBar,
    pub ribbon: Ribbon,
    pub status_bar: StatusBar,

    clipboard: Option<Clipboard>,
    ops_line: String,
    pub status_line: String,

    sched: fs::Scheduler,
    fs_rx: Receiver<FsEvent>,

    port: Box<dyn WindowPort>,
    /// Physical window size in px, kept current by resize events (future use:
    /// per-monitor metrics). UI code uses logical units — never draw with this.
    #[allow(dead_code)]
    window: (f32, f32),
    focused: bool,
    exit: bool,
    demo: bool,
}

impl App {
    pub fn new(port: Box<dyn WindowPort>, opts: &Options) -> Self {
        let (sched, fs_rx) = fs::spawn();
        let theme = if opts.dark { Theme::dark() } else { Theme::light() };
        let cwd = opts.folder.clone().unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")));
        let cwd_label = label_of(&cwd);
        Self {
            theme,
            last_theme_gen: u64::MAX, // force first apply
            cwd,
            cwd_label,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            nav: NavPane::new(),
            fv: FileView::new(),
            addr: AddressBar::new(),
            ribbon: Ribbon::new(),
            status_bar: StatusBar::new(),
            clipboard: None,
            ops_line: String::new(),
            status_line: String::new(),
            sched,
            fs_rx,
            port,
            window: (1152.0, 720.0),
            focused: true,
            exit: false,
            demo: false,
        }
    }

    /// Headless instance with deterministic sample content (D8).
    pub fn new_headless(opts: Options) -> Self {
        let mut app = Self::new(Box::new(NullPort), &opts);
        app.demo = true;
        app.fv.load_demo();
        app.nav.load_demo();
        app
    }

    pub fn bootstrap(&mut self) {
        if self.demo {
            return;
        }
        self.sched.send(fs::Job::List(self.cwd.clone()));
        self.sched.send(fs::Job::Volumes);
        self.fv.set_loading();
    }

    pub fn set_window_size(&mut self, w: f32, h: f32) {
        self.window = (w, h);
    }

    pub fn exit_requested(&self) -> bool {
        self.exit
    }

    /// One UI frame: theme, fs events, keyboard, layout bands, route commands.
    /// `ui` is egui's root Ui for this pass (egui 0.36 `run_ui`).
    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.theme.generation != self.last_theme_gen {
            self.theme.apply(&ctx);
            self.last_theme_gen = self.theme.generation;
        }
        self.focused = ctx.input(|i| i.focused);
        self.poll_fs();

        let mut pending: Vec<Cmd> = Vec::new();
        input::gather(&ctx, &mut pending);
        border::edge_resize(&ctx, &self.theme, &*self.port);

        // Read-only snapshot for UI modules. Built from disjoint field
        // borrows so modules can still take &mut of their own state (D6).
        let cx = Cx {
            theme: &self.theme,
            cwd: &self.cwd,
            cwd_label: &self.cwd_label,
            focused: self.focused,
            maximized: self.port.is_maximized(),
            can_back: !self.history_back.is_empty(),
            can_forward: !self.history_forward.is_empty(),
            can_up: self.cwd.parent().is_some(),
            item_count: self.fv.visible_count(),
            selected_count: self.fv.selected_count(),
            selected_bytes: self.fv.selected_bytes(),
            single_selected: if self.fv.selected_count() == 1 { self.fv.first_selected() } else { None },
            view: self.fv.view,
            show_hidden: self.fv.show_hidden,
            has_clipboard: self.clipboard.as_ref().map(|c| !c.paths.is_empty()).unwrap_or(false),
            clipboard_cut: self.clipboard.as_ref().map(|c| c.mode == ClipMode::Cut).unwrap_or(false),
            nav_visible: self.nav.visible,
            ops_line: self.ops_line.clone(),
            status_line: self.status_line.clone(),
            port: &*self.port,
        };

        border::titlebar(ui, &cx, &mut pending);
        self.ribbon.show(ui, &cx, &mut pending);
        self.addr.show(ui, &cx, &mut pending);
        self.status_bar.show(ui, &cx, &mut pending);
        if cx.nav_visible {
            self.nav.show(ui, &cx, &mut pending);
        }
        self.fv.show(ui, &cx, &mut pending);
        border::frame_stroke(&ctx, &cx);

        for cmd in pending {
            self.command(cmd);
        }
    }

    fn poll_fs(&mut self) {
        let events: Vec<_> = self.fs_rx.try_iter().collect();
        for ev in events {
            match ev {
                FsEvent::Listed { path, result } => {
                    if path == self.cwd {
                        match result {
                            Ok(entries) => self.fv.set_entries(entries),
                            Err(e) => {
                                self.fv.set_error(e.clone());
                                self.status_line = e;
                            }
                        }
                    }
                }
                FsEvent::Volumes(v) => self.nav.set_volumes(v),
                FsEvent::DirCreated { parent, result } => {
                    if parent == self.cwd {
                        self.request_listing();
                        if let Ok(name) = result {
                            self.fv.begin_rename_of(&name);
                        }
                    }
                }
                FsEvent::Renamed { result } => match result {
                    Ok(()) => self.request_listing(),
                    Err(e) => {
                        self.status_line = format!("Rename failed: {e}");
                        self.request_listing();
                    }
                },
                FsEvent::Trashed { result } => {
                    match result {
                        Ok(()) => self.status_line.clear(),
                        Err(e) => self.status_line = format!("Delete failed: {e}"),
                    }
                    self.fv.clear_selection();
                    self.request_listing();
                }
                FsEvent::OpProgress { done_bytes, total_bytes, current } => {
                    let pct = if total_bytes > 0 { done_bytes * 100 / total_bytes } else { 0 };
                    self.ops_line = format!(
                        "Copying {} — {}%  ({}/{})",
                        current.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                        pct,
                        crate::ui::file_view::format_size(done_bytes),
                        crate::ui::file_view::format_size(total_bytes)
                    );
                }
                FsEvent::OpDone { result } => {
                    self.ops_line.clear();
                    match result {
                        Ok(()) => self.status_line.clear(),
                        Err(e) => self.status_line = format!("Operation failed: {e}"),
                    }
                    self.fv.clear_selection();
                    self.request_listing();
                }
            }
        }
    }

    fn request_listing(&mut self) {
        if !self.demo {
            self.sched.send(fs::Job::List(self.cwd.clone()));
        }
    }

    fn navigate(&mut self, path: PathBuf) {
        if self.demo || path == self.cwd {
            return;
        }
        self.history_back.push(self.cwd.clone());
        self.history_forward.clear();
        self.set_cwd(path);
    }

    fn set_cwd(&mut self, path: PathBuf) {
        self.cwd = path;
        self.cwd_label = label_of(&self.cwd);
        self.fv.clear_selection();
        self.fv.set_loading();
        self.request_listing();
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::DragMove => self.port.drag_move(),
            Cmd::Minimize => self.port.minimize(),
            Cmd::ToggleMaximize => self.port.toggle_maximize(),
            Cmd::Exit => self.exit = true,

            Cmd::Navigate(p) => self.navigate(p),
            Cmd::Back => {
                if let Some(p) = self.history_back.pop() {
                    self.history_forward.push(self.cwd.clone());
                    self.set_cwd(p);
                }
            }
            Cmd::Forward => {
                if let Some(p) = self.history_forward.pop() {
                    self.history_back.push(self.cwd.clone());
                    self.set_cwd(p);
                }
            }
            Cmd::Up => {
                if let Some(p) = self.cwd.parent().map(Path::to_path_buf) {
                    self.navigate(p);
                }
            }
            Cmd::Refresh => self.request_listing(),

            Cmd::NewFolder => {
                self.sched.send(fs::Job::Mkdir(self.cwd.clone()));
            }
            Cmd::RenameSelected => {
                if let Some(i) = self.fv.first_selected() {
                    self.fv.start_rename(i);
                }
            }
            Cmd::CommitRename { index, name } => {
                if let Some(entry) = self.fv.entry(index) {
                    let from = entry.path.clone();
                    let to = from.with_file_name(name.trim());
                    if to != from && !name.trim().is_empty() {
                        self.sched.send(fs::Job::Rename(from, to));
                    }
                }
                self.fv.cancel_rename();
            }
            Cmd::CancelRename => self.fv.cancel_rename(),

            Cmd::CopySelection => {
                let paths = self.fv.selected_paths();
                if !paths.is_empty() {
                    let n = paths.len();
                    self.clipboard = Some(Clipboard::new_copy(paths));
                    self.status_line = format!("Copied {n} item{}", if n == 1 { "" } else { "s" });
                }
            }
            Cmd::CutSelection => {
                let paths = self.fv.selected_paths();
                if !paths.is_empty() {
                    self.clipboard = Some(Clipboard::new_cut(paths));
                    self.status_line = "Cut selection".into();
                }
            }
            Cmd::Paste => {
                if let Some(clip) = self.clipboard.clone() {
                    if !clip.paths.is_empty() {
                        let cut = clip.mode == ClipMode::Cut;
                        self.sched.send(fs::Job::Copy { sources: clip.paths, dest_dir: self.cwd.clone(), cut });
                        if cut {
                            self.clipboard = None;
                        }
                    }
                }
            }
            Cmd::DeleteSelection => {
                let paths = self.fv.selected_paths();
                if !paths.is_empty() {
                    self.sched.send(fs::Job::Trash(paths));
                }
            }

            Cmd::SelectAll => self.fv.select_all(),
            Cmd::SelectNone => self.fv.clear_selection(),
            Cmd::InvertSelection => self.fv.invert_selection(),

            Cmd::SetView(v) => self.fv.view = v,
            Cmd::ToggleHidden => self.fv.toggle_hidden(),
            Cmd::ToggleNavPane => self.nav.visible = !self.nav.visible,
            Cmd::TogglePreviewPane => self.status_line = "Preview pane: coming soon".into(),

            Cmd::SetTab(t) => {
                self.ribbon.tab = t;
                self.ribbon.backstage = false;
                self.ribbon.collapsed = false;
            }
            Cmd::ToggleCollapsed => self.ribbon.collapsed = !self.ribbon.collapsed,
            Cmd::ToggleBackstage => self.ribbon.backstage = !self.ribbon.backstage,

            Cmd::SetDark(dark) => {
                if self.theme.dark != dark {
                    self.theme.toggle_dark();
                }
            }
            Cmd::Status(s) => self.status_line = s,
        }
    }
}

pub fn label_of(path: &Path) -> String {
    if path == Path::new("/") {
        "Computer".to_string()
    } else {
        path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
    }
}

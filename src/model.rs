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
    /// Hover state of the address/search splitter (drives tint + cursor).
    SplitterHover(bool),
    Navigate(PathBuf),
    Back,
    Forward,
    Up,
    Refresh,
    Select(usize),
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
    SetView(ViewMode),
    SetSort(SortBy),
    /// A ribbon framework event (IUICommandHandler::Execute and friends).
    Ribbon(crate::ribbon::RibbonEvent),
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
    Scrolled { y: f32, height: f32 },
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
pub enum ViewMode {
    Details,
    Large,
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

pub struct Explorer {
    pub window: Option<iced::window::Id>,
    pub cwd: PathBuf,
    pub entries: Vec<fs::Entry>,
    pub selected: std::collections::BTreeSet<usize>,
    pub hovered: Option<usize>,
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
    /// Last title-bar press (double-click detection: the 2nd press must not
    /// start a window drag, or the in-flight move scrambles the placement of
    /// the subsequent maximize).
    last_title_press: Option<std::time::Instant>,
    /// Self-rendered window border/title bar (off = native decorations).
    pub self_border: bool,
    /// Win8 double border vs Win10 1px border.
    pub frame_style: FrameStyle,
    pub clipboard: Option<(ClipMode, Vec<PathBuf>)>,
    pub view: ViewMode,
    pub sort: SortBy,
    pub show_hidden: bool,
    /// The ribbon (rs-ribbon framework + explorer commanding).
    pub ribbon: crate::ribbon::Ribbon,
    pub scroll_y: f32,
    pub viewport_h: f32,
    /// Measured window width (resize events) for title-bar layout.
    pub win_w: f32,
    /// Inline rename: (row index, text buffer).
    pub renaming: Option<(usize, String)>,
    pub status: String,
    pub last_click: Option<(usize, std::time::Instant)>,
    pub loading: bool,
    pub volumes: Vec<fs::Volume>,
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
        let mut s = Self {
            window: None,
            cwd,
            entries: Vec::new(),
            selected: Default::default(),
            hovered: None,
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
            last_title_press: None,
            self_border: true,
            frame_style: FrameStyle::Win8,
            clipboard: None,
            view: ViewMode::Details,
            sort: SortBy::Name,
            show_hidden: false,
            ribbon: crate::ribbon::Ribbon::new(),
            scroll_y: 0.0,
            viewport_h: 600.0,
            win_w: 1152.0,
            renaming: None,
            status: String::new(),
            last_click: None,
            loading: false,
            volumes: Vec::new(),
            sched,
            pending_rename: None,
        };
        s.request_listing();
        s.sched.send(Job::Volumes);
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

    fn navigate(&mut self, path: PathBuf) {
        if path == self.cwd {
            return;
        }
        self.back.push(self.cwd.clone());
        self.forward.clear();
        self.cwd = path;
        self.selected.clear();
        self.renaming = None;
        self.scroll_y = 0.0;
        self.request_listing();
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

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        // ribbon-originated events must not re-run contextual
        // auto-activation (the user just clicked a tab)
        let auto = !matches!(message, Message::Ribbon(_));
        let task = self.handle(message);
        // every model change refreshes the ribbon (enablement, toggle
        // values, contextual-tab availability)
        let snap = self.ribbon_snapshot();
        self.ribbon.sync(snap);
        if auto {
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
            Message::Navigate(p) => {
                self.navigate(p);
                iced::Task::none()
            }
            Message::Back => {
                if let Some(p) = self.back.pop() {
                    self.forward.push(self.cwd.clone());
                    self.cwd = p;
                    self.selected.clear();
                    self.renaming = None;
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
                    self.request_listing();
                }
                iced::Task::none()
            }
            Message::Up => {
                if let Some(p) = self.cwd.parent().map(Path::to_path_buf) {
                    self.navigate(p);
                }
                iced::Task::none()
            }
            Message::Refresh => {
                self.request_listing();
                iced::Task::none()
            }
            Message::Select(i) => {
                self.selected.clear();
                self.selected.insert(i);
                // double-click emulation: two presses on the same row within 400ms
                let now = std::time::Instant::now();
                if let Some((last_i, t)) = self.last_click {
                    if last_i == i && now.duration_since(t).as_millis() < 400 {
                        self.last_click = None;
                        if let Some(e) = self.entries.get(i) {
                            if e.kind == fs::EntryKind::Dir || fs::archive::is_archive_path(&e.path) {
                                let p = e.path.clone();
                                self.navigate(p);
                            }
                        }
                        return iced::Task::none();
                    }
                }
                self.last_click = Some((i, now));
                iced::Task::none()
            }
            Message::Hover(h) => {
                self.hovered = h;
                iced::Task::none()
            }
            Message::Open(i) => {
                if let Some(e) = self.entries.get(i) {
                    if e.kind == fs::EntryKind::Dir || fs::archive::is_archive_path(&e.path) {
                        let p = e.path.clone();
                        self.navigate(p);
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
            Message::SetView(v) => {
                self.view = v;
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
            Message::Scrolled { y, height } => {
                self.scroll_y = y;
                self.viewport_h = height;
                iced::Task::none()
            }
            Message::WindowResized(w, h) => {
                self.viewport_h = h;
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
                if let Some(m) = crate::handle_key(e, self.renaming.is_some()) {
                    return self.update(m);
                }
                iced::Task::none()
            }
            Message::BeginBarSplit => {
                self.bar_split = true;
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
                let search_w = (content_w - (x - fl) - 10.0).clamp(120.0, content_w - 280.0);
                self.search_share = (search_w / content_w).clamp(0.08, 0.6);
                iced::Task::none()
            }
            Message::FsEvent(ev) => {
                self.handle_fs_event(ev);
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

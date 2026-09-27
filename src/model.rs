//! Application state (MVU model) — pure logic, no widgets.

use std::path::{Path, PathBuf};
use crate::fs::{self, FsEvent, Job};

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(iced::window::Id),
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
    ToggleHidden,
    SetTab(Tab),
    RenameInput(String),
    Status(String),
    Event(iced::Event),
    FsEvent(FsEvent),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Details,
    Large,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipMode {
    Copy,
    Cut,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    File,
    Home,
    Share,
    View,
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
    pub maximized: bool,
    pub clipboard: Option<(ClipMode, Vec<PathBuf>)>,
    pub view: ViewMode,
    pub show_hidden: bool,
    pub tab: Tab,
    /// Inline rename: (row index, text buffer).
    pub renaming: Option<(usize, String)>,
    pub status: String,
    pub last_click: Option<(usize, std::time::Instant)>,
    pub loading: bool,
    pub volumes: Vec<fs::Volume>,
    sched: fs::Scheduler,
    pending_rename: Option<String>,
}

impl Explorer {
    pub fn new() -> Self {
        let (sched, ev_rx) = fs::spawn();
        let _ = crate::FS_STREAM_RX.set(std::sync::Mutex::new(Some(ev_rx)));
        let cwd = std::env::args()
            .nth(1)
            .map(PathBuf::from)
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
            maximized: false,
            clipboard: None,
            view: ViewMode::Details,
            show_hidden: false,
            tab: Tab::Home,
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
        s
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
            iced::Theme::custom("win8-dark", crate::ui::skin::DARK.palette())
        } else {
            iced::Theme::custom("win8-light", crate::ui::skin::LIGHT.palette())
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
        self.request_listing();
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
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
                            self.entries = entries;
                            self.selected.clear();
                            self.hovered = None;
                            self.renaming = None;
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

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window = Some(id);
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
            Message::SetTab(t) => {
                self.tab = t;
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
                if let Some(m) = crate::handle_key(e, self.renaming.is_some()) {
                    return self.update(m);
                }
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

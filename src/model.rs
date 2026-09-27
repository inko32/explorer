//! Application state (MVU model) — pure logic, no widgets.

use std::path::{Path, PathBuf};

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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    File,
    Home,
    Share,
    View,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryKind {
    Dir,
    File,
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

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<i64>,
    pub is_hidden: bool,
}

pub struct Explorer {
    pub window: Option<iced::window::Id>,
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
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
}

impl Explorer {
    pub fn new() -> Self {
        let cwd = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
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
        };
        s.reload();
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

    fn reload(&mut self) {
        self.entries = list_dir(&self.cwd, self.show_hidden);
        self.selected.clear();
        self.hovered = None;
        self.renaming = None;
    }

    fn navigate(&mut self, path: PathBuf) {
        if path == self.cwd {
            return;
        }
        self.back.push(self.cwd.clone());
        self.forward.clear();
        self.cwd = path;
        self.reload();
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
        let dest = self.cwd.clone();
        let mut errors = 0usize;
        for src in &sources {
            if let Err(e) = copy_recursive(src, &dest) {
                self.status = format!("{}: {}", src.display(), e);
                errors += 1;
            }
            if mode == ClipMode::Cut {
                let _ = if src.is_dir() {
                    std::fs::remove_dir_all(src)
                } else {
                    std::fs::remove_file(src)
                };
            }
        }
        if errors == 0 {
            self.status.clear();
        }
        if mode == ClipMode::Cut {
            self.clipboard = None;
        }
        self.reload();
    }

    fn delete_selection(&mut self) {
        let paths = self.selected_paths();
        for p in &paths {
            let r = if p.is_dir() {
                std::fs::remove_dir_all(p)
            } else {
                std::fs::remove_file(p)
            };
            if let Err(e) = r {
                self.status = format!("{}: {}", p.display(), e);
            }
        }
        self.reload();
    }

    fn new_folder(&mut self) {
        for n in 0..100 {
            let name = if n == 0 { "New folder".to_string() } else { format!("New folder ({})", n + 1) };
            let path = self.cwd.join(&name);
            if !path.exists() {
                if let Err(e) = std::fs::create_dir(&path) {
                    self.status = format!("{}", e);
                    return;
                }
                self.reload();
                if let Some(i) = self.entries.iter().position(|e| e.name == name) {
                    self.selected.clear();
                    self.selected.insert(i);
                    self.renaming = Some((i, name));
                }
                return;
            }
        }
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
                if let Err(e) = std::fs::rename(&from, &to) {
                    self.status = format!("Rename failed: {e}");
                }
            }
        }
        self.reload();
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
                    self.reload();
                }
                iced::Task::none()
            }
            Message::Forward => {
                if let Some(p) = self.forward.pop() {
                    self.back.push(self.cwd.clone());
                    self.cwd = p;
                    self.reload();
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
                self.reload();
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
                        let p = self.entries[i].path.clone();
                        if self.entries[i].kind == EntryKind::Dir {
                            self.navigate(p);
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
                    if e.kind == EntryKind::Dir {
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
                self.new_folder();
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
                self.reload();
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
        }
    }
}

/// Copy src (file or dir tree) into dest_dir; conflicts auto-rename " (2)".
fn copy_recursive(src: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let name = src.file_name().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no name"))?;
    let dst = auto_rename(&dest_dir.join(name));
    if src.is_dir() {
        std::fs::create_dir_all(&dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, &dst)?;
    }
    Ok(())
}

fn auto_rename(dst: &Path) -> PathBuf {
    if !dst.exists() {
        return dst.to_path_buf();
    }
    let parent = dst.parent().unwrap_or(Path::new("/"));
    let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = dst.extension().map(|e| e.to_string_lossy().into_owned());
    for n in 2..1000 {
        let name = match &ext {
            Some(ext) if !ext.is_empty() => format!("{} ({}).{}", stem, n, ext),
            _ => format!("{} ({})", stem, n),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dst.to_path_buf()
}

fn list_dir(path: &Path, show_hidden: bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = match std::fs::read_dir(path) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| {
                let p = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                let meta = std::fs::metadata(&p).ok();
                let (kind, size, modified) = match &meta {
                    Some(m) => (
                        if m.is_dir() { EntryKind::Dir } else { EntryKind::File },
                        m.len(),
                        m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64),
                    ),
                    None => (EntryKind::File, 0, None),
                };
                let is_hidden = name.starts_with('.');
                Entry { name, path: p, kind, size, modified, is_hidden }
            })
            .filter(|e| show_hidden || !e.is_hidden)
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort_by(|a, b| {
        let dir_first = (b.kind == EntryKind::Dir).cmp(&(a.kind == EntryKind::Dir));
        if dir_first != std::cmp::Ordering::Equal {
            return dir_first;
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    out
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

pub fn type_label(e: &Entry) -> &'static str {
    if e.kind == EntryKind::Dir {
        return "File folder";
    }
    match e.name.rsplit_once('.').map(|(_, x)| x.to_lowercase()).unwrap_or_default().as_str() {
        "png" => "PNG File",
        "jpg" | "jpeg" => "JPEG Image",
        "gif" => "GIF Image",
        "txt" => "Text Document",
        "md" => "Markdown Document",
        "pdf" => "PDF Document",
        "zip" => "Compressed Folder",
        "rs" => "Rust Source File",
        "sh" => "Shell Script",
        "mp3" | "flac" | "ogg" => "Audio File",
        "mp4" | "mkv" | "mov" => "Video File",
        _ => "File",
    }
}

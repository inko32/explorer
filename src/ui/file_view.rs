//! File view: details & large-icon layouts, selection model, sorting,
//! inline rename. Emits Cmd; never touches fs directly (D6).

use crate::app::{Cmd, Cx};
use crate::fs::Entry;
use crate::icons::{self, Icon};
use egui::{Align2, Id, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Details,
    Large,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Name,
    Date,
    Type,
    Size,
}

pub struct FileView {
    entries: Vec<Entry>,
    loading: bool,
    error: Option<String>,
    sel: std::collections::BTreeSet<usize>,
    anchor: Option<usize>,
    pub view: ViewMode,
    pub show_hidden: bool,
    pub sort: (Column, bool), // (column, ascending)
    rename: Option<usize>,
    rename_buf: String,
}

impl Default for FileView {
    fn default() -> Self {
        Self::new()
    }
}

impl FileView {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            loading: false,
            error: None,
            sel: Default::default(),
            anchor: None,
            view: ViewMode::Details,
            show_hidden: false,
            sort: (Column::Name, true),
            rename: None,
            rename_buf: String::new(),
        }
    }

    // ---- state API used by App ----

    pub fn set_loading(&mut self) {
        self.loading = true;
        self.error = None;
    }

    pub fn set_error(&mut self, e: String) {
        self.loading = false;
        self.error = Some(e);
        self.entries.clear();
        self.sel.clear();
    }

    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        self.loading = false;
        self.error = None;
        self.sort_entries();
        self.sel.clear();
        self.anchor = None;
        self.rename = None;
    }

    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.visible_indices()
            .get(index)
            .and_then(|&vi| self.entries.get(vi))
    }

    pub fn first_selected(&self) -> Option<usize> {
        self.sel.iter().next().copied()
    }

    pub fn selected_count(&self) -> usize {
        self.sel.len()
    }

    pub fn selected_bytes(&self) -> u64 {
        self.sel
            .iter()
            .filter_map(|&i| self.entry(i))
            .map(|e| e.size)
            .sum()
    }

    pub fn selected_paths(&self) -> Vec<std::path::PathBuf> {
        self.sel
            .iter()
            .filter_map(|&i| self.entry(i))
            .map(|e| e.path.clone())
            .collect()
    }

    pub fn visible_count(&self) -> usize {
        self.visible_indices().len()
    }

    pub fn clear_selection(&mut self) {
        self.sel.clear();
        self.anchor = None;
    }

    pub fn select_all(&mut self) {
        let n = self.visible_indices().len();
        self.sel = (0..n).collect();
    }

    pub fn invert_selection(&mut self) {
        let n = self.visible_indices().len();
        let old = std::mem::take(&mut self.sel);
        self.sel = (0..n).filter(|i| !old.contains(i)).collect();
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.sel.clear();
    }

    pub fn start_rename(&mut self, index: usize) {
        if let Some(e) = self.entry(index) {
            self.rename_buf = e.name.clone();
            self.rename = Some(index);
            self.sel.clear();
            self.sel.insert(index);
        }
    }

    pub fn begin_rename_of(&mut self, name: &str) {
        if let Some(i) = self.visible_indices().iter().position(|&vi| self.entries[vi].name == name) {
            self.start_rename(i);
        }
    }

    pub fn cancel_rename(&mut self) {
        self.rename = None;
        self.rename_buf.clear();
    }

    // ---- internals ----

    /// Indices into `entries` that are currently visible (hidden filter).
    fn visible_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.show_hidden || !e.is_hidden)
            .map(|(i, _)| i)
            .collect()
    }

    fn sort_entries(&mut self) {
        let (col, asc) = self.sort;
        self.entries.sort_by(|a, b| {
            let dir_first = b.is_dir().cmp(&a.is_dir());
            if dir_first != std::cmp::Ordering::Equal {
                return dir_first;
            }
            let ord = match col {
                Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Column::Date => a.modified.unwrap_or(0).cmp(&b.modified.unwrap_or(0)),
                Column::Type => type_label(a).cmp(type_label(b)),
                Column::Size => a.size.cmp(&b.size),
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    // ---- rendering ----

    pub fn show(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(cx.theme.palette.window_bg))
            .show(ui, |ui| match self.view {
                ViewMode::Details => self.show_details(ui, cx, pending),
                ViewMode::Large => self.show_large(ui, cx, pending),
            });
    }

    fn show_details(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let p = &cx.theme.palette;
        let m = &cx.theme.metrics;
        let width = ui.max_rect().width();

        // column headers
        let header = Rect::from_min_size(ui.max_rect().min, Vec2::new(width, m.col_header_h));
        ui.painter().rect_filled(header, 0.0, p.col_header_bg);
        ui.painter().line_segment(
            [header.left_bottom(), header.right_bottom()],
            Stroke::new(1.0, p.separator),
        );
        let x_date = ui.max_rect().right() - m.col_date_w - m.col_type_w - m.col_size_w;
        let x_type = ui.max_rect().right() - m.col_type_w - m.col_size_w;
        let x_size = ui.max_rect().right() - m.col_size_w;
        let header_font = egui::FontId::proportional(cx.theme.fonts.small);
        for (col, label, rect) in [
            (Column::Name, "Name", Rect::from_min_max(header.min, Pos2::new(x_date, header.bottom()))),
            (
                Column::Date,
                "Date modified",
                Rect::from_min_max(Pos2::new(x_date, header.top()), Pos2::new(x_type, header.bottom())),
            ),
            (
                Column::Type,
                "Type",
                Rect::from_min_max(Pos2::new(x_type, header.top()), Pos2::new(x_size, header.bottom())),
            ),
            (
                Column::Size,
                "Size",
                Rect::from_min_max(Pos2::new(x_size, header.top()), header.right_bottom()),
            ),
        ] {
            let id = Id::new(("col", col as u8));
            let resp = ui.interact(rect, id, Sense::click());
            if resp.clicked() {
                let (c, asc) = self.sort;
                self.sort = if c == col { (col, !asc) } else { (col, true) };
                self.sort_entries();
            }
            let color = if resp.hovered() { p.text } else { p.text_subtle };
            let arrow = if self.sort.0 == col { if self.sort.1 { " ▲" } else { " ▼" } } else { "" };
            ui.painter().text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{label}{arrow}"),
                header_font.clone(),
                color,
            );
        }

        // rows (index-based iteration keeps borrows disjoint)
        let body_top = header.bottom();
        let row_h = m.row_h;
        // advance past the header so the scroll area starts below it
        ui.allocate_space(Vec2::new(width, m.col_header_h));
        egui::ScrollArea::vertical()
            .id_salt("fv_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let visible = self.visible_indices();
                let total = visible.len() as f32 * row_h;
                ui.set_min_height(total);

                // click on empty space clears selection
                let blank_top = ui.cursor().top() + total;
                let blank = Rect::from_min_size(
                    Pos2::new(ui.max_rect().left(), blank_top),
                    Vec2::new(width, row_h.max(1.0)),
                );
                let blank_resp = ui.interact(blank, Id::new("fv_blank"), Sense::click());
                if blank_resp.clicked() {
                    self.sel.clear();
                    self.anchor = None;
                }

                let base_y = ui.cursor().top();
                let font = egui::FontId::proportional(cx.theme.fonts.size);
                let small = egui::FontId::proportional(cx.theme.fonts.size);
                for (disp_i, &vi) in visible.iter().enumerate() {
                    let rect = Rect::from_min_size(
                        Pos2::new(ui.max_rect().left(), base_y + disp_i as f32 * row_h),
                        Vec2::new(width, row_h),
                    );
                    let resp = ui.interact(rect, Id::new(("row", disp_i)), Sense::click());

                    // selection (index-only; no entry borrow needed)
                    if resp.clicked() {
                        let mods = ui.ctx().input(|in_| in_.modifiers);
                        if mods.ctrl || mods.command {
                            if !self.sel.insert(disp_i) {
                                self.sel.remove(&disp_i);
                            }
                            self.anchor = Some(disp_i);
                        } else if mods.shift {
                            let a = self.anchor.unwrap_or(disp_i);
                            let (lo, hi) = if a <= disp_i { (a, disp_i) } else { (disp_i, a) };
                            self.sel = (lo..=hi).collect();
                        } else {
                            self.sel.clear();
                            self.sel.insert(disp_i);
                            self.anchor = Some(disp_i);
                        }
                    }
                    let e = &self.entries[vi];
                    if resp.double_clicked() {
                        if e.is_dir() {
                            pending.push(Cmd::Navigate(e.path.clone()));
                        } else {
                            pending.push(Cmd::Status(format!("Open “{}”: coming soon", e.name)));
                        }
                    }

                    // selection band
                    if self.sel.contains(&disp_i) {
                        let fill = if cx.focused { p.sel_fill } else { p.sel_fill_unfocused };
                        ui.painter().rect_filled(rect, 0.0, fill);
                        if cx.focused {
                            ui.painter().rect_stroke(
                                rect.shrink(0.5),
                                0.0,
                                Stroke::new(1.0, p.sel_border),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }

                    // icon + name (or inline rename editor)
                    let icon_rect = Rect::from_center_size(
                        Pos2::new(rect.left() + 6.0 + m.details_icon / 2.0, rect.center().y),
                        Vec2::splat(m.details_icon),
                    );
                    let icon = if e.is_dir() {
                        Icon::Folder
                    } else if is_image(&e.name) {
                        Icon::FileImage
                    } else {
                        Icon::File
                    };
                    icons::draw(ui.painter(), icon, icon_rect, cx.theme, true);

                    let name_x = icon_rect.right() + 4.0;
                    if self.rename == Some(disp_i) {
                        let edit_rect = Rect::from_min_max(
                            Pos2::new(name_x, rect.top() + 1.0),
                            Pos2::new(x_date - 4.0, rect.bottom() - 1.0),
                        );
                        ui.painter().rect_filled(edit_rect, 1.0, p.window_bg);
                        ui.painter().rect_stroke(
                            edit_rect.shrink(0.5),
                            1.0,
                            Stroke::new(1.0, p.accent),
                            egui::StrokeKind::Inside,
                        );
                        let mut buf = std::mem::take(&mut self.rename_buf);
                        let edit = egui::TextEdit::singleline(&mut buf)
                            .id(Id::new("fv_rename"))
                            .desired_width(edit_rect.width())
                            .margin(egui::Margin::symmetric(2, 0))
                            .frame(egui::Frame::NONE);
                        let out = ui.put(edit_rect, edit);
                        self.rename_buf = buf;
                        // singleline TextEdit surrenders focus on Enter;
                        // clicking away also loses focus — both commit.
                        if out.lost_focus() {
                            let name = self.rename_buf.clone();
                            pending.push(Cmd::CommitRename { index: disp_i, name });
                        } else {
                            ui.ctx().memory_mut(|mem| mem.request_focus(Id::new("fv_rename")));
                        }
                    } else {
                        let max_w = (x_date - 12.0 - name_x).max(10.0);
                        let shown = elide(ui.painter(), &e.name, font.clone(), p.text, max_w);
                        ui.painter().text(
                            Pos2::new(name_x, rect.center().y),
                            Align2::LEFT_CENTER,
                            shown,
                            font.clone(),
                            p.text,
                        );
                    }

                    // fixed columns
                    ui.painter().text(
                        Pos2::new(x_date + 8.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        e.modified.map(format_date).unwrap_or_default(),
                        small.clone(),
                        p.text,
                    );
                    ui.painter().text(
                        Pos2::new(x_type + 8.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        type_label(e),
                        small.clone(),
                        p.text,
                    );
                    if !e.is_dir() {
                        ui.painter().text(
                            Pos2::new(x_size - 8.0 + m.col_size_w, rect.center().y),
                            Align2::RIGHT_CENTER,
                            format_size(e.size),
                            small.clone(),
                            p.text,
                        );
                    }
                }
                ui.allocate_space(Vec2::new(width, total.max(row_h)));
            });

        if self.loading {
            ui.painter().text(
                Pos2::new(width / 2.0, body_top + 40.0),
                Align2::CENTER_CENTER,
                "Loading…",
                egui::FontId::proportional(cx.theme.fonts.size),
                p.text_subtle,
            );
        }
        if let Some(err) = &self.error {
            ui.painter().text(
                Pos2::new(width / 2.0, body_top + 40.0),
                Align2::CENTER_CENTER,
                err,
                egui::FontId::proportional(cx.theme.fonts.size),
                p.text_disabled,
            );
        }
    }

    fn show_large(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let p = &cx.theme.palette;
        let m = &cx.theme.metrics;
        let width = ui.max_rect().width();
        egui::ScrollArea::vertical().id_salt("fv_scroll_large").auto_shrink([false, false]).show(ui, |ui| {
            let visible = self.visible_indices();
            let cols = (width / m.large_cell_w).max(1.0) as usize;
            let rows = visible.len().div_ceil(cols.max(1));
            let total_h = rows as f32 * m.large_cell_h;
            ui.set_min_height(total_h);

            let base_y = ui.cursor().top();
            for (disp_i, &vi) in visible.iter().enumerate() {
                let r = disp_i / cols.max(1);
                let cidx = disp_i % cols.max(1);
                let cell = Rect::from_min_size(
                    Pos2::new(
                        ui.max_rect().left() + cidx as f32 * m.large_cell_w,
                        base_y + r as f32 * m.large_cell_h,
                    ),
                    Vec2::new(m.large_cell_w, m.large_cell_h),
                );
                let resp = ui.interact(cell, Id::new(("cell", disp_i)), Sense::click());
                if resp.clicked() {
                    let mods = ui.ctx().input(|in_| in_.modifiers);
                    if mods.ctrl || mods.command {
                        if !self.sel.insert(disp_i) {
                            self.sel.remove(&disp_i);
                        }
                        self.anchor = Some(disp_i);
                    } else {
                        self.sel.clear();
                        self.sel.insert(disp_i);
                        self.anchor = Some(disp_i);
                    }
                }
                let e = &self.entries[vi];
                if resp.double_clicked() && e.is_dir() {
                    pending.push(Cmd::Navigate(e.path.clone()));
                }
                if self.sel.contains(&disp_i) {
                    let fill = if cx.focused { p.sel_fill } else { p.sel_fill_unfocused };
                    ui.painter().rect_filled(cell.shrink(2.0), 0.0, fill);
                    if cx.focused {
                        ui.painter().rect_stroke(
                            cell.shrink(2.5),
                            0.0,
                            Stroke::new(1.0, p.sel_border),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                let icon_rect = Rect::from_center_size(
                    Pos2::new(cell.center().x, cell.top() + m.large_icon / 2.0 + 6.0),
                    Vec2::splat(m.large_icon),
                );
                let icon = if e.is_dir() { Icon::Folder } else if is_image(&e.name) { Icon::FileImage } else { Icon::File };
                icons::draw(ui.painter(), icon, icon_rect, cx.theme, true);
                // wrapped label, clipped to the cell
                let mut label_painter = ui.painter().clone();
                label_painter.set_clip_rect(cell.shrink(2.0));
                let galley = label_painter.layout(
                    e.name.clone(),
                    egui::FontId::proportional(cx.theme.fonts.small),
                    p.text,
                    cell.width() - 8.0,
                );
                label_painter.galley(
                    Pos2::new(cell.center().x - galley.size().x / 2.0, icon_rect.bottom() + 6.0),
                    galley,
                    p.text,
                );
            }
            ui.allocate_space(Vec2::new(width, total_h.max(m.large_cell_h)));
        });
    }

    // ---- demo content for headless snapshots ----

    pub fn load_demo(&mut self) {
        self.loading = false;
        self.entries = demo_entries();
        self.sort_entries();
        self.sel.clear();
    }
}

/// Single-line text truncated with "…" to fit `max_w` (no wrap, cheap binary
/// search — names are short).
fn elide(painter: &Painter, text: &str, font: egui::FontId, color: egui::Color32, max_w: f32) -> String {
    let width_of = |s: &str| painter.layout_no_wrap(s.to_string(), font.clone(), color).rect.width();
    if width_of(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let dots = width_of("…");
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let prefix: String = chars[..mid].iter().collect();
        if width_of(&prefix) + dots <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    format!("{}…", chars[..lo].iter().collect::<String>())
}

// ---- formatting helpers (pure functions — UI-spec owned) ----

pub fn format_size(bytes: u64) -> String {
    const K: u64 = 1024;
    if bytes < K {
        format!("{} B", bytes)
    } else if bytes < K * K {
        dec(bytes as f64 / K as f64, "KB")
    } else if bytes < K * K * K {
        dec(bytes as f64 / (K * K) as f64, "MB")
    } else if bytes < K * K * K * K {
        dec(bytes as f64 / (K * K * K) as f64, "GB")
    } else {
        dec(bytes as f64 / (K as f64).powi(4), "TB")
    }
}

fn dec(v: f64, unit: &str) -> String {
    if v >= 100.0 {
        format!("{:.0} {}", v, unit)
    } else if v >= 10.0 {
        format!("{:.1} {}", v, unit)
    } else {
        format!("{:.2} {}", v, unit)
    }
}

/// "3/12/2012 10:20 PM" — Win8 en-US short format (hand-rolled, no chrono).
pub fn format_date(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86400);
    let secs = epoch_secs.rem_euclid(86400);
    let (y, mo, d) = civil_from_days(days);
    let h24 = secs / 3600;
    let mi = (secs % 3600) / 60;
    let (h12, ampm) = if h24 == 0 { (12, "AM") } else if h24 < 12 { (h24, "AM") } else if h24 == 12 { (12, "PM") } else { (h24 - 12, "PM") };
    format!("{}/{}/{} {}:{:02} {}", mo, d, y, h12, mi, ampm)
}

/// Howard Hinnant's civil_from_days.
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

pub fn type_label(e: &Entry) -> &'static str {
    if e.is_symlink {
        return "Shortcut";
    }
    if e.is_dir() {
        return "File folder";
    }
    match ext(&e.name).as_str() {
        "png" => "PNG File",
        "jpg" | "jpeg" => "JPEG Image",
        "gif" => "GIF Image",
        "svg" | "webp" => "Image File",
        "txt" | "text" => "Text Document",
        "md" => "Markdown Document",
        "doc" | "docx" => "Microsoft Word Document",
        "xls" | "xlsx" => "Microsoft Excel Worksheet",
        "pdf" => "PDF Document",
        "mp3" | "flac" | "ogg" => "Audio File",
        "mp4" | "mkv" | "mov" | "webm" => "Video File",
        "zip" => "Compressed Folder",
        "tar" | "gz" | "xz" | "zst" => "Archive File",
        "rs" => "Rust Source File",
        "toml" => "TOML Configuration",
        "sh" => "Shell Script",
        "ttf" | "otf" => "TrueType Font File",
        "exe" => "Application",
        _ => "File",
    }
}

fn ext(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

fn is_image(name: &str) -> bool {
    matches!(ext(name).as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg")
}

fn demo_entries() -> Vec<Entry> {
    let mut v = Vec::new();
    let mut add = |name: &str, dir: bool, size: u64, modified: i64| {
        v.push(Entry {
            name: name.into(),
            path: std::path::PathBuf::from("/demo").join(name),
            kind: if dir { crate::fs::EntryKind::Dir } else { crate::fs::EntryKind::File },
            size,
            modified: Some(modified),
            is_hidden: name.starts_with('.'),
            is_symlink: false,
            symlink_target: None,
        });
    };
    let t = |y: i64, mo: i64, d: i64, h: i64, mi: i64| -> i64 {
        days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60
    };
    add("Data", true, 0, t(2012, 3, 12, 10, 20));
    add("Data - Copy", true, 0, t(2012, 3, 12, 17, 48));
    add("Excel-CLE", true, 0, t(2012, 2, 18, 11, 2));
    add("Grading Sheets", true, 0, t(2012, 6, 6, 17, 16));
    add("iBook Covers", true, 0, t(2012, 7, 24, 15, 47));
    add("Inluit", true, 0, t(2012, 9, 26, 11, 0));
    add("iPhone", true, 0, t(2012, 9, 27, 12, 22));
    add("OneNote", true, 0, t(2012, 10, 5, 8, 29));
    add("Outlook-CLE", true, 0, t(2012, 3, 27, 14, 28));
    add("Photoshop", true, 0, t(2012, 7, 18, 11, 8));
    add("Public", true, 0, t(2012, 7, 14, 18, 43));
    add("Vacation Pics", true, 0, t(2012, 9, 16, 18, 33));
    add("Videos", true, 0, t(2012, 9, 2, 10, 16));
    add("background.png", false, 302_350, t(2012, 5, 30, 10, 27));
    add("CLE Submittal Form.docx", false, 47_104, t(2011, 12, 27, 9, 21));
    add("Collegiate-Normal.ttf", false, 23_552, t(1995, 7, 26, 17, 36));
    v
}

/// Inverse of civil_from_days (days since epoch for y/m/d).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

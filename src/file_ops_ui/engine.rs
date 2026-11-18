//! The (fake, /tmp-only) file-operation engine.
//!
//! Operations are planned by scanning the real directory trees under
//! `/tmp/fileops_demo`, progress is simulated at a lively rate, and the
//! actual disk work (copy/delete) is batched at completion — also under
//! the demo root. Nothing outside `/tmp` is ever read or written here.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Everything the engine is allowed to touch. Hard rule: no user data.
pub const DEMO_ROOT: &str = "/tmp/fileops_demo";

/// Returns Ok if `p` is inside the tmp demo root (lexical check; the
/// root is created by the demo seeder, so canonicalize works after
/// seeding, and parent-dir walking handles not-yet-created subpaths).
pub fn guard_tmp(p: &Path) -> Result<PathBuf, String> {
    let root = Path::new(DEMO_ROOT);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(p)
    };
    // normalize "."/".." lexically without requiring existence
    let mut norm = PathBuf::from("/");
    for comp in abs.components() {
        match comp {
            std::path::Component::RootDir => {}
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                norm.pop();
            }
            other => norm.push(other.as_os_str()),
        }
    }
    if norm.starts_with(root) {
        Ok(norm)
    } else {
        Err(format!(
            "file-ops-ui sandbox: refusing to touch '{}' (only {} is allowed)",
            norm.display(),
            DEMO_ROOT
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Copy,
    Move,
    Delete,
}

impl OpKind {
    pub fn verb(self) -> &'static str {
        match self {
            OpKind::Copy => "Copying",
            OpKind::Move => "Moving",
            OpKind::Delete => "Deleting",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Failed feeds D1 panel type F
pub enum OpState {
    /// counting items; shows the shimmer graph + "Discovered N items..."
    Preparing,
    Running,
    Paused,
    /// D2 prompt is up (embedded or standalone)
    NeedsDecision,
    /// D3 is open on top
    WaitingForD3,
    Cancelling,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    ReplaceAll,
    SkipAll,
    PerFile,
}

/// One planned unit of work (a flattened file, or a top-level tree entry
/// for deletes which we treat as one item with an aggregate size).
#[derive(Debug, Clone)]
pub struct Item {
    pub rel: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Conflict {
    pub name: String,
    pub src_modified: String,
    pub src_size: u64,
    pub dst_modified: String,
    pub dst_size: u64,
    /// keep the source version (replace / copy as "name (2)")
    pub keep_src: bool,
    /// keep the destination version (skip)
    pub keep_dst: bool,
    /// same size and modified-to-the-second (D3 "Skip N files..." rows)
    pub same_date_size: bool,
}

#[derive(Debug, Clone)]
pub struct FileOp {
    pub id: u64,
    pub kind: OpKind,
    /// display labels: last path components
    pub src_label: String,
    pub dst_label: String,
    pub src_path: PathBuf,
    pub dst_path: Option<PathBuf>,
    pub items: Vec<Item>,
    pub total_items: u64,
    pub total_bytes: u64,
    pub done_items: u64,
    pub done_bytes: u64,
    pub cursor: usize,
    /// cumulative bytes of the items before `cursor` (O(1) cursor advance)
    pub prefix_bytes: u64,
    pub current_name: String,
    pub state: OpState,
    pub conflicts: Vec<Conflict>,
    pub resolution: Option<ConflictChoice>,
    /// speed history, one entry per graph column (spec: 391 columns)
    pub samples: Vec<f32>,
    pub col_cursor: f32,
    pub rate: f32,
    pub base_rate: f32,
    pub scale_max: f32,
    /// discovery animation progress 0..1
    pub disc_t: f32,
    pub phase: f32,
    pub started: Instant,
    pub state_since: Instant,
    pub finished_at: Option<Instant>,
    /// error message for panel type F
    pub error: Option<String>,
}

impl FileOp {
    fn transition(&mut self, state: OpState) {
        self.state = state;
        self.state_since = Instant::now();
    }

    pub fn progress(&self) -> f32 {
        if self.total_bytes == 0 {
            return if self.disc_t >= 1.0 { 1.0 } else { 0.0 };
        }
        (self.done_bytes as f32 / self.total_bytes as f32).min(1.0)
    }

    pub fn fraction_percent(&self) -> u32 {
        let p = self.progress();
        if p >= 1.0 {
            100
        } else {
            (p * 100.0) as u32
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.state, OpState::Completed)
    }

    pub fn needs_removal(&self) -> bool {
        match self.state {
            OpState::Completed => self
                .finished_at
                .is_some_and(|t| t.elapsed() > Duration::from_millis(1500)),
            OpState::Cancelling => self.state_since.elapsed() > Duration::from_millis(700),
            _ => false,
        }
    }

    pub fn headline(&self) -> String {
        match self.state {
            OpState::Preparing => format!(
                "Discovered {} items ({})...",
                crate::file_ops_ui::format::count(self.discovered_items()),
                crate::file_ops_ui::format::size(self.discovered_bytes(), 1),
            ),
            OpState::Paused => format!("Paused - {}% complete", self.fraction_percent()),
            OpState::Cancelling => "Cancelling...".to_string(),
            _ => format!("{}% complete", self.fraction_percent()),
        }
    }

    pub fn discovered_items(&self) -> u64 {
        (ease_out(self.disc_t) * self.total_items as f32) as u64
    }

    pub fn discovered_bytes(&self) -> u64 {
        (ease_out(self.disc_t) * self.total_bytes as f32) as u64
    }

    pub fn remaining_secs(&self) -> Option<f64> {
        if self.rate <= 1.0 || self.done_bytes >= self.total_bytes {
            return None;
        }
        Some((self.total_bytes - self.done_bytes) as f64 / self.rate as f64)
    }

    /// "About 5 seconds" / "Calculating..." / "Paused"
    pub fn time_text(&self) -> String {
        match self.state {
            OpState::Paused => "Paused".to_string(),
            OpState::Preparing => "Calculating...".to_string(),
            _ => match self.remaining_secs() {
                None if self.started.elapsed() < Duration::from_secs(3) => {
                    "Calculating...".to_string()
                }
                None => "About 5 seconds".to_string(),
                Some(s) => crate::file_ops_ui::format::time_remaining(s),
            },
        }
    }

    /// Items remaining + size: "2,597 (4.40 GB)"
    pub fn remaining_items_text(&self) -> String {
        let items = self.total_items.saturating_sub(self.done_items);
        let bytes = self.total_bytes.saturating_sub(self.done_bytes);
        format!(
            "{} ({})",
            crate::file_ops_ui::format::count(items),
            crate::file_ops_ui::format::size(bytes, 2)
        )
    }
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

fn label_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn modified_string(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| {
            let dt: chrono_like::DateTime = t.into();
            dt.format()
        })
        .unwrap_or_default()
}

/// Tiny date formatter (d/m/yyyy h:mm am/pm) — std-only, no chrono.
mod chrono_like {
    use std::time::{SystemTime, UNIX_EPOCH};
    pub struct DateTime {
        secs: u64,
    }
    impl From<SystemTime> for DateTime {
        fn from(t: SystemTime) -> Self {
            Self {
                secs: t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            }
        }
    }
    impl DateTime {
        pub fn format(&self) -> String {
            let days = self.secs / 86400;
            let tod = self.secs % 86400;
            let (y, m, d) = civil_from_days(days as i64);
            let hour = tod / 3600;
            let min = (tod % 3600) / 60;
            let (h12, ampm) = if hour.is_multiple_of(12) { (12, "am") } else if hour < 12 {
                (hour, "am")
            } else {
                (hour - 12, "pm")
            };
            format!("{d}/{m}/{y} {h12}:{min:02} {ampm}")
        }
    }
    /// Howard Hinnant's civil_from_days.
    fn civil_from_days(z: i64) -> (i64, u64, u64) {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }
}

/// Recursive scan: flattened files with sizes (for copy/move), or
/// top-level entries with aggregate sizes (for delete).
fn scan_files(dir: &Path, out: &mut Vec<Item>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        let rel = PathBuf::from(entry.file_name());
        if meta.is_dir() {
            scan_files(&path, out)?;
        } else {
            out.push(Item { rel, size: meta.len() });
        }
    }
    Ok(())
}


fn aggregate_size(path: &Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| aggregate_size(&e.path()))
                .sum()
        })
        .unwrap_or(0)
}

pub fn build_copy(id: u64, src: PathBuf, dst: PathBuf) -> Result<FileOp, String> {
    let src = guard_tmp(&src)?;
    let dst = guard_tmp(&dst)?;
    if !src.is_dir() {
        return Err(format!("copy source '{}' is not a demo directory", src.display()));
    }
    let mut items = Vec::new();
    scan_files(&src, &mut items)?;
    if items.is_empty() {
        return Err(format!("copy source '{}' is empty", src.display()));
    }
    let total_bytes = items.iter().map(|i| i.size).sum();
    let mut conflicts = Vec::new();
    if dst.is_dir() {
        for item in items
            .iter()
            .filter(|i| i.rel.components().count() == 1)
        {
            let dst_file = dst.join(&item.rel);
            if dst_file.exists() {
                let src_file = src.join(&item.rel);
                let (src_size, src_modified) = std::fs::metadata(&src_file)
                    .map(|m| (m.len(), modified_string(&src_file)))
                    .unwrap_or((item.size, String::new()));
                let (dst_size, dst_modified) = std::fs::metadata(&dst_file)
                    .map(|m| (m.len(), modified_string(&dst_file)))
                    .unwrap_or((0, String::new()));
                let same_date_size =
                    src_size == dst_size && src_modified == dst_modified;
                conflicts.push(Conflict {
                    name: item.rel.display().to_string(),
                    src_modified,
                    src_size,
                    dst_modified,
                    dst_size,
                    keep_src: true,
                    keep_dst: false,
                    same_date_size,
                });
            }
        }
    }
    let total_items = items.len() as u64;
    Ok(FileOp {
        id,
        kind: OpKind::Copy,
        src_label: label_of(&src),
        dst_label: label_of(&dst),
        src_path: src,
        dst_path: Some(dst),
        items,
        total_items,
        total_bytes,
        done_items: 0,
        done_bytes: 0,
        cursor: 0,
        prefix_bytes: 0,
        current_name: String::new(),
        state: OpState::Preparing,
        conflicts,
        resolution: None,
        samples: vec![0.0; crate::file_ops_ui::d1::GRAPH_COLS],
        col_cursor: 0.0,
        rate: 0.0,
        base_rate: (40.0 + 30.0 * (id as f32 * 0.7).sin().abs()) * 1024.0 * 1024.0,
        scale_max: 1.0,
        disc_t: 0.0,
        phase: id as f32,
        started: Instant::now(),
        state_since: Instant::now(),
        finished_at: None,
        error: None,
    })
}

pub fn build_delete(id: u64, paths: Vec<PathBuf>, src_dir: PathBuf) -> Result<FileOp, String> {
    let mut items = Vec::new();
    for p in &paths {
        let p = guard_tmp(p)?;
        let meta = std::fs::metadata(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let size = if meta.is_dir() {
            aggregate_size(&p)
        } else {
            meta.len()
        };
        items.push(Item {
            rel: PathBuf::from(p.file_name().unwrap_or_default()),
            size,
        });
    }
    let total_bytes = items.iter().map(|i| i.size).sum();
    let total_items = aggregate_items(&paths);
    Ok(FileOp {
        id,
        kind: OpKind::Delete,
        src_label: label_of(&src_dir),
        dst_label: String::new(),
        src_path: src_dir,
        dst_path: None,
        items,
        total_items,
        total_bytes,
        done_items: 0,
        done_bytes: 0,
        cursor: 0,
        prefix_bytes: 0,
        current_name: String::new(),
        state: OpState::Preparing,
        conflicts: Vec::new(),
        resolution: None,
        samples: vec![0.0; crate::file_ops_ui::d1::GRAPH_COLS],
        col_cursor: 0.0,
        rate: 0.0,
        // deletes count items/s; base_rate is recycled as items/s
        base_rate: 400.0 + 150.0 * (id as f32).sin().abs(),
        scale_max: 1.0,
        disc_t: 0.0,
        phase: id as f32,
        started: Instant::now(),
        state_since: Instant::now(),
        finished_at: None,
        error: None,
    })
}

/// Total leaf count across the deleted trees (for "Deleting N items").
fn aggregate_items(paths: &[PathBuf]) -> u64 {
    fn count(path: &Path) -> u64 {
        if path.is_file() {
            return 1;
        }
        std::fs::read_dir(path)
            .map(|entries| entries.filter_map(|e| e.ok()).map(|e| count(&e.path())).sum::<u64>().max(1))
            .unwrap_or(1)
    }
    paths.iter().map(|p| count(p)).sum()
}

/// One 100 ms tick of the simulation.
pub fn tick(op: &mut FileOp, dt: f32) {
    match op.state {
        OpState::Preparing => {
            op.disc_t += dt / 2.2;
            if op.disc_t >= 1.0 {
                op.disc_t = 1.0;
                // a fresh conflict needs a decision before it can run
                if !op.conflicts.is_empty() {
                    op.transition(OpState::NeedsDecision);
                } else {
                    op.transition(OpState::Running);
                }
            }
        }
        OpState::Running => {
            advance(op, dt);
        }
        _ => {}
    }
}

fn advance(op: &mut FileOp, dt: f32) {
    op.phase += dt * 2.1;
    let noise = 0.72 + 0.28 * op.phase.sin() * op.phase.mul_add(0.37, 1.0).cos().abs();
    op.rate = op.base_rate * noise;
    if op.kind == OpKind::Delete {
        // item-driven: ~items/s rate
        let items = (op.rate * dt).max(1.0);
        let mut bytes = 0u64;
        for _ in 0..items as u64 {
            if let Some(item) = op.items.get(op.cursor) {
                bytes += item.size;
                op.prefix_bytes += item.size.max(1);
                op.current_name = item.rel.display().to_string();
                op.cursor += 1;
                op.done_items += 1;
            } else {
                break;
            }
        }
        op.done_bytes = (op.done_bytes + bytes).min(op.total_bytes);
    } else {
        let bytes = (op.rate * dt) as u64;
        op.done_bytes = (op.done_bytes + bytes).min(op.total_bytes);
        // advance the item cursor so "Name:" tracks the current file
        while let Some(item) = op.items.get(op.cursor) {
            let end = op.prefix_bytes + item.size.max(1);
            if end > op.done_bytes && op.done_bytes < op.total_bytes {
                break;
            }
            op.prefix_bytes = end;
            op.current_name = item.rel.display().to_string();
            op.cursor += 1;
            op.done_items += 1;
        }
        if op.done_bytes >= op.total_bytes {
            op.done_items = op.total_items;
        }
    }
    // graph columns: one column per 1/391 of the total (spec D1.3)
    let new_col = op.progress() * crate::file_ops_ui::d1::GRAPH_COLS as f32;
    let from = op.col_cursor as usize;
    let to = (new_col as usize).min(crate::file_ops_ui::d1::GRAPH_COLS);
    for s in op.samples.iter_mut().take(to).skip(from) {
        *s = op.rate;
    }
    op.col_cursor = new_col;
    let hi = op.samples.iter().copied().fold(0.0f32, f32::max);
    op.scale_max = (hi * 1.6).max(1.0);
    if op.done_bytes >= op.total_bytes {
        op.finished_at = Some(Instant::now());
        op.transition(OpState::Completed);
        materialize(op);
    }
}

/// Apply the result to disk — the ONLY place that writes, always under
/// the demo root (paths were guarded at build time).
fn materialize(op: &FileOp) {
    match op.kind {
        OpKind::Copy | OpKind::Move => {
            let Some(dst) = &op.dst_path else { return };
            for item in &op.items {
                let src = op.src_path.join(&item.rel);
                let mut target = dst.join(&item.rel);
                if let Some(parent) = target.parent()
                    && !parent.exists()
                    && std::fs::create_dir_all(parent).is_err()
                {
                    continue;
                }
                let conflict = op.conflicts.iter().find(|c| c.name == item.rel.display().to_string());
                let overwrite = match (conflict, op.resolution) {
                    (None, _) => true,
                    (Some(c), Some(ConflictChoice::ReplaceAll)) => c.keep_src || !c.keep_dst,
                    (Some(_), Some(ConflictChoice::SkipAll)) => false,
                    (Some(c), Some(ConflictChoice::PerFile)) => {
                        if c.keep_src && c.keep_dst {
                            target = next_free_name(&target);
                            true
                        } else {
                            c.keep_src
                        }
                    }
                    (Some(_), None) => false,
                };
                if !overwrite {
                    continue;
                }
                if std::fs::copy(&src, &target).is_err() {
                    // best effort at this stage
                }
                if op.kind == OpKind::Move {
                    let _ = std::fs::remove_file(&src);
                }
            }
        }
        OpKind::Delete => {
            for item in &op.items {
                let _ = std::fs::remove_dir_all(op.src_path.join(&item.rel));
            }
        }
    }
}

/// "name.ext" -> "name (2).ext" with the next free number (spec D3.3).
fn next_free_name(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..1000 {
        let candidate = parent.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// Conflict apply used by D3 Continue: snapshot into the op's resolution.
pub fn apply_per_file(op: &mut FileOp, conflicts: Vec<Conflict>) {
    op.conflicts = conflicts;
    op.resolution = Some(ConflictChoice::PerFile);
    op.transition(OpState::Running);
}

// ---------------------------------------------------------------------------
// Demo seeding: builds the /tmp/fileops_demo tree used by `--fileop=demo`.
// Sparse files (truncate) keep this fast and allocation-free.
// ---------------------------------------------------------------------------

pub fn seed_demo() -> Result<(), String> {
    let root = Path::new(DEMO_ROOT);
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;

    let src = root.join("Software");
    let dst = root.join("{cf3a7d2a-256f-4ef3-b129-6400c44ebd42}");
    let del = root.join("Old Backups");
    for d in [&src, &dst, &del] {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }

    // src: a believable pile of "files" with two names that also exist in dst
    let files: &[(&str, u64)] = &[
        ("archive.zip", 700 * 1024 * 1024),
        ("package.7z", 119 * 1024 * 1024),
        ("Editor.dll", 42 * 1024 * 1024),
        ("System.Numerics.Vectors.dll", 38 * 1024 * 1024),
        ("release-notes.txt", 120 * 1024),
        ("setup.exe", 260 * 1024 * 1024),
        ("README.md", 48 * 1024),
        ("core.dat", 512 * 1024 * 1024),
    ];
    for (name, size) in files {
        make_sparse(&src.join(name), *size)?;
    }
    // a subdirectory so the tree copy is visible
    std::fs::create_dir_all(src.join("docs")).map_err(|e| e.to_string())?;
    for i in 0..12 {
        make_sparse(&src.join("docs").join(format!("doc-{i:02}.pdf")), 2 * 1024 * 1024)?;
    }

    // dst: the two conflicts with different sizes/dates
    make_sparse(&dst.join("archive.zip"), 650 * 1024 * 1024)?;
    make_sparse(&dst.join("package.7z"), 119 * 1024 * 1024)?;
    make_sparse(&dst.join("existing-file.bin"), 1024 * 1024)?;

    // del: items for the delete flow
    for i in 0..5 {
        make_sparse(&del.join(format!("backup-{i}.dat")), 8 * 1024 * 1024)?;
    }

    Ok(())
}

fn make_sparse(path: &Path, size: u64) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.set_len(size).map_err(|e| e.to_string())?;
    Ok(())
}

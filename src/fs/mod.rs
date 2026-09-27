//! Filesystem layer.
//!
//! The rest of the app only ever sees [`Backend`], [`Scheduler`] and the
//! plain-data types below. GIO is the default backend; POSIX / Windows /
//! Rust-native implementations can replace it without UI changes (D4).

mod gio_backend;
pub mod model;

pub use model::{Entry, EntryKind, FsError, Volume};

use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Swappable filesystem backend. All methods are synchronous; they run on the
/// scheduler's worker thread so the UI never blocks.
pub trait Backend: Send + 'static {
    fn home(&self) -> PathBuf;
    /// List the immediate children of `path`.
    fn list(&self, path: &Path) -> Result<Vec<Entry>, FsError>;
    /// Mounted volumes for "This PC" (pseudo filesystems filtered out).
    fn volumes(&self) -> Vec<Volume>;
    fn create_dir(&self, path: &Path) -> Result<(), FsError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    /// Move to trash (GIO trash / freedesktop trash on POSIX).
    fn trash(&self, path: &Path) -> Result<(), FsError>;
    /// Remove a file or an *empty* directory (used after cut-paste of files).
    fn remove(&self, path: &Path) -> Result<(), FsError>;
    /// Copy one file; `progress` reports (bytes_done, bytes_total).
    fn copy_file(&self, src: &Path, dst: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), FsError>;
    /// True if `path` exists and is a directory.
    fn is_dir(&self, path: &Path) -> bool;
    /// Filesystem type of a mounted path (for filtering pseudo filesystems).
    fn fs_type(&self, path: &Path) -> Option<String>;
}

#[derive(Debug)]
pub enum Job {
    List(PathBuf),
    Volumes,
    Mkdir(PathBuf),
    Rename(PathBuf, PathBuf),
    Trash(Vec<PathBuf>),
    Copy { sources: Vec<PathBuf>, dest_dir: PathBuf, cut: bool },
}

#[derive(Debug)]
pub enum FsEvent {
    Listed { path: PathBuf, result: Result<Vec<Entry>, String> },
    Volumes(Vec<Volume>),
    DirCreated { parent: PathBuf, result: Result<String, String> },
    Renamed { result: Result<(), String> },
    Trashed { result: Result<(), String> },
    /// Progress of the active file operation.
    OpProgress { done_bytes: u64, total_bytes: u64, current: PathBuf },
    OpDone { result: Result<(), String> },
}

/// Job queue handle (cloneable). App holds one; commands enqueue jobs.
#[derive(Clone)]
pub struct Scheduler {
    tx: mpsc::Sender<Job>,
}

impl Scheduler {
    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

/// Spawn the worker thread with the default (GIO) backend.
pub fn spawn() -> (Scheduler, mpsc::Receiver<FsEvent>) {
    spawn_with(gio_backend::GioBackend::new())
}

pub fn spawn_with(backend: impl Backend) -> (Scheduler, mpsc::Receiver<FsEvent>) {
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (ev_tx, ev_rx) = mpsc::channel::<FsEvent>();
    std::thread::Builder::new()
        .name("explorer-fs".into())
        .spawn(move || worker(backend, job_rx, ev_tx))
        .expect("failed to spawn fs worker");
    (Scheduler { tx: job_tx }, ev_rx)
}

fn worker(backend: impl Backend, jobs: mpsc::Receiver<Job>, tx: mpsc::Sender<FsEvent>) {
    while let Ok(job) = jobs.recv() {
        match job {
            Job::List(path) => {
                let result = backend.list(&path).map_err(|e| e.to_string());
                let _ = tx.send(FsEvent::Listed { path, result });
            }
            Job::Volumes => {
                let _ = tx.send(FsEvent::Volumes(backend.volumes()));
            }
            Job::Mkdir(parent) => {
                let result = new_folder_name(&backend, &parent);
                let _ = tx.send(FsEvent::DirCreated { parent, result });
            }
            Job::Rename(from, to) => {
                let result = backend.rename(&from, &to).map_err(|e| e.to_string());
                let _ = tx.send(FsEvent::Renamed { result });
            }
            Job::Trash(paths) => {
                let mut result = Ok(());
                for p in &paths {
                    if let Err(e) = backend.trash(p) {
                        result = Err(e.to_string());
                        break;
                    }
                }
                let _ = tx.send(FsEvent::Trashed { result });
            }
            Job::Copy { sources, dest_dir, cut } => {
                let result = copy_tree(&backend, &sources, &dest_dir, cut, &tx);
                let _ = tx.send(FsEvent::OpDone { result });
            }
        }
    }
}

/// Pick "New folder", "New folder (2)", ... and create it.
fn new_folder_name(backend: &impl Backend, parent: &Path) -> Result<String, String> {
    for n in 0..100 {
        let name = if n == 0 { "New folder".to_string() } else { format!("New folder ({})", n + 1) };
        let path = parent.join(&name);
        if !backend.is_dir(&path) {
            backend.create_dir(&path).map_err(|e| e.to_string())?;
            return Ok(name);
        }
    }
    Err("could not pick a folder name".into())
}

/// Copy (or move) `sources` into `dest_dir` recursively, auto-renaming
/// conflicts to "name (2).ext" (D9 — real conflict dialog comes later).
fn copy_tree(
    backend: &impl Backend,
    sources: &[PathBuf],
    dest_dir: &Path,
    cut: bool,
    tx: &mpsc::Sender<FsEvent>,
) -> Result<(), String> {
    // Gather all files with sizes first for progress reporting.
    let mut plan: Vec<(PathBuf, PathBuf, u64)> = Vec::new(); // (src, dst, bytes)
    let mut total = 0u64;
    for src in sources {
        collect_plan(backend, src, dest_dir, &mut plan, &mut total)?;
    }
    let mut done = 0u64;
    for (src, dst, bytes) in &plan {
        if let Some(parent) = dst.parent() {
            if !backend.is_dir(parent) {
                // create intermediate dirs (best effort, errors surface at copy)
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let dst = if backend.is_dir(&dst) || dst.exists() {
            auto_rename(&dst)
        } else {
            dst.clone()
        };
        let mut last_report = std::time::Instant::now();
        let mut current_file_done = 0u64;
        let result = backend.copy_file(src, &dst, &mut |d, _t| {
            current_file_done = d;
            if last_report.elapsed().as_millis() > 100 {
                last_report = std::time::Instant::now();
                let _ = tx.send(FsEvent::OpProgress {
                    done_bytes: done + d,
                    total_bytes: total,
                    current: src.clone(),
                });
            }
        });
        match result {
            Ok(()) => {
                done += bytes.max(current_file_done);
                let _ = tx.send(FsEvent::OpProgress { done_bytes: done, total_bytes: total, current: src.clone() });
            }
            Err(e) => return Err(format!("{}: {}", src.display(), e)),
        }
    }
    if cut {
        // Remove sources deepest-first so dirs are emptied before removal.
        let mut sorted: Vec<&PathBuf> = sources.iter().collect();
        sorted.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for src in sorted {
            if src.is_dir() {
                let _ = std::fs::remove_dir_all(src); // backend.remove handles empty dirs only
            } else {
                let _ = backend.remove(src);
            }
        }
    }
    Ok(())
}

fn collect_plan(
    backend: &impl Backend,
    src: &Path,
    dst_dir: &Path,
    plan: &mut Vec<(PathBuf, PathBuf, u64)>,
    total: &mut u64,
) -> Result<(), String> {
    let name = src
        .file_name()
        .ok_or_else(|| format!("no filename in {}", src.display()))?;
    let dst = dst_dir.join(name);
    if backend.is_dir(src) {
        plan.push((src.to_path_buf(), dst.clone(), 0));
        for e in backend.list(src).map_err(|e| e.to_string())? {
            collect_plan(backend, &e.path, &dst, plan, total)?;
        }
    } else {
        let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
        *total += size;
        plan.push((src.to_path_buf(), dst, size));
    }
    Ok(())
}

fn auto_rename(dst: &Path) -> PathBuf {
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

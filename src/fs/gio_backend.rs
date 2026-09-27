//! GIO backend (glib-rs). Runs synchronously on the worker thread.

use super::{Backend, Entry, EntryKind, FsError, Volume};
use gio::prelude::*;
use std::path::{Path, PathBuf};

const ATTRIBUTES: &str = "standard::name,standard::type,standard::size,standard::is-hidden,standard::is-symlink,standard::symlink-target,time::modified";

#[allow(dead_code)]
pub struct GioBackend {
    home: PathBuf,
}

impl GioBackend {
    pub fn new() -> Self {
        Self { home: glib::home_dir() }
    }
}

impl Backend for GioBackend {
    fn home(&self) -> PathBuf {
        self.home.clone()
    }

    fn list(&self, path: &Path) -> Result<Vec<Entry>, FsError> {
        let file = gio::File::for_path(path);
        let enumerator = file.enumerate_children(
            ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )?;
        let mut out = Vec::new();
        for info in enumerator.into_iter().flatten() {
            let info_name = info.name(); // PathBuf in gio 0.21
            let name = info_name.to_string_lossy().into_owned();
            let child = file.child(&info_name);
            let path = child.path().unwrap_or_else(|| path.join(&name));
            let kind = match info.file_type() {
                gio::FileType::Directory => EntryKind::Dir,
                gio::FileType::Regular => EntryKind::File,
                _ => EntryKind::Other,
            };
            let modified = info
                .modification_time()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
                .or_else(|| {
                    // fall back to std metadata when GIO has no time (e.g. some symlinks)
                    std::fs::metadata(&path).ok().and_then(|m| {
                        m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64)
                    })
                });
            let is_symlink = info.is_symlink();
            let is_hidden = info.is_hidden() || name.starts_with('.');
            let symlink_target = if is_symlink {
                std::fs::read_link(&path).ok().map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            };
            out.push(Entry {
                name,
                path,
                kind,
                size: info.size().max(0) as u64,
                modified,
                is_hidden,
                is_symlink,
                symlink_target,
            });
        }
        Ok(out)
    }

    fn volumes(&self) -> Vec<Volume> {
        let monitor = gio::VolumeMonitor::get();
        let mut out: Vec<Volume> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for mount in monitor.mounts() {
            if mount.is_shadowed() {
                continue;
            }
            let root = mount.root();
            let Some(path) = root.path() else { continue };
            if !seen.insert(path.clone()) {
                continue;
            }
            let fs_type = root
                .query_filesystem_info("filesystem::type", None::<&gio::Cancellable>)
                .ok()
                .and_then(|fi| fi.attribute_string("filesystem::type").map(|s| s.to_string()));
            if fs_type.as_deref().map(is_pseudo_fs).unwrap_or(false) {
                continue;
            }
            let name = mount.name().to_string();
            out.push(Volume { name, mount: path, fs_type });
        }
        // Sort by mount path for stable ordering.
        out.sort_by(|a, b| a.mount.cmp(&b.mount));
        out
    }

    fn create_dir(&self, path: &Path) -> Result<(), FsError> {
        gio::File::for_path(path).make_directory(None::<&gio::Cancellable>)?;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        gio::File::for_path(from).move_(
            &gio::File::for_path(to),
            gio::FileCopyFlags::NONE,
            None::<&gio::Cancellable>,
            None::<&mut dyn FnMut(i64, i64)>,
        )?;
        Ok(())
    }

    fn trash(&self, path: &Path) -> Result<(), FsError> {
        // Fall back to permanent delete when no trash is available (rare).
        let file = gio::File::for_path(path);
        match file.trash(None::<&gio::Cancellable>) {
            Ok(()) => Ok(()),
            Err(_) => {
                file.delete(None::<&gio::Cancellable>)?;
                Ok(())
            }
        }
    }

    fn remove(&self, path: &Path) -> Result<(), FsError> {
        gio::File::for_path(path).delete(None::<&gio::Cancellable>)?;
        Ok(())
    }

    fn copy_file(&self, src: &Path, dst: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), FsError> {
        let total = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
        gio::File::for_path(src).copy(
            &gio::File::for_path(dst),
            gio::FileCopyFlags::NONE,
            None::<&gio::Cancellable>,
            Some(&mut |done: i64, _total: i64| progress(done.max(0) as u64, total)),
        )?;
        Ok(())
    }

    fn is_dir(&self, path: &Path) -> bool {
        gio::File::for_path(path)
            .query_file_type(gio::FileQueryInfoFlags::NONE, None::<&gio::Cancellable>)
            == gio::FileType::Directory
    }

    fn fs_type(&self, path: &Path) -> Option<String> {
        gio::File::for_path(path)
            .query_filesystem_info("filesystem::type", None::<&gio::Cancellable>)
            .ok()
            .and_then(|fi| fi.attribute_string("filesystem::type").map(|s| s.to_string()))
    }
}

fn is_pseudo_fs(fs: &str) -> bool {
    matches!(
        fs,
        "proc" | "sysfs" | "devpts" | "devtmpfs" | "tmpfs" | "cgroup" | "cgroup2" | "pstore"
            | "securityfs" | "debugfs" | "tracefs" | "configfs" | "fusectl" | "mqueue"
            | "hugetlbfs" | "ramfs" | "overlay" | "autofs" | "binfmt_misc" | "bpf" | "efivarfs"
            | "nfsd" | "nsfs" | "selinuxfs" | "squashfs"
    )
}

//! std::fs VFS backend (non-unix: Windows under Wine/native). The
//! `Backend` trait seam lets the app run without GIO; anything GIO did
//! beyond plain std calls is called out per method (trash, fs_type,
//! access probing).

use super::{Backend, Entry, EntryKind, FsError, Volume};
use std::path::{Path, PathBuf};

pub struct StdBackend {
    home: PathBuf,
}

impl StdBackend {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| "C:\\".into());
        Self { home }
    }
}

/// Windows extensions that can be launched ("Run with" drag target).
/// std has no mode bits there; the extension list is the launch rule.
fn is_executable_name(name: &str, kind: EntryKind) -> bool {
    if kind != EntryKind::File {
        return false;
    }
    name.rsplit_once('.')
        .map(|(_, ext)| matches!(ext.to_ascii_lowercase().as_str(), "exe" | "bat" | "cmd" | "com" | "ps1"))
        .unwrap_or(false)
}

impl Backend for StdBackend {
    fn home(&self) -> PathBuf {
        self.home.clone()
    }

    fn list(&self, path: &Path) -> Result<Vec<Entry>, FsError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let Ok(entry) = entry else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
            // kind follows links, matching GIO's default query; a broken
            // link still lists (as Other) instead of aborting the read
            let meta = std::fs::metadata(entry.path());
            let (kind, size, modified) = meta
                .as_ref()
                .map(|m| {
                    let kind = if m.is_dir() {
                        EntryKind::Dir
                    } else if m.is_file() {
                        EntryKind::File
                    } else {
                        EntryKind::Other
                    };
                    let modified = m.modified().ok().and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
                    });
                    (kind, m.len(), modified)
                })
                .unwrap_or((EntryKind::Other, 0, None));
            out.push(Entry {
                path: entry.path(),
                is_executable: is_executable_name(&name, kind),
                is_hidden: name.starts_with('.'),
                is_symlink,
                symlink_target: if is_symlink {
                    std::fs::read_link(entry.path()).ok().map(|p| p.to_string_lossy().into_owned())
                } else {
                    None
                },
                name,
                kind,
                size,
                modified,
            });
        }
        Ok(out)
    }

    fn volumes(&self) -> Vec<Volume> {
        // No volume monitor without a platform API: enumerate the
        // classic drive letters and keep the ones that exist. Under
        // Wine this yields C: (the prefix) and Z: (the host fs).
        let mut out: Vec<Volume> = ('A'..='Z')
            .map(|letter| {
                let mount = PathBuf::from(format!("{letter}:\\"));
                (format!("{letter}:"), mount)
            })
            .filter(|(_, mount)| mount.is_dir())
            .map(|(name, mount)| Volume { name, mount, fs_type: None })
            .collect();
        out.sort_by(|a, b| a.mount.cmp(&b.mount));
        out
    }

    fn create_dir(&self, path: &Path) -> Result<(), FsError> {
        std::fs::create_dir(path)?;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        std::fs::rename(from, to)?;
        Ok(())
    }

    fn trash(&self, _path: &Path) -> Result<(), FsError> {
        // No std trash; a wrong "permanent delete" is destructive, so
        // report unsupported until a shell-based backend lands.
        Err(FsError("trash is not supported on this platform yet".into()))
    }

    fn remove(&self, path: &Path) -> Result<(), FsError> {
        std::fs::remove_file(path)?;
        Ok(())
    }

    fn copy_file(&self, src: &Path, dst: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), FsError> {
        let total = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
        // chunked copy so the progress callback reports real movement
        // (std::fs::copy is one opaque call)
        let mut reader = std::fs::File::open(src)?;
        let mut writer = std::fs::File::create(dst)?;
        let mut done = 0u64;
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = std::io::Read::read(&mut reader, &mut buf)?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut writer, &buf[..n])?;
            done += n as u64;
            progress(done, total);
        }
        Ok(())
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn fs_type(&self, _path: &Path) -> Option<String> {
        // no pseudo-fs filtering without a platform query; every drive
        // listed by `volumes` is a real filesystem
        None
    }
}

/// Whether the current user can write to `path`. Windows has no
/// access(2)-equivalent in std; probe by creating (and removing) a
/// marker file, like the dnd policy probe in dnd.rs. Runs on target
/// enter only, never per frame.
pub(crate) fn writable(path: &Path) -> bool {
    let probe = path.join(".explorer-writable-probe");
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

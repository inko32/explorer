//! Archive VFS — archives openable as folders (TechSpec-after-iced).
//!
//! Extensible backend registry (D16), tried in order with fallthrough on
//! error:
//!   1. `seven_so` — dlopen `7z.so` (7-Zip / 7zip-zstd shared library),
//!      native COM `IInArchive` interface. Primary; no per-call process spawn.
//!   2. `tar`      — `tar` CLI for .tar.gz / .tar.xz (and friends).
//!   3. `cli`      — the original `7z` executable backend (reserved fallback).
//!
//! Reserved for later: a RARLab (closed-source) backend for rar-exclusive
//! volumes — implement `ArchiveBackend` and insert into `registry()`.
//!
//! Read-only for now (extract/copy-in planned).

mod cli;
mod seven_so;
mod tar;

use super::{Entry, EntryKind, FsError};
use std::path::{Path, PathBuf};

/// Extensions we treat as navigable archives (7z's format list, archives only).
pub const ARCHIVE_EXTS: &[&str] = &[
    "7z", "zip", "rar", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst",
    "cab", "iso", "arj", "lzh", "lha", "uue", "xxe", "b64", "ar", "cpio", "deb", "rpm",
    "squashfs", "wim", "swm", "esd", "fat", "ntfs", "dmg", "hfs", "lzma", "lz4", "z", "taz",
];

/// One raw record from an archive listing (full inner path, '/' separators).
pub struct ArchiveItem {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: Option<i64>,
}

/// Archive listing seam. New backends (RARLab…) implement this and join the
/// registry; the rest of the app only sees [`list`]/[`resolve`].
pub trait ArchiveBackend: Send + Sync {
    fn name(&self) -> &'static str;
    /// Whether this backend claims the file (usually by extension).
    fn can_open(&self, path: &Path) -> bool;
    /// All entries of the archive (flat, full inner paths).
    fn list(&self, archive: &Path) -> Result<Vec<ArchiveItem>, FsError>;
}

/// Backend registry, ordered (overridable via `EXPLORER_ARCHIVE_ORDER`,
/// e.g. "tar,so,cli").
fn registry() -> &'static [Box<dyn ArchiveBackend>] {
    static REG: std::sync::OnceLock<Vec<Box<dyn ArchiveBackend>>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let mut all: Vec<Box<dyn ArchiveBackend>> = vec![
            Box::new(seven_so::SevenZipSo),
            Box::new(tar::TarCli),
            Box::new(cli::SevenZipCli),
        ];
        if let Ok(order) = std::env::var("EXPLORER_ARCHIVE_ORDER") {
            let wanted: Vec<&str> = order.split(',').map(str::trim).collect();
            all.sort_by_key(|b| {
                wanted.iter().position(|w| *w == b.name()).unwrap_or(usize::MAX)
            });
        }
        all
    })
}

pub fn is_archive_path(path: &Path) -> bool {
    ext_of(path)
        .map(|e| ARCHIVE_EXTS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Lowercased extension of `path` (None if no extension).
pub(crate) fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
}

/// Split a path into (archive_path, inner_path). The archive is the longest
/// prefix with an archive extension; inner uses '/' separators.
/// Returns None if no archive prefix exists.
pub fn resolve(path: &Path) -> Option<(PathBuf, String)> {
    let comps: Vec<_> = path.components().collect();
    let mut archive_end = None;
    for i in 1..=comps.len() {
        let prefix: PathBuf = comps[..i].iter().collect();
        if is_archive_path(&prefix) {
            archive_end = Some(i);
        }
    }
    let end = archive_end?;
    let archive: PathBuf = comps[..end].iter().collect();
    let inner: Vec<String> = comps[end..]
        .iter()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    Some((archive, inner.join("/")))
}

/// List the immediate children of `inner` inside `archive`, via the first
/// backend that claims the file and succeeds.
pub fn list(archive: &Path, inner: &str) -> Result<Vec<Entry>, FsError> {
    let items = list_items(archive)?;
    Ok(children(items, archive, inner))
}

/// True if `path` points at a directory inside an archive (or an archive root).
pub fn is_dir(archive: &Path, inner: &str) -> bool {
    if inner.is_empty() {
        return true;
    }
    list(archive, &parent_of(inner))
        .map(|entries| entries.iter().any(|e| e.name == base_of(inner) && e.is_dir()))
        .unwrap_or(false)
}

fn parent_of(inner: &str) -> String {
    match inner.rfind('/') {
        Some(i) => inner[..i].to_string(),
        None => String::new(),
    }
}

fn base_of(inner: &str) -> &str {
    inner.rsplit('/').next().unwrap_or(inner)
}

/// Try every claiming backend in order; remember the last error for the
/// caller if all fail.
fn list_items(archive: &Path) -> Result<Vec<ArchiveItem>, FsError> {
    let mut last_err = None;
    for b in registry() {
        if !b.can_open(archive) {
            continue;
        }
        match b.list(archive) {
            Ok(v) => {
                tracing_backend(b.name());
                return Ok(v);
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| FsError("no archive backend claims this file".into())))
}

fn tracing_backend(name: &str) {
    if std::env::var_os("EXPLORER_DEBUG").is_some() {
        eprintln!("[archive] backend: {name}");
    }
}

/// Turn a flat item list into the immediate children of `inner`.
fn children(items: Vec<ArchiveItem>, archive: &Path, inner: &str) -> Vec<Entry> {
    let prefix = if inner.is_empty() {
        String::new()
    } else {
        format!("{}/", inner.trim_end_matches('/'))
    };
    let mut out: Vec<Entry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let rel = item.path.trim_start_matches("./").trim_end_matches('/').to_string();
        if !rel.starts_with(&prefix) {
            continue;
        }
        let rest = &rel[prefix.len()..];
        if rest.is_empty() {
            continue;
        }
        let (name, deeper) = match rest.find('/') {
            Some(i) => (&rest[..i], true),
            None => (rest, false),
        };
        if name.is_empty() || !seen.insert(name.to_string()) {
            continue;
        }
        let is_dir = item.is_dir || deeper;
        out.push(Entry {
            name: name.to_string(),
            path: archive.join(&prefix).join(name),
            kind: if is_dir { EntryKind::Dir } else { EntryKind::File },
            size: if is_dir { 0 } else { item.size },
            modified: item.modified,
            is_hidden: name.starts_with('.'),
            is_symlink: false,
            is_executable: false,
            symlink_target: None,
        });
    }
    out
}

/// days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_picks_longest_archive_prefix() {
        let (a, inner) = resolve(Path::new("/tmp/x/test.7z/dir/file.txt")).unwrap();
        assert_eq!(a, PathBuf::from("/tmp/x/test.7z"));
        assert_eq!(inner, "dir/file.txt");
        assert!(resolve(Path::new("/tmp/plain/dir")).is_none());
    }

    /// Integration smoke test: builds real fixtures with the system tools
    /// (skipped silently when they are missing) and lists them through every
    /// backend that claims them.
    #[test]
    fn backends_list_real_fixtures() {
        let dir = std::env::temp_dir().join("explorer_archive_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/sub")).unwrap();
        std::fs::write(dir.join("src/hello.txt"), b"hello world").unwrap();
        std::fs::write(dir.join("src/sub/nested.bin"), b"12345").unwrap();

        let mut fixtures: Vec<PathBuf> = Vec::new();
        let seven = std::process::Command::new("7z")
            .args(["a", "-y", "-bd"])
            .arg(dir.join("fix.7z"))
            .arg(dir.join("src"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if seven {
            fixtures.push(dir.join("fix.7z"));
        }
        for (name, args) in [
            ("fix.tar.gz", vec!["-czf"]),
            ("fix.tar.xz", vec!["-cJf"]),
        ] {
            let ok = std::process::Command::new("tar")
                .args(args)
                .arg(dir.join(name))
                .arg("-C")
                .arg(&dir)
                .arg("src")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                fixtures.push(dir.join(name));
            }
        }
        assert!(!fixtures.is_empty(), "neither 7z nor tar available to build fixtures");

        for fx in &fixtures {
            let claims: Vec<&str> = registry()
                .iter()
                .filter(|b| b.can_open(fx))
                .map(|b| b.name())
                .collect();
            assert!(!claims.is_empty(), "no backend claims {}", fx.display());
            let items = list_items(fx)
                .unwrap_or_else(|e| panic!("listing {} failed: {e}", fx.display()));
            let names: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
            let hello = items
                .iter()
                .find(|i| i.path.ends_with("hello.txt"))
                .unwrap_or_else(|| panic!("{}: hello.txt missing ({names:?}, backend order: {claims:?})", fx.display()));
            assert_eq!(hello.size, 11, "size via COM/CLI ({claims:?})",);
            assert!(
                hello.modified.is_some(),
                "mtime missing for {} ({claims:?})",
                fx.display()
            );
            assert!(
                items.iter().any(|i| i.path.contains("sub")),
                "{}: sub/ missing ({names:?})",
                fx.display()
            );
            // with tar forced first, the tar backend must serve tarballs
            if fx.extension().is_some_and(|e| e == "gz" || e == "xz") {
                if std::env::var("EXPLORER_ARCHIVE_ORDER").is_ok() {
                    tracing_backend("(order forced)");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

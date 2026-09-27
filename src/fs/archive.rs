//! 7-Zip archive backend — archives openable as folders (TechSpec-after-iced).
//!
//! Uses the `7z` CLI (7zip-zstd on this system) with the `-slt` technical
//! listing; a native binding can replace the spawn later without touching
//! the `Backend` seam. Read-only for now (extract/copy-in planned).

use super::{Entry, EntryKind, FsError};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extensions we treat as navigable archives (7z's format list, archives only).
const ARCHIVE_EXTS: &[&str] = &[
    "7z", "zip", "rar", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst",
    "cab", "iso", "arj", "lzh", "lha", "uue", "xxe", "b64", "ar", "cpio", "deb", "rpm",
    "squashfs", "wim", "swm", "esd", "fat", "ntfs", "dmg", "hfs", "lzma", "lz4", "z", "taz",
];

pub fn is_archive_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .map(|e| ARCHIVE_EXTS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Split a path into (archive_path, inner_path). The archive is the longest
/// prefix with an archive extension; inner uses '/' separators (7z convention).
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

/// True if `path` points at a directory inside an archive (or an archive root).
pub fn is_dir(archive: &Path, inner: &str) -> bool {
    if inner.is_empty() {
        return true;
    }
    list(archive, &parent_of(inner))
        .map(|entries| entries.iter().any(|e| e.name == base_of(inner) && e.is_dir()))
        .unwrap_or(false)
}

/// List the immediate children of `inner` inside `archive`.
pub fn list(archive: &Path, inner: &str) -> Result<Vec<Entry>, FsError> {
    let output = Command::new("7z")
        .arg("l")
        .arg("-slt")
        .arg("--")
        .arg(archive)
        .output()
        .map_err(|e| FsError(format!("7z not available: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FsError(format!("7z: {}", stderr.trim().lines().next().unwrap_or("error"))));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let prefix = if inner.is_empty() {
        String::new()
    } else {
        format!("{}/", inner.trim_end_matches('/'))
    };

    let mut out: Vec<Entry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for block in text.split("\n\n") {
        let mut path_v = None;
        let mut size_v = 0u64;
        let mut modified_v: Option<i64> = None;
        let mut is_dir_v = false;
        for line in block.lines() {
            if let Some((k, v)) = line.split_once(" = ") {
                match k.trim() {
                    "Path" => path_v = Some(v.trim().to_string()),
                    "Size" => size_v = v.trim().parse().unwrap_or(0),
                    "Modified" => modified_v = parse_7z_time(v.trim()),
                    "Attributes" => is_dir_v = v.contains('D'),
                    _ => {}
                }
            }
        }
        let Some(p) = path_v else { continue };
        if p == archive.to_string_lossy().as_ref() {
            continue; // the archive itself appears as first entry
        }
        let rel = p.trim_start_matches("./").trim_end_matches('/');
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
        let is_dir = is_dir_v || deeper;
        out.push(Entry {
            name: name.to_string(),
            path: archive.join(&prefix).join(name),
            kind: if is_dir { EntryKind::Dir } else { EntryKind::File },
            size: if is_dir { 0 } else { size_v },
            modified: modified_v,
            is_hidden: name.starts_with('.'),
            is_symlink: false,
            symlink_target: None,
        });
    }
    Ok(out)
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

/// "2024-03-15 10:20:30" -> epoch seconds.
fn parse_7z_time(s: &str) -> Option<i64> {
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let mut t = time.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let mi: i64 = t.next()?.parse().ok()?;
    let sec: i64 = t
        .next()
        .unwrap_or("0")
        .split('.')
        .next()
        .unwrap_or("0")
        .parse()
        .ok()?;
    Some(days_from_civil(y, m, day) * 86400 + h * 3600 + mi * 60 + sec)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

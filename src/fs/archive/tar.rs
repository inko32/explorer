//! tar backend — .tar.gz / .tar.xz (plus bz2/zst variants and plain tar) via
//! the system `tar` CLI with explicit compression flags per extension.
//!
//! GNU tar's long listing (`-tv`) is parsed by locating the date token
//! (`YYYY-MM-DD`) — everything after the time token is the name, so names
//! with spaces survive.

use super::{days_from_civil, ext_of, ArchiveBackend, ArchiveItem, FsError};
use std::path::Path;
use std::process::Command;

const TAR_EXTS: &[(&str, &str)] = &[
    ("tar", ""),
    ("tgz", "z"),
    ("gz", "z"),
    ("txz", "J"),
    ("xz", "J"),
    ("tbz2", "j"),
    ("bz2", "j"),
    ("tzst", "--zstd"),
    ("zst", "--zstd"),
];

pub struct TarCli;

impl ArchiveBackend for TarCli {
    fn name(&self) -> &'static str {
        "tar"
    }

    fn can_open(&self, path: &Path) -> bool {
        ext_of(path).map(|e| TAR_EXTS.iter().any(|(x, _)| *x == e)).unwrap_or(false)
    }

    fn list(&self, archive: &Path) -> Result<Vec<ArchiveItem>, FsError> {
        let ext = ext_of(archive).unwrap_or_default();
        let flag = TAR_EXTS
            .iter()
            .find(|(x, _)| *x == ext)
            .map(|(_, f)| *f)
            .unwrap_or("");
        let mut cmd = Command::new("tar");
        cmd.arg("-tv");
        if !flag.is_empty() {
            cmd.arg(format!("-{flag}"));
        }
        let output = cmd
            .arg("-f")
            .arg(archive)
            .output()
            .map_err(|e| FsError(format!("tar not available: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FsError(format!(
                "tar: {}",
                stderr.trim().lines().next().unwrap_or("error")
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut out = Vec::new();
        for line in text.lines() {
            if let Some(item) = parse_tv_line(line) {
                out.push(item);
            }
        }
        Ok(out)
    }
}

/// `-rw-r--r-- user/group 12345 2024-03-15 10:20 name with spaces`
fn parse_tv_line(line: &str) -> Option<ArchiveItem> {
    let mut tokens = line.split_whitespace();
    tokens.next()?; // permissions
    tokens.next()?; // owner[/group]
    let size: u64 = tokens.next()?.parse().ok()?;
    let date = tokens.next()?; // YYYY-MM-DD
    let time = tokens.next()?; // HH:MM
    let name = tokens.collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return None;
    }
    let is_dir = name.ends_with('/');
    let modified = parse_tar_time(date, time);
    Some(ArchiveItem {
        path: name.trim_end_matches('/').to_string(),
        size,
        is_dir,
        modified,
    })
}

/// "2024-03-15" "10:20" -> epoch seconds.
fn parse_tar_time(date: &str, time: &str) -> Option<i64> {
    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let mut t = time.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let mi: i64 = t.next()?.parse().ok()?;
    Some(days_from_civil(y, m, day) * 86400 + h * 3600 + mi * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_line_with_spaces_in_name() {
        let item = parse_tv_line("-rw-r--r-- inko/inko 11 2024-03-15 10:20 my dir/hello.txt").unwrap();
        assert_eq!(item.path, "my dir/hello.txt");
        assert_eq!(item.size, 11);
        assert!(!item.is_dir);
        assert!(item.modified.is_some());
    }

    #[test]
    fn tv_line_dir() {
        let item = parse_tv_line("drwxr-xr-x inko/inko 0 2024-03-15 10:20 sub/").unwrap();
        assert!(item.is_dir);
        assert_eq!(item.path, "sub");
    }
}

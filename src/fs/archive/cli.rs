//! 7z CLI backend — the original spawn-per-list implementation, reserved as
//! the last-resort fallback (works wherever a `7z` executable exists).

use super::{days_from_civil, ArchiveBackend, ArchiveItem};
use super::FsError;
use std::path::Path;
use std::process::Command;

pub struct SevenZipCli;

impl ArchiveBackend for SevenZipCli {
    fn name(&self) -> &'static str {
        "cli"
    }

    fn can_open(&self, path: &Path) -> bool {
        // `7z -slt` only sees the compression stream of a tarball
        // ("fix.tar"), not its contents — leave compressed/tar containers to
        // the tar/7z.so backends. (A plain .gz opens via 7z.so as a
        // single-item container.)
        if matches!(
            super::ext_of(path).as_deref(),
            Some("tar" | "tgz" | "gz" | "txz" | "xz" | "tbz2" | "bz2" | "tzst" | "zst")
        ) {
            return false;
        }
        super::is_archive_path(path)
    }

    fn list(&self, archive: &Path) -> Result<Vec<ArchiveItem>, FsError> {
        let output = Command::new("7z")
            .arg("l")
            .arg("-slt")
            .arg("--")
            .arg(archive)
            .output()
            .map_err(|e| FsError(format!("7z not available: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FsError(format!(
                "7z: {}",
                stderr.trim().lines().next().unwrap_or("error")
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut out = Vec::new();
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
            out.push(ArchiveItem {
                path: p,
                size: size_v,
                is_dir: is_dir_v,
                modified: modified_v,
            });
        }
        Ok(out)
    }
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

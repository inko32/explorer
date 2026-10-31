//! Date/size/type-label formatting for the folder view, preview pane
//! and status bar.

use crate::fs;

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

pub fn type_label(e: &fs::Entry) -> &'static str {
    if e.is_symlink {
        return "Shortcut";
    }
    if e.is_dir() {
        return "File folder";
    }
    let ext = e
        .name
        .rsplit_once('.')
        .map(|(_, x)| x.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "7z" => "7-Zip Archive",
        "zip" => "ZIP Archive",
        "rar" => "WinRAR Archive",
        "tar" => "TAR Archive",
        "gz" | "tgz" => "GZIP Archive",
        "bz2" => "Bzip2 Archive",
        "xz" => "XZ Archive",
        "zst" => "Zstandard Archive",
        "iso" => "ISO Image",
        "png" => "PNG File",
        "jpg" | "jpeg" => "JPEG Image",
        "gif" => "GIF Image",
        "txt" => "Text Document",
        "md" => "Markdown Document",
        "pdf" => "PDF Document",
        "rs" => "Rust Source File",
        "sh" => "Shell Script",
        "mp3" | "flac" | "ogg" => "Audio File",
        "mp4" | "mkv" | "mov" => "Video File",
        _ if fs::archive::is_archive_path(&e.path) => "Archive File",
        _ => "File",
    }
}

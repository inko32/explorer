//! Number, size and duration formatting per spec appendix C3.

/// Thousands-separated integer (locale-fixed "," at this stage).
pub fn count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Human size: "35.0 GB" (1 decimal) for the discovery headline,
/// "4.40 GB" (2 decimals) for the details line, "742 MB" for round MB.
pub fn size(bytes: u64, decimals: u32) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        return format!("{bytes} bytes");
    }
    // integral values print without a fraction ("742 MB"), the rest keep
    // the requested precision ("35.0 GB", "4.10 GB" — spec C3)
    if v.fract() == 0.0 && v < 1000.0 {
        return format!("{v:.0} {}", UNITS[unit]);
    }
    format!("{:.*} {}", decimals as usize, v, UNITS[unit])
}

/// Speed: MB/s with 3 significant digits ("182 MB/s", "25.4 MB/s");
/// items/s as an integer.
pub fn speed(bytes_per_s: f32, items_mode: bool) -> String {
    if items_mode {
        return format!("{} items/s", count(bytes_per_s.max(0.0).round() as u64));
    }
    let mb = bytes_per_s / (1024.0 * 1024.0);
    if mb >= 100.0 {
        format!("{mb:.0} MB/s")
    } else if mb >= 10.0 {
        format!("{mb:.1} MB/s")
    } else {
        format!("{mb:.2} MB/s")
    }
}

/// "About 5 seconds", "About 2 minutes and 30 seconds", "About 18 minutes",
/// ">2 h: About N hours and M minutes" (spec D1.7).
pub fn time_remaining(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return "Calculating...".to_string();
    }
    if secs < 60.0 {
        let s = (secs / 5.0).round() * 5.0;
        let s = s.max(1.0) as u64;
        if s == 1 {
            return "About 1 second".to_string();
        }
        return format!("About {s} seconds");
    }
    let total_min = secs / 60.0;
    if total_min <= 10.0 {
        let min = (total_min - 0.5).max(0.0).round() as u64;
        let rem = secs - min as f64 * 60.0;
        let sec = (rem / 10.0).round() * 10.0;
        let sec = sec.clamp(0.0, 59.0) as u64;
        if min == 0 {
            return format!("About {sec} seconds");
        }
        if sec == 0 {
            return if min == 1 {
                "About 1 minute".to_string()
            } else {
                format!("About {min} minutes")
            };
        }
        return format!(
            "About {} and {}",
            if min == 1 { "1 minute".into() } else { format!("{min} minutes") },
            if sec == 1 { "1 second".into() } else { format!("{sec} seconds") }
        );
    }
    if total_min <= 120.0 {
        return format!("About {} minutes", total_min.round() as u64);
    }
    let hours = (total_min / 60.0) as u64;
    let mins = (total_min % 60.0).round() as u64;
    format!("About {hours} hours and {mins} minutes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(count(5), "5");
        assert_eq!(count(5330), "5,330");
        assert_eq!(count(158837), "158,837");
    }

    #[test]
    fn sizes() {
        assert_eq!(size(742 * 1024 * 1024, 1), "742 MB");
        assert_eq!(size(24_600_000_000, 1), "22.9 GB");
        assert_eq!(size(4_400_000_000, 2), "4.10 GB");
    }

    #[test]
    fn speeds() {
        assert_eq!(speed(182.0 * 1024.0 * 1024.0, false), "182 MB/s");
        assert_eq!(speed(25.4 * 1024.0 * 1024.0, false), "25.4 MB/s");
        assert_eq!(speed(115.0, true), "115 items/s");
    }

    #[test]
    fn times() {
        assert_eq!(time_remaining(4.2), "About 5 seconds");
        assert_eq!(time_remaining(150.0), "About 2 minutes and 30 seconds");
        assert_eq!(time_remaining(1080.0), "About 18 minutes");
        assert_eq!(time_remaining(8000.0), "About 2 hours and 13 minutes");
    }
}

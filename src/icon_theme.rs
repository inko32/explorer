//! Freedesktop icon-theme lookup for the `icon:` entries of a system
//! theme in icons.yaml (e.g. `breeze`). Follows the Icon Theme
//! Specification: the XDG search paths, per-size subdirectories declared
//! in index.theme (a `16@2x` dir is a 32px asset), the Inherits chain,
//! and a final unthemed fallback in /usr/share/pixmaps.
//!
//! Results are cached per (theme, name, size): the negative lookups
//! matter as much as the hits, otherwise every render of an icon the
//! theme lacks would re-stat the search paths.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A parsed index.theme.
#[derive(Clone)]
struct ThemeIndex {
    inherits: Vec<String>,
    /// (effective pixel size, scalable, subdirectory), ascending size.
    dirs: Vec<(u32, bool, String)>,
}

type ThemeCache = HashMap<String, Option<(PathBuf, ThemeIndex)>>;
type HitCache = HashMap<(String, String, u32), Option<PathBuf>>;

fn themes() -> &'static Mutex<ThemeCache> {
    static T: OnceLock<Mutex<ThemeCache>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hits() -> &'static Mutex<HitCache> {
    static H: OnceLock<Mutex<HitCache>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Search roots in lookup order: `~/.icons`, `$XDG_DATA_HOME/icons`,
/// `$XDG_DATA_DIRS/*/icons` (default `/usr/local/share:/usr/share`),
/// `/usr/share/pixmaps` (unthemed fallback).
fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(h) = &home {
        roots.push(h.join(".icons"));
    }
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")));
    if let Some(d) = data_home {
        roots.push(d.join("icons"));
    }
    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|v| v.to_string_lossy().split(':').map(PathBuf::from).collect())
        .unwrap_or_else(|| vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]);
    for d in data_dirs {
        roots.push(d.join("icons"));
    }
    roots.push(PathBuf::from("/usr/share/pixmaps"));
    roots
}

/// Locate a theme directory and parse its index.theme (cached per theme).
fn theme_index(theme: &str) -> Option<(PathBuf, ThemeIndex)> {
    if let Some(cached) = themes().lock().ok().and_then(|c| c.get(theme).cloned()) {
        return cached;
    }
    let found = search_roots()
        .iter()
        .map(|r| r.join(theme))
        .find(|p| p.is_dir())
        .and_then(|dir| parse_index(&dir).map(|idx| (dir, idx)));
    if let Ok(mut c) = themes().lock() {
        c.insert(theme.to_string(), found.clone());
    }
    found
}

/// Minimal INI reader for index.theme: `[Icon Theme]` carries
/// `Directories` and `Inherits`; each listed section carries `Size`,
/// `Scale`, and `Type`. Sizes/scale/type are recorded for every section
/// and filtered by `Directories` afterwards — the key order inside
/// index.theme is not guaranteed.
fn parse_index(dir: &Path) -> Option<ThemeIndex> {
    let text = std::fs::read_to_string(dir.join("index.theme")).ok()?;
    let mut section = String::new();
    let mut listed: Vec<String> = Vec::new();
    let mut inherits = Vec::new();
    let mut size_of: HashMap<String, u32> = HashMap::new();
    let mut scale_of: HashMap<String, u32> = HashMap::new();
    let mut scalable: HashSet<String> = HashSet::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if section == "Icon Theme" {
            match k {
                "Directories" => {
                    listed = v
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
                "Inherits" => {
                    inherits = v
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
                _ => {}
            }
        } else {
            match k {
                "Size" => {
                    if let Ok(n) = v.parse::<u32>() {
                        size_of.insert(section.clone(), n.max(1));
                    }
                }
                "Scale" => {
                    if let Ok(n) = v.parse::<u32>() {
                        scale_of.insert(section.clone(), n.max(1));
                    }
                }
                "Type"
                    if v.eq_ignore_ascii_case("scalable") =>
                {
                    scalable.insert(section.clone());
                }
                _ => {}
            }
        }
    }
    let mut dirs: Vec<(u32, bool, String)> = listed
        .iter()
        .filter_map(|d| {
            let size = size_of.get(d)? * scale_of.get(d).copied().unwrap_or(1);
            Some((size.max(1), scalable.contains(d), d.clone()))
        })
        .collect();
    dirs.sort_by_key(|(s, _, _)| *s);
    Some(ThemeIndex { inherits, dirs })
}

/// Subdirectories that can serve `want` pixels, best first: exact size,
/// scalable, nearest larger, nearest smaller.
fn dir_candidates(index: &ThemeIndex, want: u32) -> Vec<String> {
    let mut scored: Vec<((u32, u32), String)> = index
        .dirs
        .iter()
        .map(|(s, scalable, d)| {
            let rank = if *s == want {
                0
            } else if *scalable {
                1
            } else if *s > want {
                2
            } else {
                3
            };
            let dist = s.abs_diff(want);
            ((rank, dist), d.clone())
        })
        .collect();
    scored.sort_by_key(|(k, _)| *k);
    scored.into_iter().map(|(_, d)| d).collect()
}

/// Resolve an icon-theme icon (e.g. `edit-paste` in `breeze`) to a file.
/// SVG is preferred over PNG within a subdirectory, the theme's
/// `Inherits` chain is searched breadth-first, and an uninstalled theme
/// degrades to the unthemed roots instead of erroring.
pub fn lookup(theme: &str, name: &str, size: u32) -> Option<PathBuf> {
    let key = (theme.to_string(), name.to_string(), size.max(1));
    if let Some(hit) = hits().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let found = lookup_uncached(theme, name, size.max(1));
    if let Ok(mut c) = hits().lock() {
        c.insert(key, found.clone());
    }
    found
}

fn lookup_uncached(theme: &str, name: &str, size: u32) -> Option<PathBuf> {
    // BFS over the Inherits chain (cycle-safe); parents after children.
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = vec![theme.to_string()];
    let mut i = 0;
    while i < queue.len() {
        let t = queue[i].clone();
        i += 1;
        if !visited.insert(t.clone()) {
            continue;
        }
        let Some((dir, index)) = theme_index(&t) else { continue };
        queue.extend(index.inherits.iter().cloned());
        for sub in dir_candidates(&index, size) {
            for ext in ["svg", "png"] {
                let path = dir.join(&sub).join(format!("{name}.{ext}"));
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    // unthemed fallback (covers /usr/share/pixmaps)
    for root in search_roots() {
        for ext in ["svg", "png"] {
            let path = root.join(format!("{name}.{ext}"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_breeze_index_theme() {
        // The system breeze theme ships a standard index.theme; parsing
        // it must yield size-sorted subdirectories and the hicolor parent.
        let Some((dir, index)) = theme_index("breeze") else {
            eprintln!("breeze not installed; skipping");
            return;
        };
        assert!(dir.ends_with("breeze"));
        assert_eq!(index.inherits, vec!["hicolor".to_string()]);
        let sizes: Vec<u32> = index.dirs.iter().map(|(s, _, _)| *s).collect();
        assert!(sizes.windows(2).all(|w| w[0] <= w[1]), "sizes ascending");
        assert!(sizes.contains(&16) && sizes.contains(&32), "has 16/32px dirs");
    }

    #[test]
    fn finds_breeze_icons_by_size() {
        if theme_index("breeze").is_none() {
            eprintln!("breeze not installed; skipping");
            return;
        }
        let icon = lookup("breeze", "folder", 16).expect("folder icon");
        assert!(icon.to_string_lossy().ends_with("folder.svg"));
        // larger request resolves into a larger asset directory
        let big = lookup("breeze", "folder", 64).expect("folder icon 64");
        assert!(big.to_string_lossy().contains("/64/") || big.to_string_lossy().contains("/48/"));
        // unknown names miss (and stay cached as misses)
        assert!(lookup("breeze", "definitely-not-an-icon-name", 16).is_none());
    }

    #[test]
    fn missing_theme_degrades_to_none() {
        assert!(lookup("no-such-theme-xyz", "folder", 16).is_none());
    }

    #[test]
    fn dir_candidates_prefer_exact_then_larger() {
        let index = ThemeIndex {
            inherits: vec![],
            dirs: vec![
                (16, false, "16".into()),
                (32, false, "32".into()),
                (48, true, "48".into()),
            ],
        };
        assert_eq!(dir_candidates(&index, 16)[0], "16".to_string());
        // no exact match: the scalable 48 wins over larger/smaller fixed
        assert_eq!(dir_candidates(&index, 20)[0], "48".to_string());
        assert_eq!(dir_candidates(&index, 20)[1], "32".to_string());
        // nothing larger: the scalable 48 beats the smaller 32/16
        assert_eq!(dir_candidates(&index, 64)[0], "48".to_string());
        // larger requests prefer the scalable dir over exact-size misses
        assert_eq!(dir_candidates(&index, 40)[0], "48".to_string());
    }
}

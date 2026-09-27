//! Windows 8 style file explorer — early skeleton.
//!
//! Module map: see docs/ARCHITECTURE.md (kept updated).

mod app;
mod border;
mod clipboard;
mod fs;
mod icons;
mod input;
mod platform;
mod render;
mod theme;
mod ui;

use std::path::PathBuf;

#[derive(Clone)]
pub struct Options {
    /// Render one frame offscreen to a BMP instead of opening a window.
    pub snapshot: Option<PathBuf>,
    pub snapshot_size: (u32, u32),
    pub snapshot_scale: f32,
    pub dark: bool,
    /// Initial folder (defaults to home directory).
    pub folder: Option<PathBuf>,
}

fn parse_args() -> Options {
    let mut opts = Options {
        snapshot: None,
        snapshot_size: (1100, 700),
        snapshot_scale: 1.0,
        dark: false,
        folder: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--snapshot" => {
                i += 1;
                opts.snapshot = Some(PathBuf::from(args.get(i).map(String::as_str).unwrap_or("/tmp/explorer_snapshot.bmp")));
            }
            "--size" => {
                i += 1;
                if let Some(s) = args.get(i) {
                    if let Some((w, h)) = s.split_once('x') {
                        opts.snapshot_size = (w.parse().unwrap_or(1100), h.parse().unwrap_or(700));
                    }
                }
            }
            "--scale" => {
                i += 1;
                opts.snapshot_scale = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            }
            "--dark" => opts.dark = true,
            s if !s.starts_with('-') => opts.folder = Some(PathBuf::from(s)),
            _ => {}
        }
        i += 1;
    }
    opts
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    platform::run(parse_args())
}

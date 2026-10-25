//! Performance tuning — `assets/performance.yaml` (user override
//! ~/.config/explorer/performance.yaml). Pointer-rate caps: a file
//! manager does not need display-rate mouse processing, and every move
//! message costs a state update + tree rebuild + possible repaint.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct PerfCfg {
    /// Cap on mouse-move message processing (Hz; 0 = unlimited).
    /// Splitter drags and the rubber band always run live.
    pub input_hz: f32,
    /// Cap on hover-driven highlight updates (Hz; 0 = unlimited).
    pub render_hz: f32,
}

impl Default for PerfCfg {
    fn default() -> Self {
        Self { input_hz: 60.0, render_hz: 30.0 }
    }
}

impl PerfCfg {
    pub fn load() -> Self {
        let text = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .and_then(|h| {
                std::fs::read_to_string(h.join(".config/explorer/performance.yaml")).ok()
            })
            .unwrap_or_else(|| include_str!("../assets/performance.yaml").to_string());
        match serde_yaml::from_str::<PerfCfg>(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("performance.yaml: {e} — using defaults");
                PerfCfg::default()
            }
        }
    }

    /// True when `last` is older than the interval for `hz` (0 = always).
    pub fn past(&self, hz: f32, last: std::time::Instant) -> bool {
        hz <= 0.0
            || last.elapsed().as_secs_f32() >= 1.0 / hz
    }
}

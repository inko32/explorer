//! Ribbon images (`IUIImage`).
//!
//! Named icons resolve through the **explorer icon registry** (`src/icon.rs`:
//! Win8 PNG assets + skin-colored builtin SVG generators — the same icons
//! the rest of the app uses), falling back to the ribbon's own builtin SVG
//! glyph set (`src/ribbon/icons.rs`) for chrome glyphs like the orb.
//! Inline SVG data and `file:` references work as before.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use iced::widget::svg::Handle as SvgHandle;

/// `IUIImage`.
#[derive(Debug, Clone, PartialEq)]
pub enum UiImage {
    /// A builtin icon (explorer registry, then [`crate::ribbon::icons`]).
    Icon(&'static str),
    /// Same, looked up by dynamic name.
    Named(String),
    /// Inline SVG data (utf-8).
    Svg(String),
    /// An image loaded from a file path (SVG or raster).
    File(std::path::PathBuf),
}

/// A resolved icon: raster (PNG asset) or vector.
#[derive(Debug, Clone)]
pub enum IconHandle {
    Svg(SvgHandle),
    Raster(iced::widget::image::Handle),
}

/// The ribbon draws SVG fallbacks with the explorer skin; `ribbon::view`
/// publishes the dark flag here so the generators pick the right palette.
static DARK: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

fn skin() -> crate::ui_common::skin::Skin {
    if DARK.load(Ordering::Relaxed) {
        crate::ui_common::skin::DARK
    } else {
        crate::ui_common::skin::LIGHT
    }
}

fn svg_cache() -> &'static Mutex<std::collections::HashMap<String, SvgHandle>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, SvgHandle>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

impl UiImage {
    /// Resolve to a draw handle for a `size`-px slot (nearest PNG variant).
    pub fn handle(&self, size: u32) -> IconHandle {
        match self {
            UiImage::Icon(name) => named(name, size),
            UiImage::Named(name) => named(name, size),
            UiImage::Svg(data) => {
                IconHandle::Svg(SvgHandle::from_memory(data.clone().into_bytes()))
            }
            UiImage::File(path) => file_handle(path),
        }
    }
}

fn named(name: &str, size: u32) -> IconHandle {
    // 1. explorer icon registry (same icons as the rest of the app)
    match crate::icon::handle(name, size, &skin()) {
        Some(crate::icon::IconHandle::Png(h)) => return IconHandle::Raster(h),
        Some(crate::icon::IconHandle::Svg(h)) => return IconHandle::Svg(h),
        None => {}
    }
    // 2. the ribbon's builtin glyph set (orb, chevrons, ...)
    if let Some(h) = ribbon_builtin(name) {
        return IconHandle::Svg(h);
    }
    blank_handle()
}

fn ribbon_builtin(name: &str) -> Option<SvgHandle> {
    let mut cache = svg_cache().lock().ok()?;
    if let Some(handle) = cache.get(name) {
        return Some(handle.clone());
    }
    let data = crate::ribbon::icons::get(name)?;
    let handle = SvgHandle::from_memory(data.as_bytes().to_vec());
    cache.insert(name.to_string(), handle.clone());
    Some(handle)
}

fn file_handle(path: &std::path::Path) -> IconHandle {
    let raster = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp"
            )
        })
        .unwrap_or(false);
    if raster {
        IconHandle::Raster(iced::widget::image::Handle::from_path(path))
    } else {
        IconHandle::Svg(SvgHandle::from_path(path))
    }
}

/// Blank transparent svg used as fallback.
pub(crate) fn blank_handle() -> IconHandle {
    IconHandle::Svg(SvgHandle::from_memory(
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"/>".to_vec(),
    ))
}

impl Default for UiImage {
    fn default() -> Self {
        UiImage::Icon("blank")
    }
}

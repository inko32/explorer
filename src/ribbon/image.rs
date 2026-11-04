//! Ribbon images (`IUIImage`).
//!
//! Named icons resolve through the **explorer icon registry** (`src/icon.rs`:
//! Win8 PNG assets + colorful skin-aware SVG templates — the same icons
//! the rest of the app uses), falling back to the ribbon's own chrome
//! glyphs (`src/ribbon/icons.rs`: keytip chevron, blank placeholder)
//! for names the registry intentionally does not know. Inline SVG data
//! and `file:` references work as before.

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

/// Grayscale variant cache (`handle_gray`): SVG source strings and the
/// raster-to-SVG wrappers are computed once per icon.
fn gray_cache() -> &'static Mutex<std::collections::HashMap<String, IconHandle>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, IconHandle>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn skin() -> crate::ui_common::skin::Skin {
    if DARK.load(Ordering::Relaxed) {
        crate::ui_common::skin::dark()
    } else {
        crate::ui_common::skin::light()
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

    /// Desaturated variant of [`handle`](Self::handle) for the
    /// `gray_scaled_faded` disabled style. Falls back to the plain
    /// handle when the source cannot be wrapped.
    pub fn handle_gray(&self, size: u32) -> IconHandle {
        let key = match self {
            UiImage::Icon(n) => format!("i:{n}:{size}"),
            UiImage::Named(n) => format!("n:{n}:{size}"),
            UiImage::Svg(d) => format!("s:{:x}:{size}", fnv(d)),
            UiImage::File(p) => format!("f:{:?}:{size}", p),
        };
        if let Some(h) = gray_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
            return h;
        }
        let handle = self.gray_uncached(size);
        if let Ok(mut c) = gray_cache().lock() {
            c.insert(key, handle.clone());
        }
        handle
    }

    fn gray_uncached(&self, size: u32) -> IconHandle {
        fn wrap_svg(data: &[u8]) -> Option<IconHandle> {
            std::str::from_utf8(data).ok().map(|s| {
                IconHandle::Svg(SvgHandle::from_memory(
                    grayscale_svg(s).into_bytes(),
                ))
            })
        }
        fn named_gray(name: &str, size: u32) -> IconHandle {
            match crate::icon::handle(name, size, &skin()) {
                Some(crate::icon::IconHandle::Svg(h)) => match h.data() {
                    iced::advanced::svg::Data::Bytes(b) => {
                        wrap_svg(b).unwrap_or(IconHandle::Svg(h))
                    }
                    iced::advanced::svg::Data::Path(p) => {
                        std::fs::read(p)
                            .ok()
                            .and_then(|b| wrap_svg(&b))
                            .unwrap_or(IconHandle::Svg(h))
                    }
                },
                Some(crate::icon::IconHandle::Png(h)) => {
                    gray_png(&h).unwrap_or(IconHandle::Raster(h))
                }
                None => named(name, size),
            }
        }
        match self {
            UiImage::Icon(name) => named_gray(name, size),
            UiImage::Named(name) => named_gray(name, size),
            UiImage::Svg(data) => {
                IconHandle::Svg(SvgHandle::from_memory(grayscale_svg(data).into_bytes()))
            }
            UiImage::File(path) => {
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
                let wrapped: Option<IconHandle> = if raster {
                    std::fs::read(path).ok().and_then(|b| gray_png_bytes(&b))
                } else {
                    std::fs::read(path).ok().and_then(|b| wrap_svg(&b))
                };
                wrapped.unwrap_or_else(|| file_handle(path))
            }
        }
    }
}

/// Luminance `feColorMatrix` (Rec 709) scaled by 0.7: the raw luminance
/// gray reads too pale behind the opacity dim, so disabled icons are
/// pushed towards black.
const GRAY_FILTER: &str = "<filter id='gs'><feColorMatrix type='matrix' \
    values='0.1488 0.5006 0.0505 0 0 0.1488 0.5006 0.0505 0 0 \
    0.1488 0.5006 0.0505 0 0 0 0 0 1 0'/></filter>";

/// Wrap an SVG document in the grayscale filter (resvg renders it),
/// producing a desaturated variant of the icon.
fn grayscale_svg(s: &str) -> String {
    let defs = format!("<defs>{GRAY_FILTER}</defs>");
    // insert after the opening <svg ...> tag (past any <?xml?> prolog),
    // wrap the body in the filtered group
    let Some(svg_at) = s.find("<svg") else {
        return s.to_string();
    };
    let Some(tag_end) = s[svg_at..].find('>') else {
        return s.to_string();
    };
    let tag_end = svg_at + tag_end;
    let mut out = String::with_capacity(s.len() + defs.len() + 32);
    out.push_str(&s[..=tag_end]);
    out.push_str(&defs);
    out.push_str("<g filter='url(#gs)'>");
    let body = s[tag_end + 1..].trim_end();
    out.push_str(body.strip_suffix("</svg>").unwrap_or(body));
    out.push_str("</g></svg>");
    out
}

/// Desaturate a raster icon by embedding its PNG bytes in a filtered
/// SVG (`<image>` + `feColorMatrix`). Needs the pixel size for the
/// viewBox; read from the PNG IHDR chunk.
fn gray_png(h: &iced::widget::image::Handle) -> Option<IconHandle> {
    match h {
        iced::widget::image::Handle::Bytes(_, bytes) => gray_png_bytes(bytes),
        iced::widget::image::Handle::Path(_, path) => {
            std::fs::read(path).ok().and_then(|b| gray_png_bytes(&b))
        }
        iced::widget::image::Handle::Rgba { .. } => None,
    }
}

fn gray_png_bytes(bytes: &[u8]) -> Option<IconHandle> {
    const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[..8] != PNG_SIG {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if w == 0 || h == 0 {
        return None;
    }
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w} {h}' \
         width='{w}' height='{h}'>\
         {GRAY_FILTER}\
         <image href='data:image/png;base64,{0}' width='{w}' height='{h}' \
         filter='url(#gs)'/></svg>",
        base64_encode(bytes),
    );
    Some(IconHandle::Svg(SvgHandle::from_memory(svg.into_bytes())))
}

/// Tiny standard-alphabet base64 encoder (no crate dependency).
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// FNV-1a (cache key for inline SVG data).
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_wraps_body_and_keeps_structure() {
        let s = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'>\
                 <path d='M0 0h10v10z' fill='#F6C356'/></svg>";
        let g = grayscale_svg(s);
        assert!(g.contains("<svg xmlns='http://www.w3.org/2000/svg'"));
        assert!(g.contains("<defs><filter id='gs'>"));
        assert!(g.contains("<g filter='url(#gs)'>"));
        assert!(g.contains("<path d='M0 0h10v10z' fill='#F6C356'/>"));
        assert!(g.trim_end().ends_with("</g></svg>"));
        // the original closing tag must not be duplicated inside
        assert_eq!(g.matches("</svg>").count(), 1);
    }

    #[test]
    fn grayscale_handles_prolog_and_garbage() {
        let s = "<?xml version='1.0'?><svg><rect width='1'/></svg>";
        let g = grayscale_svg(s);
        // filter goes after the <svg> tag, not after the <?xml?> prolog
        let svg_at = g.find("<svg").unwrap();
        let defs_at = g.find("<defs>").unwrap();
        assert!(defs_at > svg_at);
        assert_eq!(grayscale_svg("not svg"), "not svg");
    }

    #[test]
    fn base64_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn gray_png_wraps_ihdr() {
        // minimal fake PNG: signature + IHDR length/type + 13 data bytes
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(&13u32.to_be_bytes()); // IHDR len
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&16u32.to_be_bytes()); // width
        png.extend_from_slice(&20u32.to_be_bytes()); // height
        png.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, color type, ...
        let out = gray_png_bytes(&png).expect("wraps");
        let IconHandle::Svg(_) = out else {
            panic!("expected svg wrapper");
        };
        assert!(gray_png_bytes(b"junk").is_none());
    }

    /// End-to-end: a colorful PNG embedded through the grayscale wrapper
    /// must rasterize (non-empty) and come out desaturated. This is the
    /// exact pipeline the renderer uses (usvg + resvg).
    #[test]
    fn gray_png_renders_desaturated() {
        let mut src = resvg::tiny_skia::Pixmap::new(4, 4).unwrap();
        // checkerboard of saturated colors, fully opaque
        let colors = [
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 0),
        ];
        for (i, px) in src.pixels_mut().iter_mut().enumerate() {
            let (r, g, b) = colors[i % 4];
            *px = resvg::tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255).unwrap();
        }
        let png = src.encode_png().unwrap();

        let IconHandle::Svg(handle) = gray_png_bytes(&png).expect("wraps") else {
            panic!("expected svg wrapper");
        };
        let iced::advanced::svg::Data::Bytes(bytes) = handle.data() else {
            panic!("expected in-memory svg");
        };
        let tree = resvg::usvg::Tree::from_str(
            std::str::from_utf8(bytes).unwrap(),
            &resvg::usvg::Options::default(),
        )
        .expect("wrapped svg parses");
        assert!(tree.size().width() > 0.0 && tree.size().height() > 0.0);
        let mut out = resvg::tiny_skia::Pixmap::new(4, 4).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut out.as_mut(),
        );
        // every pixel must be drawn and (approximately) gray:
        // |r - g| and |g - b| small, and not fully transparent
        for p in out.pixels() {
            let (r, g, b, a) = (p.red(), p.green(), p.blue(), p.alpha());
            assert!(a > 200, "pixel must be opaque, got alpha {a}");
            assert!(
                (r as i16 - g as i16).abs() <= 8 && (g as i16 - b as i16).abs() <= 8,
                "pixel must be desaturated, got r{r} g{g} b{b}",
            );
        }
    }
}

//! Off-window pixel painting — drag visuals, menu popup pixels and the
//! toplevel icon, plus the shared raster/text helpers behind them.
//!
//! Portable (no Wayland types): the Wayland side-channel thread
//! (`dnd_wayland.rs`, unix only) transports the painted pixels to the
//! compositor, and the same painters back the in-window drag overlays
//! on every platform.
/// is (0,0); the painter pre-offsets content so the compositor-placed
/// image lands exactly where the in-window overlays used to.
#[derive(Clone)]
// fields are read by the unix side-channel only (the Windows stub
// handle ignores visuals) — silence dead_code on non-unix
#[cfg_attr(not(unix), allow(dead_code))]
pub struct DragVisual {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// RGBA -> BGRA (in place copy).
// unix-only caller (shm buffers want BGRA)
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn swizzle(rgba: &[u8]) -> Vec<u8> {
    let (chunks, _) = rgba.as_chunks::<4>();
    let mut out = Vec::with_capacity(rgba.len());
    for px in chunks {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}
// ---------------------------------------------------------------------------
// Drag-image painting (GUI thread)
// ---------------------------------------------------------------------------

/// The visual the compositor shows at the cursor during a drag: the
/// faded icon-stack badge plus the caption plate — the two tooltips the
/// in-window overlay (`ui::drag_caption` + `ui::drag_icon_badge`) shows,
/// composed into one surface (Wayland has a single drag icon).
///
/// Geometry is SHARED with the in-window overlays (ui.rs): the badge's
/// top-left sits at cursor + (2,2), the caption plate at cursor +
/// (71,71). The drag surface's hotspot is fixed at its top-left corner
/// (= the cursor), which cannot express "cursor inside the bbox" — so
/// both hosts use this same origin and the handoff is seamless.
pub fn paint_drag_visual(
    items: &[crate::dnd::DraggedItem],
    top: Option<&str>,
    total: usize,
    caption: &crate::dnd::DragCaption,
    skin: &crate::theme::Palette,
    scale: f32,
) -> DragVisual {
    const BADGE: f32 = 50.0;
    const PAD: f32 = 3.0;
    const AREA: f32 = BADGE - 2.0 * PAD; // 44
    const STEP: f32 = 3.0;
    const MAX: usize = 5;
    const COUNT: f32 = 18.0;
    const GAP: f32 = 19.0; // badge <-> caption
    const FADE: u16 = 153; // 0.6

    let scale = scale.max(1.0);
    // caption plate extent (logical)
    let plate = match caption {
        crate::dnd::DragCaption::Hidden => None,
        crate::dnd::DragCaption::Banned => Some((20.0, 20.0, None)),
        crate::dnd::DragCaption::Undecided => plate_size("…", scale),
        crate::dnd::DragCaption::Action { label, .. } => plate_size(label, scale),
    };
    let (pw, ph) = plate
        .as_ref()
        .map(|(w, h, _)| (*w, *h))
        .unwrap_or((0.0, 0.0));
    // surface covers badge + plate; the plate's top-left is at
    // (DX + BADGE + GAP - (DX - BADGE - GAP)) = 2*(BADGE+GAP) = 138? no:
    // caption_tl - badge_tl = ((DX) - (DX - BADGE - GAP), (DY) - (DY - BADGE - GAP))
    //                      = (BADGE + GAP, BADGE + GAP) = (69, 69)
    let plate_at = BADGE + GAP; // 69
    let w = ((plate_at + pw + OFF).max(BADGE + OFF) * scale).ceil() as u32;
    let h = ((plate_at + ph + OFF).max(BADGE + OFF) * scale).ceil() as u32;
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut cv = Canvas { px: &mut px, w, h };

    // --- the badge square (accent @ 0.6), offset (2,2) from the hotspot
    let accent = skin.accent;
    const OFF: f32 = 2.0;
    cv.fill_rect(scale, OFF, OFF, BADGE, BADGE, accent, FADE);

    // --- the icon stack, deepest first ---
    let top = top.map(str::to_string);
    let mut shown: Vec<&crate::dnd::DraggedItem> = items
        .iter()
        .filter(|i| Some(i.uri.as_str()) != top.as_deref())
        .take(MAX - 1)
        .collect();
    if let Some(t) = items.iter().find(|i| Some(i.uri.as_str()) == top.as_deref()) {
        shown.insert(0, t);
    } else {
        shown = items.iter().take(MAX).collect();
    }
    let n = shown.len().max(1);
    let icon_size = AREA - (n as f32 - 1.0) * STEP;
    for (j, it) in shown.iter().enumerate().rev() {
        // deepest icon first: index 0 is the top item, drawn last at the
        // largest offset — matches the in-window stack
        let off = (n - 1 - j) as f32 * STEP;
        let name = crate::dnd::uri_to_path(&it.uri)
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_else(|| it.uri.clone());
        let icon = icon_pix(skin, it.kind == crate::dnd::ItemKind::Directory, &name, icon_size * scale);
        cv.blend_icon(scale, OFF + PAD + off, OFF + PAD + off, icon_size, &icon, FADE);
    }

    // --- the count plate (solid accent, bottom-left) ---
    if total > 1 {
        cv.fill_rect(scale, OFF + BADGE - COUNT, OFF + BADGE - COUNT, COUNT, COUNT, accent, 255);
        let label = total.to_string();
        let (tw, th) = text_size(&label, 11.0 * scale);
        draw_text(
            &mut cv,
            (OFF + BADGE - COUNT + (COUNT - tw / scale) / 2.0) * scale,
            (OFF + BADGE - COUNT + (COUNT - th / scale) / 2.0) * scale,
            11.0 * scale,
            &label,
            [255, 255, 255, 255],
        );
    }

    // --- the caption plate ---
    if let Some((pw, ph, text)) = plate {
        let x = (plate_at + OFF) * scale;
        let y = (plate_at + OFF) * scale;
        let pw = pw * scale;
        let ph = ph * scale;
        cv.fill_rect(1.0, x, y, pw, ph, skin.menu_bg, 255);
        // 1px border (physical)
        cv.stroke_rect(x, y, pw, ph, skin.menu_border);
        match (text, caption) {
            (Some(text), _) => {
                let size = 13.0 * scale;
                let (tw, th) = text_size(&text, size);
                draw_text(
                    &mut cv,
                    x + (pw - tw) / 2.0,
                    y + (ph - th) / 2.0,
                    size,
                    &text,
                    rgba255(skin.text),
                );
            }
            // banned glyph: red circle + slash, vector-drawn via resvg
            _ if matches!(caption, crate::dnd::DragCaption::Banned) => {
                let icon = raster_svg_str(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" fill="none" stroke="#E81123" stroke-width="2.2"/><line x1="5.6" y1="14.4" x2="14.4" y2="5.6" stroke="#E81123" stroke-width="2.2"/></svg>"##,
                    20.0 * scale,
                );
                cv.blend_icon(1.0, x, y, 20.0 * scale, &icon, 255);
            }
            _ => {}
        }
    }

    DragVisual {
        width: w,
        height: h,
        pixels: px,
    }
}

fn icon_pix(skin: &crate::theme::Palette, is_dir: bool, name: &str, size: f32) -> crate::icon::IconPix {
    crate::icon::raster_for(is_dir, name, size.round().max(1.0) as u32, skin)
}

fn raster_svg_str(svg: &str, size: f32) -> crate::icon::IconPix {
    crate::icon::raster_svg(svg, size.round().max(1.0) as u32).unwrap_or(crate::icon::IconPix {
        width: 1,
        height: 1,
        pixels: vec![0; 4],
    })
}

// --- tiny raster helpers (all coordinates physical px unless *scale) ---
// pub(crate): shared with the off-window menu painter (menu.rs) and the
// toplevel-icon painter (icon_wayland.rs)

pub(crate) fn rgba255(c: iced::Color) -> [u8; 4] {
    [
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    ]
}

pub(crate) struct Canvas<'a> {
    pub(crate) px: &'a mut [u8],
    pub(crate) w: u32,
    pub(crate) h: u32,
}

impl Canvas<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fill_rect(&mut self, scale: f32, x: f32, y: f32, rw: f32, rh: f32, c: iced::Color, alpha: u16) {
        let c = rgba255(c);
        let x0 = (x * scale) as i32;
        let y0 = (y * scale) as i32;
        let x1 = ((x + rw) * scale).ceil() as i32;
        let y1 = ((y + rh) * scale).ceil() as i32;
        for yy in y0.max(0)..y1.min(self.h as i32) {
            for xx in x0.max(0)..x1.min(self.w as i32) {
                self.blend(xx, yy, c, alpha as u8);
            }
        }
    }

    pub(crate) fn stroke_rect(&mut self, x: f32, y: f32, rw: f32, rh: f32, c: iced::Color) {
        let c = rgba255(c);
        let x1 = (x + rw - 1.0) as i32;
        let y1 = (y + rh - 1.0) as i32;
        for xx in x as i32..=x1 {
            self.blend(xx, y as i32, c, 255);
            self.blend(xx, y1, c, 255);
        }
        for yy in y as i32..=y1 {
            self.blend(x as i32, yy, c, 255);
            self.blend(x1, yy, c, 255);
        }
    }

    pub(crate) fn blend_icon(&mut self, scale: f32, x: f32, y: f32, size: f32, icon: &crate::icon::IconPix, fade: u16) {
        // fit the icon into size*size, centered, blended at `fade`/255
        let (iw, ih) = (icon.width as f32, icon.height as f32);
        if iw <= 0.0 || ih <= 0.0 {
            return;
        }
        let fit = (size * scale) / iw.max(ih);
        let dw = (iw * fit) as i32;
        let dh = (ih * fit) as i32;
        let ox = (x * scale) as i32 + ((size * scale) as i32 - dw) / 2;
        let oy = (y * scale) as i32 + ((size * scale) as i32 - dh) / 2;
        for j in 0..dh {
            for i in 0..dw {
                let sx = (i as f32 / fit) as u32;
                let sy = (j as f32 / fit) as u32;
                if sx >= icon.width || sy >= icon.height {
                    continue;
                }
                let o = ((sy * icon.width + sx) * 4) as usize;
                let a = icon.pixels[o + 3] as u16 * fade / 255;
                self.blend(
                    ox + i,
                    oy + j,
                    [icon.pixels[o], icon.pixels[o + 1], icon.pixels[o + 2], 255],
                    a as u8,
                );
            }
        }
    }

    /// Source-over blend of `color` (premultiplied by mask) at (x, y).
    pub(crate) fn blend(&mut self, x: i32, y: i32, color: [u8; 4], mask: u8) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return;
        }
        let a = color[3] as u16 * mask as u16 / 255;
        let inv = 255 - a;
        let p = &mut self.px[((y as usize * self.w as usize) + x as usize) * 4..][..4];
        for c in 0..3 {
            let src = color[c] as u16 * a / 255;
            p[c] = (src + p[c] as u16 * inv / 255) as u8;
        }
        p[3] = (a + p[3] as u16 * inv / 255) as u8;
    }
}

// --- text (cosmic-text, cached font system) ---

fn text_system() -> &'static std::sync::Mutex<(cosmic_text::FontSystem, cosmic_text::SwashCache)> {
    static TS: std::sync::OnceLock<std::sync::Mutex<(cosmic_text::FontSystem, cosmic_text::SwashCache)>> =
        std::sync::OnceLock::new();
    TS.get_or_init(|| {
        let fs = cosmic_text::FontSystem::new();
        #[cfg(not(unix))]
        let fs = {
            // fresh wine prefix: FontSystem::new found zero faces (its
            // fontdb scan sees the empty C:\Windows\Fonts); load the
            // registry-listed fonts so painted captions/menus show text
            let mut fs = fs;
            if fs.db_mut().faces().next().is_none() {
                for path in crate::fonts::supplementary_font_paths() {
                    if let Ok(data) = std::fs::read(&path) {
                        fs.db_mut().load_font_data(data);
                    }
                }
            }
            fs
        };
        // the embedded UI font (feature `embed-selawik`) is loaded into
        // the renderer's font system at startup (main.rs app.fonts) but
        // NOT into system fontdb copies: without it, `attrs`' primary
        // family ("Selawik") is unresolvable here and every painted
        // menu/caption glyph falls back to a generic sans — a visibly
        // different family from the in-window UI
        #[cfg(feature = "embed-selawik")]
        fs.db_mut()
            .load_font_data(crate::fonts::EMBED_BYTES.to_vec());
        std::sync::Mutex::new((fs, cosmic_text::SwashCache::new()))
    })
}

fn plate_size(text: &str, scale: f32) -> Option<(f32, f32, Option<String>)> {
    let size = 13.0 * scale;
    let (w, h) = text_size(text, size);
    // padding [2, 7] logical + border, like boxed_caption
    Some((w / scale + 14.0 + 2.0, h / scale + 4.0 + 2.0, Some(text.to_string())))
}

fn text_size(text: &str, size: f32) -> (f32, f32) {
    text_size_weighted(text, size, false)
}

/// Text attributes with the app's primary family (fonts.yaml fallback
/// list) — the same face the in-window UI renders with.
pub(crate) fn attrs(bold: bool) -> cosmic_text::Attrs<'static> {
    let mut a = cosmic_text::Attrs::new();
    if let iced::font::Family::Name(name) = crate::fonts::primary().family {
        a = a.family(cosmic_text::Family::Name(name));
    }
    if bold {
        a = a.weight(cosmic_text::Weight::BOLD);
    }
    a
}

/// Real text metrics (physical px). `bold` matches the menu's default
/// action row.
pub(crate) fn text_size_weighted(text: &str, size: f32, bold: bool) -> (f32, f32) {
    let mut ts = text_system().lock().unwrap();
    let (fs, _) = &mut *ts;
    let attrs = attrs(bold);
    let mut buffer = cosmic_text::Buffer::new(fs, cosmic_text::Metrics::new(size, size * 1.25));
    buffer.set_text(text, &attrs, cosmic_text::Shaping::Basic, None);
    buffer.shape_until_scroll(fs, false);
    let mut w = 0.0f32;
    let mut h = 0.0f32;
    for run in buffer.layout_runs() {
        w = w.max(run.line_w);
        h = h.max(run.line_top + run.line_height);
    }
    (w, h)
}

/// Draw `text` with an optional bold weight (menus: default action).
pub(crate) fn draw_text(cv: &mut Canvas, x: f32, y: f32, size: f32, text: &str, color: [u8; 4]) {
    draw_text_weighted(cv, x, y, size, text, color, false)
}

pub(crate) fn draw_text_weighted(
    cv: &mut Canvas,
    x: f32,
    y: f32,
    size: f32,
    text: &str,
    color: [u8; 4],
    bold: bool,
) {
    let mut ts = text_system().lock().unwrap();
    let (fs, cache) = &mut *ts;
    let attrs = attrs(bold);
    let mut buffer = cosmic_text::Buffer::new(fs, cosmic_text::Metrics::new(size, size * 1.25));
    buffer.set_text(text, &attrs, cosmic_text::Shaping::Basic, None);
    buffer.shape_until_scroll(fs, false);
    for run in buffer.layout_runs() {
        // round the line origin ONCE: per-glyph truncation snaps glyphs
        // onto different pixel rows (the "up and down" text)
        let origin_y = (y + run.line_y).round();
        let origin_x = x.round();
        for glyph in run.glyphs {
            let px = (origin_x + glyph.x).round() as i32;
            let py = (origin_y + glyph.y).round() as i32;
            // the cache key still needs the subpixel bins (placement)
            let physical = glyph.physical((origin_x, origin_y), 1.0);
            let Some(img) = cache.get_image(fs, physical.cache_key).clone() else {
                continue;
            };
            let iw = img.placement.width as i32;
            let ih = img.placement.height as i32;
            let ox = px + img.placement.left;
            // swash placement is Y-UP (zeno convention): the ink top
            // sits `top` px ABOVE the baseline — add it and every
            // glyph lands below the baseline, colliding with the row
            // beneath (the "up and down" text + ghost row)
            let oy = py - img.placement.top;
            for j in 0..ih {
                for i in 0..iw {
                    let m = img.data[(j * iw + i) as usize];
                    cv.blend(ox + i, oy + j, color, m);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Toplevel-icon painting (GUI thread)
// ---------------------------------------------------------------------------

/// Pixels for one toplevel-icon assignment: one square buffer per
/// wl scale, premultiplied RGBA (like [`DragVisual`]).
#[cfg_attr(not(unix), allow(dead_code))]
pub struct ToplevelIcon {
    pub sizes: Vec<IconSize>,
}

#[cfg_attr(not(unix), allow(dead_code))]
pub struct IconSize {
    /// wl buffer scale the pixels were rendered for (e.g. 1, 2)
    pub scale: i32,
    /// square `width == height`
    pub visual: DragVisual,
}

/// Paint the toplevel (taskbar/window-switcher) icon: the location icon
/// centered on a transparent SQUARE canvas at each wl scale — the
/// Wayland protocol only accepts square shm buffers (on other platforms
/// winit takes a plain RGBA icon). Two sizes cover common taskbars
/// (16-32px at scale 1, hidpi at scale 2).
pub fn paint_toplevel_icon(name: &str, skin: &crate::theme::Palette) -> ToplevelIcon {
    let sizes = [(1, 64), (2, 128)]
        .into_iter()
        .map(|(scale, px)| {
            let icon = crate::icon::raster(name, px, skin);
            let mut visual = DragVisual {
                width: px,
                height: px,
                pixels: vec![0; (px * px * 4) as usize],
            };
            let mut cv = Canvas {
                px: &mut visual.pixels,
                w: px,
                h: px,
            };
            cv.blend_icon(1.0, 0.0, 0.0, px as f32, &icon, 255);
            IconSize { scale, visual }
        })
        .collect();
    ToplevelIcon { sizes }
}

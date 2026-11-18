//! Text rendering for the demo: cosmic-text lays out + rasterizes glyphs,
//! we blend them into the premultiplied RGBA shm canvas (same pixel layout
//! as tiny-skia and wl_shm `Abgr8888`).

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, SwashContent};

pub struct Text {
    font_system: FontSystem,
    swash: SwashCache,
}

impl Text {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash: SwashCache::new(),
        }
    }

    /// Draw `s` with its top-left corner at (`x`, `y`), in premultiplied
    /// RGBA (`color`), onto a `w` x `h` premultiplied RGBA canvas.
    pub fn draw(
        &mut self,
        canvas: &mut [u8],
        w: usize,
        h: usize,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [u8; 4],
    ) {
        if s.is_empty() {
            return;
        }
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(size, size * 1.25));
        buffer.set_text(s, &Attrs::new(), Shaping::Basic, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((x, y + run.line_y), 1.0);
                let px = physical.x;
                let py = physical.y;
                let Some(img) = self
                    .swash
                    .get_image(&mut self.font_system, physical.cache_key)
                    .clone()
                else {
                    continue;
                };
                let iw = img.placement.width as i32;
                let ih = img.placement.height as i32;
                let ox = px + img.placement.left;
                let oy = py + img.placement.top;
                match img.content {
                    SwashContent::Mask => {
                        for j in 0..ih {
                            for i in 0..iw {
                                let m = img.data[(j * iw + i) as usize];
                                blend(canvas, w, h, ox + i, oy + j, color, m);
                            }
                        }
                    }
                    SwashContent::Color => {
                        // swash color images are already premultiplied
                        for j in 0..ih {
                            for i in 0..iw {
                                let o = ((j * iw + i) * 4) as usize;
                                let c = [img.data[o], img.data[o + 1], img.data[o + 2], img.data[o + 3]];
                                blend_premul(canvas, w, h, ox + i, oy + j, c);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Source-over blend of `color` premultiplied by (`mask`/255 * color.a)
/// into the premultiplied destination pixel.
fn blend(canvas: &mut [u8], w: usize, h: usize, x: i32, y: i32, color: [u8; 4], mask: u8) {
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
        return;
    }
    let a = color[3] as u16 * mask as u16 / 255;
    let inv = 255 - a;
    let p = &mut canvas[((y as usize * w) + x as usize) * 4..][..4];
    for c in 0..3 {
        let src = color[c] as u16 * a / 255;
        p[c] = (src + p[c] as u16 * inv / 255) as u8;
    }
    p[3] = (a + p[3] as u16 * inv / 255) as u8;
}

/// Source-over blend of an already-premultiplied source pixel.
fn blend_premul(canvas: &mut [u8], w: usize, h: usize, x: i32, y: i32, src: [u8; 4]) {
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
        return;
    }
    let a = src[3] as u16;
    let inv = 255 - a;
    let p = &mut canvas[((y as usize * w) + x as usize) * 4..][..4];
    for c in 0..3 {
        p[c] = (src[c] as u16 + p[c] as u16 * inv / 255) as u8;
    }
    p[3] = (a + p[3] as u16 * inv / 255) as u8;
}

//! Software rasterizer for egui's textured triangles.
//!
//! - Output format: one `u32` per pixel, `0x00RRGGBB` (softbuffer contract).
//! - Textures: premultiplied sRGB (see render/mod.rs), sampled bilinearly.
//! - Blending: premultiplied source-over in sRGB space (gamma-correct
//!   compositing is a possible future upgrade).
//! - No MSAA yet; egui's font textures carry their own AA. Straight shape
//!   edges are currently 1px-crisp (acceptable for the Win8 flat look).

use super::TextureStore;
use egui::epaint::ClippedPrimitive;

pub struct CpuRenderer {
    pub width: usize,
    pub height: usize,
    /// 0x00RRGGBB
    pub buffer: Vec<u32>,
    pub textures: TextureStore,
}

impl CpuRenderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0x00FF00FF; width * height],
            textures: TextureStore::new(),
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.buffer.resize(width * height, 0x00FF00FF);
        }
    }

    /// Rasterize a frame. Caller applies texture deltas before and frees
    /// textures afterwards (platform does both around this call).
    ///
    /// egui 0.36 tessellates in **logical points** (verified against epaint
    /// source: ppp is only used for pixel-snapping); the painter scales to
    /// physical pixels here.
    pub fn render(&mut self, primitives: &[ClippedPrimitive], pixels_per_point: f32) {
        self.buffer.fill(0x00FFFFFF);
        let screen = (self.width as f32, self.height as f32);
        let width_i = self.width as i32;
        // Destructure so `texture` (borrow of `textures`) and `buffer` (mut
        // borrow) can coexist through the loop.
        let CpuRenderer { buffer, textures, .. } = self;
        for prim in primitives {
            let Some(mesh) = (match &prim.primitive {
                egui::epaint::Primitive::Mesh(m) => Some(m),
                egui::epaint::Primitive::Callback(_) => None,
            }) else {
                continue;
            };
            if mesh.indices.len() < 3 || mesh.vertices.is_empty() {
                continue;
            }
            // clip rect: points -> px, intersect screen
            let clip = prim.clip_rect;
            let cx0 = (clip.min.x * pixels_per_point).round().max(0.0) as i32;
            let cy0 = (clip.min.y * pixels_per_point).round().max(0.0) as i32;
            let cx1 = (clip.max.x * pixels_per_point).round().min(screen.0) as i32;
            let cy1 = (clip.max.y * pixels_per_point).round().min(screen.1) as i32;
            if cx1 <= cx0 || cy1 <= cy0 {
                continue;
            }
            let texture = textures.get(mesh.texture_id);
            for tri in mesh.indices.chunks_exact(3) {
                let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                if a >= mesh.vertices.len() || b >= mesh.vertices.len() || c >= mesh.vertices.len() {
                    continue;
                }
                let v = [&mesh.vertices[a], &mesh.vertices[b], &mesh.vertices[c]];
                let p: [(f32, f32); 3] = [
                    (v[0].pos.x * pixels_per_point, v[0].pos.y * pixels_per_point),
                    (v[1].pos.x * pixels_per_point, v[1].pos.y * pixels_per_point),
                    (v[2].pos.x * pixels_per_point, v[2].pos.y * pixels_per_point),
                ];
                let min_x = p[0].0.min(p[1].0).min(p[2].0).round().max(cx0 as f32) as i32;
                let max_x = p[0].0.max(p[1].0).max(p[2].0).round().min(cx1 as f32) as i32;
                let min_y = p[0].1.min(p[1].1).min(p[2].1).round().max(cy0 as f32) as i32;
                let max_y = p[0].1.max(p[1].1).max(p[2].1).round().min(cy1 as f32) as i32;
                if min_x >= max_x || min_y >= max_y {
                    continue;
                }
                raster_tri(buffer, width_i, p, v, texture, min_x, min_y, max_x, max_y);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn raster_tri(
    buffer: &mut [u32],
    w: i32,
    p: [(f32, f32); 3],
    v: [&egui::epaint::Vertex; 3],
    texture: Option<&super::Texture>,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
) {
    let (x0, y0) = (p[0].0, p[0].1);
    let (x1, y1) = (p[1].0, p[1].1);
    let (x2, y2) = (p[2].0, p[2].1);
    let area = edge(x0, y0, x1, y1, x2, y2);
    if area.abs() < 0.0001 {
        return;
    }
    for py in min_y..max_y {
        let sy = py as f32 + 0.5;
        for px in min_x..max_x {
            let sx = px as f32 + 0.5;
            let w0 = edge(x1, y1, x2, y2, sx, sy);
            let w1 = edge(x2, y2, x0, y0, sx, sy);
            let w2 = edge(x0, y0, x1, y1, sx, sy);
            let inside = if area > 0.0 {
                w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
            } else {
                w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
            };
            if !inside {
                continue;
            }
            // barycentric weights
            let (l0, l1, l2) = (w0 / area, w1 / area, w2 / area);
            // interpolate vertex color
            let vc = |f: fn(&egui::epaint::Vertex) -> u8| -> u32 {
                (f(v[0]) as f32 * l0 + f(v[1]) as f32 * l1 + f(v[2]) as f32 * l2) as u32
            };
            let cr = vc(|v| v.color.r());
            let cg = vc(|v| v.color.g());
            let cb = vc(|v| v.color.b());
            let ca = vc(|v| v.color.a());
            // sample texture
            let (tr, tg, tb, ta) = if let Some(tex) = texture {
                let u = v[0].uv.x * l0 + v[1].uv.x * l1 + v[2].uv.x * l2;
                let vv = v[0].uv.y * l0 + v[1].uv.y * l1 + v[2].uv.y * l2;
                sample(tex, u, vv)
            } else {
                (255, 255, 255, 255)
            };
            // premultiplied source-over: src = tex * color(straight)
            let a = ta * ca / 255;
            let sr = tr * cr / 255;
            let sg = tg * cg / 255;
            let sb = tb * cb / 255;
            if a == 0 {
                continue;
            }
            let idx = (py * w + px) as usize;
            if idx >= buffer.len() {
                continue;
            }
            let d = buffer[idx];
            let dr = (d >> 16) & 0xFF;
            let dg = (d >> 8) & 0xFF;
            let db = d & 0xFF;
            let inv = 255 - a;
            let or = sr + (dr * inv + 127) / 255;
            let og = sg + (dg * inv + 127) / 255;
            let ob = sb + (db * inv + 127) / 255;
            buffer[idx] = 0xFF000000 | (or.min(255) << 16) | (og.min(255) << 8) | ob.min(255);
        }
    }
}

#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Sample a premultiplied sRGB texture at normalized uv (origin top-left),
/// honoring its filtering mode: nearest for font atlases (egui default),
/// bilinear for linear-registered images. Out-of-bounds reads = WHITE.
fn sample(tex: &super::Texture, u: f32, v: f32) -> (u32, u32, u32, u32) {
    let (w, h) = (tex.width as f32, tex.height as f32);
    if w <= 0.0 || h <= 0.0 {
        return (255, 255, 255, 255);
    }
    if tex.options.magnification == egui::epaint::textures::TextureFilter::Nearest {
        // egui uv for texel i is (i + 0.5) / w
        let x = (u * w).floor().clamp(0.0, w - 1.0) as usize;
        let y = (v * h).floor().clamp(0.0, h - 1.0) as usize;
        let Some(px) = tex.pixels.get(y * tex.width + x).copied() else {
            return (255, 255, 255, 255);
        };
        ((px >> 16) & 0xFF, (px >> 8) & 0xFF, px & 0xFF, (px >> 24) & 0xFF)
    } else {
        sample_bilinear(tex, u, v)
    }
}

/// Bilinear sample of a premultiplied sRGB texture at normalized uv.
/// uv origin is top-left; clamps to edges. `WHITE` when out of bounds.
fn sample_bilinear(tex: &super::Texture, u: f32, v: f32) -> (u32, u32, u32, u32) {
    let (w, h) = (tex.width as f32, tex.height as f32);
    if w <= 0.0 || h <= 0.0 {
        return (255, 255, 255, 255);
    }
    let x = u * w - 0.5;
    let y = v * h - 0.5;
    if x < -1.0 || y < -1.0 || x > w || y > h {
        return (255, 255, 255, 255);
    }
    let x = x.clamp(0.0, w - 1.0);
    let y = y.clamp(0.0, h - 1.0);
    let x0 = x as usize;
    let y0 = y as usize;
    let x1 = (x0 + 1).min(tex.width - 1);
    let y1 = (y0 + 1).min(tex.height - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let p = |xi: usize, yi: usize| -> u32 { tex.pixels[yi * tex.width + xi] };
    let mix = |shift: u32| -> u32 {
        let get = |px: u32| (px >> shift) & 0xFF;
        let a = get(p(x0, y0)) as f32 * (1.0 - fx) + get(p(x1, y0)) as f32 * fx;
        let b = get(p(x0, y1)) as f32 * (1.0 - fx) + get(p(x1, y1)) as f32 * fx;
        (a * (1.0 - fy) + b * fy) as u32
    };
    let a = mix(24);
    let r = mix(16);
    let g = mix(8);
    let b = mix(0);
    (r, g, b, a)
}

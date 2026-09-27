//! Rendering seam (D3). The UI produces egui meshes; a Renderer turns them
//! into pixels on some surface. CPU/softbuffer today; wgpu/Vulkan slots in
//! behind the same trait. UI code must never know which backend is active.

pub mod bmp;
pub mod cpu;

use egui::epaint::textures::TextureOptions;
use egui::epaint::{ImageData, ImageDelta, TextureId};

/// CPU-side texture in premultiplied sRGB, 8 bits per channel (RGBA8).
pub struct Texture {
    pub width: usize,
    pub height: usize,
    /// 0xAARRGGBB premultiplied alpha, native endian.
    pub pixels: Vec<u32>,
    /// Sampling mode egui registered the texture with (fonts = nearest).
    pub options: TextureOptions,
}

impl Texture {
    fn from_image(image: &egui::ColorImage, options: TextureOptions) -> Self {
        let [w, h] = image.size;
        let pixels: Vec<u32> = image
            .pixels
            .iter()
            .map(|c| {
                let [r, g, b, a] = c.to_array();
                let r = r as u32;
                let g = g as u32;
                let b = b as u32;
                let a = a as u32;
                // premultiply (epaint color images arrive straight)
                ((a << 24) | (((r * a / 255) & 0xFF) << 16) | (((g * a / 255) & 0xFF) << 8) | ((b * a / 255) & 0xFF)) as u32
            })
            .collect();
        Texture { width: w, height: h, pixels, options }
    }
}

pub struct TextureStore {
    textures: std::collections::HashMap<TextureId, Texture>,
}

impl Default for TextureStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureStore {
    pub fn new() -> Self {
        Self { textures: std::collections::HashMap::new() }
    }

    pub fn apply(&mut self, id: TextureId, deltas: &[ImageDelta]) {
        for delta in deltas {
            self.apply_one(id, delta);
        }
    }

    fn apply_one(&mut self, id: TextureId, delta: &ImageDelta) {
        match &delta.image {
            ImageData::Color(image) => {
                let tex = Texture::from_image(image, delta.options);
                if let Some([ox, oy]) = delta.pos {
                    if let Some(existing) = self.textures.get_mut(&id) {
                        patch(existing, &tex, ox, oy);
                        existing.options = delta.options;
                        return;
                    }
                }
                self.textures.insert(id, tex);
            }
            #[allow(unreachable_patterns)]
            _ => {
                // egui 0.36+ rasterizes fonts into colored meshes — no font
                // texture variants exist anymore.
            }
        }
    }

    pub fn free(&mut self, id: TextureId) {
        self.textures.remove(&id);
    }

    pub fn get(&self, id: TextureId) -> Option<&Texture> {
        self.textures.get(&id)
    }
}

fn patch(dst: &mut Texture, src: &Texture, ox: usize, oy: usize) {
    for y in 0..src.height {
        let dy = oy + y;
        if dy >= dst.height {
            break;
        }
        for x in 0..src.width {
            let dx = ox + x;
            if dx >= dst.width {
                break;
            }
            let si = y * src.width + x;
            let di = dy * dst.width + dx;
            if si < src.pixels.len() && di < dst.pixels.len() {
                dst.pixels[di] = src.pixels[si];
            }
        }
    }
}

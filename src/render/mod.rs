//! Rendering seam (D3). The UI produces egui meshes; a Renderer turns them
//! into pixels on some surface. CPU/softbuffer today; wgpu/Vulkan slots in
//! behind the same trait. UI code must never know which backend is active.

pub mod bmp;
pub mod cpu;

use egui::epaint::{ImageData, ImageDelta, TextureId};

/// CPU-side texture in premultiplied sRGB, 8 bits per channel (RGBA8).
pub struct Texture {
    pub width: usize,
    pub height: usize,
    /// 0xAARRGGBB premultiplied alpha, native endian.
    pub pixels: Vec<u32>,
}

impl Texture {
    fn from_image(image: &egui::ColorImage) -> Self {
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
        Texture { width: w, height: h, pixels }
    }

    fn from_alpha8(size: [usize; 2], data: &[u8]) -> Self {
        let [w, h] = size;
        let mut pixels = Vec::with_capacity(w * h);
        for &a in data {
            let a = a as u32;
            pixels.push((a << 24) | (0xFF * a / 255) << 16 | (0xFF * a / 255) << 8 | (0xFF * a / 255));
        }
        Texture { width: w, height: h, pixels }
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

    pub fn apply(&mut self, delta: &ImageDelta) {
        match &delta.image {
            ImageData::Color(image) => {
                let tex = Texture::from_image(image);
                if let Some([ox, oy]) = delta.pos {
                    if let Some(existing) = self.textures.get_mut(&delta.texture_id) {
                        patch(existing, &tex, ox, oy);
                        return;
                    }
                }
                self.textures.insert(delta.texture_id, tex);
            }
            ImageData::Font(font) => {
                let tex = Texture::from_alpha8(font.size, &font.pixels);
                if let Some([ox, oy]) = delta.pos {
                    if let Some(existing) = self.textures.get_mut(&delta.texture_id) {
                        patch(existing, &tex, ox, oy);
                        return;
                    }
                }
                self.textures.insert(delta.texture_id, tex);
            }
            _ => {
                // Svg/CompliedShape variants — not produced by egui itself; ignore.
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

//! Minimal 24-bit BMP writer — used by `--snapshot` verification renders only.

use std::path::Path;

pub fn write(path: &Path, width: u32, height: u32, pixels: &[u32]) -> std::io::Result<()> {
    let row_stride = (width * 3 + 3) & !3;
    let image_size = row_stride * height;
    let file_size = 54 + image_size;

    let mut data = Vec::with_capacity(file_size as usize);
    // BITMAPFILEHEADER
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&file_size.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&width.to_le_bytes());
    data.extend_from_slice(&(height as i32).to_le_bytes()); // positive = bottom-up
    data.extend_from_slice(&1u16.to_le_bytes()); // planes
    data.extend_from_slice(&24u16.to_le_bytes()); // bpp
    data.extend_from_slice(&0u32.to_le_bytes()); // compression
    data.extend_from_slice(&image_size.to_le_bytes());
    data.extend_from_slice(&2835i32.to_le_bytes()); // ~72dpi
    data.extend_from_slice(&2835i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    let mut row = vec![0u8; row_stride as usize];
    for y in (0..height).rev() {
        for x in 0..width {
            let px = pixels[(y * width + x) as usize];
            let (r, g, b) = (((px >> 16) & 0xFF) as u8, ((px >> 8) & 0xFF) as u8, (px & 0xFF) as u8);
            row[(x * 3) as usize] = b;
            row[(x * 3 + 1) as usize] = g;
            row[(x * 3 + 2) as usize] = r;
        }
        data.extend_from_slice(&row);
    }
    std::fs::write(path, data)
}

//! Offscreen verification render (D8): build the app, run one egui frame,
//! rasterize with the CPU backend, write a BMP. No window, no compositor.

use crate::app::{App, Cmd};
use crate::render::bmp;
use crate::Options;

pub fn run(opts: Options, out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = opts.snapshot_size;
    let scale = opts.snapshot_scale;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);

    let mut app = App::new_headless(opts);
    // Set a deterministic fake listing if the real fs is unavailable.
    app.bootstrap();

    // Two frames: the first uploads the full font atlas, the second patches
    // it — ONE renderer must persist across both (patches build on frame 1).
    let mut renderer = crate::render::cpu::CpuRenderer::new(
        (w as f32 * scale) as usize,
        (h as f32 * scale) as usize,
    );
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(w as f32, h as f32),
    ));
    for _ in 0..2 {
        let mut output = ctx.run_ui(input.clone(), |ui| {
            app.frame(ui);
        });
        let clipped = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            renderer.textures.apply(*id, delta);
        }
        renderer.render(&clipped);
        bmp::write(out, renderer.width as u32, renderer.height as u32, &renderer.buffer)?;
        for id in &output.textures_delta.free {
            renderer.textures.free(*id);
        }
        // TexturesDelta panics in Drop if not fully consumed.
        output.textures_delta.clear();
    }
    println!("snapshot written to {}", out.display());
    Ok(())
}

/// Convenience used by docs: the Cmd a frame would emit.
#[allow(dead_code)]
pub fn unused(_: Cmd) {}

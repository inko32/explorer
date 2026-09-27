//! Platform glue: winit event loop <-> egui-winit <-> renderer.
//! This is the only module that knows about all three (D2/D3).

mod ports;
mod snapshot;

pub use ports::{NullPort, ResizeDir, WindowPort};

use crate::app::App;
use crate::render::cpu::CpuRenderer;
use crate::Options;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

pub fn run(opts: Options) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(out) = opts.snapshot.clone() {
        return snapshot::run(opts, &out);
    }

    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut runner = Runner {
        app: None,
        ctx: egui::Context::default(),
        egui_state: None,
        window: None,
        surface: None,
        renderer: CpuRenderer::new(1, 1),
        repaint_at: None,
        opts,
    };
    event_loop.run_app(&mut runner)?;
    Ok(())
}

struct Runner {
    app: Option<App>,
    ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    renderer: CpuRenderer,
    repaint_at: Option<Instant>,
    opts: Options,
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, elwt: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("explorer")
            .with_inner_size(winit::dpi::LogicalSize::new(1152.0, 720.0))
            .with_decorations(false) // border module draws our frame (D5)
            .with_resizable(true)
            .with_min_inner_size(winit::dpi::LogicalSize::new(560.0, 360.0));
        let window = match elwt.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("failed to create window: {e}");
                elwt.exit();
                return;
            }
        };
        window.set_ime_allowed(true);

        let state = egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None, // system theme; the theme module owns colors
            None,
        );

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("softbuffer context failed: {e}");
                elwt.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("softbuffer surface failed: {e}");
                elwt.exit();
                return;
            }
        };

        let port = Box::new(ports::WinitPort { window: window.clone() });
        let mut app = App::new(port, &self.opts);
        app.bootstrap();
        let s = window.inner_size();
        app.set_window_size(s.width as f32, s.height as f32);

        self.egui_state = Some(state);
        self.app = Some(app);
        self.surface = Some(surface);
        self.window = Some(window);
        self.window().request_redraw();
    }

    fn window_event(&mut self, elwt: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.paint(elwt);
            return;
        }
        match &event {
            WindowEvent::CloseRequested => {
                elwt.exit();
                return;
            }
            WindowEvent::Resized(size) => {
                if let Some(app) = &mut self.app {
                    app.set_window_size(size.width as f32, size.height as f32);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(app), Some(window)) = (&mut self.app, &self.window) {
                    let s = window.inner_size();
                    app.set_window_size(s.width as f32, s.height as f32);
                }
            }
            _ => {}
        }

        let Some(window) = self.window.clone() else { return };
        let Some(state) = &mut self.egui_state else { return };
        let response = state.on_window_event(&window, &event);
        if response.repaint {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, elwt: &ActiveEventLoop) {
        match self.repaint_at {
            Some(at) if at > Instant::now() => elwt.set_control_flow(ControlFlow::WaitUntil(at)),
            _ => elwt.set_control_flow(ControlFlow::Poll),
        }
    }
}

impl Runner {
    fn window(&self) -> &Arc<Window> {
        self.window.as_ref().expect("window in runner")
    }

    fn paint(&mut self, elwt: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else { return };
        let Some(state) = &mut self.egui_state else { return };
        let Some(app) = &mut self.app else { return };

        let raw_input = state.take_egui_input(&window);
        let mut output = self.ctx.run_ui(raw_input, |ui| {
            app.frame(ui);
        });
        state.handle_platform_output(&window, output.platform_output);

        let clipped = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let size = window.inner_size();
        self.renderer.resize(size.width as usize, size.height as usize);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.textures.apply(*id, delta);
        }
        self.renderer.render(&clipped);

        if let Some(surface) = &mut self.surface {
            // wayland backend requires an explicit size before buffer_mut()
            if size.width > 0 && size.height > 0 {
                let _ = surface.resize(
                    std::num::NonZeroU32::new(size.width).unwrap(),
                    std::num::NonZeroU32::new(size.height).unwrap(),
                );
            }
            match surface.buffer_mut() {
                Ok(mut buffer) => {
                    let (bw, bh) = (buffer.width().get() as usize, buffer.height().get() as usize);
                    if (bw, bh) == (self.renderer.width, self.renderer.height) {
                        buffer.copy_from_slice(&self.renderer.buffer);
                    } else {
                        // surface is one frame behind a resize; paint neutral
                        for px in buffer.iter_mut() {
                            *px = 0x00FFFFFF;
                        }
                    }
                    window.pre_present_notify();
                    if let Err(e) = buffer.present() {
                        eprintln!("present failed: {e}");
                    }
                }
                Err(e) => eprintln!("buffer failed: {e}"),
            }
        }
        for id in &output.textures_delta.free {
            self.renderer.textures.free(*id);
        }
        // TexturesDelta panics in Drop if not fully consumed.
        output.textures_delta.clear();

        // schedule the next frame from egui's repaint hint (cursor blink etc.)
        let delay = output
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::from_millis(33))
            // cap the busy loop: ~90 fps is plenty for a file manager
            .clamp(Duration::from_millis(11), Duration::from_millis(1000));
        self.repaint_at = Some(Instant::now() + delay);

        if app.exit_requested() {
            elwt.exit();
        }
    }
}

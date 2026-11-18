//! Scene painting for the demo window, shared between the window itself
//! and the drag-icon surface (`paint_icon`).

use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect as SkRect, Stroke, Transform};

use crate::text::Text;

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < (self.x + self.w) as f64
            && y < (self.y + self.h) as f64
    }

    fn sk(&self) -> SkRect {
        SkRect::from_xywh(self.x, self.y, self.w, self.h).unwrap()
    }
}

/// The draggable file icon, logical coordinates.
pub const ICON: Rect = Rect { x: 48.0, y: 84.0, w: 104.0, h: 128.0 };

pub struct Scene<'a> {
    pub width: u32,  // logical
    pub height: u32, // logical
    pub dragging_out: bool,
    pub hover: bool,
    pub hover_mime: Option<&'a str>,
    pub received: Option<&'a (String, String)>, // (mime, body)
    pub status: &'a str,
}

pub fn paint(canvas: &mut [u8], phys_w: u32, phys_h: u32, scale: f32, scene: &Scene, text: &mut Text) {
    let mut pm = PixmapMut::from_bytes(canvas, phys_w, phys_h).unwrap();
    let ts = Transform::from_scale(scale, scale);
    let w = phys_w as usize;
    let h = phys_h as usize;
    macro_rules! txt {
        ($s:expr, $x:expr, $y:expr, $size:expr, $c:expr) => {
            text.draw(pm.data_mut(), w, h, $s, ($x) * scale, ($y) * scale, ($size) * scale, $c)
        };
    }

    // background + header
    fill(&mut pm, ts, rect(0.0, 0.0, scene.width as f32, scene.height as f32), rgb(0xEC, 0xEF, 0xF1));
    fill(&mut pm, ts, rect(0.0, 0.0, scene.width as f32, 52.0), rgb(0x24, 0x4A, 0x73));
    txt!("WAYLAND DRAG & DROP DEMO", 16.0, 10.0, 17.0, rgb(0xFF, 0xFF, 0xFF));
    txt!("drag the file out of this window — or drop a file onto it", 16.0, 30.0, 12.0, rgb(0xBF, 0xD5, 0xE8));

    // the file icon (a ghost while it is being dragged outside: the
    // compositor-owned drag icon is what follows the cursor)
    paint_icon(&mut pm, ts, ICON, if scene.dragging_out { 90 } else { 255 }, text, scale);
    let hint = if scene.dragging_out {
        "dragging… the icon you see is a compositor-owned surface"
    } else {
        "press and drag me out →"
    };
    txt!(hint, ICON.x - 6.0, ICON.y + ICON.h + 14.0, 11.0, rgb(0x51, 0x6A, 0x7A));

    // drop zone
    let zone = Rect {
        x: 48.0,
        y: scene.height as f32 - 158.0,
        w: scene.width as f32 - 96.0,
        h: 96.0,
    };
    if scene.hover {
        fill(&mut pm, ts, zone.sk(), rgb(0xC5, 0xDD, 0xF2));
    }
    stroke(&mut pm, ts, zone.sk(), rgb(0x35, 0x71, 0xA4), 2.0);
    let zone_label = if scene.hover { "RELEASE TO DROP" } else { "DROP A FILE HERE" };
    txt!(zone_label, zone.x + 14.0, zone.y + 12.0, 13.0, rgb(0x35, 0x71, 0xA4));
    match scene.received {
        Some((mime, body)) => {
            let shown: String = body.chars().take(64).collect();
            txt!(&format!("{mime}: {shown}"), zone.x + 14.0, zone.y + 40.0, 12.0, rgb(0x21, 0x2A, 0x30));
        }
        None => {
            if let Some(mime) = scene.hover_mime {
                txt!(&format!("incoming: {mime}"), zone.x + 14.0, zone.y + 40.0, 12.0, rgb(0x21, 0x2A, 0x30));
            }
        }
    }

    // status bar
    fill(&mut pm, ts, rect(0.0, scene.height as f32 - 28.0, scene.width as f32, 28.0), rgb(0x26, 0x32, 0x38));
    txt!(scene.status, 12.0, scene.height as f32 - 19.0, 12.0, rgb(0xCF, 0xD8, 0xDC));
}

/// The file glyph: white sheet, folded corner, text lines, "A.TXT" label.
/// Used both in the window and (centered, with padding) as the drag icon.
pub fn paint_icon(pm: &mut PixmapMut, ts: Transform, r: Rect, alpha: u8, text: &mut Text, scale: f32) {
    let w = pm.width() as usize;
    let h = pm.height() as usize;

    // drop shadow + sheet
    fill_a(pm, ts, rect(r.x + 3.0, r.y + 4.0, r.w, r.h), rgb(0x00, 0x00, 0x00), 60 * alpha / 255);
    fill_a(pm, ts, r.sk(), rgb(0xFF, 0xFF, 0xFF), alpha);
    stroke_a(pm, ts, r.sk(), [0x78, 0x90, 0x9C, alpha], 1.5);

    // folded corner (top-right)
    let fold = 18.0;
    let mut pb = PathBuilder::new();
    pb.move_to(r.x + r.w - fold, r.y);
    pb.line_to(r.x + r.w, r.y + fold);
    pb.line_to(r.x + r.w - fold, r.y + fold);
    pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(0xCF, 0xD8, 0xDC, alpha));
    pm.fill_path(&path, &paint, Default::default(), ts, None);

    // fake text lines
    let mut line_y = r.y + 26.0;
    for frac in [0.72f32, 0.78, 0.6] {
        fill_a(pm, ts, rect(r.x + 12.0, line_y, (r.w - 24.0) * frac, 5.0), rgb(0xB0, 0xBE, 0xC5), alpha);
        line_y += 12.0;
    }

    // label
    text.draw(
        pm.data_mut(),
        w,
        h,
        "A.TXT",
        (r.x + 14.0) * scale,
        (r.y + r.h - 26.0) * scale,
        14.0 * scale,
        [0x37, 0x47, 0x4F, alpha],
    );
}

// --- tiny helpers ---------------------------------------------------------

fn rgb(r: u8, g: u8, b: u8) -> [u8; 4] {
    [r, g, b, 255]
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> SkRect {
    SkRect::from_xywh(x, y, w, h).unwrap()
}

 fn mk_paint(color: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
    p.anti_alias = true;
    p
}

fn fill(pm: &mut PixmapMut, ts: Transform, r: SkRect, color: [u8; 4]) {
    pm.fill_rect(r, &mk_paint(color), ts, None);
}

fn fill_a(pm: &mut PixmapMut, ts: Transform, r: SkRect, color: [u8; 4], alpha: u8) {
    pm.fill_rect(r, &mk_paint([color[0], color[1], color[2], alpha]), ts, None);
}

fn stroke(pm: &mut PixmapMut, ts: Transform, r: SkRect, color: [u8; 4], width: f32) {
    stroke_a(pm, ts, r, color, width);
}

fn stroke_a(pm: &mut PixmapMut, ts: Transform, r: SkRect, color: [u8; 4], width: f32) {
    let stroke = Stroke { width, ..Default::default() };
    let path = PathBuilder::from_rect(r);
    pm.stroke_path(&path, &mk_paint(color), &stroke, ts, None);
}

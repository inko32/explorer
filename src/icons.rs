//! Placeholder vector icons behind a single API (DECISION D7).
//!
//! Every icon in the app is one of these; swap this file's implementation for
//! an SVG atlas or system-theme lookup later without touching UI modules.

use crate::theme::Theme;
use egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // icon vocabulary grows with features
pub enum Icon {
    // nav pane / items
    Folder,
    File,
    FileImage,
    Computer,
    Drive,
    Network,
    Star,
    StarOutline,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Recent,
    // navigation buttons
    Back,
    Forward,
    Up,
    Refresh,
    Search,
    ChevronDown,
    ChevronRight,
    // ribbon commands
    Copy,
    Paste,
    Cut,
    Delete,
    Rename,
    NewFolder,
    Properties,
    Open,
    SelectAll,
    SelectNone,
    InvertSel,
    History,
    Edit,
    MoveTo,
    CopyTo,
    EasyAccess,
    NewItem,
    CopyPath,
    PasteShortcut,
    // window
    Minimize,
    Maximize,
    Restore,
    Close,
    Help,
    // view
    ViewDetails,
    ViewLarge,
    Gear,
}

type ColorFn<'a> = dyn Fn(Color32) -> Color32 + 'a;

pub fn draw(painter: &Painter, icon: Icon, rect: Rect, theme: &Theme, enabled: bool) {
    let c: Box<ColorFn> = if enabled {
        Box::new(|col| col)
    } else {
        let d = theme.palette.text_disabled;
        Box::new(move |col: Color32| {
            let g = ((col.r() as u32 + col.g() as u32 + col.b() as u32) / 3) as u8;
            Color32::from_rgb(
                ((g as u32 + d.r() as u32) / 2) as u8,
                ((g as u32 + d.g() as u32) / 2) as u8,
                ((g as u32 + d.b() as u32) / 2) as u8,
            )
        })
    };
    let p = &theme.palette;
    let stroke = Stroke::new((rect.width().max(2.0) * 0.06).min(3.0), c(p.icon_glyph));
    match icon {
        Icon::Folder => folder(painter, rect, c(p.icon_folder), c(p.icon_folder_edge)),
        Icon::NewFolder => {
            folder(
                painter,
                rect.shrink2(Vec2::new(rect.width() * 0.14, rect.height() * 0.18)),
                c(p.icon_folder),
                c(p.icon_folder_edge),
            );
            let s = rect.width() * 0.32;
            let center = rect.right_bottom() - Vec2::new(s * 0.62, s * 0.52);
            plus(painter, Rect::from_center_size(center, Vec2::splat(s)), c(p.icon_glyph), stroke.width * 1.4);
        }
        Icon::File | Icon::FileImage => {
            let fold = rect.width() * 0.28;
            let pts = vec![
                rect.left_top() + Vec2::new(0.0, fold),
                rect.left_top() + Vec2::new(rect.width() - fold, fold),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ];
            painter.add(Shape::convex_polygon(pts, c(p.icon_file), stroke));
            painter.line_segment(
                [
                    rect.left_top() + Vec2::new(rect.width() - fold, 0.0),
                    rect.left_top() + Vec2::new(rect.width() - fold, fold),
                ],
                stroke,
            );
            painter.line_segment([rect.left_top() + Vec2::new(rect.width() - fold, fold), rect.right_top()], stroke);
            if icon == Icon::FileImage {
                let inner = rect.shrink(rect.width() * 0.22);
                painter.line_segment(
                    [inner.left_bottom() - Vec2::new(0.0, 2.0), inner.center() - Vec2::new(0.0, 2.0)],
                    stroke,
                );
                painter.line_segment([inner.center() - Vec2::new(0.0, 2.0), inner.center_bottom()], stroke);
                painter.circle_filled(inner.center_top() + Vec2::new(0.0, 2.0), stroke.width, c(p.icon_glyph));
            }
        }
        Icon::Computer | Icon::Desktop => {
            let screen = Rect::from_min_max(
                rect.left_top() + Vec2::new(0.0, rect.height() * 0.08),
                rect.right_bottom() - Vec2::new(0.0, rect.height() * 0.32),
            );
            painter.rect_filled(screen, 1.0, c(p.icon_file));
            painter.rect_stroke(screen, 1.0, stroke, egui::StrokeKind::Inside);
            let stand_w = rect.width() * 0.5;
            let stand = Rect::from_min_size(
                Pos2::new(rect.center().x - stand_w / 2.0, rect.bottom() - rect.height() * 0.26),
                Vec2::new(stand_w, rect.height() * 0.14),
            );
            painter.rect_filled(stand, 1.0, c(p.icon_file));
            painter.rect_stroke(stand, 1.0, stroke, egui::StrokeKind::Inside);
            let base = Rect::from_min_size(
                Pos2::new(rect.center().x - rect.width() * 0.38, rect.bottom() - rect.height() * 0.09),
                Vec2::new(rect.width() * 0.76, rect.height() * 0.06),
            );
            painter.rect_filled(base, 1.0, c(p.icon_file));
            painter.rect_stroke(base, 1.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::Drive => {
            let body = Rect::from_min_max(
                rect.left_top() + Vec2::new(0.0, rect.height() * 0.2),
                rect.right_bottom() - Vec2::new(0.0, rect.height() * 0.25),
            );
            painter.rect_filled(body, 2.0, c(p.icon_file));
            painter.rect_stroke(body, 2.0, stroke, egui::StrokeKind::Inside);
            let led_pos = body.right_bottom() - Vec2::new(rect.width() * 0.16, rect.height() * 0.13);
            painter.circle_filled(led_pos, stroke.width * 1.2, c(Color32::from_rgb(0x2E, 0xB8, 0x2E)));
        }
        Icon::Network => {
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.0;
                    let x = rect.left() + t * rect.width();
                    let y = rect.top() + rect.height() * 0.35 - (t * std::f32::consts::PI).sin() * rect.height() * 0.3;
                    Pos2::new(x, y)
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
            for (dx, dy) in [(0.08, 0.28), (0.5, 0.05), (0.92, 0.28)] {
                let a = Pos2::new(rect.left() + rect.width() * dx, rect.top() + rect.height() * dy);
                painter.circle_filled(a, stroke.width * 1.5, c(p.icon_file));
                painter.circle_stroke(a, stroke.width * 1.5, stroke);
            }
        }
        Icon::Star | Icon::StarOutline => {
            let cx = rect.center().x;
            let cy = rect.center().y;
            let ro = rect.width() * 0.48;
            let ri = ro * 0.45;
            let mut pts = Vec::new();
            for i in 0..10 {
                let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { ro } else { ri };
                pts.push(Pos2::new(cx + r * ang.cos(), cy + r * ang.sin()));
            }
            if icon == Icon::Star {
                painter.add(Shape::convex_polygon(pts, c(Color32::from_rgb(0xFF, 0xB9, 0x00)), Stroke::NONE));
            } else {
                painter.add(Shape::closed_line(pts, stroke));
            }
        }
        Icon::Home => {
            let roof = Shape::convex_polygon(
                vec![
                    rect.left_top() + Vec2::new(0.0, rect.height() * 0.42),
                    rect.center_top(),
                    rect.right_top() + Vec2::new(0.0, rect.height() * 0.42),
                ],
                c(Color32::from_rgb(0xC0, 0x50, 0x46)),
                Stroke::NONE,
            );
            painter.add(roof);
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + rect.width() * 0.16, rect.top() + rect.height() * 0.42),
                rect.right_bottom() - Vec2::new(rect.width() * 0.16, 0.0),
            );
            painter.rect_filled(body, 1.0, c(Color32::from_rgb(0xE8, 0xE4, 0xD8)));
            painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Inside);
            let door = Rect::from_min_size(
                Pos2::new(rect.center().x - rect.width() * 0.08, rect.top() + rect.height() * 0.6),
                Vec2::new(rect.width() * 0.16, rect.height() * 0.4),
            );
            painter.rect_filled(door, 0.0, c(p.icon_glyph));
        }
        Icon::Documents => {
            let doc = Rect::from_min_size(
                rect.left_top() + Vec2::new(rect.width() * 0.1, rect.height() * 0.08),
                Vec2::new(rect.width() * 0.6, rect.height() * 0.84),
            );
            painter.rect_filled(doc, 1.0, c(p.icon_file));
            painter.rect_stroke(doc, 1.0, stroke, egui::StrokeKind::Inside);
            for i in 0..3 {
                let y = doc.top() + doc.height() * (0.25 + i as f32 * 0.2);
                painter.line_segment(
                    [Pos2::new(doc.left() + 3.0, y), Pos2::new(doc.right() - 3.0, y)],
                    Stroke::new(stroke.width * 0.8, c(p.icon_glyph)),
                );
            }
        }
        Icon::Downloads => {
            let tray = rect.shrink2(Vec2::new(rect.width() * 0.06, rect.height() * 0.28));
            painter.rect_filled(tray, 2.0, c(p.icon_folder));
            painter.rect_stroke(tray, 2.0, stroke, egui::StrokeKind::Inside);
            arrow(
                painter,
                Pos2::new(rect.center().x, rect.top() + rect.height() * 0.12),
                rect.height() * 0.5,
                1.0,
                c(p.icon_glyph),
                stroke.width,
            );
        }
        Icon::Pictures => {
            let frame = rect.shrink(rect.width() * 0.08);
            painter.rect_filled(frame, 1.0, c(p.icon_file));
            painter.rect_stroke(frame, 1.0, stroke, egui::StrokeKind::Inside);
            let sun = frame.center() + Vec2::new(frame.width() * 0.18, -frame.height() * 0.18);
            painter.circle_filled(sun, stroke.width * 1.6, c(p.icon_glyph));
            painter.line_segment([frame.left_bottom() + Vec2::new(3.0, -2.0), frame.center() - Vec2::new(0.0, 1.0)], stroke);
            painter.line_segment([frame.center() - Vec2::new(0.0, 1.0), frame.right_bottom() - Vec2::new(3.0, 1.0)], stroke);
        }
        Icon::Music => {
            let head = rect.left_top() + Vec2::new(rect.width() * 0.42, rect.height() * 0.1);
            painter.circle_filled(head + Vec2::new(0.0, rect.height() * 0.62), rect.width() * 0.14, c(p.icon_glyph));
            painter.line_segment([head, head + Vec2::new(0.0, rect.height() * 0.62)], stroke);
            painter.line_segment([head, head + Vec2::new(rect.width() * 0.4, -rect.height() * 0.08)], stroke);
            painter.line_segment(
                [
                    head + Vec2::new(rect.width() * 0.4, -rect.height() * 0.08),
                    head + Vec2::new(rect.width() * 0.4, rect.height() * 0.55),
                ],
                stroke,
            );
            painter.circle_filled(head + Vec2::new(rect.width() * 0.4, rect.height() * 0.58), rect.width() * 0.14, c(p.icon_glyph));
        }
        Icon::Videos => {
            let body = rect.shrink2(Vec2::new(rect.width() * 0.24, rect.height() * 0.22));
            painter.rect_filled(body, 2.0, c(p.icon_file));
            painter.rect_stroke(body, 2.0, stroke, egui::StrokeKind::Inside);
            painter.add(Shape::convex_polygon(
                vec![
                    body.center() + Vec2::new(-body.width() * 0.12, -body.height() * 0.18),
                    body.center() + Vec2::new(-body.width() * 0.12, body.height() * 0.18),
                    body.center() + Vec2::new(body.width() * 0.18, 0.0),
                ],
                c(p.icon_glyph),
                Stroke::NONE,
            ));
        }
        Icon::Recent | Icon::History => {
            painter.circle_stroke(rect.center(), rect.width() * 0.42, stroke);
            painter.line_segment([rect.center(), rect.center_top() + Vec2::new(0.0, rect.height() * 0.1)], stroke);
            painter.line_segment([rect.center(), rect.center() + Vec2::new(rect.width() * 0.24, rect.height() * 0.12)], stroke);
        }
        Icon::Back | Icon::Forward => {
            let dir = if icon == Icon::Back { 1.0f32 } else { -1.0f32 };
            let tip = rect.center() + Vec2::new(dir * rect.width() * 0.34, 0.0);
            let a = rect.center() - Vec2::new(dir * rect.width() * 0.3, 0.0);
            let perp = Vec2::new(0.0, rect.height() * 0.3);
            painter.add(Shape::convex_polygon(
                vec![tip, a + perp, a + Vec2::new(dir * rect.width() * 0.12, 0.0) + perp * 0.55, a + Vec2::new(dir * rect.width() * 0.12, 0.0) - perp * 0.55, a - perp],
                c(p.icon_glyph),
                Stroke::NONE,
            ));
        }
        Icon::Up => {
            let tip = rect.center_top() + Vec2::new(0.0, rect.height() * 0.12);
            let a = rect.center_bottom() - Vec2::new(0.0, rect.height() * 0.14);
            let half = Vec2::new(rect.width() * 0.32, 0.0);
            painter.add(Shape::convex_polygon(
                vec![tip, a + half, a + Vec2::new(half.x * 0.45, 0.0), a - Vec2::new(half.x * 0.45, 0.0), a - half],
                c(p.icon_glyph),
                Stroke::NONE,
            ));
        }
        Icon::Refresh => {
            let center = rect.center();
            let r = rect.width() * 0.36;
            let start = -std::f32::consts::FRAC_PI_2 + 0.5;
            let end = start + std::f32::consts::TAU - 1.1;
            let mid = (start + end) / 2.0;
            let k = 4.0 / 3.0 * ((end - start) / 4.0).tan();
            let pt = |a: f32| center + Vec2::new(r * a.cos(), r * a.sin());
            let dd = |a: f32| Vec2::new(-a.sin(), a.cos());
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [pt(start), pt(start) + dd(start) * (r * k), pt(mid) - dd(mid) * (r * k), pt(mid)],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
            let tang = end + std::f32::consts::FRAC_PI_2;
            let td = Vec2::new(tang.cos(), tang.sin()) * r * 0.4;
            let tip = pt(end);
            painter.add(Shape::convex_polygon(
                vec![tip + td, tip - td * 0.4 + Vec2::new(-td.y, td.x) * 0.6, tip - td * 0.4 - Vec2::new(-td.y, td.x) * 0.6],
                c(p.icon_glyph),
                Stroke::NONE,
            ));
        }
        Icon::Search => {
            let r = rect.width() * 0.3;
            let cpos = rect.left_top() + Vec2::new(rect.width() * 0.42, rect.height() * 0.42);
            painter.circle_stroke(cpos, r, stroke);
            let dir = Vec2::new(0.7, 0.7).normalized() * r * 0.9;
            painter.line_segment(
                [cpos + dir, cpos + dir * 1.7],
                Stroke::new(stroke.width * 1.5, c(p.icon_glyph)),
            );
        }
        Icon::ChevronDown | Icon::ChevronRight => {
            let dir = if icon == Icon::ChevronDown { Vec2::new(0.0, 1.0) } else { Vec2::new(1.0, 0.0) };
            let tip = rect.center() + dir * rect.width() * 0.34;
            let base = rect.center() - dir * rect.width() * 0.34;
            let perp = Vec2::new(-dir.y, dir.x) * rect.width() * 0.3;
            painter.add(Shape::convex_polygon(vec![tip, base + perp, base - perp], c(p.icon_glyph), Stroke::NONE));
        }
        Icon::Copy | Icon::CopyTo | Icon::MoveTo => {
            let back = rect
                .shrink2(Vec2::new(rect.width() * 0.3, rect.height() * 0.12))
                .translate(Vec2::new(-rect.width() * 0.14, 0.0));
            painter.rect_filled(back, 1.0, c(p.icon_file));
            painter.rect_stroke(back, 1.0, stroke, egui::StrokeKind::Inside);
            let front = rect
                .shrink2(Vec2::new(rect.width() * 0.3, rect.height() * 0.12))
                .translate(Vec2::new(rect.width() * 0.14, rect.height() * 0.1));
            painter.rect_filled(front, 1.0, c(p.icon_file));
            painter.rect_stroke(front, 1.0, stroke, egui::StrokeKind::Inside);
            if icon != Icon::Copy {
                let d = if icon == Icon::MoveTo { Vec2::new(1.0, 0.0) } else { Vec2::new(0.0, -1.0) };
                let tip = front.center() + d * front.height() * 0.4;
                let base = front.center() - d * front.height() * 0.4;
                let perp = Vec2::new(-d.y, d.x) * front.height() * 0.3;
                painter.add(Shape::convex_polygon(vec![tip, base + perp, base - perp], c(p.icon_glyph), Stroke::NONE));
            }
        }
        Icon::Paste | Icon::PasteShortcut => {
            let board = rect.shrink2(Vec2::new(rect.width() * 0.16, rect.height() * 0.2));
            painter.rect_filled(board, 1.0, c(Color32::from_rgb(0xE3, 0xD9, 0xB9)));
            painter.rect_stroke(board, 1.0, stroke, egui::StrokeKind::Inside);
            let clip = Rect::from_min_size(
                Pos2::new(board.center().x - rect.width() * 0.14, board.top() - rect.height() * 0.06),
                Vec2::new(rect.width() * 0.28, rect.height() * 0.13),
            );
            painter.rect_filled(clip, 1.0, c(p.icon_file));
            painter.rect_stroke(clip, 1.0, stroke, egui::StrokeKind::Inside);
            if icon == Icon::PasteShortcut {
                let s = rect.width() * 0.4;
                let r2 = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, rect.height() - s), Vec2::splat(s));
                painter.circle_stroke(r2.center(), s * 0.4, stroke);
            }
        }
        Icon::Cut => {
            let a = rect.left_top() + Vec2::new(rect.width() * 0.15, rect.height() * 0.12);
            let b = rect.left_top() + Vec2::new(rect.width() * 0.85, rect.height() * 0.12);
            let cpos = rect.center_bottom() - Vec2::new(0.0, rect.height() * 0.1);
            painter.line_segment([a, cpos], stroke);
            painter.line_segment([b, cpos], stroke);
            painter.circle_stroke(a, stroke.width * 1.6, stroke);
            painter.circle_stroke(b, stroke.width * 1.6, stroke);
        }
        Icon::Delete => {
            let cx = rect.center().x;
            let top = rect.top() + rect.height() * 0.18;
            let bottom = rect.bottom() - rect.height() * 0.14;
            let red = Stroke::new(stroke.width * 1.3, c(Color32::from_rgb(0xD0, 0x30, 0x30)));
            painter.line_segment(
                [Pos2::new(rect.left() + rect.width() * 0.2, top), Pos2::new(rect.right() - rect.width() * 0.2, top)],
                red,
            );
            painter.line_segment(
                [Pos2::new(cx, rect.top() + rect.height() * 0.08), Pos2::new(cx, top)],
                red,
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + rect.width() * 0.28, top),
                Pos2::new(rect.right() - rect.width() * 0.28, bottom),
            );
            painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Inside);
            for i in [-0.5f32, 0.0, 0.5] {
                let x = cx + body.width() * i * 0.5;
                painter.line_segment(
                    [Pos2::new(x, top + 3.0), Pos2::new(x, bottom - 3.0)],
                    Stroke::new(stroke.width * 0.8, c(p.icon_glyph)),
                );
            }
        }
        Icon::Rename => {
            let tag = rect.shrink2(Vec2::new(rect.width() * 0.14, rect.height() * 0.24));
            painter.rect_stroke(tag, 2.0, stroke, egui::StrokeKind::Inside);
            let hole_c = tag.center() - Vec2::new(tag.width() * 0.25, 0.0);
            painter.circle_filled(hole_c, stroke.width * 1.3, c(p.window_bg));
            painter.circle_stroke(hole_c, stroke.width * 1.3, stroke);
        }
        Icon::Properties => {
            painter.add(Shape::line(
                vec![
                    rect.left_top() + Vec2::new(rect.width() * 0.15, rect.height() * 0.55),
                    rect.left_top() + Vec2::new(rect.width() * 0.42, rect.height() * 0.82),
                    rect.left_top() + Vec2::new(rect.width() * 0.88, rect.height() * 0.2),
                ],
                Stroke::new(stroke.width * 1.8, c(Color32::from_rgb(0x1F, 0x9E, 0x3E))),
            ));
        }
        Icon::Open => {
            let body = rect.shrink2(Vec2::new(rect.width() * 0.12, rect.height() * 0.2));
            painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Inside);
            arrow(
                painter,
                Pos2::new(body.center().x, body.bottom() - 2.0),
                body.height() * 0.6,
                -1.0,
                c(p.icon_glyph),
                stroke.width,
            );
        }
        Icon::SelectAll => {
            let r = rect.shrink2(Vec2::new(rect.width() * 0.12, rect.height() *0.12));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            painter.add(Shape::line(
                vec![
                    r.left_top() + Vec2::new(r.width() * 0.15, r.height() * 0.5),
                    r.left_top() + Vec2::new(r.width() * 0.4, r.height() * 0.75),
                    r.left_top() + Vec2::new(r.width() * 0.85, r.height() * 0.2),
                ],
                Stroke::new(stroke.width * 1.5, c(p.icon_glyph)),
            ));
        }
        Icon::SelectNone => {
            let r = rect.shrink2(Vec2::new(rect.width() * 0.12, rect.height() * 0.12));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::InvertSel => {
            let r = rect.shrink2(Vec2::new(rect.width() * 0.12, rect.height() * 0.12));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            painter.rect_filled(r.shrink2(Vec2::new(r.width() * 0.25, r.height() * 0.25)), 0.0, c(p.icon_glyph));
        }
        Icon::Edit => {
            let pen_tip = rect.center_bottom() - Vec2::new(rect.width() * 0.15, rect.height() * 0.1);
            painter.add(Shape::convex_polygon(
                vec![
                    pen_tip,
                    rect.left_top() + Vec2::new(rect.width() * 0.72, rect.height() * 0.12),
                    rect.left_top() + Vec2::new(rect.width() * 0.88, rect.height() * 0.28),
                    pen_tip + Vec2::new(rect.width() * 0.14, 0.0),
                ],
                c(p.icon_glyph),
                Stroke::NONE,
            ));
        }
        Icon::EasyAccess => {
            let r = rect.shrink(rect.width() * 0.1);
            painter.circle_stroke(r.center(), r.width() * 0.42, stroke);
            painter.line_segment([r.center() - Vec2::new(r.width() * 0.2, 0.0), r.center() + Vec2::new(r.width() * 0.2, 0.0)], stroke);
            painter.line_segment([r.center() - Vec2::new(0.0, r.height() * 0.2), r.center() + Vec2::new(0.0, r.height() * 0.2)], stroke);
        }
        Icon::NewItem => {
            let r = rect.shrink2(Vec2::new(rect.width() * 0.3, rect.height() * 0.1));
            painter.rect_filled(r, 1.0, c(p.icon_file));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            plus(painter, rect.shrink(rect.width() * 0.34), c(p.icon_glyph), stroke.width * 1.5);
        }
        Icon::CopyPath => {
            let r = rect.shrink2(Vec2::new(rect.width() * 0.1, rect.height() * 0.28));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment([r.left_center() + Vec2::new(2.0, 0.0), r.right_center() - Vec2::new(2.0, 0.0)], stroke);
            painter.circle_filled(r.left_center(), stroke.width, c(p.icon_glyph));
            painter.circle_filled(r.right_center(), stroke.width, c(p.icon_glyph));
        }
        Icon::Minimize => {
            let y = rect.center().y;
            painter.line_segment(
                [
                    Pos2::new(rect.left() + rect.width() * 0.22, y),
                    Pos2::new(rect.right() - rect.width() * 0.22, y),
                ],
                Stroke::new(stroke.width * 1.2, c(p.icon_glyph)),
            );
        }
        Icon::Maximize => {
            painter.rect_stroke(rect.shrink(rect.width() * 0.24), 0.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::Restore => {
            let back = rect
                .shrink2(Vec2::new(rect.width() * 0.34, rect.height() * 0.14))
                .translate(Vec2::new(-rect.width() * 0.06, -rect.height() * 0.08));
            painter.rect_stroke(back, 0.0, stroke, egui::StrokeKind::Inside);
            let front = rect
                .shrink2(Vec2::new(rect.width() * 0.34, rect.height() * 0.14))
                .translate(Vec2::new(rect.width() * 0.1, rect.height() * 0.14));
            painter.rect_filled(front, 0.0, c(p.window_bg));
            painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::Close => {
            let r = rect.shrink(rect.width() * 0.28);
            let w = Stroke::new(stroke.width * 1.2, c(p.icon_glyph));
            painter.line_segment([r.left_top(), r.right_bottom()], w);
            painter.line_segment([r.right_top(), r.left_bottom()], w);
        }
        Icon::Help => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(rect.height() * 0.8),
                c(p.icon_glyph),
            );
        }
        Icon::ViewDetails => {
            for i in 0..3 {
                let y = rect.top() + rect.height() * (0.18 + i as f32 * 0.3);
                painter.circle_filled(Pos2::new(rect.left() + rect.width() * 0.2, y), stroke.width * 1.1, c(p.icon_glyph));
                painter.line_segment(
                    [Pos2::new(rect.left() + rect.width() * 0.42, y), Pos2::new(rect.right() - rect.width() * 0.1, y)],
                    stroke,
                );
            }
        }
        Icon::ViewLarge => {
            let r = rect.shrink(rect.width() * 0.2);
            painter.rect_filled(r, 1.0, c(p.icon_folder));
            painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::Gear => {
            let cx = rect.center().x;
            let cy = rect.center().y;
            let ro = rect.width() * 0.42;
            let ri = ro * 0.72;
            let mut pts = Vec::new();
            for i in 0..16 {
                let ang = i as f32 * std::f32::consts::TAU / 16.0;
                let r = if i % 2 == 0 { ro } else { ri };
                pts.push(Pos2::new(cx + r * ang.cos(), cy + r * ang.sin()));
            }
            painter.add(PathShape::convex_polygon(pts, c(p.icon_glyph), Stroke::NONE));
            painter.circle_filled(rect.center(), ro * 0.34, c(p.window_bg));
        }
    }
}

fn folder(painter: &Painter, rect: Rect, fill: Color32, edge: Color32) {
    let tab_h = rect.height() * 0.22;
    let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + tab_h * 0.6), rect.right_bottom());
    let stroke = Stroke::new((rect.width() * 0.06).max(1.0), edge);
    painter.rect_filled(body, 1.0, fill);
    let tab = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width() * 0.45, tab_h));
    painter.rect_filled(tab, 1.0, fill);
    let pts = vec![
        body.left_top() + Vec2::new(0.0, -tab_h * 0.4),
        body.left_top() + Vec2::new(rect.width() * 0.45, -tab_h * 0.4),
        body.left_top() + Vec2::new(rect.width() * 0.45, 0.0),
        body.right_top(),
        body.right_bottom(),
        body.left_bottom(),
        body.left_top() + Vec2::new(0.0, -tab_h * 0.4),
    ];
    painter.add(Shape::line(pts, stroke));
}

fn plus(painter: &Painter, rect: Rect, color: Color32, width: f32) {
    let c = rect.center();
    let w = Stroke::new(width, color);
    painter.line_segment([Pos2::new(rect.left() + 1.0, c.y), Pos2::new(rect.right() - 1.0, c.y)], w);
    painter.line_segment([Pos2::new(c.x, rect.top() + 1.0), Pos2::new(c.x, rect.bottom() - 1.0)], w);
}

fn arrow(painter: &Painter, tip: Pos2, len: f32, dir_y: f32, color: Color32, width: f32) {
    let shaft_top = Pos2::new(tip.x, tip.y - dir_y * len);
    let w = Stroke::new(width, color);
    painter.line_segment([shaft_top, tip], w);
    let head = Vec2::new(len * 0.28, 0.0);
    let perp = Vec2::new(0.0, len * 0.24) * dir_y;
    painter.add(Shape::convex_polygon(
        vec![tip, tip - perp + head, tip - perp - head],
        color,
        Stroke::NONE,
    ));
}

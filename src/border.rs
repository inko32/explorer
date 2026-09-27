//! Self-rendered window frame (D5): title bar, caption buttons, resize
//! hit-testing. Shadow remains the compositor's job.

use crate::app::{Cmd, Cx};
use crate::icons::{self, Icon};
use crate::platform::{ResizeDir, WindowPort};
use crate::theme::Theme;
use egui::{Align2, Color32, CursorIcon, Id, LayerId, Order, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

pub fn titlebar(ctx: &egui::Context, cx: &Cx, pending: &mut Vec<Cmd>) {
    let m = &cx.theme.metrics;
    egui::TopBottomPanel::top("titlebar")
        .exact_height(m.title_h)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let p = &cx.theme.palette;
            let bg = if cx.focused { p.accent } else { p.titlebar_inactive };
            let text = if cx.focused { Color32::WHITE } else { p.titlebar_text_inactive };
            ui.painter().rect_filled(rect, 0.0, bg);

            // window icon
            let icon_rect = Rect::from_min_size(
                Pos2::new(rect.left() + 6.0, rect.center().y - 8.0),
                Vec2::splat(16.0),
            );
            icons::draw(ui.painter(), Icon::Folder, icon_rect, cx.theme, true);

            // title, centered over the whole window
            ui.painter().text(
                Pos2::new(cx.window.0 / 2.0, rect.center().y),
                Align2::CENTER_CENTER,
                cx.cwd_label,
                egui::FontId::proportional(cx.theme.fonts.title),
                text,
            );

            // caption buttons
            let btn_h = rect.height();
            let close = caption_btn(ui, rect.right_top() - Vec2::new(m.caption_btn_w, 0.0), Vec2::new(m.caption_btn_w, btn_h), "caption_close", cx);
            let max = caption_btn(
                ui,
                rect.right_top() - Vec2::new(m.caption_btn_w * 2.0, 0.0),
                Vec2::new(m.caption_btn_w, btn_h),
                "caption_max",
                cx,
            );
            let min = caption_btn(
                ui,
                rect.right_top() - Vec2::new(m.caption_btn_w * 3.0, 0.0),
                Vec2::new(m.caption_btn_w, btn_h),
                "caption_min",
                cx,
            );

            if close.clicked() {
                pending.push(Cmd::Exit);
            }
            if max.clicked() {
                pending.push(Cmd::ToggleMaximize);
            }
            if min.clicked() {
                pending.push(Cmd::Minimize);
            }

            // drag + double-click on the remaining title area
            let drag_rect = Rect::from_min_max(
                Pos2::new(rect.left() + 26.0, rect.top()),
                Pos2::new(rect.right() - m.caption_btn_w * 3.0, rect.bottom()),
            );
            let drag = ui.interact(drag_rect, Id::new("titlebar_drag"), Sense::click_and_drag());
            if drag.double_clicked() {
                pending.push(Cmd::ToggleMaximize);
            } else if drag.drag_started() {
                pending.push(Cmd::DragMove);
            }
        });
}

fn caption_btn(ui: &mut Ui, pos: Pos2, size: Vec2, id: &str, cx: &Cx) -> Response {
    let rect = Rect::from_min_size(pos, size);
    let p = &cx.theme.palette;
    let response = ui.interact(rect, Id::new(id), Sense::click());
    let is_close = id == "caption_close";
    let hovered = response.hovered();
    let pressed = response.is_pointer_button_down_on();
    if hovered || pressed {
        let bg = if is_close {
            if pressed { p.close_pressed } else { p.close_hover }
        } else if pressed {
            p.caption_pressed
        } else {
            p.caption_hover
        };
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    let icon = match id {
        "caption_close" => Icon::Close,
        "caption_max" => {
            if cx.maximized {
                Icon::Restore
            } else {
                Icon::Maximize
            }
        }
        _ => Icon::Minimize,
    };
    let pad = 15.0;
    let icon_rect = rect.shrink2(Vec2::new((rect.width() - pad) / 2.0, (rect.height() - pad) / 2.0));
    let white = Color32::WHITE;
    // draw glyph directly in white (icons::draw would grey it out on unfocused)
    let painter = ui.painter();
    match icon {
        Icon::Minimize => {
            painter.line_segment(
                [
                    Pos2::new(icon_rect.left(), icon_rect.center().y),
                    Pos2::new(icon_rect.right(), icon_rect.center().y),
                ],
                Stroke::new(1.2, white),
            );
        }
        Icon::Maximize => {
            painter.rect_stroke(icon_rect.shrink(1.0), 0.0, Stroke::new(1.2, white), egui::StrokeKind::Inside);
        }
        Icon::Restore => {
            let back = icon_rect.shrink2(Vec2::new(3.0, 1.0)).translate(Vec2::new(-2.0, -1.5));
            painter.rect_stroke(back, 0.0, Stroke::new(1.0, white), egui::StrokeKind::Inside);
            let front = icon_rect.shrink2(Vec2::new(3.0, 1.0)).translate(Vec2::new(2.0, 2.0));
            painter.rect_filled(front, 0.0, if response.hovered() { p.close_hover } else { p.accent });
            painter.rect_filled(front, 0.0, p.window_bg);
            painter.rect_stroke(front, 0.0, Stroke::new(1.2, white), egui::StrokeKind::Inside);
        }
        Icon::Close => {
            painter.line_segment([icon_rect.left_top(), icon_rect.right_bottom()], Stroke::new(1.3, white));
            painter.line_segment([icon_rect.right_top(), icon_rect.left_bottom()], Stroke::new(1.3, white));
        }
        _ => {}
    }
    response
}

/// Invisible resize ring around the window edge; sets the cursor and starts a
/// compositor resize on press.
pub fn edge_resize(ctx: &egui::Context, theme: &Theme, port: &dyn WindowPort, pending: &mut Vec<Cmd>) {
    let m = &theme.metrics;
    let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else { return };
    let screen = ctx.screen_rect();
    let margin = m.resize_margin;
    let left = pos.x < screen.left() + margin;
    let right = pos.x > screen.right() - margin;
    let top = pos.y < screen.top() + margin;
    let bottom = pos.y > screen.bottom() - margin;

    // ignore the title bar area — only the thin ring below it resizes
    let in_title_zone = pos.y < screen.top() + theme.metrics.title_h && (left || right);
    let dir = match (left, right, top, bottom) {
        (true, _, true, _) if !in_title_zone => Some((ResizeDir::TopLeft, CursorIcon::NwResize)),
        (_, true, true, _) if !in_title_zone => Some((ResizeDir::TopRight, CursorIcon::NeResize)),
        (true, _, _, true) => Some((ResizeDir::BottomLeft, CursorIcon::SwResize)),
        (_, true, _, true) => Some((ResizeDir::BottomRight, CursorIcon::SeResize)),
        (true, _, _, _) if !in_title_zone => Some((ResizeDir::Left, CursorIcon::EwResize)),
        (_, true, _, _) if !in_title_zone => Some((ResizeDir::Right, CursorIcon::EwResize)),
        (_, _, true, _) if !in_title_zone => Some((ResizeDir::Top, CursorIcon::NsResize)),
        (_, _, _, true) => Some((ResizeDir::Bottom, CursorIcon::NsResize)),
        _ => None,
    };
    if let Some((dir, cursor)) = dir {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            port.drag_resize(dir);
        }
    }
}

/// 1 px accent border around the client area, drawn above everything.
pub fn frame_stroke(ctx: &egui::Context, cx: &Cx) {
    let p = &cx.theme.palette;
    let m = &cx.theme.metrics;
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("frame")));
    let rect = ctx.screen_rect();
    painter.rect_stroke(
        Rect::from_min_size(rect.min + Vec2::splat(m.border / 2.0), rect.size() - Vec2::splat(m.border)),
        0.0,
        Stroke::new(m.border, p.accent_dark),
        egui::StrokeKind::Inside,
    );
}

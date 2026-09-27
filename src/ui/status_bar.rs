//! Status bar: item/selection counts + view switch (D6: emits Cmd only).

use crate::app::{Cmd, Cx};
use crate::icons::{self, Icon};
use crate::ui::file_view::{format_size, ViewMode};
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let m = &cx.theme.metrics;
        let p = &cx.theme.palette;
        egui::Panel::bottom("statusbar")
            .exact_size(m.status_h)
            .frame(egui::Frame::NONE.fill(p.status_bg))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [rect.left_top(), rect.right_top()],
                    Stroke::new(1.0, p.separator),
                );
                let mut left = format!("{} item{}", cx.item_count, if cx.item_count == 1 { "" } else { "s" });
                if cx.selected_count > 0 {
                    left += &format!(
                        "    {} selected    {}",
                        cx.selected_count,
                        format_size(cx.selected_bytes)
                    );
                }
                if !cx.ops_line.is_empty() {
                    left += &format!("    {}", cx.ops_line);
                } else if !cx.status_line.is_empty() {
                    left += &format!("    {}", cx.status_line);
                }
                ui.painter().text(
                    Pos2::new(rect.left() + 10.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    left,
                    egui::FontId::proportional(cx.theme.fonts.small),
                    p.text_subtle,
                );

                // right: view switch
                let btn = 22.0;
                let details = Rect::from_min_size(
                    Pos2::new(rect.right() - btn * 2.0 - 6.0, rect.center().y - btn / 2.0),
                    Vec2::splat(btn),
                );
                let large = Rect::from_min_size(
                    Pos2::new(rect.right() - btn - 4.0, rect.center().y - btn / 2.0),
                    Vec2::splat(btn),
                );
                view_btn(ui, cx, details, Icon::ViewDetails, cx.view == ViewMode::Details, "status_details", pending, Cmd::SetView(ViewMode::Details));
                view_btn(ui, cx, large, Icon::ViewLarge, cx.view == ViewMode::Large, "status_large", pending, Cmd::SetView(ViewMode::Large));
            });
    }
}

fn view_btn(
    ui: &mut Ui,
    cx: &Cx,
    rect: Rect,
    icon: Icon,
    active: bool,
    id: &str,
    pending: &mut Vec<Cmd>,
    cmd: Cmd,
) {
    let resp = ui.interact(rect, Id::new(id), Sense::click());
    if active {
        ui.painter().rect_filled(rect.shrink(1.0), 2.0, cx.theme.palette.pressed_bg);
        ui.painter().rect_stroke(
            rect.shrink(1.5),
            2.0,
            Stroke::new(1.0, cx.theme.palette.accent),
            egui::StrokeKind::Inside,
        );
    } else if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(1.0), 2.0, cx.theme.palette.hover_bg);
    }
    icons::draw(ui.painter(), icon, rect.shrink(4.0), cx.theme, true);
    if resp.clicked() {
        pending.push(cmd);
    }
}

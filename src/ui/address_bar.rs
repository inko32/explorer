//! Address bar band: back/forward/up, breadcrumb, refresh, search box.

use crate::app::{Cmd, Cx};
use crate::icons::{self, Icon};
use egui::{Align2, Id, Key, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub struct AddressBar {
    pub search: String,
}

impl Default for AddressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressBar {
    pub fn new() -> Self {
        Self { search: String::new() }
    }

    pub fn show(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let m = &cx.theme.metrics;
        egui::Panel::top("address_band")
            .exact_size(m.toolbar_h)
            .frame(egui::Frame::NONE.fill(cx.theme.palette.toolbar_bg))
            .show(ui, |ui| {
                let band = ui.max_rect();
                let y = band.center().y;
                let btn = m.nav_btn;
                let mut x = band.left() + 6.0;

                // back / forward (circular)
                nav_circle_btn(ui, cx, Pos2::new(x + btn / 2.0, y), Icon::Back, cx.can_back, "nav_back", pending, Cmd::Back);
                x += btn + 2.0;
                nav_circle_btn(ui, cx, Pos2::new(x + btn / 2.0, y), Icon::Forward, cx.can_forward, "nav_fwd", pending, Cmd::Forward);
                x += btn + 2.0;
                // history dropdown chevron (stub)
                let chev = Rect::from_min_size(Pos2::new(x, y - 6.0), Vec2::new(12.0, 12.0));
                icons::draw(ui.painter(), Icon::ChevronDown, chev, cx.theme, false);
                x += 14.0;
                // up
                nav_circle_btn(ui, cx, Pos2::new(x + btn / 2.0, y), Icon::Up, cx.can_up, "nav_up", pending, Cmd::Up);
                x += btn + 6.0;

                // breadcrumb + search share the remaining width
                let search_w = m.search_w;
                let right = band.right() - 8.0;
                let addr_rect = Rect::from_min_max(
                    Pos2::new(x, y - m.address_h / 2.0),
                    Pos2::new(right - search_w - 8.0, y + m.address_h / 2.0),
                );
                let search_rect = Rect::from_min_max(
                    Pos2::new(right - search_w, y - m.address_h / 2.0),
                    Pos2::new(right, y + m.address_h / 2.0),
                );
                self.breadcrumb(ui, cx, addr_rect, pending);
                self.search_box(ui, cx, search_rect, pending);
            });
    }

    fn breadcrumb(&self, ui: &mut Ui, cx: &Cx, rect: Rect, pending: &mut Vec<Cmd>) {
        let p = &cx.theme.palette;
        ui.painter().rect_filled(rect, 3.0, p.address_bg);
        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, p.address_border), egui::StrokeKind::Inside);

        // refresh button lives inside the bar, right-aligned
        let refresh = Rect::from_center_size(rect.right_center() - Vec2::new(15.0, 0.0), Vec2::splat(20.0));
        let rresp = ui.interact(refresh, Id::new("addr_refresh"), Sense::click());
        if rresp.hovered() {
            ui.painter().rect_filled(refresh, 2.0, p.hover_bg);
        }
        icons::draw(ui.painter(), Icon::Refresh, refresh.shrink(3.0), cx.theme, true);
        if rresp.clicked() {
            pending.push(Cmd::Refresh);
        }

        // segments
        let max_x = refresh.left() - 6.0;
        let mut x = rect.left() + 6.0;
        let cy = rect.center().y;
        let font = egui::FontId::proportional(cx.theme.fonts.size);
        let segs = segments(cx.cwd);
        let seg_count = segs.len();
        for (seg_index, seg) in segs.iter().enumerate() {
            let is_root = matches!(seg, Segment::Root);
            let is_last = seg_index == seg_count - 1;
            let label: String = match seg {
                Segment::Root => "Computer".into(),
                Segment::Name(n) => n.clone(),
            };
            let galley = ui.painter().layout_no_wrap(label.clone(), font.clone(), p.text);
            let w = galley.size().x + 12.0 + if is_root { 20.0 } else { 0.0 };
            let seg_rect = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), Vec2::new(w.min(max_x - x), rect.height() - 4.0));
            let enabled = !is_last;
            if enabled {
                let resp = ui.interact(seg_rect, Id::new(("crumb", seg_index)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(seg_rect, 2.0, p.hover_bg);
                }
                if resp.clicked() {
                    pending.push(Cmd::Navigate(prefix_of(cx.cwd, seg_index)));
                }
            }
            let mut tx = seg_rect.left() + 6.0;
            if is_root {
                icons::draw(
                    ui.painter(),
                    Icon::Computer,
                    Rect::from_center_size(Pos2::new(tx + 8.0, cy), Vec2::splat(16.0)),
                    cx.theme,
                    true,
                );
                tx += 20.0;
            }
            ui.painter().galley(Pos2::new(tx, cy - galley.size().y / 2.0), galley, p.text);
            x += w;
            if !is_last {
                icons::draw(
                    ui.painter(),
                    Icon::ChevronRight,
                    Rect::from_center_size(Pos2::new(x + 5.0, cy), Vec2::splat(9.0)),
                    cx.theme,
                    true,
                );
                x += 12.0;
            }
            if x >= max_x {
                break;
            }
        }
    }

    fn search_box(&mut self, ui: &mut Ui, cx: &Cx, rect: Rect, pending: &mut Vec<Cmd>) {
        let p = &cx.theme.palette;
        ui.painter().rect_filled(rect, 3.0, p.address_bg);
        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, p.address_border), egui::StrokeKind::Inside);
        icons::draw(
            ui.painter(),
            Icon::Search,
            Rect::from_center_size(rect.left_center() + Vec2::new(12.0, 0.0), Vec2::splat(14.0)),
            cx.theme,
            true,
        );
        let edit_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 24.0, rect.top() + 1.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 1.0),
        );
        if self.search.is_empty() && !ui.memory(|m| m.has_focus(Id::new("addr_search"))) {
            ui.painter().text(
                Pos2::new(edit_rect.left() + 2.0, edit_rect.center().y),
                Align2::LEFT_CENTER,
                format!("Search {}", cx.cwd_label),
                egui::FontId::proportional(cx.theme.fonts.size),
                p.text_disabled,
            );
        }
        let edit = egui::TextEdit::singleline(&mut self.search)
            .id(Id::new("addr_search"))
            .desired_width(edit_rect.width())
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::symmetric(2, 0));
        let out = ui.put(edit_rect, edit);
        if out.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
            pending.push(Cmd::Status(format!("Search “{}”: coming soon", self.search)));
            self.search.clear();
        }
    }
}

fn nav_circle_btn(
    ui: &mut Ui,
    cx: &Cx,
    center: Pos2,
    icon: Icon,
    enabled: bool,
    id: &str,
    pending: &mut Vec<Cmd>,
    cmd: Cmd,
) {
    let p = &cx.theme.palette;
    let d = cx.theme.metrics.nav_btn;
    let rect = Rect::from_center_size(center, Vec2::splat(d));
    let resp = ui.interact(rect, Id::new(id), if enabled { Sense::click() } else { Sense::hover() });
    let radius = d / 2.0;
    if enabled {
        if resp.is_pointer_button_down_on() {
            ui.painter().circle_filled(center, radius, p.pressed_bg);
            ui.painter().circle_stroke(center, radius, Stroke::new(1.0, p.accent));
        } else if resp.hovered() {
            ui.painter().circle_filled(center, radius, p.hover_bg);
            ui.painter().circle_stroke(center, radius, Stroke::new(1.0, p.accent));
        }
    }
    let stroke = if enabled { Stroke::new(1.0, p.accent) } else { Stroke::new(1.0, p.separator) };
    ui.painter().circle_stroke(center, radius, stroke);
    icons::draw(ui.painter(), icon, rect.shrink(6.0), cx.theme, enabled);
    if enabled && resp.clicked() {
        pending.push(cmd);
    }
}

enum Segment {
    Root,
    Name(String),
}

fn segments(path: &std::path::Path) -> Vec<Segment> {
    let mut v = vec![Segment::Root];
    for comp in path.components() {
        if let std::path::Component::Normal(c) = comp {
            v.push(Segment::Name(c.to_string_lossy().into_owned()));
        }
    }
    v
}

fn prefix_of(path: &std::path::Path, seg_index: usize) -> std::path::PathBuf {
    if seg_index == 0 {
        return std::path::PathBuf::from("/");
    }
    let comps: Vec<_> = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .collect();
    let mut out = std::path::PathBuf::from("/");
    for c in comps.iter().take(seg_index) {
        out.push(c.as_os_str());
    }
    out
}

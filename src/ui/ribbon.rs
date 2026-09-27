//! Ribbon: File (backstage stub) / Home / Share (stub) / View tabs with
//! groups of commands. Not the Office ribbon — the simpler Win8 variant.

use crate::app::{Cmd, Cx};
use crate::icons::{self, Icon};
use crate::ui::file_view::ViewMode;
use egui::{Align2, Color32, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    File,
    Home,
    Share,
    View,
}

pub struct Ribbon {
    pub tab: Tab,
    pub collapsed: bool,
    pub backstage: bool,
}

impl Default for Ribbon {
    fn default() -> Self {
        Self::new()
    }
}

impl Ribbon {
    pub fn new() -> Self {
        Self { tab: Tab::Home, collapsed: false, backstage: false }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let m = &cx.theme.metrics;
        egui::Panel::top("ribbon")
            .exact_size(m.ribbon_tab_h + if self.collapsed { 0.0 } else { m.ribbon_h })
            .frame(egui::Frame::NONE.fill(cx.theme.palette.ribbon_bg))
            .show(ui, |ui| {
                self.tab_row(ui, cx, pending);
                let body_top = ui.max_rect().top() + m.ribbon_tab_h;
                if self.backstage && self.tab == Tab::File {
                    self.backstage_panel(ui, cx, pending);
                } else if !self.collapsed {
                    let body = Rect::from_min_size(
                        Pos2::new(ui.max_rect().left(), body_top),
                        Vec2::new(ui.max_rect().width(), m.ribbon_h),
                    );
                    let mut strip = ui.new_child(egui::UiBuilder::new().max_rect(body));
                    match self.tab {
                        Tab::Home => self.home_tab(&mut strip, cx, pending),
                        Tab::Share => self.share_tab(&mut strip, cx, pending),
                        Tab::View => self.view_tab(&mut strip, cx, pending),
                        Tab::File => {}
                    }
                }
            });
    }

    fn tab_row(&mut self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let m = &cx.theme.metrics;
        let p = &cx.theme.palette;
        let rect = Rect::from_min_size(ui.max_rect().min, Vec2::new(ui.max_rect().width(), m.ribbon_tab_h));
        let mut x = rect.left();
        for (tab, label) in [(Tab::File, "File"), (Tab::Home, "Home"), (Tab::Share, "Share"), (Tab::View, "View")] {
            let w = 30.0 + label.len() as f32 * 7.0;
            let tr = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
            let active = self.tab == tab && !self.backstage;
            let is_file = tab == Tab::File;
            let resp = ui.interact(tr, Id::new(("tab", label)), Sense::click());
            let hovered = resp.hovered();
            let bg = if is_file && !active && !hovered {
                Some(p.accent)
            } else if is_file && hovered && !active {
                Some(p.accent_dark)
            } else if active {
                Some(p.ribbon_bg)
            } else if hovered {
                Some(p.hover_bg)
            } else {
                None
            };
            if let Some(bg) = bg {
                ui.painter().rect_filled(tr, 0.0, bg);
            }
            let text_color = if is_file && !active {
                Color32::WHITE
            } else if active {
                p.text
            } else {
                p.text_subtle
            };
            ui.painter().text(
                tr.center(),
                Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(cx.theme.fonts.size),
                text_color,
            );
            if resp.clicked() {
                if is_file {
                    pending.push(Cmd::ToggleBackstage);
                } else {
                    pending.push(Cmd::SetTab(tab));
                }
            }
            x += w;
        }

        // right side: collapse chevron + help
        let chev = Rect::from_center_size(
            Pos2::new(rect.right() - 34.0, rect.center().y),
            Vec2::splat(20.0),
        );
        let cresp = ui.interact(chev, Id::new("ribbon_collapse"), Sense::click());
        if cresp.hovered() {
            ui.painter().rect_filled(chev, 2.0, p.hover_bg);
        }
        icons::draw(
            ui.painter(),
            if self.collapsed { Icon::ChevronDown } else { Icon::Up },
            chev.shrink(5.0),
            cx.theme,
            true,
        );
        if cresp.clicked() {
            pending.push(Cmd::ToggleCollapsed);
        }
        let help = Rect::from_center_size(
            Pos2::new(rect.right() - 12.0, rect.center().y),
            Vec2::splat(18.0),
        );
        ui.painter().circle_stroke(help.center(), 8.0, Stroke::new(1.0, p.text_subtle));
        ui.painter().text(
            help.center(),
            Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(11.0),
            p.text_subtle,
        );
    }

    // ---- tabs ----

    fn home_tab(&self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let y = ui.max_rect().top();
        let mut x = ui.max_rect().left() + 4.0;
        let m = &cx.theme.metrics;
        let body_h = m.ribbon_h - m.ribbon_group_label_h;

        // Clipboard
        group(ui, cx, Pos2::new(x, y), 118.0, "Clipboard", |ui, w| {
            big_cmd(ui, cx, Icon::Paste, "Paste", 0.0, 6.0, cx.has_clipboard, pending, Cmd::Paste);
            small_cmd(ui, cx, Icon::Cut, "Cut", w * 0.52, 4.0, cx.selected_count > 0, pending, Cmd::CutSelection);
            small_cmd(ui, cx, Icon::Copy, "Copy", w * 0.52, 26.0, cx.selected_count > 0, pending, Cmd::CopySelection);
            small_cmd(ui, cx, Icon::CopyPath, "Copy path", w * 0.52, 48.0, cx.selected_count > 0, pending, Cmd::Status("Copy path: coming soon".into()));
        });
        x += 122.0;

        // Organize
        group(ui, cx, Pos2::new(x, y), 96.0, "Organize", |ui, w| {
            small_cmd(ui, cx, Icon::MoveTo, "Move to", 4.0, 4.0, false, pending, Cmd::Status("Move to: coming soon".into()));
            small_cmd(ui, cx, Icon::CopyTo, "Copy to", 4.0, 26.0, false, pending, Cmd::Status("Copy to: coming soon".into()));
            small_cmd(ui, cx, Icon::Delete, "Delete", 4.0, 48.0, cx.selected_count > 0, pending, Cmd::DeleteSelection);
            small_cmd(ui, cx, Icon::Rename, "Rename", w * 0.5, 48.0, cx.selected_count == 1, pending, Cmd::RenameSelected);
        });
        x += 100.0;

        // New
        group(ui, cx, Pos2::new(x, y), 112.0, "New", |ui, w| {
            big_cmd(ui, cx, Icon::NewFolder, "New folder", 0.0, 6.0, true, pending, Cmd::NewFolder);
            small_cmd(ui, cx, Icon::NewItem, "New item", w * 0.52, 4.0, false, pending, Cmd::Status("New item: coming soon".into()));
            small_cmd(ui, cx, Icon::EasyAccess, "Easy access", w * 0.52, 26.0, false, pending, Cmd::Status("Easy access: coming soon".into()));
        });
        x += 116.0;

        // Open
        group(ui, cx, Pos2::new(x, y), 96.0, "Open", |ui, w| {
            big_cmd(ui, cx, Icon::Properties, "Properties", 0.0, 6.0, cx.selected_count == 1, pending, Cmd::Status("Properties: coming soon".into()));
            small_cmd(ui, cx, Icon::Open, "Open", w * 0.52, 4.0, cx.selected_count > 0, pending, Cmd::Status("Open: coming soon".into()));
            small_cmd(ui, cx, Icon::Edit, "Edit", w * 0.52, 26.0, false, pending, Cmd::Status("Edit: coming soon".into()));
            small_cmd(ui, cx, Icon::History, "History", w * 0.52, 48.0, false, pending, Cmd::Status("History: coming soon".into()));
        });
        x += 100.0;

        // Select
        group(ui, cx, Pos2::new(x, y), 96.0, "Select", |ui, _w| {
            small_cmd(ui, cx, Icon::SelectAll, "Select all", 4.0, 4.0, cx.item_count > 0, pending, Cmd::SelectAll);
            small_cmd(ui, cx, Icon::SelectNone, "Select none", 4.0, 26.0, cx.selected_count > 0, pending, Cmd::SelectNone);
            small_cmd(ui, cx, Icon::InvertSel, "Invert selection", 4.0, 48.0, cx.item_count > 0, pending, Cmd::InvertSelection);
        });
        let _ = body_h;
    }

    fn share_tab(&self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let y = ui.max_rect().top();
        let mut x = ui.max_rect().left() + 4.0;
        group(ui, cx, Pos2::new(x, y), 120.0, "Share with", |ui, _w| {
            small_cmd(ui, cx, Icon::Properties, "Nobody", 4.0, 4.0, false, pending, Cmd::Status("Share with: coming soon".into()));
        });
        x += 124.0;
        group(ui, cx, Pos2::new(x, y), 120.0, "Send", |ui, _w| {
            small_cmd(ui, cx, Icon::CopyTo, "Email", 4.0, 4.0, false, pending, Cmd::Status("Send: coming soon".into()));
        });
        x += 124.0;
        group(ui, cx, Pos2::new(x, y), 170.0, "Share", |ui, _w| {
            // Win8-isms like Homegroup are intentionally abandoned (spec).
            small_cmd(ui, cx, Icon::Star, "Sharing: modern replacement TBD", 4.0, 4.0, false, pending, Cmd::Status("Share: coming soon".into()));
        });
    }

    fn view_tab(&self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let y = ui.max_rect().top();
        let mut x = ui.max_rect().left() + 4.0;
        group(ui, cx, Pos2::new(x, y), 96.0, "Layout", |ui, _w| {
            small_cmd(ui, cx, Icon::ViewLarge, "Large icons", 4.0, 4.0, cx.view != ViewMode::Large, pending, Cmd::SetView(ViewMode::Large));
            small_cmd(ui, cx, Icon::ViewDetails, "Details", 4.0, 26.0, cx.view != ViewMode::Details, pending, Cmd::SetView(ViewMode::Details));
        });
        x += 100.0;
        group(ui, cx, Pos2::new(x, y), 130.0, "Show/hide", |ui, _w| {
            small_cmd(ui, cx, Icon::ViewDetails, "Preview pane", 4.0, 4.0, false, pending, Cmd::TogglePreviewPane);
            small_cmd(ui, cx, Icon::Computer, "Navigation pane", 4.0, 26.0, true, pending, Cmd::ToggleNavPane);
            small_cmd(ui, cx, Icon::File, "Hidden items", 4.0, 48.0, true, pending, Cmd::ToggleHidden);
        });
        x += 134.0;
        group(ui, cx, Pos2::new(x, y), 150.0, "Options", |ui, _w| {
            small_cmd(ui, cx, Icon::Gear, "Dark mode", 4.0, 4.0, true, pending, Cmd::SetDark(!cx.theme.dark));
            small_cmd(ui, cx, Icon::Gear, "Folder options", 4.0, 26.0, false, pending, Cmd::Status("Folder options: coming soon".into()));
        });
    }

    fn backstage_panel(&self, ui: &mut Ui, cx: &Cx, pending: &mut Vec<Cmd>) {
        let p = &cx.theme.palette;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, p.accent);
        let items: [(&str, Option<Cmd>); 3] = [
            ("Open new window", Some(Cmd::Status("Open new window: coming soon".into()))),
            ("Change folder and search options", Some(Cmd::Status("Folder options: coming soon".into()))),
            ("Close", Some(Cmd::Exit)),
        ];
        let mut y = rect.top() + 24.0;
        for (i, (label, cmd)) in items.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(rect.left() + 30.0, y), Vec2::new(320.0, 34.0));
            let resp = ui.interact(r, Id::new(("backstage", i)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x14));
            }
            ui.painter().line_segment(
                [r.left_bottom() - Vec2::new(0.0, 1.0), r.right_bottom() - Vec2::new(0.0, 1.0)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x33)),
            );
            ui.painter().text(
                Pos2::new(r.left() + 6.0, r.center().y),
                Align2::LEFT_CENTER,
                *label,
                egui::FontId::proportional(cx.theme.fonts.size),
                Color32::WHITE,
            );
            if resp.clicked() {
                if let Some(c) = cmd {
                    pending.push(c.clone());
                }
                pending.push(Cmd::ToggleBackstage);
            }
            y += 36.0;
        }
    }
}

// ---- command widgets ----

fn group<R>(
    ui: &mut Ui,
    cx: &Cx,
    pos: Pos2,
    width: f32,
    label: &str,
    add_contents: impl FnOnce(&mut Ui, f32) -> R,
) {
    let m = &cx.theme.metrics;
    let p = &cx.theme.palette;
    let body_h = m.ribbon_h - m.ribbon_group_label_h;
    let rect = Rect::from_min_size(pos, Vec2::new(width, m.ribbon_h));
    // separator on the left edge
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.top() + 4.0), Pos2::new(rect.left(), rect.top() + body_h - 4.0)],
        Stroke::new(1.0, p.separator),
    );
    let body = Rect::from_min_size(pos + Vec2::new(2.0, 0.0), Vec2::new(width - 4.0, body_h));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    add_contents(&mut child, body.width());
    // caption
    ui.painter().text(
        Pos2::new(rect.center().x, rect.top() + body_h + m.ribbon_group_label_h / 2.0),
        Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(cx.theme.fonts.ribbon_label),
        p.group_label,
    );
}

fn big_cmd(
    ui: &mut Ui,
    cx: &Cx,
    icon: Icon,
    label: &str,
    rel_x: f32,
    rel_y: f32,
    enabled: bool,
    pending: &mut Vec<Cmd>,
    cmd: Cmd,
) {
    let m = &cx.theme.metrics;
    let origin = ui.max_rect().min;
    let rect = Rect::from_min_size(
        Pos2::new(origin.x + rel_x, origin.y + rel_y),
        Vec2::new(m.ribbon_big_icon + 12.0, m.ribbon_h - m.ribbon_group_label_h - 8.0),
    );
    let resp = command_hover(ui, rect, cx, enabled, label);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + m.ribbon_big_icon / 2.0 + 2.0),
        Vec2::splat(m.ribbon_big_icon),
    );
    icons::draw(ui.painter(), icon, icon_rect, cx.theme, enabled);
    ui.painter().text(
        Pos2::new(rect.center().x, icon_rect.bottom() + 12.0),
        Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(cx.theme.fonts.ribbon_label),
        if enabled { cx.theme.palette.text } else { cx.theme.palette.text_disabled },
    );
    if enabled && resp.clicked() {
        pending.push(cmd);
    }
}

fn small_cmd(
    ui: &mut Ui,
    cx: &Cx,
    icon: Icon,
    label: &str,
    rel_x: f32,
    rel_y: f32,
    enabled: bool,
    pending: &mut Vec<Cmd>,
    cmd: Cmd,
) {
    let origin = ui.max_rect().min;
    let rect = Rect::from_min_size(
        Pos2::new(origin.x + rel_x, origin.y + rel_y),
        Vec2::new(ui.max_rect().width() - rel_x - 2.0, 20.0),
    );
    let resp = command_hover(ui, rect, cx, enabled, label);
    let icon_rect = Rect::from_center_size(Pos2::new(rect.left() + 9.0, rect.center().y), Vec2::splat(16.0));
    icons::draw(ui.painter(), icon, icon_rect, cx.theme, enabled);
    ui.painter().text(
        Pos2::new(icon_rect.right() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(cx.theme.fonts.ribbon_label),
        if enabled { cx.theme.palette.text } else { cx.theme.palette.text_disabled },
    );
    if enabled && resp.clicked() {
        pending.push(cmd);
    }
}

fn command_hover(ui: &mut Ui, rect: Rect, cx: &Cx, enabled: bool, id_src: &str) -> Response {
    let resp = ui.interact(rect, Id::new(("cmd", id_src)), if enabled { Sense::click() } else { Sense::hover() });
    if enabled {
        if resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 1.0, cx.theme.palette.pressed_bg);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 1.0, cx.theme.palette.hover_bg);
        }
    }
    resp
}

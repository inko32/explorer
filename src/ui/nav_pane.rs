//! Navigation pane: Favorites / This PC / Network (D6: emits Cmd only).

use crate::app::{Cmd, Cx};
use crate::fs::Volume;
use crate::icons::{self, Icon};
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::path::{Path, PathBuf};

pub struct NavPane {
    pub visible: bool,
    volumes: Vec<Volume>,
    items: Vec<NavItem>, // rebuilt when volumes/xdg change
    rebuild: bool,
}

#[derive(Clone)]
struct NavItem {
    section: &'static str,
    label: String,
    icon: Icon,
    target: Option<PathBuf>,
    indent: f32,
}

impl NavPane {
    pub fn new() -> Self {
        Self { visible: true, volumes: Vec::new(), items: Vec::new(), rebuild: true }
    }

    pub fn set_volumes(&mut self, v: Vec<Volume>) {
        self.volumes = v;
        self.rebuild = true;
    }

    fn rebuild_items(&mut self) {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let xdg = home.as_ref().map(|h| XdgDirs::load(h)).unwrap_or_default();
        let mut items = Vec::new();
        let mut favorites = |label: &str, icon: Icon, path: Option<PathBuf>| {
            items.push(NavItem { section: "Favorites", label: label.into(), icon, target: path, indent: 0.0 });
        };
        favorites("Home", Icon::Home, home.clone());
        favorites("Desktop", Icon::Desktop, xdg.desktop.clone());
        favorites("Documents", Icon::Documents, xdg.documents.clone());
        favorites("Downloads", Icon::Downloads, xdg.downloads.clone());
        favorites("Recent places", Icon::Recent, None);

        let mut this_pc = |label: &str, icon: Icon, path: Option<PathBuf>| {
            items.push(NavItem { section: "This PC", label: label.into(), icon, target: path, indent: 0.0 });
        };
        this_pc("Computer", Icon::Computer, Some(PathBuf::from("/")));
        for v in &self.volumes {
            this_pc(&v.name, Icon::Drive, Some(v.mount.clone()));
        }
        items.push(NavItem { section: "Network", label: "Network".into(), icon: Icon::Network, target: None, indent: 0.0 });
        self.items = items;
        self.rebuild = false;
    }

    pub fn show(&mut self, ctx: &egui::Context, cx: &Cx, pending: &mut Vec<Cmd>) {
        if self.rebuild {
            self.rebuild_items();
        }
        egui::SidePanel::left("nav_pane")
            .resizable(true)
            .default_width(cx.theme.metrics.nav_w)
            .width_range(120.0..=420.0)
            .frame(egui::Frame::NONE.fill(cx.theme.palette.nav_bg))
            .show(ctx, |ui| {
                let m = &cx.theme.metrics;
                let width = ui.max_rect().width();
                egui::ScrollArea::vertical().id_salt("nav_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    let mut last_section = "";
                    let mut y = ui.cursor().top();
                    for (i, item) in self.items.clone().iter().enumerate() {
                        if item.section != last_section {
                            if !last_section.is_empty() {
                                y += 6.0;
                            }
                            let sec_icon = match item.section {
                                "Favorites" => Icon::Star,
                                "This PC" => Icon::Computer,
                                _ => Icon::Network,
                            };
                            let sec_rect = Rect::from_min_size(Pos2::new(ui.max_rect().left(), y), Vec2::new(width, m.nav_item_h));
                            icons::draw(ui.painter(), sec_icon, Rect::from_center_size(sec_rect.left_center() + Vec2::new(10.0, 0.0), Vec2::splat(14.0)), cx.theme, true);
                            ui.painter().text(
                                Pos2::new(sec_rect.left() + 22.0, sec_rect.center().y),
                                Align2::LEFT_CENTER,
                                item.section,
                                egui::FontId::proportional(cx.theme.fonts.size),
                                cx.theme.palette.text_subtle,
                            );
                            y += m.nav_item_h;
                            last_section = item.section;
                        }
                        let rect = Rect::from_min_size(Pos2::new(ui.max_rect().left(), y), Vec2::new(width, m.nav_item_h));
                        y += m.nav_item_h;
                        let enabled = item.target.is_some();
                        let resp = ui.interact(rect, Id::new(("nav", i)), Sense::click());
                        let selected = item.target.as_deref().map(|p| paths_equal(p, cx.cwd)).unwrap_or(false);
                        if selected {
                            let p = &cx.theme.palette;
                            ui.painter().rect_filled(rect.shrink2(Vec2::new(1.0, 0.0)), 0.0, p.sel_fill);
                            ui.painter().rect_stroke(
                                rect.shrink2(Vec2::new(1.5, 0.5)),
                                0.0,
                                Stroke::new(1.0, p.sel_border),
                                egui::StrokeKind::Inside,
                            );
                        } else if resp.hovered() && enabled {
                            ui.painter().rect_filled(rect.shrink2(Vec2::new(1.0, 0.0)), 0.0, cx.theme.palette.hover_bg);
                        }
                        let icon_pos = rect.left_center() + Vec2::new(m.nav_indent / 2.0 + 4.0, 0.0);
                        icons::draw(
                            ui.painter(),
                            item.icon,
                            Rect::from_center_size(icon_pos, Vec2::splat(16.0)),
                            cx.theme,
                            enabled,
                        );
                        let color = if enabled { cx.theme.palette.text } else { cx.theme.palette.text_disabled };
                        ui.painter().text(
                            Pos2::new(rect.left() + m.nav_indent + 8.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &item.label,
                            egui::FontId::proportional(cx.theme.fonts.size),
                            color,
                        );
                        if resp.clicked() {
                            if let Some(p) = &item.target {
                                pending.push(Cmd::Navigate(p.clone()));
                            } else if item.label == "Network" {
                                pending.push(Cmd::Status("Network browsing (gvfs network://): coming soon".into()));
                            } else {
                                pending.push(Cmd::Status("Recent places: coming soon".into()));
                            }
                        }
                    }
                    ui.allocate_space(Vec2::new(width, y - ui.cursor().top()));
                });
            });
    }

    /// Demo volumes for headless snapshots.
    pub fn load_demo(&mut self) {
        self.volumes = vec![
            Volume { name: "Local Disk".into(), mount: PathBuf::from("/"), fs_type: Some("ext4".into()) },
            Volume { name: "Data".into(), mount: PathBuf::from("/home"), fs_type: Some("ext4".into()) },
        ];
        self.rebuild = true;
        self.rebuild_items();
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b || a.canonicalize().ok().zip(b.canonicalize().ok()).map(|(x, y)| x == y).unwrap_or(false)
}

/// XDG user directories (~/.config/user-dirs.dirs), minimal parser.
#[derive(Default, Clone)]
struct XdgDirs {
    desktop: Option<PathBuf>,
    documents: Option<PathBuf>,
    downloads: Option<PathBuf>,
}

impl XdgDirs {
    fn load(home: &Path) -> Self {
        let mut dirs = Self::default();
        let Ok(text) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) else {
            return Self {
                desktop: Some(home.join("Desktop")),
                documents: Some(home.join("Documents")),
                downloads: Some(home.join("Downloads")),
            };
        };
        let mut get = |key: &str| -> Option<PathBuf> {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix(key).and_then(|r| r.trim_start().strip_prefix('=')) {
                    let val = rest.trim().trim_matches('"');
                    let val = val.replacen("$HOME", &home.to_string_lossy(), 1);
                    return Some(PathBuf::from(val));
                }
            }
            None
        };
        dirs.desktop = get("XDG_DESKTOP_DIR");
        dirs.documents = get("XDG_DOCUMENTS_DIR");
        dirs.downloads = get("XDG_DOWNLOAD_DIR");
        dirs
    }
}

//! Theme: single authority for every color, dimension and font setting.
//!
//! Every UI module reads from here — no magic numbers elsewhere (rule #4 in
//! ARCHITECTURE.md). Re-theming (Win7/aero-glass/Win98/dark) = swapping or
//! editing a `Theme`.

use egui::{Color32, CornerRadius, Stroke};

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    pub palette: Palette,
    pub metrics: Metrics,
    pub fonts: Fonts,
    /// Bumped on every change so App can re-apply the egui Style lazily.
    pub generation: u64,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // palette is a theme vocabulary
pub struct Palette {
    // frame
    pub accent: Color32,
    pub accent_dark: Color32, // 1px border around the window
    pub titlebar_inactive: Color32,
    pub titlebar_text_inactive: Color32,
    pub caption_hover: Color32,
    pub caption_pressed: Color32,
    pub close_hover: Color32,
    pub close_pressed: Color32,
    // surfaces
    pub window_bg: Color32,
    pub ribbon_bg: Color32,
    pub toolbar_bg: Color32,
    pub nav_bg: Color32,
    pub status_bg: Color32,
    pub col_header_bg: Color32,
    pub hover_bg: Color32,     // address crumbs, nav items, ribbon commands
    pub pressed_bg: Color32,
    pub separator: Color32,    // ribbon group separators, status top border
    pub group_label: Color32,  // ribbon group captions
    pub address_bg: Color32,
    pub address_border: Color32,
    // text
    pub text: Color32,
    pub text_disabled: Color32,
    pub text_subtle: Color32, // status bar, column headers
    // selection
    pub sel_fill: Color32,
    pub sel_border: Color32,
    pub sel_fill_unfocused: Color32,
    // icons
    pub icon_folder: Color32,
    pub icon_folder_edge: Color32,
    pub icon_file: Color32,
    pub icon_file_edge: Color32,
    pub icon_glyph: Color32, // arrows, glyphs on accent
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Metrics {
    pub resize_margin: f32,
    pub border: f32,
    pub title_h: f32,
    pub caption_btn_w: f32,
    // ribbon
    pub ribbon_tab_h: f32,
    pub ribbon_h: f32, // body incl. group-label row
    pub ribbon_group_label_h: f32,
    pub ribbon_big_icon: f32,
    pub ribbon_small_icon: f32,
    // command band
    pub toolbar_h: f32,
    pub nav_btn: f32,
    pub address_h: f32,
    pub search_w: f32,
    // nav pane
    pub nav_w: f32,
    pub nav_item_h: f32,
    pub nav_indent: f32,
    // file view
    pub col_header_h: f32,
    pub row_h: f32,
    pub details_icon: f32,
    pub large_cell_w: f32,
    pub large_cell_h: f32,
    pub large_icon: f32,
    pub col_date_w: f32,
    pub col_type_w: f32,
    pub col_size_w: f32,
    // status bar
    pub status_h: f32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Fonts {
    /// Base UI font size (points).
    pub size: f32,
    pub small: f32,    // group labels, status bar
    pub title: f32,    // window title
    pub ribbon_label: f32,
    /// Reserved: proportional font family override (loaded at runtime later).
    pub family: Option<String>,
}

impl Metrics {
    fn win8() -> Self {
        Self {
            resize_margin: 5.0,
            border: 1.0,
            title_h: 30.0,
            caption_btn_w: 46.0,
            ribbon_tab_h: 28.0,
            ribbon_h: 94.0,
            ribbon_group_label_h: 15.0,
            ribbon_big_icon: 42.0,
            ribbon_small_icon: 16.0,
            toolbar_h: 40.0,
            nav_btn: 28.0,
            address_h: 26.0,
            search_w: 200.0,
            nav_w: 210.0,
            nav_item_h: 22.0,
            nav_indent: 32.0,
            col_header_h: 24.0,
            row_h: 21.0,
            details_icon: 16.0,
            large_cell_w: 110.0,
            large_cell_h: 116.0,
            large_icon: 64.0,
            col_date_w: 170.0,
            col_type_w: 140.0,
            col_size_w: 90.0,
            status_h: 23.0,
        }
    }
}

impl Fonts {
    fn win8() -> Self {
        Self {
            size: 12.0,
            small: 11.0,
            title: 12.0,
            ribbon_label: 11.0,
            family: None,
        }
    }
}

impl Theme {
    pub fn light() -> Self {
        Self {
            name: "windows8-light",
            dark: false,
            generation: 0,
            palette: Palette {
                accent: Color32::from_rgb(0x00, 0x78, 0xD7),
                accent_dark: Color32::from_rgb(0x00, 0x55, 0x9B),
                titlebar_inactive: Color32::from_rgb(0xC8, 0xC8, 0xC8),
                titlebar_text_inactive: Color32::from_rgb(0x6B, 0x6B, 0x6B),
                caption_hover: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x2E),
                caption_pressed: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x66),
                close_hover: Color32::from_rgb(0xE8, 0x11, 0x23),
                close_pressed: Color32::from_rgb(0xB7, 0x10, 0x4B),
                window_bg: Color32::WHITE,
                ribbon_bg: Color32::WHITE,
                toolbar_bg: Color32::WHITE,
                nav_bg: Color32::WHITE,
                status_bg: Color32::WHITE,
                col_header_bg: Color32::from_rgb(0xF7, 0xF7, 0xF7),
                hover_bg: Color32::from_rgb(0xE5, 0xF1, 0xFB),
                pressed_bg: Color32::from_rgb(0xCC, 0xE4, 0xF7),
                separator: Color32::from_rgb(0xDA, 0xDA, 0xDA),
                group_label: Color32::from_rgb(0x59, 0x59, 0x59),
                address_bg: Color32::WHITE,
                address_border: Color32::from_rgb(0xA9, 0xA9, 0xA9),
                text: Color32::from_rgb(0x26, 0x26, 0x26),
                text_disabled: Color32::from_rgb(0xA0, 0xA0, 0xA0),
                text_subtle: Color32::from_rgb(0x66, 0x66, 0x66),
                sel_fill: Color32::from_rgb(0xCD, 0xE6, 0xF7),
                sel_border: Color32::from_rgb(0x00, 0x78, 0xD7),
                sel_fill_unfocused: Color32::from_rgb(0xDA, 0xDA, 0xDA),
                icon_folder: Color32::from_rgb(0xF6, 0xC3, 0x56),
                icon_folder_edge: Color32::from_rgb(0xC8, 0x8E, 0x1E),
                icon_file: Color32::WHITE,
                icon_file_edge: Color32::from_rgb(0x8A, 0x8A, 0x8A),
                icon_glyph: Color32::from_rgb(0x40, 0x40, 0x40),
            },
            metrics: Metrics::win8(),
            fonts: Fonts::win8(),
        }
    }

    pub fn dark() -> Self {
        let mut t = Self::light();
        t.name = "windows8-dark";
        t.dark = true;
        t.palette = Palette {
            accent: Color32::from_rgb(0x00, 0x78, 0xD7),
            accent_dark: Color32::from_rgb(0x00, 0x4A, 0x86),
            titlebar_inactive: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            titlebar_text_inactive: Color32::from_rgb(0x99, 0x99, 0x99),
            caption_hover: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x2E),
            caption_pressed: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x66),
            close_hover: Color32::from_rgb(0xE8, 0x11, 0x23),
            close_pressed: Color32::from_rgb(0xB7, 0x10, 0x4B),
            window_bg: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            ribbon_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
            toolbar_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
            nav_bg: Color32::from_rgb(0x25, 0x25, 0x26),
            status_bg: Color32::from_rgb(0x25, 0x25, 0x26),
            col_header_bg: Color32::from_rgb(0x2B, 0x2B, 0x2C),
            hover_bg: Color32::from_rgb(0x3A, 0x40, 0x46),
            pressed_bg: Color32::from_rgb(0x4A, 0x52, 0x5A),
            separator: Color32::from_rgb(0x45, 0x45, 0x45),
            group_label: Color32::from_rgb(0xAD, 0xAD, 0xAD),
            address_bg: Color32::from_rgb(0x1B, 0x1B, 0x1C),
            address_border: Color32::from_rgb(0x55, 0x55, 0x55),
            text: Color32::from_rgb(0xE8, 0xE8, 0xE8),
            text_disabled: Color32::from_rgb(0x6E, 0x6E, 0x6E),
            text_subtle: Color32::from_rgb(0xA0, 0xA0, 0xA0),
            sel_fill: Color32::from_rgb(0x2F, 0x54, 0x80),
            sel_border: Color32::from_rgb(0x00, 0x78, 0xD7),
            sel_fill_unfocused: Color32::from_rgb(0x3F, 0x3F, 0x3F),
            icon_folder: Color32::from_rgb(0xC8, 0x9B, 0x3C),
            icon_folder_edge: Color32::from_rgb(0x8A, 0x66, 0x18),
            icon_file: Color32::from_rgb(0xC9, 0xC9, 0xC9),
            icon_file_edge: Color32::from_rgb(0x77, 0x77, 0x77),
            icon_glyph: Color32::from_rgb(0xD0, 0xD0, 0xD0),
        };
        t
    }

    pub fn toggle_dark(&mut self) {
        let dark = !self.dark;
        let generation = self.generation + 1;
        *self = if dark { Self::dark() } else { Self::light() };
        self.generation = generation;
    }

    /// Push palette/fonts into the egui style for every system theme
    /// variant (egui 0.36 keeps per-theme styles).
    pub fn apply(&self, ctx: &egui::Context) {
        let p = &self.palette;
        ctx.all_styles_mut(|style| {
            style.visuals.dark_mode = self.dark;
            style.visuals.window_fill = p.window_bg;
            style.visuals.panel_fill = p.window_bg;
            style.visuals.override_text_color = Some(p.text);
            style.visuals.selection.bg_fill = p.sel_fill;
            style.visuals.selection.stroke = Stroke::new(1.0, p.sel_border);
            style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
            style.visuals.widgets.inactive.bg_fill = p.window_bg;
            style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
            style.visuals.widgets.hovered.bg_fill = p.hover_bg;
            style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
            style.visuals.widgets.active.bg_fill = p.pressed_bg;
            style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
            style.visuals.widgets.noninteractive.bg_fill = p.window_bg;
            style.visuals.widgets.noninteractive.weak_bg_fill = p.window_bg;
            style.visuals.widgets.inactive.weak_bg_fill = p.window_bg;
            style.visuals.widgets.hovered.weak_bg_fill = p.hover_bg;
            style.visuals.widgets.active.weak_bg_fill = p.pressed_bg;
            style.visuals.window_corner_radius = CornerRadius::ZERO;
            style.visuals.window_stroke = Stroke::new(0.0, p.window_bg);
            style.spacing.item_spacing = egui::vec2(0.0, 0.0);
            style.spacing.button_padding = egui::vec2(4.0, 2.0);
            style.spacing.scroll = egui::style::ScrollStyle::thin();
            style.text_styles = font_map(self);
        });
    }
}

/// egui text-style table from our font config.
fn font_map(theme: &Theme) -> std::collections::BTreeMap<egui::TextStyle, egui::FontId> {
    let f = &theme.fonts;
    let proportional = egui::FontId::proportional(f.size);
    let small = egui::FontId::proportional(f.small);
    let mut map = std::collections::BTreeMap::new();
    map.insert(egui::TextStyle::Body, proportional.clone());
    map.insert(egui::TextStyle::Button, proportional.clone());
    map.insert(egui::TextStyle::Heading, egui::FontId::proportional(f.size + 2.0));
    map.insert(egui::TextStyle::Small, small);
    map.insert(egui::TextStyle::Monospace, egui::FontId::monospace(f.small));
    map
}

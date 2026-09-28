//! Placeholder SVG icon set (spec: "draw an svg and leave all icons pointing
//! to it"). Every icon is a tiny SVG string with theme colors baked in.
//! Swap for a real icon theme / shell32-derived set later — only this file
//! changes. Vocabulary grows with features.
#![allow(dead_code)]

use iced::widget::svg;
use iced::Color;

use crate::ui::skin::Skin;

/// Builtin theme-colored SVG generator dispatch (registry `svg: builtin`).
pub fn builtin_svg(name: &str, s: &Skin) -> svg::Handle {
    match name {
        "home" => home(s),
        "desktop" => computer(s),
        "documents" => documents(s),
        "downloads" => downloads(s),
        "pictures" => pictures(s),
        "music" => music(s),
        "videos" => videos(s),
        "recent" => recent(s),
        "network" => network(s),
        "drive" => drive(s),
        "back" => back(s),
        "forward" => forward(s),
        "up" => up(s),
        "refresh" => refresh(s),
        "search" => search(s),
        "chevron_down" => chevron_down(s),
        "undo" => undo(s),
        "redo" => redo(s),
        "chevron_right" => chevron_right(s),
        "help" => help(s),
        "minimize" => minimize(s),
        "maximize" => maximize(s),
        "restore" => restore(s),
        "close" => close(s),
        "gear" => gear(s),
        "open" => open(s),
        "copy_path" => copy_path(s),
        _ => file(s),
    }
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8)
}

/// SVG fill attribute: fully transparent renders as `none` (alpha is dropped
/// by `hex`, and SVG's default fill is black).
fn fill_attr(c: Color) -> String {
    if c.a < 0.01 {
        "none".to_string()
    } else {
        hex(c)
    }
}

fn doc(inner: String) -> svg::Handle {
    svg::Handle::from_memory(
        format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>{inner}</svg>")
            .into_bytes(),
    )
}

fn poly(points: &[(f32, f32)], fill: Color) -> String {
    let pts: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
    format!("<polygon points='{}' fill='{}'/>", pts.join(" "), hex(fill))
}

fn rect(x: f32, y: f32, w: f32, h: f32, fill: Color, stroke: Option<(Color, f32)>) -> String {
    match stroke {
        Some((c, sw)) => format!(
            "<rect x='{x}' y='{y}' width='{w}' height='{h}' fill='{}' stroke='{}' stroke-width='{sw}'/>",
            fill_attr(fill),
            hex(c)
        ),
        None => format!("<rect x='{x}' y='{y}' width='{w}' height='{h}' fill='{}'/>", fill_attr(fill)),
    }
}

fn circle(cx: f32, cy: f32, r: f32, fill: Color, stroke: Option<(Color, f32)>) -> String {
    match stroke {
        Some((c, sw)) => format!(
            "<circle cx='{cx}' cy='{cy}' r='{r}' fill='{}' stroke='{}' stroke-width='{sw}'/>",
            fill_attr(fill),
            hex(c)
        ),
        None => format!("<circle cx='{cx}' cy='{cy}' r='{r}' fill='{}'/>", fill_attr(fill)),
    }
}

fn line(x1: f32, y1: f32, x2: f32, y2: f32, stroke: Color, sw: f32) -> String {
    format!("<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='{}' stroke-width='{sw}'/>", hex(stroke))
}

fn path(d: &str, fill: Color, stroke: Option<(Color, f32)>) -> String {
    match stroke {
        Some((c, sw)) => format!("<path d='{d}' fill='{}' stroke='{}' stroke-width='{sw}'/>", fill_attr(fill), hex(c)),
        None => format!("<path d='{d}' fill='{}'/>", fill_attr(fill)),
    }
}

fn glyph(s: &Skin) -> Color {
    s.text
}

pub fn folder(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        path("M1 4.5 h4.6 l1.4 2 H15 v7.5 H1 z", s.folder, None),
        path("M1 4.5 h4.6 l1.4 2 H15 v7.5 H1 z", Color::TRANSPARENT, Some((s.folder_edge, 1.0))),
    ))
}

pub fn file(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        path("M3.5 1 h6 l3 3 v11 h-9 z", s.window_bg, Some((s.subtle, 1.0))),
        path("M9.5 1 v3 h3", Color::TRANSPARENT, Some((s.subtle, 1.0))),
        line(5.5, 8.0, 10.5, 8.0, s.subtle, 0.8),
    ) + &line(5.5, 10.5, 10.5, 10.5, s.subtle, 0.8))
}

pub fn file_image(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}{}",
        path("M3.5 1 h6 l3 3 v11 h-9 z", s.window_bg, Some((s.subtle, 1.0))),
        path("M9.5 1 v3 h3", Color::TRANSPARENT, Some((s.subtle, 1.0))),
        circle(6.5, 7.0, 1.0, s.subtle, None),
        line(5.0, 12.0, 8.0, 8.5, s.subtle, 0.8),
        line(8.0, 8.5, 11.0, 12.0, s.subtle, 0.8),
    ))
}

pub fn computer(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}",
        rect(1.5, 2.0, 13.0, 9.0, s.window_bg, Some((glyph(s), 1.0))),
        rect(6.5, 11.5, 3.0, 2.0, s.window_bg, Some((glyph(s), 1.0))),
        rect(4.5, 13.5, 7.0, 1.2, glyph(s), None),
        rect(2.5, 3.0, 11.0, 7.0, s.hover, None),
    ))
}

#[allow(dead_code)]
pub fn drive(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(1.5, 4.0, 13.0, 8.0, s.window_bg, Some((glyph(s), 1.0))),
        circle(12.0, 10.0, 1.0, Color::from_rgb(0.18, 0.72, 0.18), None),
    ))
}

pub fn network(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}",
        line(3.0, 5.0, 13.0, 5.0, glyph(s), 1.0),
        circle(3.0, 5.0, 1.8, s.window_bg, Some((glyph(s), 1.0))),
        circle(8.0, 5.0, 1.8, s.window_bg, Some((glyph(s), 1.0))),
        circle(13.0, 5.0, 1.8, s.window_bg, Some((glyph(s), 1.0))),
    ) + &line(3.0, 5.0, 8.0, 12.0, glyph(s), 1.0)
        + &line(13.0, 5.0, 8.0, 12.0, glyph(s), 1.0)
        + &circle(8.0, 12.0, 1.8, s.window_bg, Some((glyph(s), 1.0))))
}

pub fn star(_s: &Skin) -> svg::Handle {
    let mut pts = Vec::new();
    for i in 0..10 {
        let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let r = if i % 2 == 0 { 7.0 } else { 3.2 };
        pts.push((8.0 + r * ang.cos(), 8.0 + r * ang.sin()));
    }
    doc(poly(&pts, Color::from_rgb(1.0, 0.73, 0.0)))
}

pub fn star_outline(s: &Skin) -> svg::Handle {
    let mut pts = Vec::new();
    for i in 0..10 {
        let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let r = if i % 2 == 0 { 7.0 } else { 3.2 };
        pts.push((8.0 + r * ang.cos(), 8.0 + r * ang.sin()));
    }
    let pts_s: Vec<String> = pts.iter().map(|(x, y)| format!("{x},{y}")).collect();
    doc(format!(
        "<polygon points='{}' fill='none' stroke='{}' stroke-width='1'/>",
        pts_s.join(" "),
        hex(glyph(s))
    ))
}

pub fn home(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        poly(&[(1.5, 8.0), (8.0, 1.5), (14.5, 8.0)], Color::from_rgb(0.75, 0.31, 0.27)),
        rect(3.5, 8.0, 9.0, 6.5, Color::from_rgb(0.91, 0.89, 0.85), Some((glyph(s), 0.8))),
        rect(7.0, 10.0, 2.0, 4.5, glyph(s), None),
    ))
}

pub fn documents(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        path("M3.5 1 h6 l3 3 v11 h-9 z", s.window_bg, Some((glyph(s), 1.0))),
        path("M9.5 1 v3 h3", Color::TRANSPARENT, Some((glyph(s), 1.0))),
        line(5.5, 8.0, 10.5, 8.0, glyph(s), 0.8),
    ) + &line(5.5, 10.5, 10.5, 10.5, glyph(s), 0.8))
}

pub fn downloads(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        rect(2.0, 10.5, 12.0, 3.5, s.folder, Some((s.folder_edge, 1.0))),
        line(8.0, 2.0, 8.0, 9.0, glyph(s), 1.4),
        poly(&[(5.0, 7.5), (11.0, 7.5), (8.0, 10.5)], glyph(s)),
    ))
}

pub fn pictures(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}",
        rect(2.0, 2.5, 12.0, 11.0, s.window_bg, Some((glyph(s), 1.0))),
        circle(10.5, 6.0, 1.3, glyph(s), None),
        line(3.5, 11.5, 7.5, 7.5, glyph(s), 1.0),
        line(7.5, 7.5, 12.5, 12.5, glyph(s), 1.0),
    ))
}

pub fn music(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}",
        line(6.0, 12.5, 6.0, 3.0, glyph(s), 1.2),
        line(6.0, 3.0, 12.5, 1.5, glyph(s), 1.2),
        line(12.5, 1.5, 12.5, 10.5, glyph(s), 1.2),
        circle(4.2, 12.5, 1.8, glyph(s), None),
    ) + &circle(10.7, 10.5, 1.8, glyph(s), None))
}

pub fn videos(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.5, 3.5, 11.0, 9.0, s.window_bg, Some((glyph(s), 1.0))),
        poly(&[(6.5, 5.5), (11.5, 8.0), (6.5, 10.5)], glyph(s)),
    ))
}

pub fn recent(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        circle(8.0, 8.0, 6.0, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        line(8.0, 8.0, 8.0, 4.0, glyph(s), 1.2),
        line(8.0, 8.0, 11.0, 9.5, glyph(s), 1.2),
    ))
}

pub fn back(s: &Skin) -> svg::Handle {
    doc(poly(&[(10.5, 2.5), (5.0, 8.0), (10.5, 13.5)], glyph(s)))
}

pub fn forward(s: &Skin) -> svg::Handle {
    doc(poly(&[(5.5, 2.5), (11.0, 8.0), (5.5, 13.5)], glyph(s)))
}

pub fn up(s: &Skin) -> svg::Handle {
    doc(poly(&[(2.5, 10.5), (8.0, 5.0), (13.5, 10.5)], glyph(s)))
}

pub fn refresh(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        path("M13.0 8.0 a5.0 5.0 0 1 1 -1.5 -3.5", Color::TRANSPARENT, Some((glyph(s), 1.4))),
        poly(&[(13.5, 2.0), (13.5, 6.0), (9.5, 5.0)], glyph(s)),
    ))
}

pub fn search(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        circle(7.0, 7.0, 4.5, Color::TRANSPARENT, Some((glyph(s), 1.4))),
        line(10.5, 10.5, 14.0, 14.0, glyph(s), 1.6),
    ))
}

pub fn chevron_down(s: &Skin) -> svg::Handle {
    doc(poly(&[(3.5, 5.5), (12.5, 5.5), (8.0, 11.0)], glyph(s)))
}

/// Win8 QAT undo: curved arrow sweeping back to the left.
pub fn undo(s: &Skin) -> svg::Handle {
    let c = Color::WHITE; // sits on the accent title bar
    doc(format!(
        "{}{}",
        path(
            "M12.4 11.8 a4.4 4.4 0 1 1 0.4 -5.4",
            Color::TRANSPARENT,
            Some((c, 1.6)),
        ),
        poly(&[(11.2, 1.6), (6.2, 4.4), (11.8, 6.6)], c),
    ))
}

/// Win8 QAT redo: mirror of undo.
pub fn redo(s: &Skin) -> svg::Handle {
    let c = Color::WHITE;
    doc(format!(
        "{}{}",
        path(
            "M3.6 11.8 a4.4 4.4 0 1 0 -0.4 -5.4",
            Color::TRANSPARENT,
            Some((c, 1.6)),
        ),
        poly(&[(4.8, 1.6), (9.8, 4.4), (4.2, 6.6)], c),
    ))
}

pub fn chevron_right(s: &Skin) -> svg::Handle {
    doc(poly(&[(5.5, 3.5), (11.0, 8.0), (5.5, 12.5)], glyph(s)))
}

pub fn copy(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.0, 2.0, 8.0, 10.0, s.window_bg, Some((glyph(s), 1.0))),
        rect(5.0, 5.0, 8.0, 9.0, s.window_bg, Some((glyph(s), 1.0))),
    ))
}

pub fn paste(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        rect(3.0, 3.0, 10.0, 12.0, Color::from_rgb(0.89, 0.85, 0.73), Some((glyph(s), 1.0))),
        rect(5.5, 1.5, 5.0, 2.5, s.window_bg, Some((glyph(s), 1.0))),
        rect(5.5, 6.5, 5.0, 6.0, s.window_bg, Some((glyph(s), 0.8))),
    ))
}

pub fn cut(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}{}",
        circle(4.0, 3.5, 1.8, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        circle(12.0, 3.5, 1.8, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        line(5.2, 4.8, 9.0, 13.5, glyph(s), 1.2),
        line(10.8, 4.8, 7.0, 13.5, glyph(s), 1.2),
    ))
}

pub fn delete(s: &Skin) -> svg::Handle {
    let red = Color::from_rgb(0.82, 0.19, 0.19);
    doc(format!(
        "{}{}{}{}",
        line(2.5, 4.0, 13.5, 4.0, red, 1.4),
        line(6.5, 2.0, 9.5, 2.0, red, 1.4),
        line(8.0, 2.0, 8.0, 4.0, red, 1.4),
        rect(4.0, 5.0, 8.0, 9.5, Color::TRANSPARENT, Some((glyph(s), 1.0))),
    ) + &line(6.0, 7.0, 6.0, 12.5, glyph(s), 0.8)
        + &line(8.0, 7.0, 8.0, 12.5, glyph(s), 0.8)
        + &line(10.0, 7.0, 10.0, 12.5, glyph(s), 0.8))
}

pub fn rename(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(1.5, 4.5, 13.0, 7.0, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        circle(5.0, 8.0, 1.2, s.window_bg, Some((glyph(s), 1.0))),
    ))
}

pub fn new_folder(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        path("M1 4.5 h4.6 l1.4 2 H15 v7.5 H1 z", s.folder, None),
        path("M1 4.5 h4.6 l1.4 2 H15 v7.5 H1 z", Color::TRANSPARENT, Some((s.folder_edge, 1.0))),
        line(11.0, 7.0, 11.0, 12.0, glyph(s), 1.6),
    ) + &line(8.5, 9.5, 13.5, 9.5, glyph(s), 1.6))
}

pub fn properties(_s: &Skin) -> svg::Handle {
    doc(path(
        "M2.5 8.5 L6.5 12.5 L13.5 3.5",
        Color::TRANSPARENT,
        Some((Color::from_rgb(0.12, 0.62, 0.24), 2.2)),
    ))
}

pub fn open(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.0, 2.0, 12.0, 12.0, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        poly(&[(8.0, 4.5), (11.5, 9.5), (4.5, 9.5)], glyph(s)),
    ))
}

pub fn edit(s: &Skin) -> svg::Handle {
    doc(poly(
        &[(3.0, 13.0), (4.0, 9.0), (11.5, 1.5), (14.0, 4.0), (6.5, 11.5)],
        glyph(s),
    ))
}

pub fn history(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        path("M3.0 8.0 a5.0 5.0 0 1 0 1.5 -3.5", Color::TRANSPARENT, Some((glyph(s), 1.2))),
        line(8.0, 5.0, 8.0, 8.5, glyph(s), 1.2),
        line(8.0, 8.5, 10.5, 10.0, glyph(s), 1.2),
    ) + &poly(&[(2.5, 2.5), (6.0, 4.5), (3.0, 6.0)], glyph(s)))
}

pub fn move_to(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.0, 2.0, 8.0, 10.0, s.window_bg, Some((glyph(s), 1.0))),
        poly(&[(9.5, 8.0), (14.5, 8.0), (12.0, 12.5)], glyph(s)),
    ))
}

pub fn copy_to(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        rect(2.0, 2.0, 8.0, 10.0, s.window_bg, Some((glyph(s), 1.0))),
        rect(5.0, 5.0, 8.0, 9.0, s.window_bg, Some((glyph(s), 1.0))),
        poly(&[(12.5, 3.0), (15.0, 6.0), (10.0, 6.0)], glyph(s)),
    ))
}

pub fn easy_access(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        circle(8.0, 8.0, 6.5, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        line(8.0, 4.5, 8.0, 11.5, glyph(s), 1.4),
        line(4.5, 8.0, 11.5, 8.0, glyph(s), 1.4),
    ))
}

pub fn new_item(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        path("M4.5 1 h5 l3 3 v11 h-8 z", s.window_bg, Some((glyph(s), 1.0))),
        line(8.0, 6.0, 8.0, 12.0, glyph(s), 1.4),
        line(5.0, 9.0, 11.0, 9.0, glyph(s), 1.4),
    ))
}

pub fn copy_path(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        rect(1.5, 4.0, 13.0, 8.0, Color::TRANSPARENT, Some((glyph(s), 1.0))),
        circle(4.0, 8.0, 1.0, glyph(s), None),
        circle(12.0, 8.0, 1.0, glyph(s), None),
    ) + &line(5.0, 8.0, 11.0, 8.0, glyph(s), 1.0))
}

pub fn paste_shortcut(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.5, 2.5, 9.0, 11.0, Color::from_rgb(0.89, 0.85, 0.73), Some((glyph(s), 1.0))),
        rect(4.5, 0.8, 5.0, 2.5, s.window_bg, Some((glyph(s), 1.0))),
    ) + &circle(12.0, 12.0, 2.5, s.window_bg, Some((glyph(s), 1.0))))
}

pub fn select_all(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.0, 2.0, 12.0, 12.0, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        path("M4.5 8.5 L7.0 11.0 L11.5 4.5", Color::TRANSPARENT, Some((glyph(s), 1.4))),
    ))
}

pub fn select_none(s: &Skin) -> svg::Handle {
    doc(rect(2.0, 2.0, 12.0, 12.0, Color::TRANSPARENT, Some((glyph(s), 1.2))))
}

pub fn invert_sel(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.0, 2.0, 12.0, 12.0, Color::TRANSPARENT, Some((glyph(s), 1.2))),
        rect(5.0, 5.0, 6.0, 6.0, glyph(s), None),
    ))
}

pub fn view_details(s: &Skin) -> svg::Handle {
    let mut out = String::new();
    for i in 0..3 {
        let y = 3.0 + i as f32 * 4.0;
        out += &circle(3.0, y, 1.0, glyph(s), None);
        out += &line(6.0, y, 14.0, y, glyph(s), 1.0);
    }
    doc(out)
}

pub fn view_large(s: &Skin) -> svg::Handle {
    doc(rect(2.5, 2.5, 11.0, 11.0, s.folder, Some((s.folder_edge, 1.0))))
}

pub fn archive(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}{}",
        rect(2.0, 3.5, 12.0, 10.0, s.folder, Some((s.folder_edge, 1.0))),
        rect(2.0, 3.5, 12.0, 2.5, s.folder_edge, None),
        rect(7.0, 6.5, 2.0, 3.0, s.window_bg, Some((glyph(s), 0.8))),
    ))
}

pub fn gear(s: &Skin) -> svg::Handle {
    let mut out = circle(8.0, 8.0, 4.5, glyph(s), None);
    for i in 0..8 {
        let ang = i as f32 * std::f32::consts::TAU / 8.0;
        let (x, y) = (8.0 + 6.0 * ang.cos(), 8.0 + 6.0 * ang.sin());
        out += &circle(x, y, 1.5, glyph(s), None);
    }
    out += &circle(8.0, 8.0, 2.0, s.window_bg, None);
    doc(out)
}

pub fn minimize(_s: &Skin) -> svg::Handle {
    doc(line(3.0, 8.0, 13.0, 8.0, Color::WHITE, 1.4))
}

pub fn maximize(_s: &Skin) -> svg::Handle {
    doc(rect(3.0, 3.0, 10.0, 10.0, Color::TRANSPARENT, Some((Color::WHITE, 1.4))))
}

pub fn restore(_s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        rect(2.5, 2.5, 8.0, 8.0, Color::TRANSPARENT, Some((Color::WHITE, 1.2))),
        rect(5.5, 5.5, 8.0, 8.0, Color::TRANSPARENT, Some((Color::WHITE, 1.2))),
    ))
}

pub fn close(_s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        line(3.5, 3.5, 12.5, 12.5, Color::WHITE, 1.5),
        line(12.5, 3.5, 3.5, 12.5, Color::WHITE, 1.5),
    ))
}

pub fn help(s: &Skin) -> svg::Handle {
    doc(format!(
        "{}{}",
        circle(8.0, 8.0, 6.5, Color::TRANSPARENT, Some((s.subtle, 1.0))),
        "<text x='8' y='12' font-size='11' text-anchor='middle' fill='#a0a0a0'>?</text>",
    ))
}

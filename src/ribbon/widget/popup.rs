//! Popup scenes + the iced overlay that hosts them.

use iced::alignment::{Horizontal, Vertical};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::keyboard;
use iced::mouse;
use iced::advanced::Shell;
use iced::{Color, Point, Rectangle, Size, Vector};

use crate::ribbon::enums::{ExecutionVerb, SwatchColorMode};
use crate::ribbon::framework::RibbonFramework;
use crate::ribbon::prop::{FontToggle, PropertyKey, PropValue};
use crate::ribbon::spec::{CommandRef, ControlSpec, MenuItemSpec};
use crate::ribbon::theme::{self, Theme, STANDARD_COLORS, THEME_COLORS};

use super::layout::{chevron, lw, text, wrap_width, Ctx};
use super::scene::{Cmd, Fill, Node, Scene};
use super::state::{close_all_popups, close_popup, Popup, PopupKind, State, SwatchSel, Target};
use super::{activate, publish, RibbonEvent};

fn resolved(c: Option<&CommandRef>) -> Option<u32> {
    match c {
        Some(CommandRef::Id(id)) => Some(*id),
        _ => None,
    }
}

fn quad(scene: &mut Scene, rect: Rectangle, fill: Fill, border: Option<(Color, f32)>, radius: f32) {
    scene.push(Node::new(rect, Cmd::Quad { fill, border, radius }));
}

fn text_bold(scene: &mut Scene, rect: Rectangle, s: impl Into<String>, size: f32, color: Color, h: Horizontal, v: Vertical) {
    scene.push(Node::new(
        rect,
        Cmd::Text { text: s.into(), size, color, halign: h, valign: v, bold: true },
    ));
}

fn svg(scene: &mut Scene, rect: Rectangle, image: crate::ribbon::image::UiImage, opacity: f32) {
    scene.push(Node::new(rect, Cmd::Svg { image, tint: None, opacity }));
}

fn panel(scene: &mut Scene, size: Size, theme: &Theme) {
    quad(
        scene,
        Rectangle::with_size(size),
        Fill::Color(theme.popup_bg),
        Some((theme.popup_border, theme.border_scale)),
        2.0,
    );
}

fn hover_row(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, target: Target, enabled: bool) {
    if enabled && ctx.state.popup_hover() == Some(&target) {
        quad(
            scene,
            rect,
            Fill::Color(ctx.theme.hover_bg),
            ctx.theme.hover_border.map(|c| (c, lw(ctx, 1.0))),
            0.0,
        );
    }
}

/// Build the scene for the currently open popup; `None` if the popup is
/// no longer valid.
pub fn popup_scene(ctx: &Ctx, popup: &Popup) -> Option<Scene> {
    match &popup.kind {
        PopupKind::AppMenu => Some(app_menu_scene(ctx)),
        PopupKind::MinimizedTab(tab) => Some(minimized_tab_scene(ctx, *tab, popup.width_hint)),
        PopupKind::Menu { cmd, path } => {
            let spec = ctx.spec();
            let items = find_menu_items(spec, *cmd)?;
            Some(menu_scene(ctx, items, path, *cmd))
        }
        PopupKind::ContextMenu { cmd, path } => {
            let spec = ctx.spec();
            let popup_spec = spec.context_popups.iter().find(|p| resolved(p.command.as_ref()) == Some(*cmd))?;
            Some(menu_scene(ctx, &popup_spec.items, path, *cmd))
        }
        PopupKind::Combo { cmd } => Some(combo_scene(ctx, *cmd)),
        PopupKind::Gallery { cmd } => Some(gallery_scene(ctx, *cmd, popup.scroll)),
        PopupKind::Color { cmd } => Some(color_scene(ctx, *cmd)),
        PopupKind::Font { cmd } => Some(font_scene(ctx, *cmd, popup.scroll)),
        PopupKind::Overflow => Some(overflow_scene(ctx)),
        PopupKind::OverflowGroup { tab, group } => {
            Some(overflow_group_scene(ctx, *tab, *group))
        }
        PopupKind::QatMenu => Some(qat_menu_scene(ctx)),
    }
}

/// Find the menu items of a dropdown / split button command.
fn find_menu_items(
    spec: &crate::ribbon::spec::RibbonSpec,
    cmd: u32,
) -> Option<&[MenuItemSpec]> {
    for tab in &spec.tabs {
        for group in &tab.groups {
            if let Some(items) = in_controls(&group.controls, cmd) {
                return Some(items);
            }
        }
    }
    None
}

fn in_controls(
    controls: &[ControlSpec],
    cmd: u32,
) -> Option<&[MenuItemSpec]> {
    for control in controls {
        match control {
            ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => {
                if resolved(s.command.as_ref()) == Some(cmd) {
                    return Some(&s.items);
                }
                if let Some(found) = in_items(&s.items, cmd) {
                    return Some(found);
                }
            }
            ControlSpec::ControlGroup { controls } => {
                if let Some(found) = in_controls(controls, cmd) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn in_items(items: &[MenuItemSpec], cmd: u32) -> Option<&[MenuItemSpec]> {
    for item in items {
        if resolved(item.command.as_ref()) == Some(cmd) {
            return Some(&item.items);
        }
        if let Some(found) = in_items(&item.items, cmd) {
            return Some(found);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// menus

fn menu_row_width(ctx: &Ctx, items: &[MenuItemSpec]) -> f32 {
    let mut w = 120.0f32;
    for item in items {
        let Some(cmd) = resolved(item.command.as_ref()) else {
            continue;
        };
        let label = ctx.label(cmd);
        let mut row = 8.0 + 18.0 + 6.0 + ctx.text_w(&label, 12.0);
        if !item.items.is_empty() {
            row += 16.0;
        }
        w = w.max(row + 10.0);
    }
    w
}

/// Vertical menu panel; `path` opens sub-menu panels to the right.
pub fn menu_scene(ctx: &Ctx, items: &[MenuItemSpec], path: &[usize], base_cmd: u32) -> Scene {
    let w = menu_row_width(ctx, items);
    let mut scene = Scene::new(Size::ZERO);
    panel(&mut scene, Size::new(w, menu_height(ctx, items)), ctx.theme);
    draw_menu_level(ctx, &mut scene, items, Point::ORIGIN, w, path, 0, base_cmd);

    // sub-menus
    let mut origin = Point::new(w, 0.0);
    let mut level_items = items;
    for &idx in path {
        let Some(item) = level_items.get(idx) else { break };
        if item.items.is_empty() {
            break;
        }
        let row_y = menu_row_y(ctx, level_items, idx);
        let sw = menu_row_width(ctx, &item.items);
        panel(&mut scene, Size::new(sw, menu_height(ctx, &item.items)), ctx.theme);
        // move panel origin: draw_level adds origin; we emulate by offset node positions
        draw_menu_level_offset(
            ctx,
            &mut scene,
            &item.items,
            origin + Vector::new(0.0, row_y),
            sw,
            &[],
            0,
            base_cmd,
        );
        level_items = &item.items;
        origin += Vector::new(sw, row_y);
        let _ = origin;
    }

    scene.size = Size::new(
        w + if path.is_empty() {
            0.0
        } else {
            // width of the deepest sub-menu
            deepest_width(ctx, items, path)
        },
        scene_height(ctx, items, path),
    );
    scene
}

fn deepest_width(ctx: &Ctx, items: &[MenuItemSpec], path: &[usize]) -> f32 {
    let mut items = items;
    let mut total = 0.0;
    for &idx in path {
        let Some(item) = items.get(idx) else { break };
        let w = menu_row_width(ctx, &item.items);
        total += w;
        items = &item.items;
    }
    total
}

fn scene_height(ctx: &Ctx, items: &[MenuItemSpec], path: &[usize]) -> f32 {
    let mut h = menu_height(ctx, items);
    let mut items = items;
    for &idx in path {
        let Some(item) = items.get(idx) else { break };
        let row_y = menu_row_y(ctx, items, idx);
        h = h.max(row_y + menu_height(ctx, &item.items));
        items = &item.items;
    }
    h
}

fn menu_height(_ctx: &Ctx, items: &[MenuItemSpec]) -> f32 {
    let mut h = 4.0;
    for item in items {
        if item.separator_before {
            h += 4.0;
        }
        let _ = item;
        h += 25.0;
    }
    h + 2.0
}

fn menu_row_y(ctx: &Ctx, items: &[MenuItemSpec], idx: usize) -> f32 {
    let mut y = 2.0;
    for (i, item) in items.iter().enumerate() {
        if i == idx {
            return y;
        }
        if item.separator_before {
            y += 4.0;
        }
        y += 25.0;
    }
    let _ = ctx;
    y
}

// menu drawing walks (ctx, scene, layout constants) together — the
// arity mirrors the recursion state
#[allow(clippy::too_many_arguments)]
fn draw_menu_level(
    ctx: &Ctx,
    scene: &mut Scene,
    items: &[MenuItemSpec],
    origin: Point,
    w: f32,
    path: &[usize],
    level: usize,
    base_cmd: u32,
) {
    draw_menu_level_offset(ctx, scene, items, origin, w, path, level, base_cmd)
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_level_offset(
    ctx: &Ctx,
    scene: &mut Scene,
    items: &[MenuItemSpec],
    origin: Point,
    w: f32,
    path: &[usize],
    level: usize,
    _base_cmd: u32,
) {
    // icon column is always reserved; a vertical separator runs through
    // the whole menu dividing icons from text
    quad(
        scene,
        Rectangle::new(
            Point::new(origin.x + 26.0, origin.y + 2.0),
            Size::new(lw(ctx, 1.0), menu_height(ctx, items) - 4.0),
        ),
        Fill::Color(ctx.theme.group_separator),
        None,
        0.0,
    );
    let mut y = origin.y + 2.0;
    for (idx, item) in items.iter().enumerate() {
        if item.separator_before {
            quad(
                scene,
                Rectangle::new(
                    Point::new(origin.x + 4.0, y),
                    Size::new(w - 8.0, lw(ctx, 1.0)),
                ),
                Fill::Color(ctx.theme.group_separator),
                None,
                0.0,
            );
            y += 4.0;
        }
        let row = Rectangle::new(Point::new(origin.x + 1.0, y), Size::new(w - 2.0, 25.0));
        let Some(cmd) = resolved(item.command.as_ref()) else {
            y += 25.0;
            continue;
        };
        let enabled = ctx.enabled(cmd);
        let has_sub = !item.items.is_empty();
        let target = if has_sub {
            Target::SubMenu(idx, level)
        } else {
            Target::MenuItem(cmd)
        };
        hover_row(ctx, scene, row, target.clone(), true);
        scene.push(Node::new(row, Cmd::Hit).target(target).tip(cmd));
        // icon
        let image = item
            .image
            .as_ref()
            .map(|i| crate::ribbon::image::UiImage::Named(i.clone()))
            .or_else(|| ctx.small_image(cmd))
            .unwrap_or_default();
        svg(
            scene,
            Rectangle::new(
                Point::new(origin.x + 5.0, y + 4.5),
                Size::new(16.0, 16.0),
            ),
            image,
            if enabled { 1.0 } else { 0.35 },
        );
        // the renderer clips labels to this rectangle, so the box must
        // reach the panel's right padding (12px; submenu rows reserve
        // the 16px arrow column) or the last glyph is covered
        let right_pad = if has_sub || path.contains(&idx) { 26.0 } else { 12.0 };
        text(
            scene,
            Rectangle::new(
                Point::new(origin.x + 30.0, y),
                Size::new((w - 30.0 - right_pad).max(0.0), 25.0),
            ),
            ctx.label(cmd),
            ctx.fonts.ribbon.menu,
            if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
            Horizontal::Left,
            Vertical::Center,
        );
        if has_sub || path.contains(&idx) {
            svg(
                scene,
                Rectangle::new(
                    Point::new(origin.x + w - 16.0, y + 7.5),
                    Size::new(10.0, 10.0),
                ),
                crate::ribbon::image::UiImage::Icon("keytip_arrow"),
                if enabled { 0.7 } else { 0.3 },
            );
        }
        y += 25.0;
    }
}

// ---------------------------------------------------------------------------
// application menu

fn app_menu_scene(ctx: &Ctx) -> Scene {
    let spec = ctx.spec();
    let menu = spec.application_menu.clone().unwrap_or_default();
    let recent_cmd = menu.recent_items.as_ref().and_then(|c| resolved(Some(c)));

    let left_w = 175.0f32;
    let right_w = 165.0f32;
    let has_recent = recent_cmd.is_some();
    let width = left_w + if has_recent { right_w } else { 0.0 };

    let left_h = menu_height(ctx, &menu.items);
    let mut right_h = 30.0f32;
    if let Some(rc) = recent_cmd
        && let Ok(PropValue::Items(items)) = ctx.fw.get_ui_command_property(rc, PropertyKey::RecentItems)
    {
        right_h += items.len() as f32 * 36.0 + 4.0;
    }
    let height = left_h.max(right_h) + 4.0;

    let mut scene = Scene::new(Size::new(width, height));
    panel(&mut scene, Size::new(width, height), ctx.theme);

    // left column: commands
    draw_menu_level_offset(
        ctx,
        &mut scene,
        &menu.items,
        Point::ORIGIN,
        left_w,
        &[],
        0,
        0,
    );

    // right column: recent items
    if let Some(rc) = recent_cmd {
        let x = left_w + 4.0;
        quad(
            &mut scene,
            Rectangle::new(Point::new(left_w, 2.0), Size::new(lw(ctx, 1.0), height - 4.0)),
            Fill::Color(ctx.theme.group_separator),
            None,
            0.0,
        );
        let header = ctx.label(rc);
        text_bold(
            &mut scene,
            Rectangle::new(Point::new(x + 4.0, 6.0), Size::new(right_w - 10.0, 18.0)),
            header,
            ctx.fonts.ribbon.menu,
            ctx.theme.text,
            Horizontal::Left,
            Vertical::Center,
        );
        if let Ok(PropValue::Items(items)) =
            ctx.fw.get_ui_command_property(rc, PropertyKey::RecentItems)
        {
            // pinned first
            let mut order: Vec<usize> = (0..items.len()).collect();
            order.sort_by_key(|&i| {
                let pinned = items[i]
                    .lock()
                    .ok()
                    .and_then(|it| it.get_value(PropertyKey::Pinned))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (!pinned, i)
            });
            let mut y = 28.0f32;
            for orig in order {
                let item = &items[orig];
                let label = item
                    .lock()
                    .ok()
                    .and_then(|it| it.get_value(PropertyKey::Label))
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let desc = item
                    .lock()
                    .ok()
                    .and_then(|it| it.get_value(PropertyKey::LabelDescription))
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let pinned = item
                    .lock()
                    .ok()
                    .and_then(|it| it.get_value(PropertyKey::Pinned))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let row = Rectangle::new(
                    Point::new(x, y),
                    Size::new(right_w - 8.0, 34.0),
                );
                let t_item = Target::RecentItem(rc, orig);
                hover_row(ctx, &mut scene, row, t_item.clone(), true);
                scene.push(Node::new(row, Cmd::Hit).target(t_item).tip(rc));
                text(
                    &mut scene,
                    Rectangle::new(Point::new(x + 4.0, y + 2.0), Size::new(right_w - 34.0, 16.0)),
                    label,
                    ctx.fonts.ribbon.menu,
                    ctx.theme.text,
                    Horizontal::Left,
                    Vertical::Center,
                );
                text(
                    &mut scene,
                    Rectangle::new(Point::new(x + 4.0, y + 17.0), Size::new(right_w - 34.0, 12.0)),
                    desc,
                    ctx.fonts.ribbon.menu,
                    ctx.theme.group_label,
                    Horizontal::Left,
                    Vertical::Center,
                );
                // pin
                let pin = Rectangle::new(
                    Point::new(x + right_w - 22.0, y + 4.0),
                    Size::new(13.0, 13.0),
                );
                let t_pin = Target::RecentPin(rc, orig);
                hover_row(ctx, &mut scene, pin, t_pin.clone(), true);
                scene.push(Node::new(pin, Cmd::Hit).target(t_pin));
                svg(
                    &mut scene,
                    pin.shrink(1.0),
                    crate::ribbon::image::UiImage::Icon("pin"),
                    if pinned { 1.0 } else { 0.35 },
                );
                y += 36.0;
            }
        }
    }
    scene
}

// ---------------------------------------------------------------------------
// combo box

fn combo_scene(ctx: &Ctx, cmd: u32) -> Scene {
    let mut labels: Vec<String> = Vec::new();
    if let Some(col) = ctx
        .value(cmd, PropertyKey::ItemsSource)
        .and_then(|v| v.as_collection().cloned())
    {
        let Ok(col) = col.lock() else {
            return Scene::new(Size::new(120.0, 40.0));
        };
        for i in 0..col.get_count() {
            labels.push(col.label_of(i).unwrap_or_default());
        }
    }
    let w = labels
        .iter()
        .map(|l| ctx.text_w(l, 12.0) + 30.0)
        .fold(120.0f32, f32::max);
    let h = labels.len() as f32 * 24.0 + 4.0;
    let mut scene = Scene::new(Size::new(w, h));
    panel(&mut scene, Size::new(w, h), ctx.theme);
    let selected = ctx
        .value(cmd, PropertyKey::SelectedItem)
        .and_then(|v| v.as_u32())
        .unwrap_or(0) as usize;
    let mut y = 2.0;
    for (i, label) in labels.iter().enumerate() {
        let row = Rectangle::new(Point::new(1.0, y), Size::new(w - 2.0, 24.0));
        if i == selected {
            quad(&mut scene, row, Fill::Color(ctx.theme.selection), None, 2.0);
        }
        hover_row(ctx, &mut scene, row, Target::ComboItem(cmd, i), true);
        scene.push(Node::new(row, Cmd::Hit).target(Target::ComboItem(cmd, i)).tip(cmd));
        text(
            &mut scene,
            Rectangle::new(Point::new(10.0, y), Size::new(w - 18.0, 24.0)),
            label.clone(),
            ctx.fonts.ribbon.menu,
            if i == selected { Color::WHITE } else { ctx.theme.text },
            Horizontal::Left,
            Vertical::Center,
        );
        y += 24.0;
    }
    scene
}

// ---------------------------------------------------------------------------
// galleries

fn gallery_items(ctx: &Ctx, cmd: u32) -> Vec<(String, Option<crate::ribbon::image::UiImage>, u32)> {
    let mut out = Vec::new();
    if let Some(col) = ctx
        .value(cmd, PropertyKey::ItemsSource)
        .and_then(|v| v.as_collection().cloned())
    {
        let Ok(col) = col.lock() else {
            return Vec::new();
        };
        for i in 0..col.get_count() {
            let item = col.get_item(i);
            let label = item
                .as_ref()
                .and_then(|it| it.lock().ok()?.get_value(PropertyKey::Label))
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            let image = item
                .as_ref()
                .and_then(|it| it.lock().ok()?.get_value(PropertyKey::ItemImage))
                .and_then(|v| match v {
                    PropValue::Image(i) => Some(i),
                    _ => None,
                });
            let cat = item
                .as_ref()
                .and_then(|it| it.lock().ok()?.get_value(PropertyKey::CategoryId))
                .and_then(|v| v.as_u32())
                .unwrap_or(0);
            out.push((label, image, cat));
        }
    }
    out
}

fn gallery_spec(ctx: &Ctx, cmd: u32) -> crate::ribbon::spec::GallerySpec {
    // locate the gallery control to get its item size
    fn find(controls: &[ControlSpec], cmd: u32) -> Option<crate::ribbon::spec::GallerySpec> {
        for c in controls {
            match c {
                ControlSpec::InRibbonGallery(g) if resolved(g.command.as_ref()) == Some(cmd) => {
                    return Some(g.clone())
                }
                ControlSpec::ControlGroup { controls } => {
                    if let Some(g) = find(controls, cmd) {
                        return Some(g);
                    }
                }
                _ => {}
            }
        }
        None
    }
    let spec = ctx.spec();
    for tab in &spec.tabs {
        for group in &tab.groups {
            if let Some(g) = find(&group.controls, cmd) {
                return g;
            }
        }
    }
    crate::ribbon::spec::GallerySpec::default()
}

fn gallery_scene(ctx: &Ctx, cmd: u32, scroll: usize) -> Scene {
    let g = gallery_spec(ctx, cmd);
    let items = gallery_items(ctx, cmd);
    let cols = g.columns.max(1);
    // caption below the icon: the widest caption drives the cell width
    let text_w = items
        .iter()
        .map(|(l, _, _)| ctx.text_w(l, ctx.fonts.ribbon.menu))
        .fold(0.0f32, f32::max);
    let cell_w = (g.item_width + 10.0).max(text_w + 8.0);
    let cell_h = g.item_height + 16.0;
    let header_h = 18.0f32;

    // categories
    let categories: Vec<(u32, String)> = ctx
        .value(cmd, PropertyKey::Categories)
        .and_then(|v| v.as_collection().cloned())
        .map(|col| {
            let Ok(col) = col.lock() else { return Vec::new() };
            (0..col.get_count())
                .filter_map(|i| {
                    let id = col
                        .get_item(i)
                        .and_then(|it| it.lock().ok()?.get_value(PropertyKey::CategoryId))
                        .and_then(|v| v.as_u32());
                    let label = col.label_of(i);
                    id.zip(label)
                })
                .collect()
        })
        .unwrap_or_default();

    // rows: (kind, index/category)
    enum Row {
        Header(u32),
        Items(usize, u32), // start index, category
    }
    let mut rows: Vec<Row> = Vec::new();
    let cats: Vec<u32> = if categories.is_empty() {
        vec![0]
    } else {
        categories.iter().map(|(id, _)| *id).collect()
    };
    for cat in &cats {
        if !categories.is_empty() {
            rows.push(Row::Header(*cat));
        }
        let count = items.iter().filter(|(_, _, c)| c == cat).count();
        let nrows = count.div_ceil(cols);
        for r in 0..nrows {
            rows.push(Row::Items(r, *cat));
        }
    }

    let visible_rows = (320.0f32 / cell_h).floor().max(2.0) as usize;
    let scroll = scroll.min(rows.len().saturating_sub(visible_rows));
    let shown = &rows[scroll.min(rows.len())..(scroll + visible_rows).min(rows.len())];

    let width = cols as f32 * cell_w + 14.0;
    let height = shown.len() as f32 * cell_h
        + if categories.is_empty() { 4.0 } else { rows.iter().filter(|r| matches!(r, Row::Header(_))).count() as f32 * 0.0 }
        + 6.0;
    let height = height.min(330.0);

    let mut scene = Scene::new(Size::new(width, height));
    panel(&mut scene, Size::new(width, height), ctx.theme);

    let mut y = 3.0f32;
    // map item index per category
    let mut cat_offsets: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, (_, _, cat)) in items.iter().enumerate() {
        cat_offsets.entry(*cat).or_default().push(i);
    }
    let selected = ctx
        .value(cmd, PropertyKey::SelectedItem)
        .and_then(|v| v.as_u32())
        .unwrap_or(0) as usize;

    for row in shown {
        match row {
            Row::Header(cat) => {
                let label = categories
                    .iter()
                    .find(|(id, _)| id == cat)
                    .map(|(_, l)| l.clone())
                    .unwrap_or_default();
                text_bold(
                    &mut scene,
                    Rectangle::new(Point::new(8.0, y), Size::new(width - 16.0, header_h)),
                    label,
                    ctx.fonts.ribbon.menu,
                    ctx.theme.group_label,
                    Horizontal::Left,
                    Vertical::Center,
                );
                y += header_h;
            }
            Row::Items(r, cat) => {
                let idxs = cat_offsets.get(cat).cloned().unwrap_or_default();
                for c in 0..cols {
                    let Some(&idx) = idxs.get(r * cols + c) else { continue };
                    let (label, image, _) = &items[idx];
                    let x = 7.0 + c as f32 * cell_w;
                    let cell = Rectangle::new(
                        Point::new(x, y),
                        Size::new(cell_w - 4.0, cell_h - 4.0),
                    );
                    if idx == selected {
                        quad(&mut scene, cell, Fill::Color(ctx.theme.checked_bg), ctx.theme.checked_border.map(|c| (c, lw(ctx, 1.0))), 2.0);
                    }
                    hover_row(ctx, &mut scene, cell, Target::GalleryItem { cmd, index: idx, popup: true }, true);
                    scene.push(Node::new(cell, Cmd::Hit).target(Target::GalleryItem { cmd, index: idx, popup: true }).tip(cmd));
                    if let Some(image) = image {
                        svg(
                            &mut scene,
                            Rectangle::new(Point::new(x + 2.0, y + 1.0), Size::new(g.item_width, g.item_height)),
                            image.clone(),
                            1.0,
                        );
                    }
                    text(
                        &mut scene,
                        Rectangle::new(Point::new(x - 1.0, y + g.item_height + 2.0), Size::new(cell_w - 2.0, 12.0)),
                        label.clone(),
                        ctx.fonts.ribbon.menu,
                        ctx.theme.text,
                        Horizontal::Center,
                        Vertical::Center,
                    );
                }
                y += cell_h;
            }
        }
    }

    // scroll arrows
    if rows.len() > visible_rows {
        let up = Rectangle::new(Point::new(width - 20.0, 3.0), Size::new(16.0, 14.0));
        let down = Rectangle::new(Point::new(width - 20.0, height - 17.0), Size::new(16.0, 14.0));
        for (z, delta, up_arrow) in [(up, -1i32, true), (down, 1, false)] {
            let can = if delta < 0 { scroll > 0 } else { scroll + visible_rows < rows.len() };
            if can {
                quad(&mut scene, z, Fill::Color(ctx.theme.qat_bg.0), Some((ctx.theme.border, lw(ctx, 1.0))), 2.0);
                chevron(
                    &mut scene,
                    Rectangle::new(Point::new(z.x + 3.0, z.y + 3.5), Size::new(10.0, 7.0)),
                    up_arrow,
                    ctx.theme.text,
                );
                scene.push(Node::new(z, Cmd::Hit).target(Target::GalleryScrollV(cmd, delta)));
            }
        }
    }
    scene
}

// ---------------------------------------------------------------------------
// color picker

fn color_scene(ctx: &Ctx, cmd: u32) -> Scene {
    let width = 212.0f32;
    let swatch = 18.0f32;
    let gap = 3.0f32;
    let monochrome = matches!(
        ctx.value(cmd, PropertyKey::ColorMode),
        Some(PropValue::ColorMode(SwatchColorMode::Monochrome))
    );
    let theme_colors = ctx
        .value(cmd, PropertyKey::ThemeColors)
        .and_then(|v| match v {
            PropValue::ColorList(c) => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| THEME_COLORS.to_vec());
    let standard_colors = ctx
        .value(cmd, PropertyKey::StandardColors)
        .and_then(|v| match v {
            PropValue::ColorList(c) => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| STANDARD_COLORS.to_vec());
    let recent = ctx
        .state
        .color_recent
        .get(&cmd)
        .cloned()
        .unwrap_or_default();

    let mut rows_h = 4.0;
    rows_h += 22.0; // automatic
    if !monochrome {
        rows_h += 22.0; // no color
    }
    rows_h += 16.0 + ((theme_colors.len() as f32 / 10.0).ceil() * (swatch + gap));
    rows_h += 16.0 + ((standard_colors.len() as f32 / 10.0).ceil() * (swatch + gap));
    if !recent.is_empty() {
        rows_h += 16.0 + swatch + gap;
    }
    rows_h += 22.0; // more colors
    rows_h += 4.0;

    let mut scene = Scene::new(Size::new(width, rows_h));
    panel(&mut scene, Size::new(width, rows_h), ctx.theme);

    let mut y = 3.0f32;
    let enabled = ctx.enabled(cmd);

    // automatic
    let row = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, 21.0));
    let t = Target::ColorSwatch { cmd, sel: SwatchSel::Automatic };
    hover_row(ctx, &mut scene, row, t.clone(), enabled);
    scene.push(Node::new(row, Cmd::Hit).target(t).tip(cmd));
    text(&mut scene, Rectangle::new(Point::new(10.0, y), Size::new(16.0, 21.0)), "A", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
    let auto_label = ctx.str_prop(cmd, PropertyKey::AutomaticColorLabel).unwrap_or_else(|| "Automatic".into());
    text(&mut scene, Rectangle::new(Point::new(28.0, y), Size::new(width - 40.0, 21.0)), auto_label, ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
    y += 22.0;

    if !monochrome {
        let row = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, 21.0));
        let t = Target::ColorSwatch { cmd, sel: SwatchSel::NoColor };
        hover_row(ctx, &mut scene, row, t.clone(), enabled);
        scene.push(Node::new(row, Cmd::Hit).target(t).tip(cmd));
        text(&mut scene, Rectangle::new(Point::new(10.0, y), Size::new(16.0, 21.0)), "Ø", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
        let label = ctx.str_prop(cmd, PropertyKey::NoColorLabel).unwrap_or_else(|| "No Color".into());
        text(&mut scene, Rectangle::new(Point::new(28.0, y), Size::new(width - 40.0, 21.0)), label, ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
        y += 22.0;
    }

    let current = ctx.value(cmd, PropertyKey::Color).and_then(|v| v.as_color());

    let grid = |scene: &mut Scene,
                    y: f32,
                    colors: &[u32],
                    sel_of: &dyn Fn(usize) -> SwatchSel| {
        for (i, c) in colors.iter().enumerate() {
            let x = 8.0 + (i % 10) as f32 * (swatch + gap);
            let yy = y + (i / 10) as f32 * (swatch + gap);
            let rect = Rectangle::new(Point::new(x, yy), Size::new(swatch, swatch));
            let t = Target::ColorSwatch { cmd, sel: sel_of(i) };
            let hovered = enabled && ctx.state.popup_hover() == Some(&t);
            if hovered {
                quad(scene, rect.expand(1.5), Fill::Color(ctx.theme.hover_bg), None, 2.0);
            }
            quad(scene, rect, Fill::Color(theme::rgb(*c)), Some((ctx.theme.border, 0.75)), ctx.theme.field_radius);
            if current == Some(*c) {
                quad(scene, rect.expand(2.0), Fill::Color(Color::TRANSPARENT), Some((ctx.theme.text, 1.25)), 2.0);
            }
            scene.push(Node::new(rect, Cmd::Hit).target(t).tip(cmd));
        }
    };

    let theme_label = ctx.str_prop(cmd, PropertyKey::ThemeColorsCategoryLabel).unwrap_or_else(|| "Theme Colors".into());
    text_bold(&mut scene, Rectangle::new(Point::new(8.0, y), Size::new(width - 16.0, 14.0)), theme_label, ctx.fonts.ribbon.menu, ctx.theme.group_label, Horizontal::Left, Vertical::Center);
    y += 16.0;
    let tc = theme_colors.clone();
    grid(&mut scene, y, &tc, &|i| SwatchSel::Theme(i));
    y += (theme_colors.len() as f32 / 10.0).ceil() * (swatch + gap) + 2.0;

    let std_label = ctx.str_prop(cmd, PropertyKey::StandardColorsCategoryLabel).unwrap_or_else(|| "Standard Colors".into());
    text_bold(&mut scene, Rectangle::new(Point::new(8.0, y), Size::new(width - 16.0, 14.0)), std_label, ctx.fonts.ribbon.menu, ctx.theme.group_label, Horizontal::Left, Vertical::Center);
    y += 16.0;
    let sc = standard_colors.clone();
    grid(&mut scene, y, &sc, &|i| SwatchSel::Standard(i));
    y += (standard_colors.len() as f32 / 10.0).ceil() * (swatch + gap) + 2.0;

    if !recent.is_empty() {
        let label = ctx.str_prop(cmd, PropertyKey::RecentColorsCategoryLabel).unwrap_or_else(|| "Recent Colors".into());
        text_bold(&mut scene, Rectangle::new(Point::new(8.0, y), Size::new(width - 16.0, 14.0)), label, ctx.fonts.ribbon.menu, ctx.theme.group_label, Horizontal::Left, Vertical::Center);
        y += 16.0;
        let rc = recent.clone();
        grid(&mut scene, y, &rc, &|i| SwatchSel::Recent(i));
        y += swatch + gap + 2.0;
    }

    let more = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, 21.0));
    let t = Target::ColorSwatch { cmd, sel: SwatchSel::MoreColors };
    hover_row(ctx, &mut scene, more, t.clone(), enabled);
    scene.push(Node::new(more, Cmd::Hit).target(t).tip(cmd));
    let label = ctx.str_prop(cmd, PropertyKey::MoreColorsLabel).unwrap_or_else(|| "More Colors…".into());
    text(&mut scene, Rectangle::new(Point::new(10.0, y), Size::new(width - 20.0, 21.0)), label, ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);

    scene
}

// ---------------------------------------------------------------------------
// font control

pub const FONT_FAMILIES: &[&str] = &[
    "Arial", "Calibri", "Cambria", "Comic Sans MS", "Consolas", "Courier New",
    "Georgia", "Tahoma", "Times New Roman", "Verdana",
];

pub const FONT_SIZES: &[&str] =
    &["8", "9", "10", "11", "12", "14", "16", "18", "20", "24", "28", "36", "48", "72"];

fn font_scene(ctx: &Ctx, cmd: u32, scroll: usize) -> Scene {
    let width = 252.0f32;
    let height = 208.0f32;
    let mut scene = Scene::new(Size::new(width, height));
    panel(&mut scene, Size::new(width, height), ctx.theme);

    let props = ctx
        .value(cmd, PropertyKey::FontProperties)
        .and_then(|v| v.as_font().cloned())
        .unwrap_or_default();

    // families
    let list_h = height - 34.0;
    let row_h = 20.0f32;
    let visible = (list_h / row_h).floor() as usize;
    let scroll = scroll.min(FONT_FAMILIES.len().saturating_sub(visible));
    for (row, family) in FONT_FAMILIES.iter().enumerate().skip(scroll).take(visible) {
        let y = 3.0 + (row - scroll) as f32 * row_h;
        let rect = Rectangle::new(Point::new(3.0, y), Size::new(150.0, row_h));
        let t = Target::FontFamily(cmd, family.to_string());
        let selected = props.family.as_deref() == Some(family);
        if selected {
            quad(&mut scene, rect, Fill::Color(ctx.theme.selection), None, 2.0);
        }
        hover_row(ctx, &mut scene, rect, t.clone(), true);
        scene.push(Node::new(rect, Cmd::Hit).target(t).tip(cmd));
        text(
            &mut scene,
            Rectangle::new(Point::new(9.0, y), Size::new(140.0, row_h)),
            *family,
            if selected { 12.0 } else { 11.5 },
            if selected { Color::WHITE } else { ctx.theme.text },
            Horizontal::Left,
            Vertical::Center,
        );
    }
    // sizes
    for (row, size) in FONT_SIZES.iter().enumerate().skip(scroll).take(visible) {
        let y = 3.0 + (row - scroll) as f32 * 18.0;
        let rect = Rectangle::new(Point::new(157.0, y), Size::new(48.0, 18.0));
        let t = Target::FontSize(cmd, size.to_string());
        let selected = props.size.map(|s| s.to_string()) == Some(size.to_string());
        if selected {
            quad(&mut scene, rect, Fill::Color(ctx.theme.selection), None, 2.0);
        }
        hover_row(ctx, &mut scene, rect, t.clone(), true);
        scene.push(Node::new(rect, Cmd::Hit).target(t).tip(cmd));
        text(
            &mut scene,
            Rectangle::new(Point::new(161.0, y), Size::new(42.0, 18.0)),
            *size,
            ctx.fonts.ribbon.menu,
            if selected { Color::WHITE } else { ctx.theme.text },
            Horizontal::Left,
            Vertical::Center,
        );
    }

    // divider
    quad(
        &mut scene,
        Rectangle::new(Point::new(211.0, 3.0), Size::new(lw(ctx, 1.0), height - 32.0)),
        Fill::Color(ctx.theme.group_separator),
        None,
        0.0,
    );

    // toggles
    for (i, tgl) in FontToggle::ALL.iter().enumerate() {
        let rect = Rectangle::new(
            Point::new(216.0, 4.0 + i as f32 * 24.0),
            Size::new(22.0, 22.0),
        );
        let t = Target::FontToggle(cmd, *tgl);
        let checked = tgl.is_set(&props);
        let hover = ctx.state.popup_hover() == Some(&t);
        quad(
            &mut scene,
            rect,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else if checked {
                Fill::Color(ctx.theme.checked_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            Some((
                if hover {
                    ctx.theme.hover_border.unwrap_or(ctx.theme.border)
                } else if checked {
                    ctx.theme.checked_border.unwrap_or(ctx.theme.border)
                } else {
                    ctx.theme.border
                },
                1.0,
            )),
            ctx.theme.button_radius,
        );
        let bold = *tgl == FontToggle::Bold || *tgl == FontToggle::Italic;
        scene.push(Node::new(
            rect,
            Cmd::Text {
                text: tgl.label().into(),
                size: 12.0,
                color: ctx.theme.text,
                halign: Horizontal::Center,
                valign: Vertical::Center,
                bold,
            },
        ));
        scene.push(Node::new(rect, Cmd::Hit).target(t).tip(cmd));
    }

    // bottom row: grow/shrink
    let grow = Rectangle::new(Point::new(4.0, height - 26.0), Size::new(22.0, 22.0));
    let shrink = Rectangle::new(Point::new(28.0, height - 26.0), Size::new(22.0, 22.0));
    quad(&mut scene, grow, Fill::Color(Color::TRANSPARENT), Some((ctx.theme.border, lw(ctx, 1.0))), ctx.theme.button_radius);
    quad(&mut scene, shrink, Fill::Color(Color::TRANSPARENT), Some((ctx.theme.border, lw(ctx, 1.0))), ctx.theme.button_radius);
    text(&mut scene, grow, "A↑", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Center, Vertical::Center);
    text(&mut scene, shrink, "A↓", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Center, Vertical::Center);
    scene.push(Node::new(grow, Cmd::Hit).target(Target::FontGrow(cmd)).tip(cmd));
    scene.push(Node::new(shrink, Cmd::Hit).target(Target::FontShrink(cmd)).tip(cmd));

    scene
}

// ---------------------------------------------------------------------------
// overflow

fn overflow_scene(ctx: &Ctx) -> Scene {
    let Some(tab) = *ctx.state.overflow_tab.borrow() else {
        return Scene::new(Size::new(120.0, 40.0));
    };
    let hidden = ctx.state.overflow_hidden.take().unwrap_or_default();
    ctx.state.overflow_hidden.replace(Some(hidden.clone()));
    let spec = ctx.spec();
    let mut rows: Vec<(String, Option<crate::ribbon::image::UiImage>, usize)> = Vec::new();
    for gi in &hidden {
        let group = &spec.tabs[tab].groups[*gi];
        let label = group.label.clone().unwrap_or_else(|| group.id.clone());
        let image = group
            .image
            .as_ref()
            .map(|i| crate::ribbon::image::UiImage::Named(i.clone()));
        rows.push((label, image, *gi));
    }
    let w = rows
        .iter()
        .map(|(l, _, _)| ctx.text_w(l, 12.0) + 44.0)
        .fold(140.0f32, f32::max);
    let h = rows.len() as f32 * 26.0 + 4.0;
    let mut scene = Scene::new(Size::new(w, h));
    panel(&mut scene, Size::new(w, h), ctx.theme);
    let mut y = 2.0;
    for (label, image, gi) in &rows {
        let row = Rectangle::new(Point::new(1.0, y), Size::new(w - 2.0, 26.0));
        let t = Target::OverflowGroup(tab, *gi);
        hover_row(ctx, &mut scene, row, t.clone(), true);
        scene.push(Node::new(row, Cmd::Hit).target(t));
        if let Some(img) = image {
            svg(&mut scene, Rectangle::new(Point::new(8.0, y + 5.0), Size::new(16.0, 16.0)), img.clone(), 1.0);
        }
        text(&mut scene, Rectangle::new(Point::new(30.0, y), Size::new(w - 38.0, 26.0)), label.clone(), ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
        y += 26.0;
    }
    scene
}

/// Popup of a collapsed group (Windows `Group Size="Popup"`): the
/// group's full ribbon layout — controls in their declared sizes,
/// caption footer, dialog launcher — just moved into the popup panel,
/// exactly like the Windows ribbon does.
fn overflow_group_scene(ctx: &Ctx, tab: usize, group: usize) -> Scene {
    let spec = ctx.spec();
    let g = &spec.tabs[tab].groups[group];
    let width = super::layout::measure_group_pub(ctx, g).max(150.0);
    let height = super::layout::body_h(ctx) + 8.0;
    let mut scene = Scene::new(Size::new(width, height));
    panel(&mut scene, Size::new(width, height), ctx.theme);
    // Same group body as in the ribbon, at its declared (unscaled)
    // sizes.
    super::layout::build_group_pub(
        ctx,
        &mut scene,
        g,
        4.0,
        4.0,
        width - 8.0,
        tab,
        group,
    );
    scene
}

// ---------------------------------------------------------------------------
// QAT customize menu

fn qat_menu_scene(ctx: &Ctx) -> Scene {
    let width = 250.0f32;
    let commands = ctx.fw.commands();
    let title_h = 24.0f32;
    let row_h = 22.0f32;
    let extra = 2; // dock row + separator + minimize row
    let h = title_h
        + commands.len() as f32 * row_h
        + extra as f32 * row_h
        + 6.0;
    let h = h.min(400.0);
    let mut scene = Scene::new(Size::new(width, h));
    panel(&mut scene, Size::new(width, h), ctx.theme);

    text_bold(
        &mut scene,
        Rectangle::new(Point::new(8.0, 2.0), Size::new(width - 16.0, title_h)),
        "Customize Quick Access Toolbar",
        ctx.fonts.ribbon.menu,
        ctx.theme.text,
        Horizontal::Left,
        Vertical::Center,
    );

    let in_qat = ctx.view().qat_commands();
    // only actionable commands with a label belong in the QAT list
    let commands: Vec<u32> = commands
        .into_iter()
        .filter(|c| {
            matches!(
                ctx.fw.command_type(*c),
                Some(crate::ribbon::enums::CommandType::Action)
                    | Some(crate::ribbon::enums::CommandType::Boolean)
            ) && ctx.value(*c, PropertyKey::Label).is_some()
        })
        .collect();
    let max_rows = ((h - title_h - extra as f32 * row_h - 6.0) / row_h).floor() as usize;
    let mut y = title_h;
    for (i, cmd) in commands.iter().enumerate() {
        if i >= max_rows {
            break;
        }
        let row = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, row_h));
        let t = Target::QatMenuItem(*cmd);
        hover_row(ctx, &mut scene, row, t.clone(), true);
        scene.push(Node::new(row, Cmd::Hit).target(t).tip(*cmd));
        // checkbox
        let cb = Rectangle::new(Point::new(10.0, y + 5.0), Size::new(12.0, 12.0));
        quad(&mut scene, cb, Fill::Color(Color::WHITE), Some((ctx.theme.border, lw(ctx, 1.0))), ctx.theme.field_radius);
        if in_qat.contains(cmd) {
            text(&mut scene, cb, "✓", ctx.fonts.ribbon.menu, ctx.theme.selection, Horizontal::Center, Vertical::Center);
        }
        text(&mut scene, Rectangle::new(Point::new(28.0, y), Size::new(width - 36.0, row_h)), ctx.label(*cmd), ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
        y += row_h;
    }

    // separator
    quad(&mut scene, Rectangle::new(Point::new(6.0, y + 2.0), Size::new(width - 12.0, lw(ctx, 1.0))), Fill::Color(ctx.theme.group_separator), None, 0.0);
    y += 5.0;

    // dock toggle
    let row = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, row_h));
    let t = Target::QatDockToggle;
    hover_row(ctx, &mut scene, row, t.clone(), true);
    scene.push(Node::new(row, Cmd::Hit).target(t));
    let cb = Rectangle::new(Point::new(10.0, y + 5.0), Size::new(12.0, 12.0));
    quad(&mut scene, cb, Fill::Color(Color::WHITE), Some((ctx.theme.border, lw(ctx, 1.0))), 1.0);
    if ctx.view().qat_dock() == crate::ribbon::enums::ControlDock::Bottom {
        text(&mut scene, cb, "✓", ctx.fonts.ribbon.menu, ctx.theme.selection, Horizontal::Center, Vertical::Center);
    }
    text(&mut scene, Rectangle::new(Point::new(28.0, y), Size::new(width - 36.0, row_h)), "Show Below the Ribbon", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);
    y += row_h;

    // minimize row
    let row = Rectangle::new(Point::new(4.0, y), Size::new(width - 8.0, row_h));
    let t = Target::Minimize;
    hover_row(ctx, &mut scene, row, t.clone(), true);
    scene.push(Node::new(row, Cmd::Hit).target(t));
    let cb = Rectangle::new(Point::new(10.0, y + 5.0), Size::new(12.0, 12.0));
    quad(&mut scene, cb, Fill::Color(Color::WHITE), Some((ctx.theme.border, lw(ctx, 1.0))), 1.0);
    if ctx.view().minimized() {
        text(&mut scene, cb, "✓", ctx.fonts.ribbon.menu, ctx.theme.selection, Horizontal::Center, Vertical::Center);
    }
    text(&mut scene, Rectangle::new(Point::new(28.0, y), Size::new(width - 36.0, row_h)), "Minimize the Ribbon", ctx.fonts.ribbon.menu, ctx.theme.text, Horizontal::Left, Vertical::Center);

    scene
}

// ---------------------------------------------------------------------------
// minimized tab body

/// Vertical overlap of the minimized-tab popup with the tab strip
/// bottom (the popup origin sits this many px above the strip's
/// bottom edge, see the `Tab` handler in widget/mod.rs).
pub(super) const MINIMIZED_POPUP_OVERLAP: f32 = 2.0;

/// Y position of the connecting hairline in the minimized-tab popup
/// scene: at the scene's top edge, above the 2px strip overlap.
const MINIMIZED_TAB_LINE_Y: f32 = 0.0;

fn minimized_tab_scene(ctx: &Ctx, tab: usize, width: f32) -> Scene {
    let mut scene = Scene::new(Size::new(width, super::layout::body_h(ctx)));
    // The popup is exactly window-wide; its content follows the same
    // window-too-narrow cascade as the expanded ribbon body
    // (`plan_groups`): groups shrink right-to-left and overflow into
    // the « popup instead of overflowing the window edge.
    super::layout::tab_content(ctx, &mut scene, 0.0, width, tab);
    // Hairline under the tab strip, connecting the popped-out pane to
    // the clicked tab: the tab's contextual accent, else the plain
    // strip line color (same rule as the connected tab outline).
    let tab_spec = &ctx.spec().tabs[tab];
    let color = tab_spec
        .contextual
        .as_deref()
        .and_then(theme::ContextualColor::from_name)
        .or_else(|| {
            tab_spec
                .context_color
                .as_deref()
                .and_then(theme::ContextualColor::from_name)
        })
        .map_or(ctx.theme.tab_line, |c| c.accent());
    quad(
        &mut scene,
        Rectangle::new(
            Point::new(0.0, MINIMIZED_TAB_LINE_Y),
            Size::new(width, lw(ctx, 1.0)),
        ),
        Fill::Color(color),
        None,
        0.0,
    );
    scene
}

// ---------------------------------------------------------------------------
// tooltip

/// Tooltip panel, local coordinates; the widget positions it below the
/// command pane at the related button's left side.
pub fn tooltip_scene(ctx: &Ctx, cmd: u32) -> Scene {
    let (title, desc) = ctx.tooltip(cmd);
    let width = 220.0f32;
    let desc_lines = if desc.is_empty() {
        Vec::new()
    } else {
        wrap_width(ctx, &desc, ctx.fonts.ribbon.menu, width - 20.0)
    };
    let keytip = ctx.keytip(cmd);
    let key_line = if ctx.state.keytips { None } else { keytip.map(|k| format!("Alt, {k}")) };
    let mut h = 24.0 + desc_lines.len() as f32 * 13.0 + 4.0;
    if key_line.is_some() {
        h += 15.0;
    }
    h += 4.0;

    let origin = Point::ORIGIN;
    let mut scene = Scene::new(Size::new(width, h));
    let panel_rect = Rectangle::new(origin, Size::new(width, h));
    scene.push(Node::new(
        panel_rect,
        Cmd::Quad {
            fill: Fill::Color(ctx.theme.tooltip_bg),
            border: Some((ctx.theme.tooltip_border, lw(ctx, 1.0))),
            radius: 2.0,
        },
    ));
    text_bold(
        &mut scene,
        Rectangle::new(
            Point::new(origin.x + 8.0, origin.y + 3.0),
            Size::new(width - 16.0, 18.0),
        ),
        title,
        ctx.fonts.ribbon.menu,
        ctx.theme.text,
        Horizontal::Left,
        Vertical::Center,
    );
    for (i, line) in desc_lines.iter().enumerate() {
        text(
            &mut scene,
            Rectangle::new(
                Point::new(origin.x + 8.0, origin.y + 22.0 + i as f32 * 13.0),
                Size::new(width - 16.0, 13.0),
            ),
            line.clone(),
            ctx.fonts.ribbon.menu,
            ctx.theme.group_label,
            Horizontal::Left,
            Vertical::Center,
        );
    }
    if let Some(k) = key_line {
        text(
            &mut scene,
            Rectangle::new(
                Point::new(origin.x + 8.0, origin.y + h - 18.0),
                Size::new(width - 16.0, 14.0),
            ),
            k,
            9.5,
            ctx.theme.group_label,
            Horizontal::Left,
            Vertical::Center,
        );
    }
    scene.size = Size::new(width, h);
    scene
}

// ---------------------------------------------------------------------------
// iced overlay

/// The overlay hosts one popup (or a tooltip) and its scene.
pub struct RibbonOverlay<'a, Message, Th, Renderer> {
    pub fw: &'a RibbonFramework,
    pub theme: &'a Theme,
    pub state: &'a mut State,
    pub scene: Scene,
    pub tooltip: bool,
    pub origin: Point,
    /// Width of the clicked anchor element (0 = point-like anchor).
    pub width_hint: f32,
    /// The overlay's own layout, computed at construction from the
    /// window size (the new Overlay trait has no layout phase).
    pub layout: iced::advanced::Layout,
    pub window: Size,
    /// Opacity multiplier (1.0 = opaque); the unfocused-window popup
    /// fade-out animates this (SPEC/Unfocused_Window.md §3).
    pub alpha: f32,
    pub mapper: Option<&'a dyn Fn(RibbonEvent) -> Message>,
    pub _phantom: std::marker::PhantomData<(Th, Renderer)>,
}

impl<'a, Message, Th, Renderer> RibbonOverlay<'a, Message, Th, Renderer> {
    /// Compute the flipped position and store the layout (the window
    /// size comes from Widget::overlay's `window` parameter).
    pub fn position(&mut self) {
        let size = self.scene.size;
        let bounds = self.window;
        let mut pos = self.origin;
        if pos.x + size.width > bounds.width - 2.0 {
            // out of the window on the right: align the menu's right
            // border with the clicked element's right border
            let right_aligned = self.origin.x + self.width_hint - size.width;
            pos.x = if right_aligned >= 2.0 {
                right_aligned
            } else {
                (bounds.width - size.width - 2.0).max(2.0)
            };
        }
        if pos.y + size.height > bounds.height - 2.0 {
            // flip above the origin if there is more space
            let above = self.origin.y - size.height;
            if above > 0.0 {
                pos.y = above.max(2.0);
            } else {
                pos.y = (bounds.height - size.height - 2.0).max(2.0);
            }
        }
        self.origin = pos;
        self.layout = iced::advanced::Layout::new(size).move_to(pos);
    }
}

impl<'a, Message, Th, Renderer> overlay::Overlay<Message, Th, Renderer>
    for RibbonOverlay<'a, Message, Th, Renderer>
where
    Renderer: iced::advanced::Renderer
        + iced::advanced::text::Renderer
        + iced::advanced::svg::Renderer
        + iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>,
    Message: Clone,
{
    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Th,
        _style: &renderer::Style,
        _cursor: mouse::Cursor,
    ) {
        super::draw_scene(
            &self.scene,
            renderer,
            self.layout.position(),
            &self.layout.bounds(),
            self.theme,
            self.alpha,
        );
    }

    fn update(
        &mut self,
        event: &iced::Event,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let layout = self.layout;
        // no input while showing a tooltip or fading out on focus loss
        if self.tooltip || self.state.popup_fade_start.is_some() {
            return;
        }
        if self.state.popup.is_none() {
            return;
        }
        let event = event.clone();
        let origin = layout.position();
        let local = |p: Point| Point::new(p.x - origin.x, p.y - origin.y);

        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let target =
                    self.scene.hit(local(position)).and_then(|n| n.target.clone());
                if let Some(p) = self.state.popup.as_mut() {
                    if p.hover != target {
                        shell.request_redraw();
                    }
                    p.hover = target.clone();
                }
                // preview on gallery item hover
                self.handle_preview(&target, shell);
                if let Some(node) = self.scene.hit(local(position))
                    && let Some(tip) = node.tip
                {
                    if self.state.tooltip != Some(tip) {
                        shell.request_redraw();
                    }
                    self.state.tooltip = Some(tip);
                    self.state.tooltip_anchor =
                        Some(super::translate_rect(node.rect, origin));
                }
                shell.capture_event();
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position() else {
                    return;
                };
                self.press_at(position, origin, layout.bounds(), shell);
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => {
                self.press_at(position, origin, layout.bounds(), shell);
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let Some(position) = cursor.position() else {
                    return;
                };
                self.release_at(position, origin, layout.bounds(), shell);
            }
            iced::Event::Touch(iced::touch::Event::FingerLifted { position, .. }) => {
                self.release_at(position, origin, layout.bounds(), shell);
            }
            iced::Event::Touch(iced::touch::Event::FingerLost { .. }) => {
                self.state.pressed = None;
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(position) = cursor.position() else {
                    return;
                };
                let hit = self.scene.hit(local(position)).and_then(|n| n.target.clone());
                if let Some(Target::GalleryItem { cmd, .. } | Target::GalleryScrollV(cmd, _)) = hit
                {
                    let dy = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y,
                        mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
                    };
                    if dy.abs() > 0.01 {
                        let delta = if dy > 0.0 { -1 } else { 1 };
                        if let Some(p) = self.state.popup.as_mut() {
                            p.scroll = (p.scroll as i32 + delta).max(0) as usize;
                        }
                        let _ = cmd;
                    }
                }
                shell.capture_event();
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if key == keyboard::key::Key::Named(keyboard::key::Named::Escape) {
                    close_popup(self.state);
                    self.state.previewed = None;
                    shell.request_redraw();
                    return;
                }
                shell.capture_event();
            }
            // window events (focus loss, resize) are not the popup's:
            // capturing them hides them from the app's event
            // subscription — iced::event::listen forwards only Ignored
            // events — and locks the unfocused fade
            // (SPEC/Unfocused_Window.md §1)
            iced::Event::Window(_) => {}
            _ => shell.capture_event(),
        }
    }

    fn mouse_interaction(
        &self,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.tooltip {
            return mouse::Interaction::None;
        }
        // Claim the pointer ONLY over the popup content itself. While a
        // popup is open, iced levitates the base-tree cursor wherever
        // the overlay reports an interaction — reporting it window-wide
        // froze the ribbon's tracked hover position (the overlay also
        // captures every CursorMoved), so a click-away press fell back
        // to that stale position, "hit" the tab that opened the popup,
        // and matched its fresh double-click arm: any click anywhere
        // toggled the ribbon and was swallowed. Outside the content the
        // base tree keeps a live cursor and correct hit tests.
        let bounds = Rectangle::new(self.origin, self.scene.size);
        match cursor.position() {
            Some(position) if bounds.contains(position) => mouse::Interaction::Pointer,
            _ => mouse::Interaction::None,
        }
    }
}

impl<'a, Message, Th, Renderer> RibbonOverlay<'a, Message, Th, Renderer> {
    fn handle_preview(&mut self, target: &Option<Target>, shell: &mut Shell<'_, Message>) {
        let gallery_hit = match target {
            Some(Target::GalleryItem { cmd, index, popup: true }) => Some((*cmd, *index)),
            _ => None,
        };
        if gallery_hit != self.state.previewed {
            if let Some((cmd, index)) = self.state.previewed.take() {
                publish(
                    shell,
                    self.mapper,
                    RibbonEvent::Execute {
                        command: cmd,
                        verb: ExecutionVerb::CancelPreview,
                        key: Some(PropertyKey::SelectedItem),
                        value: Some(PropValue::UInt(index as u32)),
                    },
                );
            }
            if let Some((cmd, index)) = gallery_hit {
                self.state.previewed = Some((cmd, index));
                publish(
                    shell,
                    self.mapper,
                    RibbonEvent::Execute {
                        command: cmd,
                        verb: ExecutionVerb::Preview,
                        key: Some(PropertyKey::SelectedItem),
                        value: Some(PropValue::UInt(index as u32)),
                    },
                );
            }
        }
    }

    /// Press at a position (mouse or first touch finger).
    fn press_at(
        &mut self,
        position: Point,
        origin: Point,
        bounds: Rectangle,
        shell: &mut Shell<'_, Message>,
    ) {
        if !bounds.contains(position) {
            // click-away: close and let the base widget handle it
            if let Some(p) = &self.state.popup
                && let PopupKind::MinimizedTab(i) = &p.kind
            {
                // a minimized tab's own popup: when the press lands on
                // that same tab, the base widget's release must not
                // reactivate it (it would reopen what this click closes)
                self.state.dismiss_tab_on_release = Some(*i);
            }
            close_all_popups(self.state);
            self.state.previewed = None;
            shell.request_redraw();
            return;
        }
        let local = Point::new(position.x - origin.x, position.y - origin.y);
        if let Some(node) = self.scene.hit(local)
            && let Some(t) = node.target.clone()
        {
            self.state.pressed = Some(t);
        }
        shell.capture_event();
    }

    /// Release at a position (mouse or first touch finger).
    fn release_at(
        &mut self,
        position: Point,
        origin: Point,
        bounds: Rectangle,
        shell: &mut Shell<'_, Message>,
    ) {
        // A release outside the overlay is a click-away: leave
        // `state.pressed` untouched so the base widget can act on it
        // (a tab-strip click must still select its tab / count toward
        // a tab double-click — the overlay's press already closed the
        // popup on the press, it must not also swallow the release).
        if !bounds.contains(position) {
            return;
        }
        let local = Point::new(position.x - origin.x, position.y - origin.y);
        let hit = self.scene.hit(local).and_then(|n| n.target.clone());
        let pressed = self.state.pressed.take();
        if let (Some(t), Some(p)) = (hit, pressed)
            && t == p
        {
            let anchor = match self.scene.hit(local) {
                Some(n) => super::translate_rect(n.rect, origin),
                None => bounds,
            };
            activate(
                self.fw,
                self.theme,
                self.state,
                &t,
                anchor,
                origin,
                shell,
                self.mapper,
            );
        }
        shell.capture_event();
    }
}

//! Navigation pane: Win7-style foldable tree (Favorites / Computer /
//! Network sections; folders and archives only — see fs/archive).
//!
//! Row height, per-level indention, icon and text sizes come from the
//! ribbon markup (`metrics:` in ribbon.yaml). Rows never wrap: the pane
//! scrolls horizontally instead (iced shows the scrollbar only when the
//! tree is wider than the pane).

use std::path::{Path, PathBuf};

use iced::Widget as _;
use iced::widget::{column, container, mouse_area, row, scrollable, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message, NavSection};

/// Full navigation pane width (shrinks proportionally below the yaml
/// `sacrifice.nav_resize_below` content width).
pub(crate) const NAV_W: f32 = 210.0;

/// Id of the tree scrollable (the model scrolls it to the current
/// folder after navigation).
pub(crate) fn nav_scrollable_id() -> iced::widget::Id {
    static ID: std::sync::OnceLock<iced::widget::Id> = std::sync::OnceLock::new();
    ID.get_or_init(|| iced::widget::Id::new("nav-tree")).clone()
}
use crate::ui_common::{bg, icon, skin};

/// Nav-tree metrics derived from the ribbon markup: the row height drives
/// the icon, chevron and text sizes; the indention doubles as the chevron
/// slot width, so a child icon aligns under its parent's label.
#[derive(Clone, Copy)]
struct NavMetrics {
    height: f32,
    indent: f32,
    icon: f32,
    arrow: f32,
    font: f32,
}

fn nav_metrics(state: &Explorer) -> NavMetrics {
    let (_, height, indent, font_scale) = state.ribbon.metrics();
    let icon = (height - 6.0).clamp(8.0, 32.0);
    NavMetrics {
        height,
        indent,
        icon,
        arrow: (height * 0.45).round().clamp(6.0, 16.0),
        // the label sits next to the icon: keep the glyph close to icon size
        font: (icon * font_scale).round().clamp(8.0, 24.0),
    }
}

/// One tree row: optional expand arrow, icon, label; selected = tinted
/// row, hovered = hover fill. Rows shrink to their content width (the
/// pane scrolls horizontally).
fn tree_item(
    label: String,
    icon_name: &'static str,
    indent: usize,
    arrow: Option<bool>, // Some(expanded) -> clickable arrow; None -> spacer
    selected: bool,
    hovered: bool,
    navigate: Option<PathBuf>,
    m: NavMetrics,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let bgc = if selected {
        skin.sel
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };

    let arrow_el: Element<Message> = match arrow {
        Some(expanded) => {
            let icon_name = if expanded { "chevron_down" } else { "chevron_right" };
            let path = navigate.clone();
            let a: Element<Message> = container(icon(icon_name, m.arrow, skin))
                .width(Length::Fixed(m.indent))
                .center_x(Length::Fixed(m.indent))
                .boxed();
            // the arrow toggles the node, it does not navigate to it
            match path {
                Some(p) => mouse_area(a).on_press(Message::NavToggle(p)).boxed(),
                None => a,
            }
        }
        None => Space::new().width(Length::Fixed(m.indent)).boxed(),
    };

    let item: Element<Message> = container(
        row![
            Space::new().width(Length::Fixed(indent as f32 * m.indent)),
            arrow_el,
            icon(icon_name, m.icon, skin),
            text(label)
                .size(m.font)
                .color(skin.text)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .height(Length::Fixed(m.height)),
    )
    .padding([0.0, 4.0])
    .width(Length::Shrink)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .boxed();
    match navigate {
        Some(p) => mouse_area(item)
            .on_press(Message::Navigate(p.clone()))
            .on_right_press(Message::ContextNav(p.clone()))
            .on_enter(Message::NavHover(Some(p.clone())))
            .on_exit(Message::NavHoverExit(p))
            .interaction(iced::mouse::Interaction::Pointer)
            .boxed(),
        None => item,
    }
}

/// Section header row without a backing location ("Network") — clicking
/// anywhere on it folds the section.
fn section_header(
    label: &str,
    icon_name: &'static str,
    open: bool,
    section: NavSection,
    m: NavMetrics,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let arrow = if open { "chevron_down" } else { "chevron_right" };
    let label = label.to_string();
    mouse_area(
        container(
            row![
                container(icon(arrow, m.arrow, skin))
                    .width(Length::Fixed(m.indent))
                    .center_x(Length::Fixed(m.indent)),
                icon(icon_name, m.icon, skin),
                text(label)
                    .size(m.font)
                    .color(skin.text)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(m.height)),
        )
        .padding([0.0, 4.0])
        .width(Length::Shrink),
    )
    .on_press(Message::NavSectionToggle(section))
    .interaction(iced::mouse::Interaction::Pointer)
    .boxed()
}

/// One folder/archive node with its (cached) children when expanded.
fn push_tree_node<'a>(
    list: iced::widget::Column<iced::Element<'a, Message>>,
    state: &Explorer,
    path: &Path,
    label: String,
    icon_name: &'static str,
    indent: usize,
    m: NavMetrics,
    skin: skin::Skin,
) -> iced::widget::Column<iced::Element<'a, Message>> {
    let kids = state.nav.children.get(path);
    let expanded = state.nav.expanded.contains(path);
    let has_children = kids.map(|k| !k.is_empty()).unwrap_or(false);
    let selected = state.cwd == path;
    let hovered = state.nav_hover.as_deref() == Some(path);
    let mut list = list.push(tree_item(
        label,
        icon_name,
        indent,
        has_children.then_some(expanded),
        selected,
        hovered,
        Some(path.to_path_buf()),
        m,
        skin,
    ));
    if expanded {
        if let Some(kids) = kids {
            for k in kids {
                let kicon: &'static str = if k.is_dir() { "folder" } else { "archive" };
                list = push_tree_node(list, state, &k.path, k.name.clone(), kicon, indent + 1, m, skin);
            }
        }
    }
    list
}

/// Favorites entries: plain shortcuts (no tree expansion), in Win7 order.
fn favorites() -> Vec<(String, &'static str, Option<PathBuf>)> {    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut favs: Vec<(String, &'static str, Option<PathBuf>)> = vec![
        ("Home".into(), "home", home.clone()),
        ("Desktop".into(), "desktop", home.as_ref().map(|h| h.join("Desktop"))),
        ("Downloads".into(), "downloads", home.as_ref().map(|h| h.join("Downloads"))),
        ("Recent places".into(), "recent", None),
    ];
    // only keep the entries whose target exists
    favs.retain(|(_, _, target)| match target {
        Some(p) => p.is_dir(),
        None => true, // Recent places: placeholder, no backing folder
    });
    favs
}

// ---------------- scroll-position helpers ----------------
//
// The model scrolls the tree to the current folder after navigation; the
// row offsets are computed by replaying the same expansion rules as the
// view. When a node's children are not cached yet the tree is
// pessimistically short and the helpers bail out (None) instead of
// guessing a wrong offset — the reveal is retried when the listing
// arrives.

/// Number of favorites rows (also the count the tree rows are offset by).
pub(crate) fn favorites_len() -> usize {
    favorites().len()
}

/// Visible rows of one expanded node (children only, recursively).
fn subtree_rows(state: &Explorer, path: &Path) -> Option<usize> {
    if !state.nav.expanded.contains(path) {
        return Some(0);
    }
    let kids = state.nav.children.get(path)?;
    let mut n = 0;
    for k in kids {
        n += 1;
        n += subtree_rows(state, &k.path)?;
    }
    Some(n)
}

/// Visible row index of `cwd` inside the Computer tree (0 = the
/// "Computer" row itself), or None when the row is not rendered yet.
pub(crate) fn selected_row_index(state: &Explorer) -> Option<usize> {
    let mut rows = 0usize; // rows passed so far inside the tree
    // the Computer ("/") row
    if state.cwd == Path::new("/") {
        return Some(0);
    }
    rows += 1;
    if !state.nav.expanded.contains(Path::new("/")) {
        return None;
    }
    // volumes first (same order as the view)
    for v in &state.volumes {
        if v.mount == Path::new("/") {
            continue;
        }
        if state.cwd == v.mount {
            return Some(rows);
        }
        rows += 1;
        if state.cwd.starts_with(&v.mount) {
            let rest = subtree_row_index(state, &v.mount)?;
            return Some(rows + rest);
        }
        rows += subtree_rows(state, &v.mount)?;
    }
    // then the cached root folders
    for k in state.nav.children.get(Path::new("/"))? {
        if state.cwd == k.path {
            return Some(rows);
        }
        rows += 1;
        if state.cwd.starts_with(&k.path) {
            let rest = subtree_row_index(state, &k.path)?;
            return Some(rows + rest);
        }
        rows += subtree_rows(state, &k.path)?;
    }
    None
}

/// Row index of the cwd inside the subtree of `root` (0 = first child of
/// `root`; the root row itself is counted by the caller).
fn subtree_row_index(state: &Explorer, root: &Path) -> Option<usize> {
    if !state.nav.expanded.contains(root) {
        return None;
    }
    let kids = state.nav.children.get(root)?;
    let mut rows = 0usize;
    for k in kids {
        if state.cwd == k.path {
            return Some(rows);
        }
        rows += 1;
        if state.cwd.starts_with(&k.path) {
            let rest = subtree_row_index(state, &k.path)?;
            return Some(rows + rest);
        }
        rows += subtree_rows(state, &k.path)?;
    }
    None
}

/// Total visible rows of the Computer tree (favorites excluded).
pub(crate) fn tree_row_count(state: &Explorer) -> usize {
    let mut n = 1; // the "Computer" row
    if state.nav.expanded.contains(Path::new("/")) {
        for v in &state.volumes {
            if v.mount == Path::new("/") {
                continue;
            }
            n += 1 + subtree_rows(state, &v.mount).unwrap_or(0);
        }
        if let Some(kids) = state.nav.children.get(Path::new("/")) {
            for k in kids {
                n += 1 + subtree_rows(state, &k.path).unwrap_or(0);
            }
        }
    }
    n
}

/// Total tree content height (drives the scrollbar strip geometry).
fn content_height(state: &Explorer) -> f32 {
    let (_, item_h, ..) = state.ribbon.metrics();
    let favs = if state.nav.favorites_open { favorites_len() } else { 0 };
    let net = 1 + usize::from(state.nav.network_open);
    // 1 favorites header + favorites + the Computer tree + Network rows;
    // 8 = the column's top+bottom padding
    8.0 + (1 + favs + tree_row_count(state) + net) as f32 * item_h
}

pub(crate) fn nav_pane(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let m = nav_metrics(state);
    let mut list = column![].spacing(0).padding(4);

    // ---- Favorites (foldable section; entries are shortcuts only) ----
    list = list.push(section_header("Favorites", "star", state.nav.favorites_open, NavSection::Favorites, m, skin));
    if state.nav.favorites_open {
        for (label, icon_name, target) in favorites() {
            // shortcuts: never show the selected state (only the Computer
            // tree highlights the current folder)
            let hovered = target.is_some() && target == state.nav_hover;
            list = list.push(tree_item(label, icon_name, 1, None, false, hovered, target, m, skin));
        }
    }

    // ---- Computer: the "/" root node itself (drives + folders below) ----
    let root_open = state.nav.expanded.contains(Path::new("/"));
    list = list.push(tree_item(
        "Computer".to_string(),
        "computer",
        0,
        Some(root_open),
        state.cwd == Path::new("/"),
        state.nav_hover.as_deref() == Some(Path::new("/")),
        Some(PathBuf::from("/")),
        m,
        skin,
    ));
    if root_open {
        // volumes sit directly under Computer (skip "/" — it IS Computer)
        for v in &state.volumes {
            if v.mount == Path::new("/") {
                continue;
            }
            let label = if v.name.is_empty() { v.mount.display().to_string() } else { v.name.clone() };
            list = push_tree_node(list, state, &v.mount, label, "drive", 1, m, skin);
        }
        if let Some(kids) = state.nav.children.get(Path::new("/")) {
            for k in kids {
                let kicon: &'static str = if k.is_dir() { "folder" } else { "archive" };
                list = push_tree_node(list, state, &k.path, k.name.clone(), kicon, 1, m, skin);
            }
        }
    }

    // ---- Network (no backend yet — header only, folded by default) ----
    list = list.push(section_header("Network", "network", state.nav.network_open, NavSection::Network, m, skin));
    if state.nav.network_open {
        list = list.push(tree_item(
            "Network".to_string(),
            "network",
            1,
            None,
            false,
            false,
            None,
            m,
            skin,
        ));
    }

    let (_, item_h, ..) = state.ribbon.metrics();
    let need_strip = content_height(state) > state.nav.viewport_h;
    // the scrollbar strip collapses to a 1px hairline at the right edge
    // when the tree fits the pane
    let bar: Element<Message> = if need_strip {
        crate::scrollbar::vertical(
            state.nav.scroll_y,
            state.nav.viewport_h,
            content_height(state),
            item_h,
            skin,
            Message::NavScrollTo,
        )
    } else {
        container(Space::new())
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(bg(skin.sep))
            .boxed()
    };
    container(
        // wheel speed boost (folder-view.yaml input.scroll_speed)
        crate::scrollbar::wheel_boost(
            row![
                iced::widget::Sensor::new(
                    scrollable(list)
                        .id(nav_scrollable_id())
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .direction(scrollable::Direction::Both {
                            // the Windows-style strip beside the scrollable is
                            // the visible scrollbar (arrow buttons + thumb)
                            vertical: scrollable::Scrollbar::hidden(),
                            // keep horizontal scrolling, but no visible scrollbar
                            horizontal: scrollable::Scrollbar::hidden(),
                        })
                        .on_scroll(|vp| Message::NavScrolled {
                            y: vp.viewport.absolute_offset().y,
                            height: vp.viewport.bounds.height,
                        }),
                )
                .on_show(|size| Message::NavAreaResized(size.width, size.height))
                .on_resize(|size| Message::NavAreaResized(size.width, size.height)),
                bar,
            ]
            .boxed(),
            state.nav.scroll_y,
            (content_height(state) - state.nav.viewport_h).max(0.0),
            state.view_cfg.input.scroll_speed,
            crate::scrollbar::Axis::Vertical,
            Message::NavScrollTo,
        ),
    )
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .style(bg(skin.nav_bg))
    .boxed()
}

// ---------------- file view ----------------

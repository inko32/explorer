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

/// Hover behavior of the nav tree: hovering a row tints every row that
/// points to the same path — e.g. hovering Favorites/Downloads also
/// tints the Computer/home/inko/Downloads node (and vice versa).
/// Windows Explorer tints only the row under the cursor; set this to
/// `false` to mimic Windows.
const HOVER_TINTS_SAME_PATH: bool = false;

/// Whether the row `path` (a Favorites shortcut row when `fav_row`, a
/// tree row otherwise) shows the hover tint for the current
/// `nav_hover` — the pair records the hovered path and which kind of
/// row it was, so the tint can be limited to that row when
/// HOVER_TINTS_SAME_PATH is off.
fn hover_matches(hover: Option<&(PathBuf, bool)>, path: &Path, fav_row: bool) -> bool {
    let Some((h, fav_src)) = hover else { return false };
    h == path && (HOVER_TINTS_SAME_PATH || *fav_src == fav_row)
}

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

/// Estimated content width of one row: the label is measured per
/// character (the pane has no renderer-bound text measurer; the trailing
/// slack keeps the fill from falling short on wide glyphs).
fn row_content_w(label_chars: usize, indent: usize, m: NavMetrics) -> f32 {
    const CHAR_W: f32 = 0.62; // average glyph advance as a fraction of size
    indent as f32 * m.indent
        + m.indent
        + m.icon
        + 3.0 * 6.0 // row spacing (4 elements)
        + label_chars as f32 * m.font * CHAR_W
        + 8.0 // container padding
        + 4.0 // slack
}

/// Width of the widest visible row (same traversal as the view).
fn content_width(state: &Explorer, m: NavMetrics) -> f32 {
    let mut w = row_content_w("Favorites".len(), 0, m);
    if state.nav.favorites_open {
        for (label, _, _) in favorites() {
            w = w.max(row_content_w(label.chars().count(), 1, m));
        }
    }
    w = w.max(row_content_w("Computer".len(), 0, m));
    if state.nav.expanded.contains(Path::new("/")) {
        for v in &state.volumes {
            if v.mount == Path::new("/") {
                continue;
            }
            let label = if v.name.is_empty() {
                v.mount.display().to_string()
            } else {
                v.name.clone()
            };
            w = w.max(row_content_w(label.chars().count(), 1, m));
            w = subtree_content_w(w, state, &v.mount, 2, m);
        }
        if let Some(kids) = state.nav.children.get(Path::new("/")) {
            for k in kids {
                w = w.max(row_content_w(k.name.chars().count(), 1, m));
                w = subtree_content_w(w, state, &k.path, 2, m);
            }
        }
    }
    w = w.max(row_content_w("Network".len(), 0, m));
    if state.nav.network_open {
        w = w.max(row_content_w("Network".len(), 1, m));
    }
    w
}

fn subtree_content_w(
    mut w: f32,
    state: &Explorer,
    path: &Path,
    indent: usize,
    m: NavMetrics,
) -> f32 {
    if !state.nav.expanded.contains(path) {
        return w;
    }
    if let Some(kids) = state.nav.children.get(path) {
        for k in kids {
            w = w.max(row_content_w(k.name.chars().count(), indent, m));
            w = subtree_content_w(w, state, &k.path, indent + 1, m);
        }
    }
    w
}

/// One tree row: optional expand arrow, icon, label; selected = tinted
/// row, hovered = hover fill. `row_w` spans the full pane width — or the
/// widest row when the tree scrolls horizontally, so the fill always
/// covers the line.
// row widgets are built from many independent chrome bits; a params
// struct would not pay for itself
#[allow(clippy::too_many_arguments)]
fn tree_item(
    row_w: f32,
    label: String,
    icon_name: &'static str,
    indent: usize,
    arrow: Option<bool>, // Some(expanded) -> clickable arrow; None -> spacer
    selected: bool,
    hovered: bool,
    targeted: bool, // drag & drop: the row is the current drop target
    state: &Explorer, // activity model: focus, active pane, popup
    navigate: Option<PathBuf>,
    fav_shortcut: bool, // the row is a Favorites shortcut (hover source tag)
    m: NavMetrics,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // selection fades to gray while the nav pane is inactive (window
    // unfocused, folder pane active, or a ribbon popup open);
    // hovering a grayed row (or an active drop target) restores the
    // live colors
    let popup_open = state.ribbon.framework().view().popup_open();
    let pane_live = state.focused
        && !popup_open
        && state.active_pane == crate::model::Pane::Nav;
    let live = pane_live || targeted || (hovered && !popup_open);
    let bgc = if selected || targeted {
        if live { skin.sel } else { skin.sel_inactive }
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected || targeted {
        if live { skin.sel_border } else { skin.sel_inactive_border }
    } else {
        Color::TRANSPARENT
    };

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
    .width(Length::Fixed(row_w))
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: crate::theme::border_w(1.0),
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .boxed();
    match navigate {
        Some(p) => {
            // a drag & drop target
            let item = crate::dnd::drop_zone(item, crate::dnd::DropTarget::NavigationDirectory(p.clone()));
            // cursor_seed: the context menu anchors at
            // Explorer::cursor, which idle moves no longer refresh
            // (main.rs drops them)
            mouse_area(crate::ui_common::cursor_seed(item))
                .on_press(Message::NavPress(p.clone()))
                .on_right_press(Message::ContextNav(p.clone()))
                .on_enter(Message::NavHover(Some((p.clone(), fav_shortcut))))
                .on_exit(Message::NavHoverExit(p))
                .interaction(iced::mouse::Interaction::Pointer)
                .boxed()
        }
        None => item,
    }
}

/// Section header row without a backing location ("Network") — clicking
/// anywhere on it folds the section.
fn section_header(
    row_w: f32,
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
        .width(Length::Fixed(row_w)),
    )
    .on_press(Message::NavSectionToggle(section))
    .interaction(iced::mouse::Interaction::Pointer)
    .boxed()
}

/// One folder/archive node with its (cached) children when expanded.
#[allow(clippy::too_many_arguments)]
fn push_tree_node<'a>(
    list: iced::widget::Column<iced::Element<'a, Message>>,
    state: &Explorer,
    path: &Path,
    label: String,
    icon_name: &'static str,
    indent: usize,
    row_w: f32,
    m: NavMetrics,
    skin: skin::Skin,
) -> iced::widget::Column<iced::Element<'a, Message>> {
    let kids = state.nav.children.get(path);
    let expanded = state.nav.expanded.contains(path);
    let has_children = kids.map(|k| !k.is_empty()).unwrap_or(false);
    // the cwd node is always marked; a right-clicked row is marked too
    // while its context menu is open (nav_selected)
    let selected = state.cwd == path || state.nav_selected.as_deref() == Some(path);
    let hovered = hover_matches(state.nav_hover.as_ref(), path, false);
    let targeted = state.drop_targeted(path);
    let mut list = list.push(tree_item(
        row_w,
        label,
        icon_name,
        indent,
        has_children.then_some(expanded),
        selected,
        hovered,
        targeted,
        state,
        Some(path.to_path_buf()),
        false,
        m,
        skin,
    ));
    if expanded
        && let Some(kids) = kids
    {
        for k in kids {
            let kicon: &'static str = if k.is_dir() { "folder" } else { "archive" };
            list = push_tree_node(list, state, &k.path, k.name.clone(), kicon, indent + 1, row_w, m, skin);
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
pub(crate) fn content_height(state: &Explorer) -> f32 {
    let (_, item_h, ..) = state.ribbon.metrics();
    let favs = if state.nav.favorites_open { favorites_len() } else { 0 };
    let net = 1 + usize::from(state.nav.network_open);
    // 1 favorites header + favorites + the Computer tree + Network rows
    // + the empty trailing line; 8 = the column's top+bottom padding
    8.0 + (2 + favs + tree_row_count(state) + net) as f32 * item_h
}

pub(crate) fn nav_pane(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let m = nav_metrics(state);
    let need_strip = content_height(state) > state.nav.viewport_h;
    // rows span the scrollable's viewport; when the widest row exceeds
    // it, every row takes the content width so the fill continues
    // during horizontal scrolling
    let strip_w = if need_strip { crate::scrollbar::THICKNESS } else { 1.0 };
    let row_w = (width - strip_w).max(0.0).max(content_width(state, m));
    let mut list = column![].spacing(0).padding(4);

    // ---- Favorites (foldable section; entries are shortcuts only) ----
    list = list.push(section_header(row_w, "Favorites", "star", state.nav.favorites_open, NavSection::Favorites, m, skin));
    if state.nav.favorites_open {
        for (label, icon_name, target) in favorites() {
            // shortcuts never show the cwd state; a right-clicked row
            // shows the context-menu mark like the tree does
            let selected = target.is_some() && target == state.nav_selected;
            let hovered = target.as_ref().is_some_and(|t| hover_matches(state.nav_hover.as_ref(), t, true));
            let targeted = target.as_ref().is_some_and(|t| state.drop_targeted(t));
            list = list.push(tree_item(row_w, label, icon_name, 1, None, selected, hovered, targeted, state, target, true, m, skin));
        }
    }

    // ---- Computer: the "/" root node itself (drives + folders below) ----
    let root_open = state.nav.expanded.contains(Path::new("/"));
    list = list.push(tree_item(
        row_w,
        "Computer".to_string(),
        "computer",
        0,
        Some(root_open),
        state.cwd == Path::new("/") || state.nav_selected.as_deref() == Some(Path::new("/")),
        hover_matches(state.nav_hover.as_ref(), Path::new("/"), false),
        state.drop_targeted(Path::new("/")),
        state,
        Some(PathBuf::from("/")),
        false,
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
            list = push_tree_node(list, state, &v.mount, label, "drive", 1, row_w, m, skin);
        }
        if let Some(kids) = state.nav.children.get(Path::new("/")) {
            for k in kids {
                let kicon: &'static str = if k.is_dir() { "folder" } else { "archive" };
                list = push_tree_node(list, state, &k.path, k.name.clone(), kicon, 1, row_w, m, skin);
            }
        }
    }

    // ---- Network (no backend yet — header only, folded by default) ----
    list = list.push(section_header(row_w, "Network", "network", state.nav.network_open, NavSection::Network, m, skin));
    if state.nav.network_open {
        list = list.push(tree_item(
            row_w,
            "Network".to_string(),
            "network",
            1,
            None,
            false,
            false,
            false,
            state,
            None,
            false,
            m,
            skin,
        ));
    }

    // breathing room after the last row: an empty line with no function
    list = list.push(Space::new().height(Length::Fixed(m.height)).boxed());

    let (_, item_h, ..) = state.ribbon.metrics();
    // one tree row per wheel tick: iced converts a tick to 60px, so the
    // multiplier is the row height over 60 (the folder view keeps its
    // own configured scroll_speed)
    let row_per_tick = item_h / 60.0;
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
            .width(Length::Fixed(crate::theme::border_w(1.0)))
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
            row_per_tick,
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

//! Ribbon layout: builds the [`Scene`] for the widget chrome
//! (QAT row, tab strip, ribbon body) plus the shared control builders
//! reused by the overflow popup.

use iced::alignment::{Horizontal, Vertical};
use iced::{Color, Point, Rectangle, Size};

use crate::ribbon::enums::{ControlDock, SwatchColorMode};
use crate::ribbon::framework::{
    RibbonFramework, RibbonView, QAT_ROW_HEIGHT, RIBBON_BODY_HEIGHT,
    TAB_STRIP_HEIGHT,
};
use crate::ribbon::prop::{FontToggle, PropertyKey, PropValue};
use crate::ribbon::spec::{
    ButtonSpec, CommandRef, ControlSpec, CtrlSize, GallerySpec, GroupScale,
    GroupSpec, RibbonSpec, ScalingSpec, SizeRule, TabSpec,
};
use crate::ribbon::theme::{ContextualColor, Theme};

use super::scene::{Cmd, Fill, Node, Scene};
use super::state::{EditState, Popup, PopupKind};
use super::Target;

pub const QAT_H: f32 = QAT_ROW_HEIGHT;

/// Horizontal gap multiplier (yaml `metrics.gap_scale`): scales group
/// edge padding (4px), the inter-group gap (6px) and the column gap
/// (2px).
pub fn gap(ctx: &Ctx) -> f32 {
    ctx.spec().metrics.gap_scale
}
pub const STRIP_H: f32 = TAB_STRIP_HEIGHT;
/// Base body height (without the yaml `metrics.footer_shift`).
pub const BODY_H: f32 = RIBBON_BODY_HEIGHT;
/// Body height for the loaded spec: the group titles sit
/// `footer_shift` px lower and the bottom separator line follows.
pub fn body_h(ctx: &Ctx) -> f32 {
    BODY_H + ctx.spec().metrics.footer_shift
}
pub const FOOTER_H: f32 = 18.0;
pub const CONTENT_H: f32 = BODY_H - FOOTER_H - 8.0; // 66
pub const ROW_H: f32 = CONTENT_H / 3.0; // 22
pub const ORB_SIZE: f32 = 22.0;

/// Estimated text width (no renderer measurement; deterministic).
pub fn text_w(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * 0.55
}

/// Greedy word wrap by measured width.
pub fn wrap_width(ctx: &Ctx, s: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        let candidate =
            if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && ctx.text_w(&candidate, size) > max_w {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = candidate;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Balanced word wrap: split into up to `max_lines` lines so the widest
/// line (and thus the button width) is as small as possible. Large
/// ribbon buttons wrap this way ("Move\nto", "File name\nextensions").
pub fn wrap_balanced(ctx: &Ctx, s: &str, size: f32, max_lines: usize) -> Vec<String> {
    let words: Vec<&str> = s.split(' ').collect();
    if words.len() < 2 || max_lines < 2 {
        return vec![s.to_string()];
    }
    if max_lines == 2 {
        let mut best: Option<(f32, usize)> = None;
        for i in 1..words.len() {
            let w = ctx
                .text_w(&words[..i].join(" "), size)
                .max(ctx.text_w(&words[i..].join(" "), size));
            if best.map(|(bw, _)| w < bw).unwrap_or(true) {
                best = Some((w, i));
            }
        }
        let i = best.map(|(_, i)| i).unwrap_or(1);
        return vec![words[..i].join(" "), words[i..].join(" ")];
    }
    wrap_width(ctx, s, size, 76.0)
}

/// Greedy word wrap by character count (no renderer at hand).
pub fn wrap(s: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        let extra = if cur.is_empty() { 0 } else { 1 };
        if !cur.is_empty() && cur.chars().count() + extra + word.chars().count()
            > max_chars
        {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Read-only context handed to all scene builders.
pub struct Ctx<'a> {
    pub fw: &'a RibbonFramework,
    pub theme: &'a Theme,
    pub state: &'a super::State,
    /// Accurate text measurement (paragraph shaping), keyed by the widget.
    pub measure: &'a dyn Fn(&str, f32) -> f32,
}

impl<'a> Ctx<'a> {
    /// Measured text width.
    pub fn text_w(&self, s: &str, size: f32) -> f32 {
        (self.measure)(s, size)
    }
}

impl<'a> Ctx<'a> {
    pub fn view(&self) -> RibbonView {
        self.fw.get_view(0).expect("view")
    }

    pub fn spec(&self) -> &'a RibbonSpec {
        self.fw.spec().expect("loaded")
    }

    pub fn value(&self, cmd: u32, key: PropertyKey) -> Option<PropValue> {
        self.fw.cached(cmd, key)
    }

    pub fn str_prop(&self, cmd: u32, key: PropertyKey) -> Option<String> {
        self.value(cmd, key)
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    pub fn label(&self, cmd: u32) -> String {
        self.str_prop(cmd, PropertyKey::Label)
            .unwrap_or_else(|| format!("#{cmd}"))
    }

    pub fn keytip(&self, cmd: u32) -> Option<String> {
        self.str_prop(cmd, PropertyKey::Keytip)
    }

    pub fn tooltip(&self, cmd: u32) -> (String, String) {
        let title = self
            .str_prop(cmd, PropertyKey::TooltipTitle)
            .unwrap_or_else(|| self.label(cmd));
        let desc = self
            .str_prop(cmd, PropertyKey::TooltipDescription)
            .unwrap_or_default();
        (title, desc)
    }

    pub fn enabled(&self, cmd: u32) -> bool {
        self.fw.command_enabled(cmd)
    }

    pub fn bool_value(&self, cmd: u32) -> bool {
        self.value(cmd, PropertyKey::BooleanValue)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub fn large_image(&self, cmd: u32) -> Option<crate::ribbon::image::UiImage> {
        self.value(cmd, PropertyKey::LargeImage)
            .and_then(|v| match v {
                PropValue::Image(i) => Some(i),
                _ => None,
            })
    }

    pub fn small_image(&self, cmd: u32) -> Option<crate::ribbon::image::UiImage> {
        self.value(cmd, PropertyKey::SmallImage)
            .and_then(|v| match v {
                PropValue::Image(i) => Some(i),
                _ => None,
            })
    }

    pub fn hovered(&self, t: &Target) -> bool {
        if self.state.popup_drawing {
            self.state.popup_hover() == Some(t)
        } else {
            self.state.hover.as_ref() == Some(t)
        }
    }

    pub fn pressed(&self, t: &Target) -> bool {
        self.state.pressed.as_ref() == Some(t)
    }

    pub fn keytips_visible(&self, strip: bool) -> bool {
        self.state.keytips && self.state.keytips_strip == strip
    }
}

// ---------------------------------------------------------------------------
// primitives

fn quad(
    scene: &mut Scene,
    rect: Rectangle,
    fill: Fill,
    border: Option<(Color, f32)>,
    radius: f32,
) {
    scene.push(Node::new(
        rect,
        Cmd::Quad {
            fill,
            border,
            radius,
        },
    ));
}

pub fn text(
    scene: &mut Scene,
    rect: Rectangle,
    s: impl Into<String>,
    size: f32,
    color: Color,
    halign: Horizontal,
    valign: Vertical,
) {
    scene.push(Node::new(
        rect,
        Cmd::Text {
            text: s.into(),
            size,
            color,
            halign,
            valign,
            bold: false,
        },
    ));
}

fn text_bold(
    scene: &mut Scene,
    rect: Rectangle,
    s: impl Into<String>,
    size: f32,
    color: Color,
    halign: Horizontal,
    valign: Vertical,
) {
    scene.push(Node::new(
        rect,
        Cmd::Text {
            text: s.into(),
            size,
            color,
            halign,
            valign,
            bold: true,
        },
    ));
}

fn svg_node(
    scene: &mut Scene,
    rect: Rectangle,
    image: crate::ribbon::image::UiImage,
    opacity: f32,
) {
    scene.push(Node::new(
        rect,
        Cmd::Svg {
            image,
            tint: None,
            opacity,
        },
    ));
}

pub fn chevron(scene: &mut Scene, rect: Rectangle, up: bool, color: Color) {
    let path = if up {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 8"><path d="M1 6.4L6 1.6l5 4.8" fill="none" stroke="COLOR" stroke-width="1.8"/></svg>"#
    } else {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 8"><path d="M1 1.6L6 6.4l5-4.8" fill="none" stroke="COLOR" stroke-width="1.8"/></svg>"#
    };
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8
    );
    scene.push(Node::new(
        rect,
        Cmd::Svg {
            image: crate::ribbon::image::UiImage::Svg(path.replace("COLOR", &hex)),
            tint: None,
            opacity: 1.0,
        },
    ));
}

/// Push a keytip badge if keytips are visible in this stage.
pub fn keytip_badge(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, cmd: u32, strip: bool) {
    if !ctx.keytips_visible(strip) {
        return;
    }
    let Some(k) = ctx.keytip(cmd) else {
        return;
    };
    let anchor = Point::new(rect.x + rect.width - 4.0, rect.y - 2.0);
    scene.push(Node::new(
        Rectangle::with_size(Size::new(18.0, 13.0)),
        Cmd::KeytipBadge { text: k, anchor },
    ));
}

/// Standard ribbon button chrome.
fn push_button(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    target: Target,
    cmd: u32,
    checked: bool,
    radius: f32,
) -> bool {
    let enabled = ctx.enabled(cmd);
    // disabled buttons still highlight and show their tooltip on hover
    let hover = ctx.hovered(&target);
    let pressed = enabled && ctx.pressed(&target);
    let theme = ctx.theme;
    let (fill, border) = if pressed {
        (
            Fill::Color(theme.pressed_bg),
            theme.pressed_border.or(theme.hover_border).map(|c| (c, 1.0)),
        )
    } else if hover {
        (Fill::Color(theme.hover_bg), theme.hover_border.map(|c| (c, 1.0)))
    } else if checked {
        (Fill::Color(theme.checked_bg), theme.checked_border.map(|c| (c, 1.0)))
    } else {
        (Fill::Color(Color::TRANSPARENT), None)
    };
    let node = Node::new(
        rect,
        Cmd::Quad {
            fill,
            border,
            radius,
        },
    )
    .target(target)
    .tip(cmd);
    scene.push(node);
    enabled
}

// ---------------------------------------------------------------------------
// chrome

pub struct TabLayout {
    pub index: usize,
    pub rect: Rectangle,
}

/// Visible tabs (modes + contextual availability) of the spec.
pub fn visible_tabs(ctx: &Ctx) -> Vec<usize> {
    let spec = ctx.spec();
    let view = ctx.view();
    let modes = view.modes();
    spec.tabs
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            if !t.modes.is_empty() && !t.modes.iter().any(|b| modes & (1 << b) != 0)
            {
                return false;
            }
            if let Some(cmd) = tab_context_command(t) {
                return ctx.fw.context_availability(cmd)
                    != crate::ribbon::enums::ContextAvailability::NotAvailable;
            }
            true
        })
        .map(|(i, _)| i)
        .collect()
}

/// The context command of a tab (spec stores it unresolved-friendly).
fn tab_context_command(tab: &TabSpec) -> Option<u32> {
    tab.context_command.as_ref().and_then(|c| match c {
        crate::ribbon::spec::CommandRef::Id(id) => Some(*id),
        _ => None,
    })
}

pub fn tab_label(_ctx: &Ctx, tab: &TabSpec) -> String {
    tab.label.clone().unwrap_or_else(|| tab.id.clone())
}

pub fn ribbon_height(view: &RibbonView) -> f32 {
    let mut h = STRIP_H;
    if !view.minimized() {
        h += view.body_height();
    }
    if view.qat_dock() == ControlDock::Bottom {
        h += QAT_H;
    }
    h
}

/// Full ribbon scene.
pub fn ribbon_scene(ctx: &Ctx, width: f32) -> (Scene, Vec<TabLayout>) {
    let view = ctx.view();
    let minimized = view.minimized();
    let mut scene = Scene::new(Size::ZERO);
    let mut y = 0.0;

    // QAT docked "top" lives in the window title bar (border.rs); the
    // ribbon draws no row for it. Docked "bottom" is drawn below the body.

    let strip_y = y;
    // tab strip background
    quad(
        &mut scene,
        Rectangle::new(Point::new(0.0, strip_y), Size::new(width, STRIP_H)),
        Fill::Gradient(
            ctx.theme.tab_strip.0,
            ctx.theme.tab_strip.1,
        ),
        None,
        0.0,
    );
    y += STRIP_H;

    let mut tabs_layout = Vec::new();
    if !minimized {
        let bh = body_h(ctx);
        body(ctx, &mut scene, y, width);
        y += bh;
    }
    tab_strip(ctx, &mut scene, strip_y, width, &mut tabs_layout);

    if view.qat_dock() == ControlDock::Bottom {
        qat_row(ctx, &mut scene, y, width);
        y += QAT_H;
    }

    scene.size = Size::new(width, y);
    (scene, tabs_layout)
}

#[allow(clippy::too_many_lines)]
fn tab_strip(
    ctx: &Ctx,
    scene: &mut Scene,
    strip_y: f32,
    width: f32,
    tabs_layout: &mut Vec<TabLayout>,
) {
    let view = ctx.view();
    let spec = ctx.spec();
    let selected = view.selected_tab();

    let mut x = (ctx.text_w("File", 12.5) + 26.0) * 8.0 / 7.0;
    let visible = visible_tabs(ctx);
    // Pre-scan: base tab widths, then widen the last tab of each
    // contextual run so the whole run is at least as wide as its
    // title-bar block caption (block = run, see
    // border::contextual_blocks) — the caption must fit over the run.
    let widths: Vec<f32> = {
        let mut ws: Vec<f32> = visible
            .iter()
            .map(|&idx| {
                let tab = &spec.tabs[idx];
                ctx.text_w(&tab_label(ctx, tab), 12.5) + 26.0
            })
            .collect();
        let mut i = 0;
        while i < visible.len() {
            let name = spec.tabs[visible[i]].contextual.clone();
            let mut j = i;
            while j + 1 < visible.len()
                && name.is_some()
                && spec.tabs[visible[j + 1]].contextual == name
            {
                j += 1;
            }
            if let Some(name) = &name {
                let run_w: f32 =
                    ws[i..=j].iter().sum::<f32>() + 2.0 * (j - i) as f32;
                let target = ctx.text_w(name, 12.0) + 6.0;
                if run_w < target {
                    ws[j] += target - run_w;
                }
            }
            i = j + 1;
        }
        ws
    };
    let mut contextual_run: Option<(ContextualColor, f32, f32, String)> = None;
    // Geometry of the active tab, for the connected line (Filled style).
    let mut active_tab: Option<(f32, f32, Option<ContextualColor>)> = None;

    for (pos, &idx) in visible.iter().enumerate() {
        let tab = &spec.tabs[idx];
        let label = tab_label(ctx, tab);
        let w = widths[pos];
        let rect =
            Rectangle::new(Point::new(x, strip_y + 1.0), Size::new(w, STRIP_H - 1.0));
        // While minimized, no tab is pinned active: the clicked tab
        // only highlights while its popup is open. The stored selection
        // is left untouched so expand restores the previous active tab.
        let is_active = if view.minimized() {
            matches!(
                &ctx.state.popup,
                Some(Popup { kind: PopupKind::MinimizedTab(i), .. }) if *i == idx
            )
        } else {
            idx == selected
        };
        let accent = tab
            .contextual
            .as_deref()
            .and_then(ContextualColor::from_name)
            .or_else(|| {
                tab.context_color
                    .as_deref()
                    .and_then(ContextualColor::from_name)
            });
        if is_active {
            active_tab = Some((x, x + w, accent));
        }

        let filled = ctx.theme.tab_style == crate::ribbon::theme::TabStyle::Filled;
        let fill = if is_active {
            // Filled (win7/8): the tab carries the command-pane color so
            // it connects to the pane; Underline (win10): strip color.
            Fill::Color(if filled {
                ctx.theme.ribbon.0
            } else {
                ctx.theme.tab_active
            })
        } else if ctx.hovered(&Target::Tab(idx)) {
            Fill::Color(ctx.theme.tab_hover)
        } else {
            Fill::Color(Color::TRANSPARENT)
        };
        let mut node = Node::new(
            rect,
            Cmd::Quad {
                fill,
                border: None,
                radius: 0.0,
            },
        )
        .target(Target::Tab(idx));
        if let Some(cmd) = tab_context_command(tab) {
            node = node.tip(cmd);
        }
        let text_color = match (accent, is_active) {
            (Some(c), _) => c.accent(),
            (None, true) => ctx.theme.text,
            (None, false) => ctx.theme.tab_text,
        };
        scene.push(node);
        text(
            scene,
            Rectangle::new(Point::new(x, strip_y), Size::new(w, STRIP_H)),
            label,
            12.5,
            text_color,
            Horizontal::Center,
            Vertical::Center,
        );
        if is_active && ctx.theme.tab_style == crate::ribbon::theme::TabStyle::Underline
        {
            // Windows 10: activity is an accent (or contextual-color)
            // underline, not a filled tab.
            let underline = match accent {
                Some(c) => c.accent(),
                None => ctx.theme.accent,
            };
            quad(
                scene,
                Rectangle::new(
                    Point::new(x + 3.0, strip_y + STRIP_H - 3.0),
                    Size::new((w - 6.0).max(8.0), 2.5),
                ),
                Fill::Color(underline),
                None,
                0.0,
            );
        }
        if let Some(k) = &tab.keytip {
            if ctx.keytips_visible(true) {
                scene.push(Node::new(
                    Rectangle::with_size(Size::new(18.0, 13.0)),
                    Cmd::KeytipBadge {
                        text: k.clone(),
                        anchor: Point::new(x + w - 6.0, strip_y + 1.0),
                    },
                ));
            }
        }

        // contextual set tracking (underline themes draw the set bar +
        // name; Filled themes show contextual chrome on the tab itself
        // and in the window title bar)
        let track_set = ctx.theme.tab_style != crate::ribbon::theme::TabStyle::Filled;
        let set_name = if track_set { tab.contextual.as_ref() } else { None };
        match (set_name, &mut contextual_run) {
            (Some(name), run @ None) => {
                *run = Some((
                    accent.unwrap_or(ContextualColor::Blue),
                    x,
                    x + w,
                    name.clone(),
                ));
            }

            (Some(name), Some(r)) if r.3 == *name => {
                r.2 = x + w;
            }
            (Some(name), Some(_)) => {
                finish_contextual(ctx, scene, strip_y, contextual_run.take());
                contextual_run = Some((
                    accent.unwrap_or(ContextualColor::Blue),
                    x,
                    x + w,
                    name.clone(),
                ));
            }
            (None, _) => {
                finish_contextual(ctx, scene, strip_y, contextual_run.take());
            }
        }
        let _ = track_set;

        tabs_layout.push(TabLayout {
            index: idx,
            rect: Rectangle::new(
                Point::new(x, strip_y),
                Size::new(w, STRIP_H),
            ),
        });
        x += w + 2.0;
    }
    let _ = finish_contextual(ctx, scene, strip_y, contextual_run.take());

    // Connected line (win7/8 "filled" tabs): a 1px line along the bottom
    // of the tab bar that turns up, right and down to wrap around the
    // active tab, so tab and command pane read as one connected surface.
    // A contextual active tab colors the whole line with its accent.
    if ctx.theme.tab_style == crate::ribbon::theme::TabStyle::Filled {
        let (tab_l, tab_r, accent) =
            active_tab.unwrap_or((f32::MAX, f32::MAX, None));
        let color = accent.map_or(ctx.theme.tab_line, |c| c.accent());
        let line_y = strip_y + STRIP_H - 1.0;
        let mut line = |lx: f32, ly: f32, lw: f32, lh: f32| {
            quad(
                scene,
                Rectangle::new(Point::new(lx, ly), Size::new(lw, lh)),
                Fill::Color(color),
                None,
                0.0,
            );
        };
        // baseline, left of the active tab
        line(0.0, line_y, tab_l.min(width), 1.0);
        if tab_l < width {
            // up, right, down around the active tab (open at the pane)
            line(tab_l, strip_y, 1.0, STRIP_H);
            line(tab_l, strip_y, tab_r - tab_l, 1.0);
            line(tab_r - 1.0, strip_y, 1.0, STRIP_H);
            // baseline, right of the active tab
            line(tab_r, line_y, width - tab_r, 1.0);
        }
    }

    // application menu: Win8 "File" tab (solid brand rectangle with a
    // caption, not an orb)
    let file_label = "File";
    let file_w = (ctx.text_w(file_label, 12.5) + 26.0) * 8.0 / 7.0;
    let file_rect = Rectangle::new(
        Point::new(0.0, strip_y),
        Size::new(file_w, STRIP_H - 1.0),
    );
    let hover = ctx.hovered(&Target::AppMenu);
    quad(
        scene,
        file_rect,
        Fill::Color(if hover {
            crate::ribbon::theme::tint(ctx.theme.orb.0, 0.12)
        } else {
            ctx.theme.orb.0
        }),
        None,
        0.0,
    );
    text(
        scene,
        file_rect,
        file_label,
        12.5,
        Color::WHITE,
        Horizontal::Center,
        Vertical::Center,
    );
    scene.push(
        Node::new(file_rect, Cmd::Hit)
        .target(Target::AppMenu)
        .tip(orb_tip(ctx)),
    );
    if ctx.keytips_visible(true) {
        scene.push(Node::new(
            Rectangle::with_size(Size::new(18.0, 13.0)),
            Cmd::KeytipBadge {
                text: "F".into(),
                anchor: Point::new(file_rect.x + file_w - 2.0, file_rect.y - 2.0),
            },
        ));
    }

    // right side: help + minimize
    let mut rx = width - 24.0;
    if let Some(help_cmd) = help_command(ctx) {
        let rect =
            Rectangle::new(Point::new(rx, strip_y + 4.0), Size::new(18.0, 18.0));
        let hover = ctx.hovered(&Target::Help(help_cmd));
        quad(
            scene,
            rect,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
            ctx.theme.round_radius,
        );
        text_bold(
            scene,
            rect,
            "?",
            12.0,
            ctx.theme.text,
            Horizontal::Center,
            Vertical::Center,
        );
        scene.push(
            Node::new(rect, Cmd::Hit)
            .target(Target::Help(help_cmd))
            .tip(help_cmd),
        );
        rx -= 22.0;
    }

    let min_rect =
        Rectangle::new(Point::new(rx, strip_y + 4.0), Size::new(18.0, 18.0));
    let min_target = Target::Minimize;
    let hover = ctx.hovered(&min_target);
    quad(
        scene,
        min_rect,
        if hover {
            Fill::Color(ctx.theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        },
        ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
        ctx.theme.round_radius,
    );
    chevron(
        scene,
        Rectangle::new(
            Point::new(min_rect.x + 3.0, min_rect.y + 5.0),
            Size::new(12.0, 8.0),
        ),
        !ctx.view().minimized(),
        ctx.theme.text,
    );
    scene.push(Node::new(min_rect, Cmd::Hit).target(min_target));
}

/// Draw a contextual set's underline + label; returns the extra horizontal
/// space the label consumed (so the next tab set is pushed right).
fn finish_contextual(
    ctx: &Ctx,
    scene: &mut Scene,
    strip_y: f32,
    run: Option<(ContextualColor, f32, f32, String)>,
) -> f32 {
    if let Some((color, start, end, name)) = run {
        quad(
            scene,
            Rectangle::new(
                Point::new(start, strip_y + STRIP_H - 3.0),
                Size::new(end - start, 2.5),
            ),
            Fill::Color(color.accent()),
            None,
            0.0,
        );
        let label_w = ctx.text_w(&name, 10.0) + 6.0;
        text(
            scene,
            Rectangle::new(
                Point::new(end + 4.0, strip_y + STRIP_H - 12.0),
                Size::new(label_w, 11.0),
            ),
            name,
            10.0,
            color.accent(),
            Horizontal::Left,
            Vertical::Center,
        );
        label_w + 8.0
    } else {
        0.0
    }
}

fn orb_tip(ctx: &Ctx) -> u32 {
    ctx.spec()
        .application_menu
        .as_ref()
        .and_then(|m| m.command.as_ref())
        .and_then(|c| match c {
            crate::ribbon::spec::CommandRef::Id(id) => Some(*id),
            _ => None,
        })
        .unwrap_or(0)
}

fn help_command(ctx: &Ctx) -> Option<u32> {
    ctx.spec().help_button.as_ref().and_then(|c| match c {
        crate::ribbon::spec::CommandRef::Id(id) => Some(*id),
        _ => None,
    })
}

fn qat_row(ctx: &Ctx, scene: &mut Scene, y: f32, width: f32) {
    let _ = ctx;
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y), Size::new(width, QAT_H)),
        Fill::Gradient(ctx.theme.qat_bg.0, ctx.theme.qat_bg.1),
        None,
        0.0,
    );
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y + QAT_H - 1.0), Size::new(width, 1.0)),
        Fill::Color(ctx.theme.group_separator),
        None,
        0.0,
    );
    let view = ctx.view();
    let mut x = 6.0;
    for &cmd in &view.qat_commands() {
        let rect = Rectangle::new(Point::new(x, y + 1.0), Size::new(20.0, 20.0));
        let target = Target::Qat(cmd);
        let checked = ctx.bool_value(cmd);
        if push_button(ctx, scene, rect, target.clone(), cmd, checked, ctx.theme.button_radius) {
            let img = ctx.small_image(cmd).unwrap_or_default();
            svg_node(
                scene,
                Rectangle::new(Point::new(x + 2.0, y + 3.0), Size::new(16.0, 16.0)),
                img,
                1.0,
            );
            keytip_badge(ctx, scene, rect, cmd, true);
        }
        x += 22.0;
    }
    // customize dropdown
    let rect = Rectangle::new(
        Point::new(width - 26.0, y + 1.0),
        Size::new(20.0, 20.0),
    );
    let target = Target::QatMenu;
    let hover = ctx.hovered(&target);
    quad(
        scene,
        rect,
        if hover {
            Fill::Color(ctx.theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        },
        ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
        ctx.theme.button_radius,
    );
    chevron(
        scene,
        Rectangle::new(Point::new(rect.x + 4.0, rect.y + 7.0), Size::new(12.0, 7.0)),
        false,
        ctx.theme.text,
    );
    scene.push(Node::new(rect, Cmd::Hit).target(target));
}

// ---------------------------------------------------------------------------
// body / groups

fn body(ctx: &Ctx, scene: &mut Scene, y: f32, width: f32) {
    tab_content(ctx, scene, y, width, active_tab(ctx), true);
}

// ---------------------------------------------------------------------------
// cascading (Windows ScalingPolicy / SizeDefinitions)

/// Effective size of one control under a group scale (`None` = declared).
#[derive(Debug, Clone)]
pub enum ControlScale {
    /// Keep the size declared on the control.
    Declared,
    /// Cap every control at this size (only shrinks; declared-small
    /// buttons stay small).
    Cap(CtrlSize),
    /// `GroupSizeDefinition`: per-command overrides (resolved ids).
    Rules(Vec<(u32, CtrlSize)>),
}

/// One group's layout decision for the current ribbon width.
#[derive(Debug, Clone)]
pub struct GroupPlan {
    pub gi: usize,
    pub scale: ControlScale,
    /// Right-most columns with small-button captions hidden (cascade
    /// stage 2).
    pub caption_cols: usize,
    /// Collapsed to a popup group button.
    pub popup: bool,
    pub w: f32,
}

impl ControlScale {
    fn for_command(&self, cmd: Option<u32>) -> Option<CtrlSize> {
        match self {
            ControlScale::Declared => None,
            ControlScale::Cap(s) => Some(*s),
            ControlScale::Rules(rules) => {
                cmd.and_then(|id| rules.iter().find(|(c, _)| *c == id).map(|(_, s)| *s))
            }
        }
    }

    /// Effective size of a control: a scale only ever *shrinks* — a
    /// group mixing large and small buttons by design keeps its small
    /// buttons small, they are never grown just because space allows.
    fn effective(&self, cmd: Option<u32>, declared: CtrlSize) -> CtrlSize {
        fn rank(s: CtrlSize) -> u8 {
            match s {
                CtrlSize::Large => 2,
                CtrlSize::Medium => 1,
                CtrlSize::Small => 0,
            }
        }
        match self.for_command(cmd) {
            Some(s) if rank(s) < rank(declared) => s,
            _ => declared,
        }
    }
}

/// Resolve a `GroupScale` against the ribbon's named size definitions.
/// Returns `(scale, popup)`; popup wins if the definition is `popup`.
fn resolve_scale(
    ctx: &Ctx,
    tab: &TabSpec,
    group: &GroupSpec,
    scale: &GroupScale,
) -> (ControlScale, bool) {
    let _ = tab;
    match scale {
        GroupScale::Large => (ControlScale::Cap(CtrlSize::Large), false),
        GroupScale::Medium => (ControlScale::Cap(CtrlSize::Medium), false),
        GroupScale::Small => (ControlScale::Cap(CtrlSize::Small), false),
        GroupScale::Popup => (ControlScale::Declared, true),
        GroupScale::Named(name) => {
            let rules = ctx
                .spec()
                .size_definitions
                .get(&name.to_ascii_lowercase())
                .or_else(|| ctx.spec().size_definitions.get(name));
            match rules {
                Some(rules) => {
                    let mut resolved = Vec::new();
                    for rule in rules {
                        resolved.extend(resolve_rule(group, rule));
                    }
                    if resolved.is_empty() {
                        (ControlScale::Declared, false)
                    } else {
                        (ControlScale::Rules(resolved), false)
                    }
                }
                None => (ControlScale::Declared, false),
            }
        }
    }
}

/// Expand a `SizeRule` to resolved `(command id, size)` pairs. A rule
/// with an empty `controls` list applies to every control of the group.
fn resolve_rule(group: &GroupSpec, rule: &SizeRule) -> Vec<(u32, CtrlSize)> {
    fn walk<'a>(
        controls: &'a [ControlSpec],
        out: &mut Vec<&'a ControlSpec>,
    ) {
        for c in controls {
            if let ControlSpec::ControlGroup { controls } = c {
                walk(controls, out);
            } else {
                out.push(c);
            }
        }
    }
    let mut all = Vec::new();
    if rule.controls.is_empty() {
        walk(&group.controls, &mut all);
        return all
            .iter()
            .filter_map(|c| resolved_ref(c.command()).map(|id| (id, rule.size)))
            .collect();
    }
    rule.controls
        .iter()
        .filter_map(|c| resolved_ref(Some(c)).map(|id| (id, rule.size)))
        .collect()
}

fn resolved_ref(c: Option<&CommandRef>) -> Option<u32> {
    match c {
        Some(CommandRef::Id(id)) => Some(*id),
        _ => None,
    }
}

/// The scaling policy entry of a group (matched by group id).
fn scaling_of<'a>(spec: &'a RibbonSpec, group: &GroupSpec) -> Option<&'a ScalingSpec> {
    spec.scaling.iter().find(|s| s.group == group.id)
}

/// The ordered size ladder of a group, widest first: ideal sizes,
/// then the declared fallback `[medium, small, popup]`-style steps.
fn scale_ladder(group: &GroupSpec, policy: Option<&ScalingSpec>) -> Vec<GroupScale> {
    let mut ladder = Vec::new();
    if let Some(p) = policy {
        ladder.extend(p.ideal.iter().cloned());
        ladder.extend(p.scale.iter().cloned());
    }
    // Default cascade: from the group's declared largest control size
    // down through medium / small to a popup group.
    let declared_max = group
        .controls
        .iter()
        .filter_map(|c| match c {
            ControlSpec::Button(b) | ControlSpec::ToggleButton(b) => Some(b.size),
            ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => Some(s.size),
            ControlSpec::ColorPicker(c) => Some(c.size),
            _ => None,
        })
        .max_by_key(|s| match s {
            CtrlSize::Large => 2,
            CtrlSize::Medium => 1,
            CtrlSize::Small => 0,
        });
    let defaults: &[GroupScale] = match declared_max {
        Some(CtrlSize::Large) => &[GroupScale::Medium, GroupScale::Small, GroupScale::Popup],
        Some(CtrlSize::Medium) => &[GroupScale::Small, GroupScale::Popup],
        _ => &[GroupScale::Popup],
    };
    for d in defaults {
        if !ladder.contains(d) {
            ladder.push(d.clone());
        }
    }
    ladder
}

/// Width of a collapsed (popup) group button — sized like a large
/// button (wrapped label + padding).
fn popup_group_width(ctx: &Ctx, group: &GroupSpec) -> f32 {
    let label = group.label.clone().unwrap_or_else(|| group.id.clone());
    let lines = wrap_balanced(ctx, &label, 11.0, 2);
    let max_line = lines
        .iter()
        .map(|l| ctx.text_w(l, 11.0))
        .fold(0.0f32, f32::max);
    max_line.max(40.0) + 12.0
}

/// Width of a group under a given scale.
fn scaled_group_width(
    ctx: &Ctx,
    tab: &TabSpec,
    group: &GroupSpec,
    scale: &GroupScale,
    caption_cols: usize,
) -> (ControlScale, bool, f32) {
    let (sc, popup) = resolve_scale(ctx, tab, group, scale);
    let w = if popup {
        popup_group_width(ctx, group)
    } else {
        measure_group_scaled(ctx, group, &sc, caption_cols)
    };
    (sc, popup, w)
}

/// Plan the groups of a tab for the given ribbon width, applying the
/// cascading policy (Windows `ScalingPolicy`): groups start exactly as
/// declared in the markup and, when the window is too narrow, shrink
/// one ladder step at a time strictly from right to left, until
/// everything fits. Groups that still do not fit are hidden into the
/// overflow popup.
pub fn plan_groups(
    ctx: &Ctx,
    width: f32,
    tab_idx: usize,
) -> (Vec<GroupPlan>, Vec<usize>) {
    let tab = &ctx.spec().tabs[tab_idx];
    let avail = width - 8.0 * gap(ctx);
    // Fallback ladders per group (ladder[gi][0] is the first shrink
    // step below the declared layout).
    let ladders: Vec<Vec<GroupScale>> = tab
        .groups
        .iter()
        .map(|g| scale_ladder(g, scaling_of(ctx.spec(), g)))
        .collect();
    // Current ladder position per group (0 = declared layout).
    let mut pos: Vec<usize> = vec![0; tab.groups.len()];
    // Stage 1 floor per group (large->small), stage 2 captionless
    // columns per group.
    let mut floor: Vec<Option<ControlScale>> = vec![None; tab.groups.len()];
    let mut caption_cols: Vec<usize> = vec![0; tab.groups.len()];
    let mut sized: Vec<(ControlScale, usize, bool, f32)> = tab
        .groups
        .iter()
        .map(|g| {
            // `ScalingPolicy.IdealSizes`: an optional starting cap
            // (Windows uses it to cap a group below its declared
            // maximum). It can only shrink, never enlarge controls.
            match scaling_of(ctx.spec(), g).and_then(|p| p.ideal.first()) {
                Some(scale) => {
                    let (sc, popup, w) = scaled_group_width(ctx, tab, g, scale, 0);
                    (sc, 0, popup, w)
                }
                None => (
                    ControlScale::Declared,
                    0,
                    false,
                    measure_group_scaled(ctx, g, &ControlScale::Declared, 0),
                ),
            }
        })
        .collect();
    let g = gap(ctx);
    let total = |sized: &[(ControlScale, usize, bool, f32)]| -> f32 {
        if sized.is_empty() {
            0.0
        } else {
            sized.iter().map(|(_, _, _, w)| *w + 6.0 * g).sum::<f32>() - 6.0 * g
        }
    };
    // Cascade stage 1: groups with at least two large buttons shrink
    // them to small, right to left.
    for gi in (0..tab.groups.len()).rev() {
        if total(&sized) <= avail {
            break;
        }
        if large_count(&tab.groups[gi]) >= 2 {
            let sc = ControlScale::Cap(CtrlSize::Small);
            floor[gi] = Some(sc.clone());
            sized[gi] = (
                sc.clone(),
                0,
                false,
                measure_group_scaled(ctx, &tab.groups[gi], &sc, 0),
            );
        }
    }
    // Cascade stage 2: hide small-button captions, right to left, one
    // column at a time (the right-most column first).
    'stage2: for gi in (0..tab.groups.len()).rev() {
        if total(&sized) <= avail {
            break 'stage2;
        }
        let sc = sized[gi].0.clone();
        let (_, ncols) = columns_of(&tab.groups[gi], &sc);
        for off in (caption_cols[gi] + 1)..=ncols {
            if total(&sized) <= avail {
                break 'stage2;
            }
            caption_cols[gi] = off;
            sized[gi] = (
                sc.clone(),
                off,
                false,
                measure_group_scaled(ctx, &tab.groups[gi], &sc, off),
            );
        }
    }
    // Cascade stage 3: walk the groups right-to-left; shrink the
    // right-most group through its whole ladder before touching the
    // next one. Stage 1/2 results are preserved.
    'outer: for gi in (0..tab.groups.len()).rev() {
        while total(&sized) > avail && pos[gi] < ladders[gi].len() {
            let step = pos[gi];
            pos[gi] += 1;
            let (sc, popup, w) =
                scaled_group_width(ctx, tab, &tab.groups[gi], &ladders[gi][step], caption_cols[gi]);
            // never grow back past the stage-1 floor
            let sc = match (&floor[gi], &sc) {
                (Some(f), ControlScale::Declared) => f.clone(),
                (Some(f), ControlScale::Cap(s))
                    if matches!(s, CtrlSize::Large | CtrlSize::Medium) =>
                {
                    f.clone()
                }
                (Some(_), ControlScale::Rules(_)) => floor[gi].clone().unwrap(),
                _ => sc,
            };
            let w = if popup {
                w
            } else {
                measure_group_scaled(ctx, &tab.groups[gi], &sc, caption_cols[gi])
            };
            sized[gi] = (sc, caption_cols[gi], popup, w);
        }
        if total(&sized) <= avail {
            break 'outer;
        }
    }
    let fits = |sized: &[(ControlScale, usize, bool, f32)], n: usize| -> bool {
        if n == 0 {
            true
        } else {
            sized[..n]
                .iter()
                .map(|(_, _, _, w)| *w + 6.0 * g)
                .sum::<f32>()
                - 6.0 * g
                <= avail
        }
    };
    // Hide trailing groups that still do not fit (overflow popup). The
    // « overflow button reserves its own 26px so appearing does not
    // push more groups out.
    let mut n = tab.groups.len();
    let fits_trim = |sized: &[(ControlScale, usize, bool, f32)], n: usize| -> bool {
        if n == 0 {
            true
        } else {
            sized[..n]
                .iter()
                .map(|(_, _, _, w)| *w + 6.0 * g)
                .sum::<f32>()
                - 6.0 * g
                <= avail - 26.0
        }
    };
    while n > 0 && !(fits(&sized, n) || (n < tab.groups.len() && fits_trim(&sized, n))) {
        n -= 1;
    }
    let placed = sized[..n]
        .iter()
        .enumerate()
        .map(|(gi, (sc, cc, popup, w))| GroupPlan {
            gi,
            scale: sc.clone(),
            caption_cols: *cc,
            popup: *popup,
            w: *w,
        })
        .collect();
    let hidden = (n..tab.groups.len()).collect();
    (placed, hidden)
}

/// The active tab index (selected if visible, else first visible).
pub fn active_tab(ctx: &Ctx) -> usize {
    let view = ctx.view();
    let visible = visible_tabs(ctx);
    visible
        .iter()
        .copied()
        .find(|&i| i == view.selected_tab())
        .or_else(|| visible.first().copied())
        .unwrap_or(0)
}

/// Lay out the groups row of a tab; returns (placed, hidden). Kept for
/// API compatibility; superseded by [`plan_groups`].
pub fn place_groups(
    ctx: &Ctx,
    _y: f32,
    width: f32,
    tab_idx: usize,
) -> (Vec<(usize, f32)>, Vec<usize>) {
    let (placed, hidden) = plan_groups(ctx, width, tab_idx);
    (
        placed.into_iter().map(|p| (p.gi, p.w)).collect(),
        hidden,
    )
}

/// Draw the tab body background + groups. Used by the widget body and by
/// the minimized-tab popup (`allow_overflow == false` sizes the popup so
/// everything fits).
pub fn tab_content(
    ctx: &Ctx,
    scene: &mut Scene,
    y: f32,
    width: f32,
    tab_idx: usize,
    allow_overflow: bool,
) {
    let bh = body_h(ctx);
    // body background
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y), Size::new(width, bh)),
        Fill::Gradient(ctx.theme.ribbon.0, ctx.theme.ribbon.1),
        None,
        0.0,
    );
    // bottom separator line — it follows the shifted group titles
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y + bh - 1.0), Size::new(width, 1.0)),
        Fill::Color(ctx.theme.tab_line),
        None,
        0.0,
    );
    let content_y = y + 4.0;
    let tab = &ctx.spec().tabs[tab_idx];
    let (placed, hidden) = if allow_overflow {
        plan_groups(ctx, width, tab_idx)
    } else {
        (
            tab.groups
                .iter()
                .enumerate()
                .map(|(gi, g)| GroupPlan {
                    gi,
                    scale: ControlScale::Declared,
                    caption_cols: 0,
                    popup: false,
                    w: measure_group(ctx, g),
                })
                .collect::<Vec<_>>(),
            Vec::new(),
        )
    };
    let mut x = 4.0 * gap(ctx);
    for (pos, plan) in placed.iter().enumerate() {
        build_group(
            ctx,
            scene,
            &tab.groups[plan.gi],
            x,
            content_y,
            plan.w,
            tab_idx,
            plan.gi,
            false,
            Some(plan),
        );
        let g = gap(ctx);
        x += plan.w + 6.0 * g;
        // Win8: a separator closes every group, the last one included
        quad(
            scene,
            Rectangle::new(
                Point::new(x - 4.0 * g, content_y),
                Size::new(1.0, body_h(ctx) - 8.0),
            ),
            Fill::Color(ctx.theme.group_separator),
            None,
            0.0,
        );
    }
    if allow_overflow {
        if hidden.is_empty() {
            ctx.state.overflow_hidden.take();
        } else {
            ctx.state.overflow_hidden.replace(Some(hidden.clone()));
            ctx.state.overflow_tab.replace(Some(tab_idx));
        }
    }
    if !hidden.is_empty() && allow_overflow {
        let rect = Rectangle::new(
            Point::new(width - 24.0, y + body_h(ctx) / 2.0 - 9.0),
            Size::new(18.0, 18.0),
        );
        let target = Target::Overflow;
        let hover = ctx.hovered(&target);
        quad(
            scene,
            rect,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
            ctx.theme.button_radius,
        );
        text(
            scene,
            rect,
            "«",
            13.0,
            ctx.theme.text,
            Horizontal::Center,
            Vertical::Center,
        );
        scene.push(Node::new(rect, Cmd::Hit).target(target));
    }
}

/// Does this control carry a hideable caption (small/medium labeled
/// button)? Cascade stage 2 hides these column by column.
fn has_caption(control: &ControlSpec, scale: &ControlScale) -> bool {
    let eff = |declared: CtrlSize| {
        scale.effective(resolved_ref(control.command()), declared)
    };
    match control {
        ControlSpec::Button(b) | ControlSpec::ToggleButton(b) => {
            eff(b.size) != CtrlSize::Large && b.command.is_some()
        }
        ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => {
            eff(s.size) != CtrlSize::Large && s.command.is_some()
        }
        ControlSpec::ControlGroup { controls } => {
            controls.iter().any(|c| has_caption(c, scale))
        }
        _ => false,
    }
}

/// Declared-size large buttons in the group (stage 1 shrinks whole
/// groups with at least two of them).
fn large_count(group: &GroupSpec) -> usize {
    fn walk(controls: &[ControlSpec]) -> usize {
        controls
            .iter()
            .map(|c| match c {
                ControlSpec::Button(b) | ControlSpec::ToggleButton(b)
                    if b.size == CtrlSize::Large =>
                {
                    1
                }
                ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s)
                    if s.size == CtrlSize::Large =>
                {
                    1
                }
                ControlSpec::ColorPicker(c) if c.size == CtrlSize::Large => 1,
                ControlSpec::ControlGroup { controls } => walk(controls),
                _ => 0,
            })
            .sum()
    }
    walk(&group.controls)
}

/// Rows per column when packing a group's controls: a stack of exactly
/// four single-row controls packs 2+2 (even columns).
fn rows_per_col(group: &GroupSpec, scale: &ControlScale) -> usize {
    if group.controls.len() == 4
        && group
            .controls
            .iter()
            .all(|c| rows_of_scaled(c, scale) == 1)
    {
        2
    } else {
        3
    }
}

/// Column index of each control plus the column count (same packing as
/// the builders use).
fn columns_of(group: &GroupSpec, scale: &ControlScale) -> (Vec<usize>, usize) {
    let per_col = rows_per_col(group, scale);
    let mut cols = Vec::new();
    let mut col = 0usize;
    let mut used = 0usize;
    for c in &group.controls {
        let span = rows_of_scaled(c, scale);
        if used + span > per_col && used > 0 {
            col += 1;
            used = 0;
        }
        cols.push(col);
        used += span;
    }
    (cols, col + 1)
}

/// Caption hidden for a control in column `col` when the last
/// `caption_cols` columns are captionless?
fn caption_off(
    cols: &[usize],
    ncols: usize,
    caption_cols: usize,
    i: usize,
    control: &ControlSpec,
    scale: &ControlScale,
) -> bool {
    caption_cols > 0
        && cols[i] >= ncols.saturating_sub(caption_cols)
        && has_caption(control, scale)
}

/// Rows (of 3) a control occupies.
pub fn rows_of(control: &ControlSpec) -> usize {
    rows_of_scaled(control, &ControlScale::Declared)
}

/// Rows under a scale (`SizeDefinition` overrides button sizes).
pub fn rows_of_scaled(control: &ControlSpec, scale: &ControlScale) -> usize {
    let declared = match control {
        ControlSpec::Button(b) | ControlSpec::ToggleButton(b) => Some(b.size),
        ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => Some(s.size),
        ControlSpec::ColorPicker(c) => Some(c.size),
        _ => None,
    };
    let size = declared.map(|d| scale.effective(resolved_ref(control.command()), d));
    match control {
        ControlSpec::ComboBox(_)
        | ControlSpec::Spinner(_)
        | ControlSpec::FontControl(_) => 1,
        ControlSpec::InRibbonGallery(_) | ControlSpec::ControlGroup { .. } => 3,
        _ => match size.unwrap_or(CtrlSize::Small) {
            CtrlSize::Large => 3,
            CtrlSize::Medium => 2,
            CtrlSize::Small => 1,
        },
    }
}

/// Natural width of a control.
pub fn width_of(ctx: &Ctx, control: &ControlSpec) -> f32 {
    width_of_scaled(ctx, control, &ControlScale::Declared)
}

/// Width of a control with a scale applied (captions shown).
pub fn width_of_scaled(
    ctx: &Ctx,
    control: &ControlSpec,
    scale: &ControlScale,
) -> f32 {
    width_of_c(ctx, control, scale, false)
}

/// Width of a control with a scale applied; `caption_off` hides the
/// label of small/medium buttons (icon-only width).
pub fn width_of_c(
    ctx: &Ctx,
    control: &ControlSpec,
    scale: &ControlScale,
    caption_off: bool,
) -> f32 {
    let scaled_size = |declared: CtrlSize| {
        scale.effective(resolved_ref(control.command()), declared)
    };
    let co = caption_off;
    match control {
        ControlSpec::Button(b) | ControlSpec::ToggleButton(b) => {
            if co && scaled_size(b.size) != CtrlSize::Large {
                22.0
            } else {
                button_width_scaled(ctx, b, scaled_size(b.size))
            }
        }
        ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => {
            let size = scaled_size(s.size);
            if co && size != CtrlSize::Large {
                return 36.0;
            }
            let w = button_width_scaled(
                ctx,
                &ButtonSpec {
                    command: s.command.clone(),
                    size: s.size,
                    label: s.label.clone(),
                    image: s.image.clone(),
                    keytip: None,
                    tooltip: None,
                    shortcut: None,
                    width: None,
                },
                size,
            );
            // leave room for the dropdown part: the "label ▾" strip on
            // large buttons, the 14px arrow zone on small/medium ones
            match size {
                CtrlSize::Large => w + 12.0,
                _ => w + 14.0,
            }
        }
        ControlSpec::ComboBox(b) => b.width.unwrap_or(110.0),
        ControlSpec::Spinner(sp) => sp.width.unwrap_or(90.0),
        ControlSpec::FontControl(b) => b.width.unwrap_or(150.0),
        ControlSpec::ColorPicker(c) => match scaled_size(c.size) {
            CtrlSize::Large => 52.0,
            _ => 48.0,
        },
        ControlSpec::InRibbonGallery(g) => gallery_width(ctx, g),
        ControlSpec::ControlGroup { controls } => {
            let max = controls
                .iter()
                .map(|c| width_of_c(ctx, c, scale, co))
                .fold(40.0f32, f32::max);
            max + 12.0
        }
    }
}

fn button_width_scaled(ctx: &Ctx, b: &ButtonSpec, size: CtrlSize) -> f32 {
    if let Some(w) = b.width {
        return w;
    }
    let label = b
        .command
        .as_ref()
        .map(|c| match c {
            CommandRef::Id(id) => ctx.label(*id),
            _ => String::new(),
        })
        .or_else(|| b.label.clone())
        .unwrap_or_default();
    match size {
        CtrlSize::Large => {
            let lines = wrap_balanced(ctx, &label, 11.0, 2);
            let max_line = lines
                .iter()
                .map(|l| ctx.text_w(l, 11.0))
                .fold(0.0f32, f32::max);
            max_line.max(28.0) + 12.0
        }
        CtrlSize::Medium => {
            let lines = wrap_width(ctx, &label, 11.0, 64.0);
            let max_line = lines
                .iter()
                .take(2)
                .map(|l| ctx.text_w(l, 11.0))
                .fold(0.0f32, f32::max);
            (max_line + 16.0 + 8.0).max(52.0)
        }
        CtrlSize::Small => (ctx.text_w(&label, 11.0) + 16.0 + 8.0).max(40.0),
    }
}

fn gallery_width(ctx: &Ctx, g: &GallerySpec) -> f32 {
    if g.popup {
        let label_w = g
            .command
            .as_ref()
            .map(|c| match c {
                CommandRef::Id(id) => ctx.text_w(&ctx.label(*id), 11.0),
                _ => 0.0,
            })
            .unwrap_or(0.0);
        return (label_w + 40.0).max(72.0);
    }
    g.columns as f32 * (g.item_width + 8.0) + 22.0
}

pub fn measure_group_pub(ctx: &Ctx, group: &GroupSpec) -> f32 {
    measure_group(ctx, group)
}

fn measure_group(ctx: &Ctx, group: &GroupSpec) -> f32 {
    measure_group_scaled(ctx, group, &ControlScale::Declared, 0)
}

/// Width of a group with a scale applied (`SizeDefinition`); the last
/// `caption_cols` columns render captionless (cascade stage 2).
pub fn measure_group_scaled(
    ctx: &Ctx,
    group: &GroupSpec,
    scale: &ControlScale,
    caption_cols: usize,
) -> f32 {
    let per_col = rows_per_col(group, scale);
    let (cols, ncols) = columns_of(group, scale);
    let mut col_ws: Vec<f32> = Vec::new();
    let mut cur_w = 0.0f32;
    let mut used = 0usize;
    for (i, control) in group.controls.iter().enumerate() {
        let span = rows_of_scaled(control, scale);
        if used + span > per_col && used > 0 {
            col_ws.push(cur_w);
            cur_w = 0.0;
            used = 0;
        }
        let co = caption_off(&cols, ncols, caption_cols, i, control, scale);
        cur_w = cur_w.max(width_of_c(ctx, control, scale, co));
        used += span;
    }
    col_ws.push(cur_w);
    let g = gap(ctx);
    let sum: f32 = col_ws.iter().sum();
    sum + 2.0 * g * (col_ws.len().saturating_sub(1)) as f32 + 8.0 * g
}

/// The group plan used while building (declared sizes by default).
fn plan_or_default(plan: Option<&GroupPlan>, gi: usize, w: f32) -> GroupPlan {
    plan.cloned().unwrap_or(GroupPlan {
        gi,
        scale: ControlScale::Declared,
        caption_cols: 0,
        popup: false,
        w,
    })
}

/// Draw a group body (declared sizes) — used by the collapsed-group
/// popup, which shows the group exactly as it appears in the ribbon.
#[allow(clippy::too_many_arguments)]
pub fn build_group_pub(
    ctx: &Ctx,
    scene: &mut Scene,
    group: &GroupSpec,
    x: f32,
    y: f32,
    width: f32,
    tab_idx: usize,
    group_idx: usize,
) {
    build_group(
        ctx, scene, group, x, y, width, tab_idx, group_idx, false, None,
    );
}

#[allow(clippy::too_many_arguments)]
fn build_group(
    ctx: &Ctx,
    scene: &mut Scene,
    group: &GroupSpec,
    x: f32,
    y: f32,
    width: f32,
    tab_idx: usize,
    group_idx: usize,
    _compact: bool,
    plan: Option<&GroupPlan>,
) {
    let plan = plan_or_default(plan, group_idx, width);
    if plan.popup {
        // Collapsed group (Windows `Group Size="Popup"`): a single
        // vertical button with the group icon, label and chevron that
        // opens the full group in a popup. It fills the whole command
        // pane — there is no separate group caption under it.
        let rect = Rectangle::new(Point::new(x, y), Size::new(width, body_h(ctx) - 10.0));
        let target = Target::OverflowGroup(tab_idx, group_idx);
        let hover = ctx.hovered(&target);
        quad(
            scene,
            rect,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
            ctx.theme.button_radius,
        );
        scene.push(Node::new(rect, Cmd::Hit).target(target));
        let img = group
            .image
            .clone()
            .map(crate::ribbon::image::UiImage::Named)
            .unwrap_or_default();
        let label = group.label.clone().unwrap_or_else(|| group.id.clone());
        large_content(
            ctx, scene, rect, img, &label, ctx.theme.text, 1.0,
            Some(ctx.theme.group_label),
        );
        return;
    }
    let scale = &plan.scale;
    let g = gap(ctx);
    // controls in columns (a stack of four single-row controls packs 2+2)
    let per_col = rows_per_col(group, scale);
    let (cols, ncols) = columns_of(group, scale);
    let mut col_x = x + 4.0 * g;
    let mut used = 0usize;
    let mut col_width = 0.0f32;
    let mut pending: Vec<(&ControlSpec, f32, f32, bool)> = Vec::new();

    let flush = |scene: &mut Scene,
                     col_x: f32,
                     col_width: f32,
                     pending: &mut Vec<(&ControlSpec, f32, f32, bool)>| {
        for (control, rel_y, h, co) in pending.drain(..) {
            let rect = Rectangle::new(
                Point::new(col_x, y + rel_y),
                Size::new(col_width, h),
            );
            build_control_scaled(
                ctx,
                scene,
                control,
                rect,
                ControlLoc {
                    tab: tab_idx,
                    group: group_idx,
                },
                false,
                scale,
                co,
            );
        }
    };

    for (i, control) in group.controls.iter().enumerate() {
        let span = rows_of_scaled(control, scale);
        if used + span > per_col && used > 0 {
            flush(scene, col_x, col_width, &mut pending);
            col_x += col_width + 2.0 * g;
            col_width = 0.0;
            used = 0;
        }
        let h = span as f32 * ROW_H;
        let co = caption_off(&cols, ncols, plan.caption_cols, i, control, scale);
        let w = width_of_c(ctx, control, scale, co);
        col_width = col_width.max(w);
        pending.push((control, used as f32 * ROW_H, h, co));
        used += span;
    }
    flush(scene, col_x, col_width, &mut pending);

    // footer: caption + dialog launcher (footer_shift px lower — the
    // bottom separator line follows, see body())
    let footer_y = y + CONTENT_H + 2.0 + ctx.spec().metrics.footer_shift;
    let label = group
        .label
        .clone()
        .unwrap_or_else(|| group.id.clone());
    // caption centered between the group's separators: the zone runs
    // from 4px left of the content to 2px right of it
    text(
        scene,
        Rectangle::new(
            Point::new(x - 3.0, footer_y),
            Size::new(width + 6.0, FOOTER_H - 2.0),
        ),
        label,
        10.5,
        ctx.theme.group_label,
        Horizontal::Center,
        Vertical::Center,
    );
    if let Some(cmd) = group.command.as_ref().and_then(|c| match c {
        crate::ribbon::spec::CommandRef::Id(id) => Some(*id),
        _ => None,
    }) {
        let rect = Rectangle::new(
            Point::new(x + width - 16.0, footer_y + 2.0),
            Size::new(12.0, 12.0),
        );
        let target = Target::Exec(cmd);
        let hover = ctx.enabled(cmd) && ctx.hovered(&target);
        quad(
            scene,
            rect,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
            ctx.theme.button_radius,
        );
        text(
            scene,
            rect,
            "↗",
            9.0,
            ctx.theme.group_label,
            Horizontal::Center,
            Vertical::Center,
        );
        scene.push(Node::new(rect, Cmd::Text {
            text: String::new(),
            size: 1.0,
            color: Color::TRANSPARENT,
            halign: Horizontal::Left,
            valign: Vertical::Top,
            bold: false,
        }).target(target).tip(cmd));
    }
}

/// Where a control lives (for overflow popups).
#[derive(Clone, Copy)]
pub struct ControlLoc {
    pub tab: usize,
    pub group: usize,
}

/// Build one control into the scene at `rect` (already sized).
pub fn build_control(
    ctx: &Ctx,
    scene: &mut Scene,
    control: &ControlSpec,
    rect: Rectangle,
    loc: ControlLoc,
    compact: bool,
) {
    build_control_scaled(ctx, scene, control, rect, loc, compact, &ControlScale::Declared, false);
}

/// Build one control with a group scale applied (`SizeDefinition`
/// overrides button sizes for listed commands).
#[allow(clippy::too_many_arguments)]
pub fn build_control_scaled(
    ctx: &Ctx,
    scene: &mut Scene,
    control: &ControlSpec,
    rect: Rectangle,
    loc: ControlLoc,
    compact: bool,
    scale: &ControlScale,
    co: bool,
) {
    let eff = |declared: CtrlSize| {
        scale.effective(resolved_ref(control.command()), declared)
    };
    match control {
        ControlSpec::Button(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_button_like(
                    ctx, scene, rect, cmd, eff(b.size), false, compact, co,
                );
            }
        }
        ControlSpec::ToggleButton(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_button_like(
                    ctx, scene, rect, cmd, eff(b.size), true, compact, co,
                );
            }
        }
        ControlSpec::SplitButton(s) => {
            if let Some(cmd) = resolved(s.command.as_ref()) {
                if eff(s.size) == CtrlSize::Large && !s.separate && !compact {
                    build_large_dropdown(ctx, scene, rect, cmd);
                } else {
                    build_split(
                        ctx, scene, rect, cmd, eff(s.size), true, compact, co,
                    );
                }
            }
        }
        ControlSpec::DropDownButton(s) => {
            if let Some(cmd) = resolved(s.command.as_ref()) {
                if eff(s.size) == CtrlSize::Large && !compact {
                    build_large_dropdown(ctx, scene, rect, cmd);
                } else {
                    build_split(
                        ctx, scene, rect, cmd, eff(s.size), false, compact, co,
                    );
                }
            }
        }
        ControlSpec::ComboBox(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_combo(ctx, scene, rect, cmd);
            }
        }
        ControlSpec::Spinner(s) => {
            if let Some(cmd) = resolved(s.command.as_ref()) {
                build_spinner(ctx, scene, rect, cmd);
            }
        }
        ControlSpec::FontControl(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_font_control(ctx, scene, rect, cmd);
            }
        }
        ControlSpec::ColorPicker(c) => {
            if let Some(cmd) = resolved(c.command.as_ref()) {
                build_color_picker(ctx, scene, rect, cmd, c);
            }
        }
        ControlSpec::InRibbonGallery(g) => {
            if let Some(cmd) = resolved(g.command.as_ref()) {
                build_gallery(ctx, scene, rect, cmd, g, loc);
            }
        }
        ControlSpec::ControlGroup { controls } => {
            build_control_group(ctx, scene, controls, rect, loc, compact, co);
        }
    }
}

/// Shared large-button geometry: a 32px icon at the top, a balanced
/// two-line label under it; with a dropdown the chevron trails the last
/// label line (like a trailing character), never a third line below.
/// Large buttons, large dropdown buttons and collapsed (popup) groups
/// all use these offsets so their icons and text align.
pub const LARGE_ICON_Y: f32 = 3.0;
pub const LARGE_TEXT_Y: f32 = 38.0;
pub const LARGE_LINE_H: f32 = 12.0;

#[allow(clippy::too_many_arguments)]
fn large_content(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    img: crate::ribbon::image::UiImage,
    label: &str,
    text_color: Color,
    opacity: f32,
    chevron_color: Option<Color>,
) {
    svg_node(
        scene,
        Rectangle::new(
            Point::new(
                rect.x + (rect.width - 32.0) / 2.0,
                rect.y + LARGE_ICON_Y,
            ),
            Size::new(32.0, 32.0),
        ),
        img,
        opacity,
    );
    // at most 2 lines; the arrow follows the last line ("Move to ▾",
    // alone on the second line for single-line labels: "Paste ▾")
    let mut lines = wrap_balanced(ctx, label, 11.0, 2);
    lines.truncate(2);
    if chevron_color.is_some() && lines.len() == 1 {
        lines.push(String::new());
    }
    let last = lines.len() - 1;
    for (i, line) in lines.iter().enumerate() {
        let lrect = Rectangle::new(
            Point::new(rect.x, rect.y + LARGE_TEXT_Y + i as f32 * LARGE_LINE_H),
            Size::new(rect.width, LARGE_LINE_H),
        );
        if i == last && chevron_color.is_some() {
            text_arrow(ctx, scene, lrect, line, 11.0, chevron_color.unwrap_or(text_color));
        } else {
            text(
                scene,
                lrect,
                line.clone(),
                11.0,
                text_color,
                Horizontal::Center,
                Vertical::Center,
            );
        }
    }
}

/// Large menu-only button (`drop_down_button`, or `split_button` with
/// the default single-element chrome): one target opening the menu,
/// content shared with large buttons plus the chevron.
fn build_large_dropdown(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, cmd: u32) {
    let enabled = ctx.enabled(cmd);
    push_button(ctx, scene, rect, Target::DropDown(cmd), cmd, false, ctx.theme.button_radius);
    let (color, opacity) = if enabled {
        (ctx.theme.text, 1.0)
    } else {
        (ctx.theme.text_disabled, 0.35)
    };
    let img = ctx.large_image(cmd).unwrap_or_default();
    let label = ctx.label(cmd);
    large_content(ctx, scene, rect, img, &label, color, opacity, Some(color));
    keytip_badge(ctx, scene, rect, cmd, false);
}

/// Advance width of the self-drawn arrow next to text (3px gap + 7px
/// glyph) — used for centering exactly as if the arrow were a trailing
/// character in the string.
pub const ARROW_ADVANCE: f32 = 10.0;

/// Self-drawn solid down-triangle (font-independent ▼), centered at
/// (`cx`, `cy`) with the given glyph size.
fn triangle_down(scene: &mut Scene, cx: f32, cy: f32, w: f32, h: f32, color: Color) {
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8
    );
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 8'><path d='M0.5 0.5h9L5 7.5z' fill='{hex}'/></svg>"
    );
    scene.push(Node::new(
        Rectangle::new(Point::new(cx - w / 2.0, cy - h / 2.0), Size::new(w, h)),
        Cmd::Svg {
            image: crate::ribbon::image::UiImage::Svg(svg),
            tint: None,
            opacity: 1.0,
        },
    ));
}

/// Draw `line` centered in `rect` with a self-drawn down triangle after
/// it — the pair is centered as one unit, exactly like a string with a
/// trailing arrow character (an empty `line` draws just the arrow).
fn text_arrow(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    line: &str,
    size: f32,
    color: Color,
) {
    let tw = if line.is_empty() {
        0.0
    } else {
        ctx.text_w(line, size)
    };
    let total = tw + if line.is_empty() { 7.0 } else { ARROW_ADVANCE };
    let x0 = rect.x + ((rect.width - total) / 2.0).max(0.0);
    if !line.is_empty() {
        text(
            scene,
            Rectangle::new(Point::new(x0, rect.y), Size::new(tw + 1.0, rect.height)),
            line,
            size,
            color,
            Horizontal::Left,
            Vertical::Center,
        );
    }
    triangle_down(scene, x0 + total - 3.5, rect.y + rect.height / 2.0 + 0.5, 7.0, 5.0, color);
}

fn resolved(c: Option<&crate::ribbon::spec::CommandRef>) -> Option<u32> {
    match c {
        Some(crate::ribbon::spec::CommandRef::Id(id)) => Some(*id),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_button_like(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    size: CtrlSize,
    toggle: bool,
    compact: bool,
    caption_off: bool,
) {
    let checked = toggle && ctx.bool_value(cmd);
    let enabled = ctx.enabled(cmd);
    let target = if toggle { Target::Toggle(cmd) } else { Target::Exec(cmd) };
    if !push_button(ctx, scene, rect, target.clone(), cmd, checked, ctx.theme.button_radius) {
        // disabled: still draw contents dimmed
    }
    let opacity = if enabled { 1.0 } else { 0.35 };
    let label = ctx.label(cmd);
    let text_color = if enabled {
        ctx.theme.text
    } else {
        ctx.theme.text_disabled
    };
    let show_label = (!compact || size == CtrlSize::Small) && !caption_off;

    match size {
        CtrlSize::Large if !compact => {
            let img = ctx.large_image(cmd).unwrap_or_default();
            large_content(ctx, scene, rect, img, &label, text_color, opacity, None);
        }
        _ => {
            // small / medium / compact rows: icon left, label right
            let img = ctx.small_image(cmd).unwrap_or_default();
            let icon_y = rect.y + (rect.height - 16.0) / 2.0;
            svg_node(
                scene,
                Rectangle::new(Point::new(rect.x + 3.0, icon_y), Size::new(16.0, 16.0)),
                img,
                opacity,
            );
            if show_label {
                let max_w = (rect.width - 22.0).max(10.0);
                let lines = wrap_width(ctx, &label, 11.0, max_w);
                let n = if size == CtrlSize::Medium && !compact {
                    2
                } else {
                    1
                };
                let line_h = 11.0f32;
                let total = n as f32 * line_h;
                let start_y = rect.y + (rect.height - total) / 2.0;
                for (i, line) in lines.iter().take(n).enumerate() {
                    text(
                        scene,
                        Rectangle::new(
                            Point::new(rect.x + 21.0, start_y + i as f32 * line_h),
                            Size::new(max_w, line_h),
                        ),
                        line.clone(),
                        11.0,
                        text_color,
                        Horizontal::Left,
                        Vertical::Center,
                    );
                }
            }
        }
    }
    keytip_badge(ctx, scene, rect, cmd, false);
}

/// Large split button (Paste, Move to): the icon sits on top in the main
/// zone; the bottom strip carries "label ▾" as one bound unit — the
/// chevron belongs to the text, not to a separate arrow zone at the
/// right. Hovering either part fills that part; the shared border and the
/// divider appear whenever either part is active.
fn build_split_large(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    has_main: bool,
) {
    let strip_h = rect.height - LARGE_TEXT_Y;
    let enabled = ctx.enabled(cmd);
    let main_t = if has_main { Target::SplitMain(cmd) } else { Target::DropDown(cmd) };
    let drop_t = if has_main { Target::SplitDrop(cmd) } else { Target::DropDown(cmd) };
    let main_hover = enabled && (ctx.hovered(&main_t) || ctx.pressed(&main_t));
    let drop_hover = enabled && (ctx.hovered(&drop_t) || ctx.pressed(&drop_t));
    let main_pressed = enabled && ctx.pressed(&main_t);
    let drop_pressed = enabled && ctx.pressed(&drop_t);
    let main_rect = Rectangle::new(
        rect.position(),
        Size::new(rect.width, rect.height - strip_h),
    );
    let drop_rect = Rectangle::new(
        Point::new(rect.x, rect.y + rect.height - strip_h),
        Size::new(rect.width, strip_h),
    );
    let theme = ctx.theme;
    let part_fill = |hover: bool, pressed: bool| {
        if pressed {
            Fill::Color(theme.pressed_bg)
        } else if hover {
            Fill::Color(theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        }
    };
    // fills per part
    quad(scene, main_rect, part_fill(main_hover, main_pressed), None, 0.0);
    quad(scene, drop_rect, part_fill(drop_hover, drop_pressed), None, 0.0);
    // bound chrome: shared border + divider whenever either part is active
    if main_hover || drop_hover {
        let border_c = theme
            .pressed_border
            .filter(|_| main_pressed || drop_pressed)
            .or(theme.hover_border);
        quad(
            scene,
            rect,
            Fill::Color(Color::TRANSPARENT),
            border_c.map(|c| (c, 1.0)),
            theme.button_radius,
        );
        if let Some(c) = border_c {
            quad(
                scene,
                Rectangle::new(
                    Point::new(rect.x, drop_rect.y),
                    Size::new(rect.width, 1.0),
                ),
                Fill::Color(c),
                None,
                0.0,
            );
        }
    }
    scene.push(Node::new(main_rect, Cmd::Hit).target(main_t).tip(cmd));
    scene.push(Node::new(drop_rect, Cmd::Hit).target(drop_t).tip(cmd));
    // icon in the main zone, same offset as every large button
    let opacity = if enabled { 1.0 } else { 0.35 };
    let img = ctx.large_image(cmd).unwrap_or_default();
    svg_node(
        scene,
        Rectangle::new(
            Point::new(
                main_rect.x + (main_rect.width - 32.0) / 2.0,
                main_rect.y + LARGE_ICON_Y,
            ),
            Size::new(32.0, 32.0),
        ),
        img,
        opacity,
    );
    // wrapped "label ▾" as one bound unit: minimal-width wrap, the
    // chevron follows the last line ("Paste\n▼", "Move\nto ▾")
    let label = ctx.label(cmd);
    let text_color = if enabled {
        ctx.theme.text
    } else {
        ctx.theme.text_disabled
    };
    // the arrow follows the label as if it were a trailing character
    // ("Move\nto ▼", alone on the second line for single-line labels:
    // "Paste\n▼") — self-drawn, so it works with any font
    let mut lines = wrap_balanced(ctx, &label, 11.0, 2);
    if lines.len() == 1 {
        lines.push(String::new());
    }
    let line_h = LARGE_LINE_H;
    let last = lines.len() - 1;
    for (i, line) in lines.iter().enumerate() {
        // same text offsets as plain large buttons
        let lrect = Rectangle::new(
            Point::new(drop_rect.x, rect.y + LARGE_TEXT_Y + i as f32 * line_h),
            Size::new(drop_rect.width, line_h),
        );
        if i == last {
            text_arrow(ctx, scene, lrect, line, 11.0, text_color);
        } else {
            text(
                scene,
                lrect,
                line.clone(),
                11.0,
                text_color,
                Horizontal::Center,
                Vertical::Center,
            );
        }
    }
    keytip_badge(ctx, scene, rect, cmd, false);
}

#[allow(clippy::too_many_arguments)]
fn build_split(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    size: CtrlSize,
    has_main: bool,
    compact: bool,
    caption_off: bool,
) {
    if size == CtrlSize::Large && !compact {
        build_split_large(ctx, scene, rect, cmd, has_main);
        return;
    }
    let drop_w = 14.0f32;
    let main_rect = Rectangle::new(
        rect.position(),
        Size::new((rect.width - drop_w).max(10.0), rect.height),
    );
    let drop_rect = Rectangle::new(
        Point::new(rect.x + main_rect.width, rect.y),
        Size::new(drop_w, rect.height),
    );
    build_button_like(ctx, scene, main_rect, cmd, size, false, compact, caption_off);
    // dropdown zone
    let target = if has_main { Target::SplitDrop(cmd) } else { Target::DropDown(cmd) };
    let enabled = ctx.enabled(cmd);
    let hover = enabled && ctx.hovered(&target);
    quad(
        scene,
        drop_rect,
        if hover {
            Fill::Color(ctx.theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        },
        ctx.theme.hover_border.filter(|_| hover).map(|c| (c, 1.0)),
        ctx.theme.button_radius,
    );
    chevron(
        scene,
        Rectangle::new(
            Point::new(drop_rect.x + 2.0, drop_rect.y + drop_rect.height / 2.0 - 3.0),
            Size::new(10.0, 6.5),
        ),
        false,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
    );
    scene.push(Node::new(drop_rect, Cmd::Hit).target(target).tip(cmd));
}

fn build_combo(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, cmd: u32) {
    let editing = matches!(
        &ctx.state.edit,
        Some(EditState { cmd: c, .. }) if *c == cmd
    );
    let enabled = ctx.enabled(cmd);
    let border = if editing {
        Some((ctx.theme.accent, 1.0))
    } else {
        Some((ctx.theme.border, 1.0))
    };
    quad(
        scene,
        rect,
        Fill::Color(Color::WHITE),
        border,
        ctx.theme.field_radius,
    );
    let text_rect = Rectangle::new(
        Point::new(rect.x + 4.0, rect.y),
        Size::new(rect.width - 20.0, rect.height),
    );
    let content = if editing {
        ctx.state
            .edit
            .as_ref()
            .map(|e| e.text.clone())
            .unwrap_or_default()
    } else {
        ctx.value(cmd, PropertyKey::StringValue)
            .and_then(|v| v.as_str().map(str::to_owned))
            .or_else(|| {
                ctx.value(cmd, PropertyKey::SelectedItem)
                    .and_then(|v| v.as_u32())
                    .and_then(|i| {
                        ctx.value(cmd, PropertyKey::ItemsSource)
                            .and_then(|c| c.as_collection().cloned())
                            .and_then(|col| col.lock().ok()?.label_of(i as usize))
                    })
            })
            .unwrap_or_default()
    };
    let caret_x = rect.x + 6.0 + ctx.text_w(&content, 11.5);
    text(
        scene,
        text_rect,
        content,
        11.5,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
        Horizontal::Left,
        Vertical::Center,
    );
    if editing {
        // caret
        quad(
            scene,
            Rectangle::new(
                Point::new(caret_x, rect.y + 4.0),
                Size::new(1.0, rect.height - 8.0),
            ),
            Fill::Color(ctx.theme.text),
            None,
            0.0,
        );
    }
    scene.push(Node::new(text_rect, Cmd::Hit).target(Target::ComboMain(cmd)).tip(cmd));

    let drop = Rectangle::new(
        Point::new(rect.x + rect.width - 16.0, rect.y),
        Size::new(16.0, rect.height),
    );
    let hover = enabled && ctx.hovered(&Target::ComboDrop(cmd));
    quad(
        scene,
        drop,
        if hover {
            Fill::Color(ctx.theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        },
        None,
        0.0,
    );
    chevron(
        scene,
        Rectangle::new(
            Point::new(drop.x + 3.0, drop.y + drop.height / 2.0 - 3.0),
            Size::new(10.0, 6.5),
        ),
        false,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
    );
    scene.push(Node::new(drop, Cmd::Hit).target(Target::ComboDrop(cmd)));
    keytip_badge(ctx, scene, rect, cmd, false);
}

fn build_spinner(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, cmd: u32) {
    let editing = matches!(
        &ctx.state.edit,
        Some(EditState { cmd: c, .. }) if *c == cmd
    );
    let enabled = ctx.enabled(cmd);
    let border = if editing {
        Some((ctx.theme.accent, 1.0))
    } else {
        Some((ctx.theme.border, 1.0))
    };
    quad(
        scene,
        rect,
        Fill::Color(Color::WHITE),
        border,
        ctx.theme.field_radius,
    );

    let value = ctx
        .value(cmd, PropertyKey::DecimalValue)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let places = ctx
        .value(cmd, PropertyKey::DecimalPlaces)
        .and_then(|v| v.as_u32())
        .unwrap_or(0);
    let formatted = if editing {
        ctx.state.edit.as_ref().map(|e| e.text.clone()).unwrap_or_default()
    } else {
        let format = ctx
            .str_prop(cmd, PropertyKey::FormatString)
            .unwrap_or_else(|| "{0}".into());
        format.replace(
            "{0}",
            &format!("{:.*}", places as usize, value),
        )
    };
    let text_rect = Rectangle::new(
        Point::new(rect.x + 4.0, rect.y),
        Size::new(rect.width - 20.0, rect.height),
    );
    let caret_x = rect.x + 6.0 + ctx.text_w(&formatted, 11.5);
    text(
        scene,
        text_rect,
        formatted,
        11.5,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
        Horizontal::Left,
        Vertical::Center,
    );
    if editing {
        quad(
            scene,
            Rectangle::new(
                Point::new(caret_x, rect.y + 4.0),
                Size::new(1.0, rect.height - 8.0),
            ),
            Fill::Color(ctx.theme.text),
            None,
            0.0,
        );
    }
    scene.push(Node::new(text_rect, Cmd::Hit).target(Target::SpinnerText(cmd)).tip(cmd));

    // up / down buttons
    let bx = rect.x + rect.width - 16.0;
    let half = rect.height / 2.0;
    for (i, target) in [
        Target::SpinnerUp(cmd),
        Target::SpinnerDown(cmd),
    ]
    .into_iter()
    .enumerate()
    {
        let z = Rectangle::new(
            Point::new(bx, rect.y + i as f32 * half),
            Size::new(16.0, half),
        );
        let hover = enabled && ctx.hovered(&target);
        quad(
            scene,
            z,
            if hover {
                Fill::Color(ctx.theme.hover_bg)
            } else {
                Fill::Color(Color::TRANSPARENT)
            },
            None,
            0.0,
        );
        chevron(
            scene,
            Rectangle::new(
                Point::new(z.x + 3.5, z.y + (half - 5.0) / 2.0),
                Size::new(9.0, 5.0),
            ),
            i == 0,
            if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
        );
        scene.push(Node::new(z, Cmd::Text {
            text: String::new(),
            size: 1.0,
            color: Color::TRANSPARENT,
            halign: Horizontal::Left,
            valign: Vertical::Top,
            bold: false,
        }).target(target));
    }
    keytip_badge(ctx, scene, rect, cmd, false);
}

fn build_font_control(ctx: &Ctx, scene: &mut Scene, rect: Rectangle, cmd: u32) {
    let enabled = ctx.enabled(cmd);
    let hover = enabled && ctx.hovered(&Target::DropDown(cmd));
    quad(
        scene,
        rect,
        Fill::Color(Color::WHITE),
        Some((
            if hover {
                ctx.theme.hover_border.unwrap_or(ctx.theme.border)
            } else {
                ctx.theme.border
            },
            1.0,
        )),
        1.0,
    );
    let props = ctx
        .value(cmd, PropertyKey::FontProperties)
        .and_then(|v| v.as_font().cloned())
        .unwrap_or_default();
    let label = format!(
        "{}\u{2000}{}",
        props.family.clone().unwrap_or_else(|| "Calibri".into()),
        props
            .size
            .map(|s| s.to_string())
            .unwrap_or_else(|| "11".into())
    );
    text(
        scene,
        Rectangle::new(
            Point::new(rect.x + 5.0, rect.y),
            Size::new(rect.width - 22.0, rect.height),
        ),
        label,
        11.5,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
        Horizontal::Left,
        Vertical::Center,
    );
    let drop = Rectangle::new(
        Point::new(rect.x + rect.width - 16.0, rect.y),
        Size::new(16.0, rect.height),
    );
    chevron(
        scene,
        Rectangle::new(
            Point::new(drop.x + 3.0, drop.y + drop.height / 2.0 - 3.0),
            Size::new(10.0, 6.5),
        ),
        false,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
    );
    scene.push(Node::new(rect, Cmd::Hit).target(Target::DropDown(cmd)).tip(cmd));
    keytip_badge(ctx, scene, rect, cmd, false);
}

fn build_color_picker(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    c: &crate::ribbon::spec::ColorSpec,
) {
    let enabled = ctx.enabled(cmd);
    let color = ctx
        .value(cmd, PropertyKey::Color)
        .and_then(|v| v.as_color())
        .map(crate::ribbon::theme::rgb)
        .unwrap_or(crate::ribbon::theme::rgb(0xC00000));
    let monochrome = matches!(
        ctx.value(cmd, PropertyKey::ColorMode),
        Some(PropValue::ColorMode(SwatchColorMode::Monochrome))
    );

    let main_rect = Rectangle::new(
        rect.position(),
        Size::new(rect.width - 14.0, rect.height),
    );
    let target = Target::ColorMain(cmd);
    if push_button(ctx, scene, main_rect, target.clone(), cmd, false, ctx.theme.button_radius) {
        let img = c
            .image
            .as_ref()
            .map(|i| crate::ribbon::image::UiImage::Named(i.clone()))
            .or_else(|| ctx.small_image(cmd))
            .unwrap_or_else(|| crate::ribbon::image::UiImage::Icon("font_color"));
        let icon_size = if c.size == CtrlSize::Large && rect.height > 40.0 {
            32.0
        } else {
            16.0
        };
        svg_node(
            scene,
            Rectangle::new(
                Point::new(
                    main_rect.x + (main_rect.width - icon_size) / 2.0,
                    main_rect.y + (main_rect.height - icon_size) / 2.0 - 2.0,
                ),
                Size::new(icon_size, icon_size),
            ),
            img,
            if enabled { 1.0 } else { 0.35 },
        );
        if !monochrome {
            quad(
                scene,
                Rectangle::new(
                    Point::new(
                        main_rect.x + (main_rect.width - icon_size) / 2.0,
                        main_rect.y + main_rect.height / 2.0 + icon_size / 2.0 - 4.0,
                    ),
                    Size::new(icon_size, 3.0),
                ),
                Fill::Color(color),
                None,
                0.0,
            );
        }
    }

    let drop = Rectangle::new(
        Point::new(main_rect.x + main_rect.width, rect.y),
        Size::new(14.0, rect.height),
    );
    let dtarget = Target::ColorDrop(cmd);
    let dhover = enabled && ctx.hovered(&dtarget);
    quad(
        scene,
        drop,
        if dhover {
            Fill::Color(ctx.theme.hover_bg)
        } else {
            Fill::Color(Color::TRANSPARENT)
        },
        ctx.theme.hover_border.filter(|_| dhover).map(|c| (c, 1.0)),
        ctx.theme.button_radius,
    );
    chevron(
        scene,
        Rectangle::new(
            Point::new(drop.x + 2.0, drop.y + drop.height / 2.0 - 3.0),
            Size::new(10.0, 6.5),
        ),
        false,
        if enabled { ctx.theme.text } else { ctx.theme.text_disabled },
    );
    scene.push(Node::new(drop, Cmd::Hit).target(dtarget).tip(cmd));
    keytip_badge(ctx, scene, rect, cmd, false);
}

fn build_gallery(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    g: &GallerySpec,
    _loc: ControlLoc,
) {
    if g.popup {
        // dropdown gallery: button-like with chevron
        build_split(ctx, scene, rect, cmd, CtrlSize::Small, false, false, false);
        return;
    }
    let enabled = ctx.enabled(cmd);
    let selected = ctx
        .value(cmd, PropertyKey::SelectedItem)
        .and_then(|v| v.as_u32())
        .unwrap_or(0) as usize;
    let items = ctx
        .value(cmd, PropertyKey::ItemsSource)
        .and_then(|v| v.as_collection().cloned());

    // frame
    quad(
        scene,
        rect,
        Fill::Color(Color::WHITE),
        Some((ctx.theme.border, 1.0)),
        ctx.theme.field_radius,
    );

    let scroll = *ctx.state.gallery_scroll.get(&cmd).unwrap_or(&0);
    if let Some(items) = items {
        let Ok(items) = items.lock() else { return };
        let count = items.get_count();
        for col in 0..g.columns {
            let idx = scroll + col;
            if idx >= count {
                break;
            }
            let item = items.get_item(idx);
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
            let ix = rect.x + 2.0 + col as f32 * (g.item_width + 6.0);
            let cell = Rectangle::new(
                Point::new(ix, rect.y + 2.0),
                Size::new(g.item_width + 4.0, rect.height - 4.0),
            );
            if idx == selected {
                quad(
                    scene,
                    cell,
                    Fill::Color(ctx.theme.checked_bg),
                    Some((ctx.theme.checked_border.unwrap_or(ctx.theme.border), 1.0)),
                    2.0,
                );
            }
            if let Some(image) = image {
                svg_node(
                    scene,
                    Rectangle::new(
                        Point::new(ix + 2.0, rect.y + 4.0),
                        Size::new(g.item_width, g.item_height),
                    ),
                    image,
                    if enabled { 1.0 } else { 0.35 },
                );
            }
            text(
                scene,
                Rectangle::new(
                    Point::new(ix, rect.y + 6.0 + g.item_height),
                    Size::new(g.item_width + 4.0, 12.0),
                ),
                label,
                9.0,
                ctx.theme.text,
                Horizontal::Center,
                Vertical::Center,
            );
            scene.push(Node::new(cell, Cmd::Hit).target(Target::GalleryItem {
                cmd,
                index: idx,
                popup: false,
            }).tip(cmd));
        }

        // scroll arrows bottom-right
        let can_left = scroll > 0;
        let can_right = scroll + g.columns < count;
        for (i, (can, target)) in [
            (can_left, Target::GalleryScrollH(cmd, -1)),
            (can_right, Target::GalleryScrollH(cmd, 1)),
        ]
        .into_iter()
        .enumerate()
        {
            let z = Rectangle::new(
                Point::new(
                    rect.x + rect.width - 36.0 + i as f32 * 18.0,
                    rect.y + rect.height - 17.0,
                ),
                Size::new(16.0, 15.0),
            );
            quad(
                scene,
                z,
                if can {
                    Fill::Color(ctx.theme.qat_bg.0)
                } else {
                    Fill::Color(Color::TRANSPARENT)
                },
                Some((ctx.theme.border, 1.0)),
                ctx.theme.button_radius,
            );
            let chev_up = i == 0;
            let chev_svg = if chev_up {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M2 6.4L5 3l3 3.4" fill="none" stroke="#333" stroke-width="1.4"/></svg>"##
            } else {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M2 3.6L5 7l3-3.4" fill="none" stroke="#333" stroke-width="1.4"/></svg>"##
            };
            if can {
                svg_node(
                    scene,
                    Rectangle::new(Point::new(z.x + 3.0, z.y + 2.5), Size::new(10.0, 10.0)),
                    crate::ribbon::image::UiImage::Svg(chev_svg.into()),
                    if enabled { 1.0 } else { 0.35 },
                );
                scene.push(Node::new(z, Cmd::Hit).target(target));
            }
        }
    }
    keytip_badge(ctx, scene, rect, cmd, false);
}

fn build_control_group(
    ctx: &Ctx,
    scene: &mut Scene,
    controls: &[ControlSpec],
    rect: Rectangle,
    loc: ControlLoc,
    compact: bool,
    co: bool,
) {
    quad(
        scene,
        rect,
        Fill::Color(Color::TRANSPARENT),
        Some((ctx.theme.group_separator, 1.0)),
        ctx.theme.field_radius,
    );
    let n = controls.len().max(1) as f32;
    let inner_h = (rect.height - 6.0) / n;
    for (i, control) in controls.iter().enumerate() {
        let crect = Rectangle::new(
            Point::new(rect.x + 4.0, rect.y + 3.0 + i as f32 * inner_h),
            Size::new(rect.width - 8.0, inner_h - 2.0),
        );
        build_control_scaled(
            ctx, scene, control, crect, loc, compact, &ControlScale::Declared, co,
        );
    }
}

/// Font toggle buttons are used in the font-control popup; expose for
/// reuse there.
pub fn font_toggle_label(t: FontToggle) -> &'static str {
    t.label()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scale step may only shrink controls; declared-small buttons in
    /// a mixed group must never be grown to large (Windows caps sizes).
    #[test]
    fn scale_only_shrinks() {
        let cap_large = ControlScale::Cap(CtrlSize::Large);
        assert_eq!(cap_large.effective(Some(1), CtrlSize::Small), CtrlSize::Small);
        assert_eq!(cap_large.effective(Some(1), CtrlSize::Large), CtrlSize::Large);

        let cap_medium = ControlScale::Cap(CtrlSize::Medium);
        assert_eq!(cap_medium.effective(Some(1), CtrlSize::Large), CtrlSize::Medium);
        assert_eq!(cap_medium.effective(Some(1), CtrlSize::Small), CtrlSize::Small);

        let rules = ControlScale::Rules(vec![(7, CtrlSize::Large)]);
        // explicit rule may not grow a small control either
        assert_eq!(rules.effective(Some(7), CtrlSize::Small), CtrlSize::Small);
        // ...but shrinks a large one; unlisted commands are untouched
        assert_eq!(rules.effective(Some(7), CtrlSize::Large), CtrlSize::Large);
        assert_eq!(rules.effective(Some(8), CtrlSize::Large), CtrlSize::Large);
    }
}

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
use super::state::EditState;
use super::Target;

pub const QAT_H: f32 = QAT_ROW_HEIGHT;
pub const STRIP_H: f32 = TAB_STRIP_HEIGHT;
pub const BODY_H: f32 = RIBBON_BODY_HEIGHT;
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
        self.state.hover.as_ref() == Some(t)
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
    let hover = enabled && ctx.hovered(&target);
    let pressed = enabled && ctx.pressed(&target);
    let theme = ctx.theme;
    let (fill, border) = if pressed {
        (Fill::Color(theme.pressed_bg), theme.hover_border.map(|c| (c, 1.0)))
    } else if hover {
        (Fill::Color(theme.hover_bg), theme.hover_border.map(|c| (c, 1.0)))
    } else if checked {
        (Fill::Color(theme.checked_bg), theme.checked_border.map(|c| (c, 1.0)))
    } else {
        (Fill::Color(Color::TRANSPARENT), None)
    };
    let mut node = Node::new(
        rect,
        Cmd::Quad {
            fill,
            border,
            radius,
        },
    )
    .target(target);
    if enabled {
        node = node.tip(cmd);
    }
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
    if view.qat_dock() == ControlDock::Top {
        h += QAT_H;
    }
    if !view.minimized() {
        h += BODY_H;
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

    if view.qat_dock() == ControlDock::Top {
        qat_row(ctx, &mut scene, y, width);
        y += QAT_H;
    }

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
        body(ctx, &mut scene, y, width);
        y += BODY_H;
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

    let mut x = 6.0 + ORB_SIZE + 10.0;
    let visible = visible_tabs(ctx);
    let mut contextual_run: Option<(ContextualColor, f32, f32, String)> = None;
    // Geometry of the active tab, for the connected line (Filled style).
    let mut active_tab: Option<(f32, f32, Option<ContextualColor>)> = None;

    for &idx in &visible {
        let tab = &spec.tabs[idx];
        let label = tab_label(ctx, tab);
        let w = ctx.text_w(&label, 12.5) + 26.0;
        let rect =
            Rectangle::new(Point::new(x, strip_y + 1.0), Size::new(w, STRIP_H + 2.0));
        let is_active = idx == selected;
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
            Rectangle::new(
                Point::new(x, strip_y + 3.0),
                Size::new(w, STRIP_H - 6.0),
            ),
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

        // contextual set tracking
        match (&tab.contextual, &mut contextual_run) {
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

    // application menu orb
    let orb = Rectangle::new(
        Point::new(6.0, strip_y + 2.0),
        Size::new(ORB_SIZE, ORB_SIZE),
    );
    let hover = ctx.hovered(&Target::AppMenu);
    quad(
        scene,
        orb,
        if hover {
            Fill::Color(crate::ribbon::theme::shade(
                ctx.theme.orb.1,
                0.85,
            ))
        } else {
            Fill::Gradient(ctx.theme.orb.0, ctx.theme.orb.1)
        },
        None,
        ctx.theme.orb_radius,
    );
    scene.push(
        Node::new(orb, Cmd::Svg {
            image: crate::ribbon::image::UiImage::Icon("orb"),
            tint: None,
            opacity: 1.0,
        })
        .target(Target::AppMenu)
        .tip(orb_tip(ctx)),
    );
    if ctx.keytips_visible(true) {
        scene.push(Node::new(
            Rectangle::with_size(Size::new(18.0, 13.0)),
            Cmd::KeytipBadge {
                text: "F".into(),
                anchor: Point::new(orb.x + ORB_SIZE - 2.0, orb.y - 2.0),
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
        ctx.view().minimized(),
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

/// Width of a collapsed (popup) group button: icon + label + chevron.
fn popup_group_width(ctx: &Ctx, group: &GroupSpec) -> f32 {
    let label = group.label.clone().unwrap_or_else(|| group.id.clone());
    (ctx.text_w(&label, 11.0) + 34.0).max(56.0)
}

/// Width of a group under a given scale.
fn scaled_group_width(
    ctx: &Ctx,
    tab: &TabSpec,
    group: &GroupSpec,
    scale: &GroupScale,
) -> (ControlScale, bool, f32) {
    let (sc, popup) = resolve_scale(ctx, tab, group, scale);
    let w = if popup {
        popup_group_width(ctx, group)
    } else {
        measure_group_scaled(ctx, group, &sc)
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
    let avail = width - 8.0;
    // Fallback ladders per group (ladder[gi][0] is the first shrink
    // step below the declared layout).
    let ladders: Vec<Vec<GroupScale>> = tab
        .groups
        .iter()
        .map(|g| scale_ladder(g, scaling_of(ctx.spec(), g)))
        .collect();
    // Current ladder position per group (0 = declared layout).
    let mut pos: Vec<usize> = vec![0; tab.groups.len()];
    let mut sized: Vec<(ControlScale, bool, f32)> = tab
        .groups
        .iter()
        .map(|g| {
            // `ScalingPolicy.IdealSizes`: an optional starting cap
            // (Windows uses it to cap a group below its declared
            // maximum). It can only shrink, never enlarge controls.
            match scaling_of(ctx.spec(), g).and_then(|p| p.ideal.first()) {
                Some(scale) => scaled_group_width(ctx, tab, g, scale),
                None => (ControlScale::Declared, false, measure_group(ctx, g)),
            }
        })
        .collect();
    let total = |sized: &[(ControlScale, bool, f32)]| -> f32 {
        if sized.is_empty() {
            0.0
        } else {
            sized.iter().map(|(_, _, w)| *w + 6.0).sum::<f32>() - 6.0
        }
    };
    // Cascade: walk the groups right-to-left; shrink the right-most
    // group through its whole ladder before touching the next one.
    'outer: for gi in (0..tab.groups.len()).rev() {
        while total(&sized) > avail && pos[gi] < ladders[gi].len() {
            let step = pos[gi];
            pos[gi] += 1;
            sized[gi] = scaled_group_width(ctx, tab, &tab.groups[gi], &ladders[gi][step]);
        }
        if total(&sized) <= avail {
            break 'outer;
        }
    }
    let fits = |sized: &[(ControlScale, bool, f32)], n: usize| -> bool {
        if n == 0 {
            true
        } else {
            sized[..n]
                .iter()
                .map(|(_, _, w)| *w + 6.0)
                .sum::<f32>()
                - 6.0
                <= avail
        }
    };
    // Hide trailing groups that still do not fit (overflow popup). The
    // « overflow button reserves its own 26px so appearing does not
    // push more groups out.
    let mut n = tab.groups.len();
    let fits_trim = |sized: &[(ControlScale, bool, f32)], n: usize| -> bool {
        if n == 0 {
            true
        } else {
            sized[..n]
                .iter()
                .map(|(_, _, w)| *w + 6.0)
                .sum::<f32>()
                - 6.0
                <= avail - 26.0
        }
    };
    while n > 0 && !(fits(&sized, n) || (n < tab.groups.len() && fits_trim(&sized, n))) {
        n -= 1;
    }
    let placed = sized[..n]
        .iter()
        .enumerate()
        .map(|(gi, (sc, popup, w))| GroupPlan {
            gi,
            scale: sc.clone(),
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
    // body background
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y), Size::new(width, BODY_H)),
        Fill::Gradient(ctx.theme.ribbon.0, ctx.theme.ribbon.1),
        None,
        0.0,
    );
    quad(
        scene,
        Rectangle::new(Point::new(0.0, y + BODY_H - 1.0), Size::new(width, 1.0)),
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
                    popup: false,
                    w: measure_group(ctx, g),
                })
                .collect::<Vec<_>>(),
            Vec::new(),
        )
    };
    let mut x = 4.0f32;
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
        x += plan.w + 6.0;
        if pos + 1 < placed.len() {
            quad(
                scene,
                Rectangle::new(
                    Point::new(x - 4.0, content_y),
                    Size::new(1.0, BODY_H - 8.0),
                ),
                Fill::Color(ctx.theme.group_separator),
                None,
                0.0,
            );
        }
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
            Point::new(width - 24.0, y + BODY_H / 2.0 - 9.0),
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

/// Width of a control with a scale applied.
pub fn width_of_scaled(
    ctx: &Ctx,
    control: &ControlSpec,
    scale: &ControlScale,
) -> f32 {
    let scaled_size = |declared: CtrlSize| {
        scale.effective(resolved_ref(control.command()), declared)
    };
    match control {
        ControlSpec::Button(b) | ControlSpec::ToggleButton(b) => button_width_scaled(
            ctx,
            b,
            scaled_size(b.size),
        ),
        ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => button_width_scaled(
            ctx,
            &ButtonSpec {
                command: s.command.clone(),
                size: s.size,
                label: s.label.clone(),
                image: s.image.clone(),
                keytip: None,
                width: None,
            },
            scaled_size(s.size),
        ),
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
                .map(|c| width_of_scaled(ctx, c, scale))
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
            let lines = wrap_width(ctx, &label, 11.0, 76.0);
            let max_line = lines
                .iter()
                .map(|l| ctx.text_w(l, 11.0))
                .fold(0.0f32, f32::max);
            max_line.max(40.0) + 12.0
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
    measure_group_scaled(ctx, group, &ControlScale::Declared)
}

/// Width of a group with a scale applied (`SizeDefinition`).
pub fn measure_group_scaled(
    ctx: &Ctx,
    group: &GroupSpec,
    scale: &ControlScale,
) -> f32 {
    let mut cols: Vec<f32> = Vec::new();
    let mut cur_w = 0.0f32;
    let mut used = 0usize;
    for control in &group.controls {
        let span = rows_of_scaled(control, scale);
        if used + span > 3 && used > 0 {
            cols.push(cur_w);
            cur_w = 0.0;
            used = 0;
        }
        cur_w = cur_w.max(width_of_scaled(ctx, control, scale));
        used += span;
    }
    cols.push(cur_w);
    let sum: f32 = cols.iter().sum();
    sum + 2.0 * (cols.len().saturating_sub(1)) as f32 + 8.0
}

/// The group plan used while building (declared sizes by default).
fn plan_or_default(plan: Option<&GroupPlan>, gi: usize, w: f32) -> GroupPlan {
    plan.cloned().unwrap_or(GroupPlan {
        gi,
        scale: ControlScale::Declared,
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
        // opens the full group in a popup.
        let rect = Rectangle::new(Point::new(x, y), Size::new(width, CONTENT_H));
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
        svg_node(
            scene,
            Rectangle::new(
                Point::new(rect.x + (rect.width - 32.0) / 2.0, rect.y + 4.0),
                Size::new(32.0, 32.0),
            ),
            img,
            1.0,
        );
        let label = group.label.clone().unwrap_or_else(|| group.id.clone());
        text(
            scene,
            Rectangle::new(
                Point::new(rect.x, rect.y + 40.0),
                Size::new(rect.width, 12.0),
            ),
            label,
            10.5,
            ctx.theme.text,
            Horizontal::Center,
            Vertical::Center,
        );
        chevron(
            scene,
            Rectangle::new(
                Point::new(rect.x + rect.width / 2.0 - 4.0, rect.y + 54.0),
                Size::new(8.0, 5.0),
            ),
            false,
            ctx.theme.group_label,
        );
        return;
    }
    let scale = &plan.scale;
    // controls in columns
    let mut col_x = x + 4.0;
    let mut used = 0usize;
    let mut col_width = 0.0f32;
    let mut pending: Vec<(&ControlSpec, f32, f32)> = Vec::new();

    let flush = |scene: &mut Scene,
                     col_x: f32,
                     col_width: f32,
                     pending: &mut Vec<(&ControlSpec, f32, f32)>| {
        for (control, rel_y, h) in pending.drain(..) {
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
            );
        }
    };

    for control in &group.controls {
        let span = rows_of_scaled(control, scale);
        if used + span > 3 && used > 0 {
            flush(scene, col_x, col_width, &mut pending);
            col_x += col_width + 2.0;
            col_width = 0.0;
            used = 0;
        }
        let h = span as f32 * ROW_H;
        let w = width_of_scaled(ctx, control, scale);
        col_width = col_width.max(w);
        pending.push((control, used as f32 * ROW_H, h));
        used += span;
    }
    flush(scene, col_x, col_width, &mut pending);

    // footer: caption + dialog launcher
    let footer_y = y + CONTENT_H + 2.0;
    let label = group
        .label
        .clone()
        .unwrap_or_else(|| group.id.clone());
    text(
        scene,
        Rectangle::new(
            Point::new(x, footer_y),
            Size::new(width - 18.0, FOOTER_H - 2.0),
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
    build_control_scaled(ctx, scene, control, rect, loc, compact, &ControlScale::Declared);
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
) {
    let eff = |declared: CtrlSize| {
        scale.effective(resolved_ref(control.command()), declared)
    };
    match control {
        ControlSpec::Button(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_button_like(
                    ctx, scene, rect, cmd, eff(b.size), false, compact,
                );
            }
        }
        ControlSpec::ToggleButton(b) => {
            if let Some(cmd) = resolved(b.command.as_ref()) {
                build_button_like(
                    ctx, scene, rect, cmd, eff(b.size), true, compact,
                );
            }
        }
        ControlSpec::SplitButton(s) => {
            if let Some(cmd) = resolved(s.command.as_ref()) {
                build_split(
                    ctx, scene, rect, cmd, eff(s.size), true, compact,
                );
            }
        }
        ControlSpec::DropDownButton(s) => {
            if let Some(cmd) = resolved(s.command.as_ref()) {
                build_split(
                    ctx, scene, rect, cmd, eff(s.size), false, compact,
                );
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
            build_control_group(ctx, scene, controls, rect, loc, compact);
        }
    }
}

fn resolved(c: Option<&crate::ribbon::spec::CommandRef>) -> Option<u32> {
    match c {
        Some(crate::ribbon::spec::CommandRef::Id(id)) => Some(*id),
        _ => None,
    }
}

fn build_button_like(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    size: CtrlSize,
    toggle: bool,
    compact: bool,
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
    let show_label = !compact || size == CtrlSize::Small;

    match size {
        CtrlSize::Large if !compact => {
            let img = ctx.large_image(cmd).unwrap_or_default();
            svg_node(
                scene,
                Rectangle::new(
                    Point::new(
                        rect.x + (rect.width - 32.0) / 2.0,
                        rect.y + 3.0,
                    ),
                    Size::new(32.0, 32.0),
                ),
                img,
                opacity,
            );
            let lines = wrap_width(ctx, &label, 11.0, rect.width - 4.0);
            let line_h = 11.0f32;
            let total = lines.len().min(3) as f32 * line_h;
            let start_y = rect.y + rect.height - 3.0 - total;
            for (i, line) in lines.iter().take(3).enumerate() {
                text(
                    scene,
                    Rectangle::new(
                        Point::new(rect.x, start_y + i as f32 * line_h),
                        Size::new(rect.width, line_h),
                    ),
                    line.clone(),
                    11.0,
                    text_color,
                    Horizontal::Center,
                    Vertical::Center,
                );
            }
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

fn build_split(
    ctx: &Ctx,
    scene: &mut Scene,
    rect: Rectangle,
    cmd: u32,
    size: CtrlSize,
    has_main: bool,
    compact: bool,
) {
    let drop_w = 14.0f32;
    let main_rect = Rectangle::new(
        rect.position(),
        Size::new((rect.width - drop_w).max(10.0), rect.height),
    );
    let drop_rect = Rectangle::new(
        Point::new(rect.x + main_rect.width, rect.y),
        Size::new(drop_w, rect.height),
    );
    build_button_like(ctx, scene, main_rect, cmd, size, false, compact);
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
        build_split(ctx, scene, rect, cmd, CtrlSize::Small, false, false);
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
        build_control(ctx, scene, control, crect, loc, compact);
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

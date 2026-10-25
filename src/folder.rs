//! File view pane — SPEC/folder-spec.md: the 48-step Ctrl+scroll zoom
//! scale (§3 index map, §8 layout metrics). Index 0 Content, 1 Tiles,
//! 2 Details, 3 List, 4..=14 small icons (name right), 15..=47 icon on
//! top (name below).

use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, overlay, renderer};
use iced::widget::{column, container, mouse_area, row, scrollable, stack, text, text_input, Space};
use iced::{Color, Element, Length, Rectangle, Size, Vector, mouse};

use crate::fs::{self, Entry};
use crate::model::{self, Explorer, Message};
use crate::ui_common::{bg, elide, skin};
use crate::view_step as vs;

pub(crate) fn file_scrollable_id() -> iced::widget::Id {
    static ID: std::sync::OnceLock<iced::widget::Id> = std::sync::OnceLock::new();
    ID.get_or_init(|| iced::widget::Id::new("file-view")).clone()
}

/// Wrap the file view so Ctrl+wheel steps the view zoom index instead of
/// scrolling (the event is captured before the scrollable can see it).
/// Transparent wrapper: state/layout/draw all delegate to the content
/// (same pattern as iced's `Container`).
struct CtrlZoom<'a> {
    content: Element<'a, Message>,
    ctrl: bool,
    /// §7/§12 NotchDelta, from folder-view.yaml.
    notch_delta: f32,
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for CtrlZoom<'a> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.ctrl
            && let iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && cursor.is_over(layout.bounds())
        {
            // §7 wheel delta: one Lines unit = one notch (NotchDelta);
            // Pixels accumulate as-is. The model accumulates + steps.
            let units = match delta {
                mouse::ScrollDelta::Lines { y, .. } => *y * self.notch_delta,
                mouse::ScrollDelta::Pixels { y, .. } => *y,
            };
            if units.abs() > 0.01 {
                shell.publish(Message::WheelZoom(units));
                shell.capture_event();
                return;
            }
        }
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

/// Rubber-band selection zone: a left press that no child captures
/// (empty space) starts the marquee; moves/ends while dragging publish
/// content coordinates (local + scroll). Items capture their own
/// presses first, so dragging from an item never starts a marquee.
struct MarqueeZone<'a> {
    content: Element<'a, Message>,
    scroll_x: f32,
    scroll_y: f32,
}

#[derive(Default)]
struct MarqueeZoneState {
    dragging: bool,
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for MarqueeZone<'a> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<MarqueeZoneState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(MarqueeZoneState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let zone = tree.state.downcast_mut::<MarqueeZoneState>();
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
                if !shell.is_event_captured() && cursor.is_over(layout.bounds()) {
                    if let Some(p) = cursor.position() {
                        zone.dragging = true;
                        let o = layout.bounds().position();
                        shell.publish(Message::MarqueeBegin {
                            x: p.x - o.x + self.scroll_x,
                            y: p.y - o.y + self.scroll_y,
                        });
                        shell.capture_event();
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) if zone.dragging => {
                // the rectangle follows the pointer but stops at the
                // pane's edge (no growth beyond it, no freeze)
                let b = layout.bounds();
                let o = b.position();
                shell.publish(Message::MarqueeMove {
                    x: (position.x - o.x).clamp(0.0, b.width) + self.scroll_x,
                    y: (position.y - o.y).clamp(0.0, b.height) + self.scroll_y,
                });
                shell.capture_event();
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if zone.dragging =>
            {
                zone.dragging = false;
                shell.publish(Message::MarqueeEnd);
                shell.capture_event();
            }
            _ => {
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

/// The rubber-band rectangle (pane-local coordinates — plain
/// spacer positioning, NOT float: float bounds are window-absolute).
fn marquee_rect(x: f32, y: f32, w: f32, h: f32, skin: skin::Skin) -> Element<'static, Message> {
    column![
        Space::new().height(Length::Fixed(y.max(0.0))),
        row![
            Space::new().width(Length::Fixed(x.max(0.0))),
            container(Space::new().width(Length::Fixed(w)).height(Length::Fixed(h))).style(
                move |_t: &iced::Theme| container::Style {
                    background: Some(Color { a: 0.35, ..skin.sel }.into()),
                    border: iced::Border {
                        color: skin.sel_border,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                },
            ),
        ],
    ]
    .into()
}

/// Selection/hover chrome shared by the per-item renderers (§8.4:
/// rounded rectangle, radius 4).
fn item_chrome<'a>(
    i: usize,
    content: impl Into<Element<'a, Message>>,
    state: &'a Explorer,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let hovered = state.hovered == Some(i);
    let bgc = if selected {
        skin.sel
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let styled = container(content.into()).style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: if selected { 1.0 } else { 0.0 },
            radius: state.view_cfg.style.corner_radius.into(),
        },
        ..Default::default()
    });
    mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_right_press(Message::ContextSelect(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::HoverExit(i))
        .into()
}

/// Vertical spacer reserving scroll height for virtualized rows.
fn vspacer(h: f32) -> Space {
    Space::new().height(Length::Fixed(h))
}

/// Visible row window for a fixed-row-height list.
fn row_window(total: usize, row_h: f32, scroll_y: f32, viewport_h: f32) -> (usize, usize) {
    let view_rows = (viewport_h / row_h).ceil() as usize + 4;
    let first = ((scroll_y / row_h).floor() as usize).saturating_sub(2);
    let last = (first + view_rows).min(total);
    (first, last)
}

/// The file-list scrollable (vertical; List uses a horizontal one). The
/// built-in scrollbar is hidden — the Windows-style strip in
/// `scrollbar.rs` sits beside the scrollable instead.
fn file_scrollable(content: iced::widget::Column<'_, Message>) -> scrollable::Scrollable<'_, Message> {
    scrollable(content)
        .id(file_scrollable_id())
        // Fill so the empty area beside a narrower grid still scrolls
        // (scrollable defaults to Shrink width)
        .width(Length::Fill)
        .height(Length::Fill)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::hidden(),
        ))
        .on_scroll(|vp| Message::Scrolled {
            x: vp.absolute_offset().x,
            y: vp.absolute_offset().y,
            height: vp.bounds().height,
        })
}

/// Grid view shared by the Tiles / SmallIcon / IconTop families:
/// fixed-size cells filled left to right, virtualized by grid row
/// (the grid gap is folded into the row pitch).
fn grid_view<'a>(
    state: &'a Explorer,
    width: f32,
    cell_w: f32,
    cell_h: f32,
    gap: f32,
    cell: impl Fn(usize, &'a Entry) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let cols = vs::grid_cols(cell_w, gap, width);
    let total_rows = state.entries.len().div_ceil(cols);
    let pitch = cell_h + gap;
    let (first, last) = row_window(total_rows, pitch, state.scroll_y, state.viewport_h);
    let mut rows = column![].spacing(0);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * pitch));
    }
    for r in first..last {
        let mut line = row![].spacing(gap);
        for c in 0..cols {
            let idx = r * cols + c;
            if let Some(e) = state.entries.get(idx) {
                line = line.push(cell(idx, e));
            } else {
                line = line.push(Space::new().width(Length::Fixed(cell_w)));
            }
        }
        rows = rows.push(container(line).height(Length::Fixed(pitch)));
    }
    if last < total_rows {
        rows = rows.push(vspacer((total_rows - last) as f32 * pitch));
    }
    file_scrollable(rows).into()
}

pub(crate) fn file_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let cfg = &state.view_cfg;
    let family = cfg.family(state.view_index);
    let total = state.entries.len();
    let longest = state.entries.iter().map(|e| e.name.chars().count()).max().unwrap_or(0);
    // The strip collapses to 0px when the content fits the viewport
    // (folder view; the nav pane keeps a 1px hairline). Probe with the
    // strip present: collapsing only ever widens/tallies the pane, so
    // content that fits with the strip still fits without it.
    let w_with_strip = if family == vs::Family::List {
        width
    } else {
        (width - crate::scrollbar::THICKNESS).max(0.0)
    };
    let viewport_axis = if family == vs::Family::List { width } else { state.viewport_h };
    let content = cfg.content_extent(
        state.view_index, total, w_with_strip, state.viewport_h, longest, state.details_compact,
    );
    let need_strip = !state.loading && content > viewport_axis;
    let strip_w = if need_strip && family != vs::Family::List {
        crate::scrollbar::THICKNESS
    } else {
        0.0
    };
    let inner_w = (width - strip_w).max(0.0);
    let s = cfg.icon_px(state.view_index);
    // arrow-button line step for the strip
    let line = match family {
        vs::Family::Content => cfg.layouts.content.row_h,
        vs::Family::Tiles => cfg.layouts.tiles.cell_h,
        vs::Family::Details => cfg.details_row_h(state.details_compact),
        vs::Family::List => cfg.list_col_w(longest),
        vs::Family::SmallIcon => {
            let m = cfg.small_icon(s);
            m.cell_h + m.gap
        }
        vs::Family::IconTop => {
            let m = cfg.icon_top(s);
            m.cell_h + m.gap
        }
    };
    let body: Element<Message> = if state.loading {
        container(
            container(text("Loading…").size(cfg.style.font_size).color(skin.subtle))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        match family {
            vs::Family::Content => content_view(state, skin, inner_w),
            vs::Family::Tiles => tiles_view(state, skin, inner_w),
            vs::Family::Details => {
                // The scrollbar strip belongs at the right edge of the
                // folder pane, running its full height — not starting
                // below the column header. So the header overlays the
                // scrollable's top strip (the content reserves that
                // strip with a spacer) and the strip sits beside both.
                stack![
                    details_view(state, skin),
                    container(details_header(state, skin)).width(Length::Fill),
                ]
                .into()
            }
            vs::Family::List => list_view(state, skin, inner_w),
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(s);
                grid_view(state, inner_w, m.cell_w, m.cell_h, m.gap, |i, e| {
                    small_icon_cell(i, e, state, skin, s, &m)
                })
            }
            vs::Family::IconTop => {
                let m = cfg.icon_top(s);
                grid_view(state, inner_w, m.cell_w, m.cell_h, m.gap, |i, e| {
                    icon_top_cell(i, e, state, skin, s, &m)
                })
            }
        }
    };

    // rubber-band selection rectangle overlay (all layouts)
    let body: Element<Message> = if let Some(m) = &state.marquee {
        let (x0, x1) = (m.x0.min(m.x1), m.x0.max(m.x1));
        let (y0, y1) = (m.y0.min(m.y1), m.y0.max(m.y1));
        stack![
            body,
            marquee_rect(x0 - state.scroll_x, y0 - state.scroll_y, x1 - x0, y1 - y0, skin),
        ]
        .into()
    } else {
        body
    };

    // The Sensor reports the exact scrollable viewport size (iced's
    // on_scroll stays silent when the content fits, and window resize
    // events carry the WINDOW size — both wrong for the collapse rule).
    let area: Element<Message> = iced::widget::Sensor::new(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(bg(skin.window_bg)),
    )
    .on_show(|size| Message::FileAreaResized(size.width, size.height))
    .on_resize(|size| Message::FileAreaResized(size.width, size.height))
    .into();
    // right-click on empty space (items capture their own first) opens
    // the background menu (SPEC/menu_spec.md)
    let area = crate::menu::on_right_free(area, Message::ContextBackground);
    // a left press on empty space starts the rubber-band selection
    let area: Element<Message> = Element::new(MarqueeZone {
        content: area,
        scroll_x: state.scroll_x,
        scroll_y: state.scroll_y,
    });
    // Windows-style scrollbar strip (arrow buttons at both ends); 0px
    // when the content fits, keeping the pane edge right-aligned
    let pane: Element<Message> = if family == vs::Family::List && !state.loading {
        let mut c = column![area];
        if need_strip {
            c = c.push(crate::scrollbar::horizontal(
                state.scroll_x, inner_w, content, line, skin, Message::FileScrollTo,
            ));
        }
        c.into()
    } else {
        let mut r = row![area];
        if need_strip {
            r = r.push(crate::scrollbar::vertical(
                state.scroll_y, state.viewport_h, content, line, skin, Message::FileScrollTo,
            ));
        }
        r.into()
    };
    // wheel speed boost (folder-view.yaml input.scroll_speed): captures
    // plain wheel events; Ctrl+wheel zoom is handled by CtrlZoom below
    let (axis, offset) = if family == vs::Family::List {
        (crate::scrollbar::Axis::Horizontal, state.scroll_x)
    } else {
        (crate::scrollbar::Axis::Vertical, state.scroll_y)
    };
    let view: Element<Message> = container(crate::scrollbar::wheel_boost(
        pane,
        offset,
        (content - viewport_axis).max(0.0),
        cfg.input.scroll_speed,
        axis,
        Message::FileScrollTo,
    ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into();
    Element::new(CtrlZoom {
        content: view,
        ctrl: state.ctrl_held,
        notch_delta: cfg.input.notch_delta,
    })
}

/// Details column header (Name / Date modified / Type / Size). The
/// fixed columns are adjustable: the 4px handle at a column's left edge
/// drags the boundary (Name takes the remaining width).
fn details_header(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let d = &state.view_cfg.layouts.details;
    let prop = state.view_cfg.style.prop_size;
    let mut cols = row![
        container(text("Name").size(prop).color(skin.subtle))
            .width(Length::Fill)
            .padding([4.0, 8.0]),
    ];
    for (col, label) in ["Date modified", "Type", "Size"].into_iter().enumerate() {
        let w = state.details_cols[col];
        // split handle: 1px separator line + 4px drag target
        let handle = mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fixed(4.0))
                .height(Length::Fill)
                .style(move |_t: &iced::Theme| container::Style {
                    border: iced::Border {
                        color: skin.group_sep,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::ColumnSplitBegin(col));
        cols = cols.push(handle);
        cols = cols.push(
            container(text(label).size(prop).color(skin.subtle))
                .width(Length::Fixed(w - 4.0))
                .padding([4.0, 8.0]),
        );
    }
    container(cols.height(Length::Fixed(d.header_h)).align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.col_header_bg.into()),
            border: iced::Border {
                color: skin.sep,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// index 0 — Content (§8.1)

fn content_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let row_h = state.view_cfg.layouts.content.row_h;
    let total = state.entries.len();
    let (first, last) = row_window(total, row_h, state.scroll_y, state.viewport_h);
    let mut rows = column![].spacing(0);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * row_h));
    }
    for i in first..last {
        if let Some(e) = state.entries.get(i) {
            rows = rows.push(content_row(i, e, state, skin, width));
        }
    }
    if last < total {
        rows = rows.push(vspacer((total - last) as f32 * row_h));
    }
    file_scrollable(rows).into()
}

/// §8.1 Content row: 32px icon, name (semibold) + 2 property lines,
/// left-aligned; divider line between rows.
fn content_row<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    width: f32,
) -> Element<'a, Message> {
    let c = &state.view_cfg.layouts.content;
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    };
    let text_w = (width - c.icon - c.icon_text_gap - 2.0 * c.pad_w - 8.0).max(60.0);
    let date = e.modified.map(model::format_date).unwrap_or_default();
    let lines = column![
        text(elide(&e.name, text_w, 7.5))
            .size(c.name_size)
            .font(semibold)
            .color(skin.text)
            .line_height(1.0),
        row![
            text("Date modified: ").size(c.prop_size).color(skin.subtle).line_height(1.0),
            text(date).size(c.prop_size).color(skin.text).line_height(1.0),
        ],
        row![
            text("Type: ").size(c.prop_size).color(skin.subtle).line_height(1.0),
            text(model::type_label(e)).size(c.prop_size).color(skin.text).line_height(1.0),
            text("   Size: ").size(c.prop_size).color(skin.subtle).line_height(1.0),
            text(item_size(e)).size(c.prop_size).color(skin.text).line_height(1.0),
        ],
    ]
    .spacing(c.spacing);
    let content = row![
        crate::icon::for_entry(e, c.icon, &skin),
        lines,
    ]
    .spacing(c.icon_text_gap)
    .padding([c.pad_h, c.pad_w])
    .align_y(iced::Alignment::Center);
    // divider line between rows (§8.1): the bottom 1px of the row pitch
    let content = column![
        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(c.row_h - 1.0)),
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(bg(skin.sep)),
    ];
    item_chrome(i, content, state, skin)
}

// ---------------------------------------------------------------------------
// index 1 — Tiles (§8.1)

fn tiles_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let t = &state.view_cfg.layouts.tiles;
    grid_view(state, width, t.cell_w, t.cell_h, 0.0, |i, e| {
        tile_cell(i, e, state, skin)
    })
}

/// §8.1 Tiles cell: icon on the left, name + Type + Size (3 lines max).
fn tile_cell<'a>(i: usize, e: &Entry, state: &'a Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let t = &state.view_cfg.layouts.tiles;
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    };
    let text_w = t.cell_w - t.icon - t.icon_text_gap - 2.0 * t.pad;
    let content = row![
        crate::icon::for_entry(e, t.icon, &skin),
        column![
            text(elide(&e.name, text_w, 7.5)).size(t.name_size).font(semibold).color(skin.text),
            text(elide(model::type_label(e), text_w, 6.6)).size(t.prop_size).color(skin.subtle),
            text(item_size(e)).size(t.prop_size).color(skin.subtle),
        ]
        .spacing(t.line_spacing),
    ]
    .spacing(t.icon_text_gap)
    .align_y(iced::Alignment::Center);
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(t.cell_w))
            .height(Length::Fixed(t.cell_h))
            .padding(t.pad),
        state,
        skin,
    )
}

// ---------------------------------------------------------------------------
// index 2 — Details (§8.1)

fn details_view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let d = &state.view_cfg.layouts.details;
    let row_h = state.view_cfg.details_row_h(state.details_compact);
    let total = state.entries.len();
    // the content reserves the header strip (the real header overlays it)
    let (first, last) = row_window(
        total,
        row_h,
        (state.scroll_y - d.header_h).max(0.0),
        state.viewport_h,
    );
    let mut rows = column![vspacer(d.header_h)].spacing(0);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * row_h));
    }
    for i in first..last {
        if let Some(e) = state.entries.get(i) {
            rows = rows.push(file_row(i, e, state, skin));
        }
    }
    if last < total {
        rows = rows.push(vspacer((total - last) as f32 * row_h));
    }
    file_scrollable(rows).into()
}

fn file_row<'a>(i: usize, e: &Entry, state: &'a Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let d = &state.view_cfg.layouts.details;
    let font = state.view_cfg.style.font_size;
    let row_h = state.view_cfg.details_row_h(state.details_compact);
    let [date_w, type_w, size_w] = state.details_cols;
    let name_cell: Element<Message> = if state.renaming.as_ref().map(|(ri, _)| *ri) == Some(i) {
        let buf = state.renaming.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
        text_input("Name", &buf)
            .on_input(Message::RenameInput)
            .on_submit(Message::CommitRename(buf))
            .size(font)
            .padding(2)
            .into()
    } else {
        text(e.name.clone()).size(font).color(skin.text).into()
    };

    let content = row![
        container(crate::icon::for_entry(e, d.icon, &skin))
            .width(Length::Fixed(d.icon_cell_w))
            .center_y(Length::Fixed(d.icon)),
        container(name_cell).width(Length::Fill).padding([d.cell_padding_v, d.cell_padding_h]),
        container(text(e.modified.map(model::format_date).unwrap_or_default()).size(font).color(skin.text))
            .width(Length::Fixed(date_w))
            .padding([d.cell_padding_v, d.cell_padding_h]),
        container(text(model::type_label(e)).size(font).color(skin.text))
            .width(Length::Fixed(type_w))
            .padding([d.cell_padding_v, d.cell_padding_h]),
        container(text(item_size(e)).size(font).color(skin.text))
            .width(Length::Fixed(size_w))
            .padding([d.cell_padding_v, d.cell_padding_h]),
    ]
    .height(Length::Fixed(row_h))
    .align_y(iced::Alignment::Center);

    item_chrome(i, container(content).width(Length::Fill), state, skin)
}

// ---------------------------------------------------------------------------
// index 3 — List (§8.1): name-only columns flowing top to bottom, then
// left to right; the pane scrolls horizontally.

fn list_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let l = &state.view_cfg.layouts.list;
    let longest = state
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0);
    let col_w = state.view_cfg.list_col_w(longest);
    let rows_per_col = ((state.viewport_h / l.row_h).floor() as usize).max(1);
    let total = state.entries.len();
    let total_cols = total.div_ceil(rows_per_col);
    // virtualize by column
    let view_cols = (width / col_w).ceil() as usize + 3;
    let first = ((state.scroll_x / col_w).floor() as usize).saturating_sub(1);
    let last = (first + view_cols).min(total_cols);
    let mut strip = row![].spacing(0);
    if first > 0 {
        strip = strip.push(Space::new().width(Length::Fixed(first as f32 * col_w)));
    }
    for c in first..last {
        let mut col = column![].spacing(0);
        for r in 0..rows_per_col {
            let idx = c * rows_per_col + r;
            if let Some(e) = state.entries.get(idx) {
                col = col.push(list_cell(idx, e, state, skin, col_w));
            }
        }
        strip = strip.push(container(col).width(Length::Fixed(col_w)));
    }
    if last < total_cols {
        strip = strip.push(Space::new().width(Length::Fixed(
            (total_cols - last) as f32 * col_w,
        )));
    }
    scrollable(strip)
        .id(file_scrollable_id())
        .width(Length::Fill)
        .height(Length::Fill)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .on_scroll(|vp| Message::Scrolled {
            x: vp.absolute_offset().x,
            y: vp.absolute_offset().y,
            height: vp.bounds().height,
        })
        .into()
}

/// §8.1 List cell: icon + name, 1 line, truncated with ellipsis.
fn list_cell<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    col_w: f32,
) -> Element<'a, Message> {
    let l = &state.view_cfg.layouts.list;
    let content = row![
        crate::icon::for_entry(e, l.icon, &skin),
        text(elide(&e.name, col_w - l.col_w_base, l.per_char))
            .size(state.view_cfg.style.font_size)
            .color(skin.text),
    ]
    .spacing(l.icon_text_gap)
    .padding([l.pad_h, l.pad_w])
    .align_y(iced::Alignment::Center);
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(col_w))
            .height(Length::Fixed(l.row_h))
            .align_y(iced::alignment::Vertical::Center),
        state,
        skin,
    )
}

// ---------------------------------------------------------------------------
// indices 4..=14 — SmallIcon (§8.2): shell icon, name on the right.

fn small_icon_cell<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    s: f32,
    m: &vs::SmallIconMetrics,
) -> Element<'a, Message> {
    let content = row![
        crate::icon::for_entry(e, s, &skin),
        text(elide(&e.name, m.label_w, 7.2))
            .size(state.view_cfg.style.font_size)
            .color(skin.text)
            .width(Length::Fixed(m.label_w)),
    ]
    .spacing(state.view_cfg.small_icon.icon_label_gap)
    .align_y(iced::Alignment::Center);
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(m.cell_w))
            .height(Length::Fixed(m.cell_h))
            .align_y(iced::alignment::Vertical::Center),
        state,
        skin,
    )
}

// ---------------------------------------------------------------------------
// indices 15..=47 — IconTop (§8.3): icon box centered, name below.

fn icon_top_cell<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    s: f32,
    m: &vs::IconTopMetrics,
) -> Element<'a, Message> {
    // §8.3: the full name is shown when the item is selected; otherwise
    // the label wraps at word boundaries over `lines` lines with an
    // ellipsis on the last line (approximated by a pre-elide).
    let c = &state.view_cfg.icon_top;
    let label = if state.selected.contains(&i) {
        e.name.clone()
    } else {
        elide(&e.name, (m.cell_w - c.label_pad) * m.lines, c.per_char)
    };
    let content = column![
        container(crate::icon::for_entry(e, s, &skin))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        container(
            text(label)
                .size(state.view_cfg.style.font_size)
                .color(skin.text)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .height(Length::Fixed(m.lines * c.label_line_h)),
    ]
    .spacing(m.gap);
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(m.cell_w))
            .height(Length::Fixed(m.cell_h))
            .padding(m.pad),
        state,
        skin,
    )
}

/// Size text for the item (empty for folders, like the details list).
fn item_size(e: &Entry) -> String {
    if e.kind == fs::EntryKind::Dir {
        String::new()
    } else {
        model::format_size(e.size)
    }
}

//! File view pane — SPEC/FOLDER-SPEC.MD: the 48-step Ctrl+scroll zoom
//! scale (§3 index map, §8 layout metrics). Index 0 Content, 1 Tiles,
//! 2 Details, 3 List, 4..=14 small icons (name right), 15..=47 icon on
//! top (name below).

use iced::Widget as _;
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Layout, Shell, layout, overlay, renderer};
use iced::widget::{column, container, mouse_area, row, scrollable, stack, text, text_input, Space};
use iced::{Color, Element, Length, Rectangle, Size, Vector, mouse};

use crate::fs::{self, Entry};
use crate::model::{self, Explorer, Message};
use crate::text_metrics;
use crate::ui_common::{bg, elide, skin, truncate_units};
use crate::view_step as vs;

pub(crate) fn file_scrollable_id() -> iced::widget::Id {
    static ID: std::sync::OnceLock<iced::widget::Id> = std::sync::OnceLock::new();
    ID.get_or_init(|| iced::widget::Id::new("file-view")).clone()
}

/// Wrap the file view so Ctrl+wheel steps the view zoom index instead of
/// scrolling (the event is captured before the scrollable can see it).
/// Transparent wrapper: state/layout/draw all delegate to the content
/// (same pattern as iced's `Container`).
impl iced::advanced::widget::Meta for CtrlZoom<'_> {}

struct CtrlZoom<'a> {
    content: Element<'a, Message>,
    ctrl: bool,
    /// §7/§12 NotchDelta, from folder-view.yaml.
    notch_delta: f32,
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for CtrlZoom<'a> {
    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) {
        self.content.layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            
            .operate(tree, layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
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
        self.content.update(
            tree, event, layout, cursor, renderer, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}

/// Rubber-band selection zone: a left press that no child captures
/// (empty space) starts the marquee; moves/ends while dragging publish
/// content coordinates (local + scroll). Items capture their own
/// presses first, so dragging from an item never starts a marquee.
impl iced::advanced::widget::Meta for MarqueeZone<'_> {}

struct MarqueeZone<'a> {
    content: Element<'a, Message>,
    scroll_x: f32,
    scroll_y: f32,
    /// Pane-local y the rubber band may not cross upward (the Details
    /// header strip overlaying the pane top); 0 in every other layout.
    top: f32,
}

#[derive(Default)]
struct MarqueeZoneState {
    dragging: bool,
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for MarqueeZone<'a> {
    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<MarqueeZoneState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(MarqueeZoneState::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) {
        self.content
            
            .layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let zone = tree.state.downcast_mut::<MarqueeZoneState>();
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.content.update(
                    &mut tree.children[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    shell,
                    viewport,
                );
                if !shell.is_event_captured()
                    && cursor.is_over(layout.bounds())
                    && let Some(p) = cursor.position()
                {
                    zone.dragging = true;
                    let b = layout.bounds();
                    let o = b.position();
                    shell.publish(Message::MarqueeBegin {
                        x: (p.x - o.x).clamp(0.0, b.width) + self.scroll_x,
                        y: (p.y - o.y).clamp(self.top, b.height) + self.scroll_y,
                    });
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) if zone.dragging => {
                // the rectangle follows the pointer but stops at the
                // pane's edge (no growth beyond it, no freeze); in
                // Details it additionally stops at the header's lower
                // border (top inset)
                let b = layout.bounds();
                let o = b.position();
                shell.publish(Message::MarqueeMove {
                    x: (position.x - o.x).clamp(0.0, b.width) + self.scroll_x,
                    y: (position.y - o.y).clamp(self.top, b.height) + self.scroll_y,
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
                self.content.update(
                    &mut tree.children[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    shell,
                    viewport,
                );
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation, window)
    }
}

/// The rubber-band rectangle (pane-local coordinates — plain
/// spacer positioning, NOT float: float bounds are window-absolute).
fn marquee_rect(x: f32, y: f32, w: f32, h: f32, rainbow: bool, skin: skin::Skin) -> Element<'static, Message> {
    column![
        Space::new().height(Length::Fixed(y.max(0.0))),
        row![
            Space::new().width(Length::Fixed(x.max(0.0))),
            container(Space::new().width(Length::Fixed(w)).height(Length::Fixed(h))).style(
                move |_t: &iced::Theme| container::Style {
                    background: Some(Color { a: 0.35, ..skin.sel }.into()),
                    border: iced::Border {
                        // TEMP(rainbow): let the backdrop pass through
                        color: if rainbow {
                            Color { a: 0.6, ..skin.sel_border }
                        } else {
                            skin.sel_border
                        },
                        width: crate::theme::border_w(1.0),
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                },
            ),
        ],
    ]
    .boxed()
}

/// Selection/hover chrome shared by the per-item renderers (§8.4:
/// rounded rectangle, radius 4).
fn item_chrome<'a>(
    i: usize,
    content: impl iced::Widget<Message, iced::Theme, iced::Renderer> + 'a,
    state: &'a Explorer,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let hovered = state.hovered == Some(i);
    // §5.1: the item is the innermost drop target under the pointer
    let targeted = state
        .entries
        .get(i)
        .is_some_and(|e| state.drop_targeted(&e.path));
    // the idle selector (keyboard focus): a border-only rectangle, no
    // fill; never drawn over a real selection (which has its own)
    let focused_idle = state.focus_shown && state.focus_item == Some(i) && !selected && !targeted;
    // selection fades to gray while the folder pane is inactive
    // (window unfocused, nav pane active, or a ribbon popup open);
    // hovering a grayed item (or an active drop target) restores the
    // live colors (SPEC/Unfocused_Window.md §4)
    let popup_open = state.ribbon.framework().view().popup_open();
    let pane_live = state.focused
        && !popup_open
        && state.active_pane == crate::model::Pane::Folder;
    let live = pane_live || targeted || (hovered && !popup_open);
    let bgc = if selected || targeted {
        if live { skin.sel } else { skin.sel_inactive }
    } else if hovered {
        skin.hover
    } else {
        Color::TRANSPARENT
    };
    let border_color = chrome_border(state, skin, selected || targeted || focused_idle);
    // TEMP(rainbow): solid chrome would hide the rainbow backdrop
    // rectangle; tint it translucent (same 0.35 convention as the
    // marquee) so the rainbow passes through. Toggle off = solid
    // palette colors, identical to the other panes.
    let bgc = if state.rainbow {
        Color { a: bgc.a * 0.35, ..bgc }
    } else {
        bgc
    };
    let styled = container(content).style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: if selected || targeted || focused_idle {
                crate::theme::border_w(1.0)
            } else {
                0.0
            },
            radius: state.view_cfg.style.corner_radius.into(),
        },
        ..Default::default()
    });
    // drop target: directories/archives accept, executables offer
    // "Run with", plain files reject per policy (the caption shows
    // the banned glyph and the status bar says "Cannot drop here")
    let zone_target = state.entries.get(i).map(|e| {
        if e.is_dir() || fs::archive::is_archive_path(&e.path) {
            crate::dnd::DropTarget::Directory(e.path.clone())
        } else if e.is_executable {
            crate::dnd::DropTarget::Executable(e.path.clone())
        } else {
            crate::dnd::DropTarget::File(e.path.clone())
        }
    });
    let el: Element<Message> = mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_right_press(Message::RightPress(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::HoverExit(i))
        .boxed();
    match zone_target {
        Some(t) => crate::dnd::drop_zone(el, t),
        None => el,
    }
}

/// Selection border color of an item chrome: the live accent while
/// the folder pane is active, the grayed pair when it is not
/// (SPEC/Unfocused_Window.md §4), transparent for unselected items.
/// Shared with the Details overlay that redraws selected borders on
/// top of the row fills.
fn chrome_border(state: &Explorer, skin: skin::Skin, selected: bool) -> Color {
    let popup_open = state.ribbon.framework().view().popup_open();
    let live = state.focused
        && !popup_open
        && state.active_pane == crate::model::Pane::Folder;
    let c = if selected {
        if live { skin.sel_border } else { skin.sel_inactive_border }
    } else {
        Color::TRANSPARENT
    };
    // TEMP(rainbow): match the chrome translucency so the redrawn
    // border is identical to the one under the fills
    if state.rainbow {
        Color { a: c.a * 0.6, ..c }
    } else {
        c
    }
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
fn file_scrollable(content: iced::Element<'_, Message>) -> iced::Element<'_, Message> {
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
            x: vp.viewport.absolute_offset().x,
            y: vp.viewport.absolute_offset().y,
            height: vp.viewport.bounds.height,
        })
        .boxed()
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
    top: f32,
    cell: impl Fn(usize, &'a Entry) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let cols = vs::grid_cols(cell_w, gap, width);
    let total_rows = state.entries.len().div_ceil(cols);
    let pitch = cell_h + gap;
    // the grid sits below the reserved files_top strip (rubber-band space)
    let (first, last) = row_window(total_rows, pitch, (state.scroll_y - top).max(0.0), state.viewport_h);
    let mut rows = column![vspacer(top).boxed()].spacing(0);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * pitch).boxed());
    }
    for r in first..last {
        let mut line = row![].spacing(gap);
        for c in 0..cols {
            let idx = r * cols + c;
            if let Some(e) = state.entries.get(idx) {
                line = line.push(cell(idx, e));
            } else {
                line = line.push(Space::new().width(Length::Fixed(cell_w)).boxed());
            }
        }
        rows = rows.push(container(line).height(Length::Fixed(pitch)).boxed());
    }
    if last < total_rows {
        rows = rows.push(vspacer((total_rows - last) as f32 * pitch).boxed());
    }
    file_scrollable(rows.boxed()).boxed()
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
        state.view_index, total, w_with_strip, state.viewport_h, longest, &state.it_label_lines,
        state.details_compact,
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
        vs::Family::Details => cfg.details_pitch(state.details_compact),
        vs::Family::List => cfg.list_col_w(longest),
        vs::Family::SmallIcon => {
            let m = cfg.small_icon(s);
            m.cell_h + m.gap
        }
        vs::Family::IconTop => {
            // dynamic row heights: the strip arrows step one 1-line row
            let m = cfg.icon_top(s);
            let block =
                crate::text_metrics::line_block(cfg.style.font_size, crate::fonts::primary());
            m.pad + s + vs::ICON_LABEL_GAP + vs::label_box_h(1.0, block)
                + vs::LABEL_PAD_BOTTOM
                + m.gap
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
        .boxed()
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
                // The press catcher covers the header only: the §12 gap
                // strip below it stays free, so the rubber band can
                // start there (it must never start on the header
                // itself — the MarqueeZone clamps its top edge to the
                // header's lower border)
                let d = &cfg.layouts.details;
                let header = column![
                    mouse_area(details_header(state, skin))
                        .on_press(Message::ColumnHeaderPress),
                    Space::new().height(Length::Fixed(d.files_gap)),
                ];
                stack![
                    details_view(state, skin),
                    container(header).width(Length::Fill),
                ]
                .boxed()
            }
            vs::Family::List => list_view(state, skin, inner_w),
            vs::Family::SmallIcon => {
                let m = cfg.small_icon(s);
                let top = cfg.files_top(state.view_index);
                grid_view(state, inner_w, m.cell_w, m.cell_h, m.gap, top, |i, e| {
                    small_icon_cell(i, e, state, skin, s, &m)
                })
            }
            vs::Family::IconTop => {
                let rows = cfg.icon_top_rows(state.view_index, total, inner_w, &state.it_label_lines);
                icon_top_grid_view(state, skin, s, &rows)
            }
        }
    };

    // a submitted search shows a placeholder line in the folder pane
    // (real result listing lands later)
    let body: Element<Message> = if let Some(q) = &state.search_query {
        column![
            container(
                text(format!("Searched for \"{q}\" in {}", state.title()))
                    .size(cfg.style.font_size)
                    .color(skin.subtle)
            )
            .width(Length::Fill)
            .padding([4.0, 8.0]),
            body,
        ]
        .boxed()
    } else {
        body
    };

    // rubber-band selection rectangle overlay (all layouts)
    let body: Element<Message> = if let Some(m) = &state.marquee {
        let (x0, x1) = (m.x0.min(m.x1), m.x0.max(m.x1));
        let (y0, y1) = (m.y0.min(m.y1), m.y0.max(m.y1));
        stack![
            body,
            marquee_rect(x0 - state.scroll_x, y0 - state.scroll_y, x1 - x0, y1 - y0, state.rainbow, skin),
        ]
        .boxed()
    } else {
        body
    };

    // The Sensor reports the exact scrollable viewport size (iced's
    // on_scroll stays silent when the content fits, and window resize
    // events carry the WINDOW size — both wrong for the collapse rule).
    // TEMP: the pane background is 100% transparent; the solid color
    // now comes from the backdrop rectangle stacked UNDER the pane
    // (see `pane_backdrop` below)
    let area: Element<Message> = iced::widget::Sensor::new(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_show(|size| Message::FileAreaResized(size.width, size.height))
    .on_resize(|size| Message::FileAreaResized(size.width, size.height))
    .boxed();
    // right-click on empty space (items capture their own first) opens
    // the background menu (SPEC/MENU_SPEC.MD)
    let area = crate::menu::on_right_free(area, Message::ContextBackground);
    // a left press on empty space starts the rubber-band selection.
    // In Details the header overlays the pane top (the content reserves
    // header_h + files_gap with a spacer): the rubber band may cover
    // the reserved files_gap strip but never the header itself, so its
    // top edge clamps to the header's lower border.
    let top = if family == vs::Family::Details {
        state.view_cfg.layouts.details.header_h
    } else {
        0.0
    };
    let area: Element<Message> = MarqueeZone {
        content: area,
        scroll_x: state.scroll_x,
        scroll_y: state.scroll_y,
        top,
    }.boxed();
    // the empty area of the current directory is a drop target
    // (SPEC/DRAGGING.md §4.2; the item zones above outrank it by §6
    // nesting)
    let area = crate::dnd::drop_zone(
        area,
        crate::dnd::DropTarget::EmptyDirectory(state.cwd.clone()),
    );
    // Windows-style scrollbar strip (arrow buttons at both ends); 0px
    // when the content fits, keeping the pane edge right-aligned
    let pane: Element<Message> = if family == vs::Family::List && !state.loading {
        let mut c = column![area];
        if need_strip {
            c = c.push(crate::scrollbar::horizontal(
                state.scroll_x, inner_w, content, line, skin, Message::FileScrollTo,
            ));
        }
        c.boxed()
    } else {
        let mut r = row![area];
        if need_strip {
            r = r.push(crate::scrollbar::vertical(
                state.scroll_y, state.viewport_h, content, line, skin, Message::FileScrollTo,
            ));
        }
        r.boxed()
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
        .boxed();
    // the solid rectangle UNDER the transparent pane
    let view: Element<Message> = stack![pane_backdrop(state.rainbow, skin), view].boxed();
    CtrlZoom {
        content: view,
        ctrl: state.ctrl_held,
        notch_delta: cfg.input.notch_delta,
    }.boxed()
}

/// TEMP: the solid rectangle drawn UNDER the folder view pane (the
/// pane background itself is fully transparent, so this shows
/// through). Normally it is the plain window color; the View-tab
/// "Rainbow" ribbon button paints it with a rainbow gradient instead.
fn pane_backdrop(rainbow: bool, skin: skin::Skin) -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(if rainbow {
                iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(std::f32::consts::FRAC_PI_4)
                        .add_stop(0.0, Color::from_rgb(0.95, 0.15, 0.15))
                        .add_stop(0.2, Color::from_rgb(0.95, 0.55, 0.05))
                        .add_stop(0.4, Color::from_rgb(0.90, 0.85, 0.05))
                        .add_stop(0.6, Color::from_rgb(0.10, 0.70, 0.25))
                        .add_stop(0.8, Color::from_rgb(0.10, 0.45, 0.95))
                        .add_stop(1.0, Color::from_rgb(0.60, 0.20, 0.90)),
                ))
            } else {
                skin.window_bg.into()
            }),
            ..Default::default()
        })
        .boxed()
}

/// The resizer hit target at a column's right edge: a 1px separator
/// line (§15) widened to 4px for the drag (the extra 3px are
/// colorless — §10).
const COL_RESIZER_W: f32 = 4.0;

/// Visible Details columns (indices into `detail_cfg.columns`), in order.
fn details_visible(state: &Explorer) -> Vec<usize> {
    (0..state.detail_cfg.columns.len())
        .filter(|&i| state.details_col_on.get(i).copied().unwrap_or(true))
        .collect()
}

/// Width of the visible Details column block: the column widths plus
/// a resizer between each pair (§15). Shared by the row builder and
/// the selection-border overlay (same geometry).
fn details_cols_w(state: &Explorer) -> f32 {
    let vis = details_visible(state);
    vis.iter().enumerate().fold(0.0, |acc, (n, &g)| {
        acc + details_col_w(state, g)
            + if n + 1 < vis.len() { COL_RESIZER_W } else { 0.0 }
    })
}

/// The width of a Details column (the header resizer adjusts it).
fn details_col_w(state: &Explorer, col: usize) -> f32 {
    state
        .details_cols
        .get(col)
        .copied()
        .unwrap_or(state.detail_cfg.columns[col].width)
}

/// SPEC/detail-view.txt §2: the sort arrow — 9px wide, 5px tall,
/// sitting on top of the column title, touching the header's top line.
/// Drawn as an inline SVG so the hover state can tint it blue (§9).
fn sort_arrow(ascending: bool, w: f32, h: f32, color: Color) -> Element<'static, Message> {
    // a fully transparent color means "hidden" — but hex() drops alpha,
    // so the polygon is omitted instead (the empty svg keeps the space)
    let points = if color.a == 0.0 {
        String::new()
    } else {
        let p = if ascending {
            "4.5,0.5 8.5,4.5 0.5,4.5" // ▲
        } else {
            "0.5,0.5 8.5,0.5 4.5,4.5" // ▼
        };
        format!("<polygon points='{p}' fill='{}'/>", crate::icons::hex(color))
    };
    let handle = iced::widget::svg::Handle::from_memory(
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 9 5'>{points}</svg>",
        )
        .into_bytes(),
    );
    iced::widget::svg(handle)
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .boxed()
}

/// SPEC/detail-view.txt column header. Every visible column is
/// `16px space + title + fill + 1px line + 15px dropdown button`
/// (§4/§6), followed by its right-edge resizer (§10/§15). The title
/// background is fully transparent, 50% blue on hover, 75% while
/// pressed; the title text is dark blue. The dropdown button and its
/// separator line are invisible until the column is hovered (they keep
/// their space). A click sorts, the dropdown opens the filter menu, a
/// right-click opens the column menu. The block is as wide as the
/// columns need — no 100% fill (§7/§10).
fn details_header(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let d = &state.view_cfg.layouts.details;
    let dc = &state.detail_cfg;
    let mut cols = row![Space::new().width(Length::Fixed(dc.files_left))];
    for &g in &details_visible(state) {
        let c = &dc.columns[g];
        let w = details_col_w(state, g);
        let hovered = state.col_hovered == Some(g);
        let pressed = state.col_pressed == Some(g);
        // the title text is dark blue; the hover/press tints are the
        // accent at 50% / 75% over the transparent background
        let color = skin.accent_dark;
        let bgc = if pressed {
            Color { a: 0.75, ..skin.accent }
        } else if hovered {
            Color { a: 0.50, ..skin.accent }
        } else {
            Color::TRANSPARENT
        };
        // §2: the sort arrow sits ON TOP of the title, centered in the
        // column (other columns reserve the same 5px so the titles
        // stay aligned)
        let arrow: Element<Message> = if c.sort.is_some() && state.sort == c.sort.unwrap() {
            sort_arrow(state.sort_asc, dc.arrow_w, dc.arrow_h, color)
        } else {
            Space::new()
                .width(Length::Fixed(dc.arrow_w))
                .height(Length::Fixed(dc.arrow_h))
                .boxed()
        };
        // §5/§6: the 1px line + the 15px dropdown button at the right —
        // invisible until the column is hovered (space is reserved)
        let dropdown = mouse_area(
            container(sort_arrow(
                false,
                dc.arrow_w,
                dc.arrow_h,
                if hovered { color } else { Color::TRANSPARENT },
            ))
            .width(Length::Fixed(dc.dropdown_w))
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::ColumnFilterMenu(g))
        .on_enter(Message::ColumnHoverEnter(g))
        .on_exit(Message::ColumnHoverExit(g));
        let cell = container(
            column![
                container(arrow)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .height(Length::Fixed(dc.arrow_h)),
                row![
                    container(text(c.label.clone()).size(dc.font_size).color(color))
                        .padding(iced::Padding {
                            left: dc.text_pad_left,
                            ..Default::default()
                        }),
                    Space::new().width(Length::Fill),
                    container(
                        Space::new()
                            .width(Length::Fixed(crate::theme::border_w(dc.sep_w)))
                            .height(Length::Fill)
                    )
                    // a faint line was almost invisible on the window
                    // background — use the darker secondary text color
                    .style(bg(if hovered { skin.subtle } else { Color::TRANSPARENT })),
                    dropdown,
                ]
                .height(Length::Fill)
                .align_y(iced::Alignment::Center),
            ]
            .height(Length::Fixed(d.header_h)),
        )
        .width(Length::Fixed(w))
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            ..Default::default()
        });
        let cell = mouse_area(cell)
            .on_press(Message::ColumnSortClick(g))
            .on_right_press(Message::ColumnSettingsMenu)
            .on_enter(Message::ColumnHoverEnter(g))
            .on_exit(Message::ColumnHoverExit(g));
        cols = cols.push(cell.boxed());
        // §10/§15: the column's right resizer — the 1px separator line
        // with a 4px drag target; resizes only its own column
        let resizer = mouse_area(
            container(
                row![
                    container(
                        Space::new()
                            .width(Length::Fixed(crate::theme::border_w(1.0)))
                            .height(Length::Fill)
                    )
                    .style(bg(skin.sep)),
                    Space::new().width(Length::Fixed(COL_RESIZER_W - crate::theme::border_w(1.0))),
                ]
                .height(Length::Fixed(d.header_h)),
            )
            .width(Length::Fixed(COL_RESIZER_W)),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::ColumnSplitBegin(g));
        cols = cols.push(resizer.boxed());
    }
    // the strip background is fully transparent (the titles tint on
    // hover/press only): the pane backdrop shows through
    container(cols.height(Length::Fixed(d.header_h)).align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .boxed()
}

// ---------------------------------------------------------------------------
// index 0 — Content (§8.1)

fn content_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let row_h = state.view_cfg.layouts.content.row_h;
    let total = state.entries.len();
    // the rows sit below the reserved files_top strip (rubber-band space)
    let top = state.view_cfg.files_top(state.view_index);
    let (first, last) = row_window(total, row_h, (state.scroll_y - top).max(0.0), state.viewport_h);
    let mut rows = column![vspacer(top).boxed()].spacing(0);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * row_h).boxed());
    }
    for i in first..last {
        if let Some(e) = state.entries.get(i) {
            rows = rows.push(content_row(i, e, state, skin, width));
        }
    }
    if last < total {
        rows = rows.push(vspacer((total - last) as f32 * row_h).boxed());
    }
    file_scrollable(rows.boxed()).boxed()
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
    // base on the resolved primary family (fonts.yaml priority list),
    // not Font::DEFAULT: an explicit .font() replaces the app's
    // default_font and would drop Selawik for generic sans-serif
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::primary()
    };
    let text_w = (width - c.icon - c.icon_text_gap - 2.0 * c.pad_w - 8.0).max(60.0);
    let date = e.modified.map(model::format_date).unwrap_or_default();
    let lines = column![
        text(text_metrics::ellipsize(&e.name, text_w, c.name_size, semibold))
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
        container(Space::new().width(Length::Fill).height(Length::Fixed(crate::theme::border_w(1.0))))
            .width(Length::Fill)
            .style(bg(skin.sep)),
    ];
    item_chrome(i, content, state, skin)
}

// ---------------------------------------------------------------------------
// index 1 — Tiles (§8.1)

fn tiles_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let t = &state.view_cfg.layouts.tiles;
    let top = state.view_cfg.files_top(state.view_index);
    grid_view(state, width, t.cell_w, t.cell_h, 0.0, top, |i, e| {
        tile_cell(i, e, state, skin)
    })
}

/// §8.1 Tiles cell: icon on the left, name + Type + Size (3 lines max).
fn tile_cell<'a>(i: usize, e: &Entry, state: &'a Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let t = &state.view_cfg.layouts.tiles;
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::primary()
    };
    let text_w = t.cell_w - t.icon - t.icon_text_gap - 2.0 * t.pad;
    let content = row![
        crate::icon::for_entry(e, t.icon, &skin),
        column![
            text(text_metrics::ellipsize(&e.name, text_w, t.name_size, semibold))
                .size(t.name_size)
                .font(semibold)
                .color(skin.text),
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
    // the pitch is row_h − 1 border px: adjacent rows overlap by one
    // border px so their selected borders share a single line. The
    // rows must land exactly on the model pitch grid (top + i·pitch)
    // that the selection overlay and the hit tests use. A Fixed(pitch)
    // wrapper per row would clamp the Shrink-height chrome into the
    // slot (a Fixed length clears the infinite limit), shifting its
    // bottom border 1 px off the overlay — and spacing(gap) inserts a
    // gap after EVERY child, so the spacers are shaved by it to
    // compensate
    let pitch = state.view_cfg.details_pitch(state.details_compact);
    let gap = pitch - row_h;
    let total = state.entries.len();
    // the content reserves the header strip + the §12 gap (the real
    // header overlays the strip)
    let top = d.header_h + d.files_gap;
    let (first, last) = row_window(total, pitch, (state.scroll_y - top).max(0.0), state.viewport_h);
    let mut rows = column![vspacer(top - gap)].spacing(gap);
    if first > 0 {
        rows = rows.push(vspacer(first as f32 * pitch - gap).boxed());
    }
    for i in first..last {
        if let Some(e) = state.entries.get(i) {
            rows = rows.push(file_row(i, e, state, skin));
        }
    }
    if last < total {
        rows = rows.push(vspacer((total - last) as f32 * pitch - gap).boxed());
    }
    // tail room for the chrome's bottom overflow (one border px past
    // the last slot): without it the stack would clamp the overlay's
    // last border box 1 px short
    rows = rows.push(vspacer((row_h - pitch).max(0.0)).boxed());
    // row_gap overlaps adjacent rows by one border px so their
    // selected borders share a single line — but the fill of a hovered
    // neighbor row (drawn later) paints over that line, so the visible
    // rows' selection borders are drawn once more, on top of the fills
    let rows: Element<Message> = stack![
        rows.boxed(),
        selection_borders(state, skin, first, last, top, pitch, row_h),
    ]
    .boxed();
    file_scrollable(rows).boxed()
}

/// Border-only rectangles of the visible selected Details rows and
/// the idle selector, stacked above the row fills (see `details_view`).
/// Spacer-positioned
/// like the marquee rectangle; the geometry mirrors `file_row`'s
/// chrome: 0 → files_left margin, the column block, the 4px chrome
/// pad at the right (§13/§16).
fn selection_borders<'a>(
    state: &'a Explorer,
    skin: skin::Skin,
    first: usize,
    last: usize,
    top: f32,
    pitch: f32,
    row_h: f32,
) -> Element<'a, Message> {
    let w = state.detail_cfg.files_left + details_cols_w(state) + state.detail_cfg.chrome_pad;
    let radius = state.view_cfg.style.corner_radius;
    let mut overlay = stack![];
    for i in first..last {
        // selected rows redraw their border above the neighbor fills;
        // the idle selector draws the same border-only rectangle
        if !state.selected.contains(&i)
            && !(state.focus_shown && state.focus_item == Some(i))
        {
            continue;
        }
        let color = chrome_border(state, skin, true);
        if color.a == 0.0 {
            continue;
        }
        let y = (top + i as f32 * pitch).max(0.0);
        overlay = overlay.push(
            column![
                vspacer(y),
                container(Space::new().width(Length::Fixed(w)).height(Length::Fixed(row_h)))
                    .style(move |_t: &iced::Theme| container::Style {
                        border: iced::Border {
                            color,
                            width: crate::theme::border_w(1.0),
                            radius: radius.into(),
                        },
                        ..Default::default()
                    }),
            ]
            .boxed(),
        );
    }
    overlay.boxed()
}

fn file_row<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let dc = &state.detail_cfg;
    let row_h = state.view_cfg.details_row_h(state.details_compact);
    let vis = details_visible(state);
    let total_w = details_cols_w(state);
    let mut cells = row![].align_y(iced::Alignment::Center);
    for (n, &g) in vis.iter().enumerate() {
        let w = details_col_w(state, g);
        let cell = if dc.columns[g].id == "name" {
            name_cell(i, e, state, skin, w, row_h)
        } else {
            value_cell(&dc.columns[g], e, state, skin, w, row_h)
        };
        cells = cells.push(cell);
        // between columns: the header's right resizers sit here (§15)
        if n + 1 < vis.len() {
            cells = cells.push(Space::new().width(Length::Fixed(COL_RESIZER_W)).boxed());
        }
    }
    // §13: 14px left of the files; §16: the chrome keeps a 4px space
    // inside the column block's right border
    let content = container(cells)
        .width(Length::Fixed(dc.chrome_pad + total_w + dc.chrome_pad))
        .height(Length::Fixed(row_h))
        .padding(iced::Padding {
            left: dc.chrome_pad,
            ..Default::default()
        });
    item_chrome(
        i,
        row![
            Space::new().width(Length::Fixed(dc.files_left - dc.chrome_pad)),
            content,
        ],
        state,
        skin,
    )
}

/// The Details name cell: icon + (elided) name; the renaming row swaps
/// the name for the edit field. The cell starts at the column edge —
/// the 14px files margin lives outside (§13).
fn name_cell<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    w: f32,
    row_h: f32,
) -> Element<'a, Message> {
    let d = &state.view_cfg.layouts.details;
    let font = state.view_cfg.style.font_size;
    const ICON_GAP: f32 = 4.0;
    let name_w = (w - d.icon - ICON_GAP).max(0.0);
    // The rename editor occupies the exact same single-line label box
    // (same width, same row height) — a bordered input where the
    // elided label was.
    let name: Element<'a, Message> = if state.renaming.as_ref().map(|(ri, _)| *ri) == Some(i) {
        // text_input borrows the value — borrow the state's buffer ('a);
        // the submit payload owns a clone
        let buf = &state.renaming.as_ref().expect("matched above").1;
        text_input("Name", buf)
            .on_input(Message::RenameInput)
            .on_submit(Message::CommitRename(buf.clone()))
            .size(font)
            .padding(2)
            .style(move |_t: &iced::Theme, _status: text_input::Status| text_input::Style {
                background: skin.menu_bg.into(),
                border: iced::Border {
                    color: skin.accent,
                    width: crate::theme::border_w(1.0),
                    radius: 0.0.into(),
                },
                placeholder: skin.disabled,
                value: skin.text,
                selection: skin.sel,
            })
            .boxed()
    } else {
        text(text_metrics::ellipsize(&e.name, name_w, font, crate::fonts::primary()))
            .size(font)
            .color(skin.text)
            .boxed()
    };
    container(
        row![
            crate::icon::for_entry(e, d.icon, &skin),
            container(name)
                .width(Length::Fixed(name_w))
                .height(Length::Fixed(row_h))
                .align_y(iced::alignment::Vertical::Center),
        ]
        .spacing(ICON_GAP)
        .height(Length::Fixed(row_h))
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(w))
    .height(Length::Fixed(row_h))
    .boxed()
}

/// A Details value cell: the column's property text, padded to the
/// header title (§4). An overlong value is tail-cut with "...".
fn value_cell<'a>(
    col: &crate::details_view::ColumnCfg,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    w: f32,
    row_h: f32,
) -> Element<'a, Message> {
    let dc = &state.detail_cfg;
    let font = state.view_cfg.style.font_size;
    let value = match col.id.as_str() {
        "size" => item_size(e),
        "type" => model::type_label(e).to_string(),
        "modified" => e.modified.map(model::format_date).unwrap_or_default(),
        _ => String::new(),
    };
    container(
        text(text_metrics::ellipsize(
            &value,
            (w - dc.text_pad_left).max(0.0),
            font,
            crate::fonts::primary(),
        ))
        .size(font)
            .color(skin.text),
    )
    .width(Length::Fixed(w))
    .height(Length::Fixed(row_h))
    .align_y(iced::alignment::Vertical::Center)
    .padding(iced::Padding {
        left: dc.text_pad_left,
        ..Default::default()
    })
    .boxed()
}

// ---------------------------------------------------------------------------
// index 3 — List (§8.1): name-only columns flowing top to bottom, then
// left to right; the pane scrolls horizontally.

fn list_view(state: &Explorer, skin: skin::Skin, width: f32) -> Element<'_, Message> {
    let longest = state
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0);
    let col_w = state.view_cfg.list_col_w(longest);
    let rows_per_col = state.view_cfg.list_rows_per_col(state.viewport_h);
    let total = state.entries.len();
    let total_cols = total.div_ceil(rows_per_col);
    // virtualize by column
    let view_cols = (width / col_w).ceil() as usize + 3;
    let first = ((state.scroll_x / col_w).floor() as usize).saturating_sub(1);
    let last = (first + view_cols).min(total_cols);
    let mut strip: iced::widget::Row<Element<'_, Message>> = row![].spacing(0);
    if first > 0 {
        strip = strip.push(Space::new().width(Length::Fixed(first as f32 * col_w)).boxed());
    }
    for c in first..last {
        let mut col = column![].spacing(0);
        for r in 0..rows_per_col {
            let idx = c * rows_per_col + r;
            if let Some(e) = state.entries.get(idx) {
                col = col.push(list_cell(idx, e, state, skin, col_w));
            }
        }
        strip = strip.push(container(col).width(Length::Fixed(col_w)).boxed());
    }
    if last < total_cols {
        strip = strip.push(Space::new().width(Length::Fixed(
            (total_cols - last) as f32 * col_w,
        )).boxed());
    }
    // the strip sits below the reserved files_top space (rubber-band space)
    let strip: Element<Message> = column![vspacer(state.view_cfg.files_top(state.view_index)).boxed(), strip.boxed()].boxed();
    scrollable(strip)
        .id(file_scrollable_id())
        .width(Length::Fill)
        .height(Length::Fill)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .on_scroll(|vp| Message::Scrolled {
            x: vp.viewport.absolute_offset().x,
            y: vp.viewport.absolute_offset().y,
            height: vp.viewport.bounds.height,
        })
        .boxed()
}

/// §8.1 List cell: icon + name on 1 line. The full name is shown (the
/// pane scrolls horizontally); only names over the unit budget
/// (name_max_units, non-ASCII = 2) are tail-cut with "...". The cell
/// clips at the column edge.
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
        text(truncate_units(&e.name, l.name_max_units))
            .size(state.view_cfg.style.font_size)
            .color(skin.text)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(l.icon_text_gap)
    .padding([l.pad_h, l.pad_w])
    .align_y(iced::Alignment::Center);
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(col_w))
            .height(Length::Fixed(l.row_h))
            .align_y(iced::alignment::Vertical::Center)
            .clip(true),
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
        text(text_metrics::ellipsize(
            &e.name,
            m.label_w,
            state.view_cfg.style.font_size,
            crate::fonts::primary(),
        ))
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

/// §8.3 icon-top grid with dynamic row heights: the row pitch follows
/// the tallest label in the row; each item's chrome (border / hover /
/// selection area) is only as tall as its own label needs.
fn icon_top_grid_view<'a>(
    state: &'a Explorer,
    skin: skin::Skin,
    s: f32,
    rows: &vs::IconTopRows,
) -> Element<'a, Message> {
    let m = state.view_cfg.icon_top(s);
    let total_rows = rows.starts.len();
    if total_rows == 0 {
        return file_scrollable(Space::new().boxed()).boxed();
    }
    // first row whose start is at or above the scroll offset
    let first = rows
        .starts
        .partition_point(|&y| y <= state.scroll_y)
        .saturating_sub(1);
    // rows intersecting the viewport, one row of overscan at the edge
    let mut last = (first + 1).min(total_rows);
    while last < total_rows && rows.starts[last] < state.scroll_y + state.viewport_h {
        last += 1;
    }
    last = (last + 1).min(total_rows);
    let mut grid = column![].spacing(0);
    // starts[first] is always > 0 now: the row prefix sums begin at
    // the reserved files_top strip, so the spacer must be pushed even
    // for first == 0 (otherwise row 0 renders on top of the strip and
    // presses there land on the items, not on empty space)
    grid = grid.push(vspacer(rows.starts[first]).boxed());
    for r in first..last {
        let mut line = row![].spacing(m.gap);
        for c in 0..rows.cols {
            let idx = r * rows.cols + c;
            if let Some(e) = state.entries.get(idx) {
                line = line.push(icon_top_cell(idx, e, state, skin, s, &m));
            } else {
                line = line.push(Space::new().width(Length::Fixed(m.cell_w)).boxed());
            }
        }
        let pitch = if r + 1 < total_rows {
            rows.starts[r + 1] - rows.starts[r]
        } else {
            rows.extent - rows.starts[r]
        };
        grid = grid.push(container(line).height(Length::Fixed(pitch)).boxed());
    }
    if last < total_rows {
        grid = grid.push(vspacer(rows.extent - rows.starts[last]).boxed());
    }
    file_scrollable(grid.boxed()).boxed()
}

fn icon_top_cell<'a>(
    i: usize,
    e: &Entry,
    state: &'a Explorer,
    skin: skin::Skin,
    s: f32,
    m: &vs::IconTopMetrics,
) -> Element<'a, Message> {
    // §8.3: the label is identical in every item state (hover /
    // selected): hard-wrapped at the cell width — filling the
    // selection rectangle — into at most `m.lines` lines, breaking
    // mid-word; when the wrap exceeds the cap, the remainder is joined
    // and ellipsized onto the last line (`text_metrics::wrap_hard`,
    // exact cosmic-text widths). The chrome height follows this item's
    // own label line count (dynamic row heights, `it_label_lines`);
    // only the icon keeps the cell padding.
    let c = &state.view_cfg.icon_top;
    let own = state.it_label_lines.get(i).copied().unwrap_or(1).max(1) as f32;
    let label = text_metrics::wrap_hard(
        &e.name,
        m.cell_w - c.label_pad,
        m.lines as usize,
        state.view_cfg.style.font_size,
        crate::fonts::primary(),
    );
    // Each line's box is the font's glyph block (measured) cropped by
    // LABEL_DESCENT_SHAVE — the descent is empty under most characters
    // and is what made the labels look loose; real descenders (g/j/p/
    // q/y) dip into the 1px gap / the bottom pad (nothing clips, the
    // container keeps the LABEL_LINE_BOTTOM pad).
    let font = crate::fonts::primary();
    let block = text_metrics::line_block(state.view_cfg.style.font_size, font);
    let line_h = (block - vs::LABEL_DESCENT_SHAVE).max(1.0);
    let line_height = iced::widget::text::LineHeight::Absolute(line_h.into());
    let label_col = column(
        label
            .iter()
            .map(|l| {
                text(l.clone())
                    .size(state.view_cfg.style.font_size)
                    .line_height(line_height)
                    .color(skin.text)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .boxed()
            })
            .collect::<Vec<Element<Message>>>(),
    )
    .spacing(vs::LABEL_LINE_GAP);
    let content = column![
        // The icon area is pinned to exactly s (the image widget's
        // height follows the graphic's aspect ratio, not s) so the
        // label's vertical position — and the chrome math in
        // ViewCfg::icon_top_rows — is exact.
        container(crate::icon::for_entry(e, s, &skin))
            .width(Length::Fill)
            .height(Length::Fixed(s))
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .padding([0.0, m.pad]),
        container(label_col)
            .width(Length::Fill)
            .height(Length::Fixed(vs::label_box_h(own, block)))
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: vs::LABEL_LINE_BOTTOM,
                left: 0.0,
            })
            .clip(true),
    ]
    .spacing(vs::ICON_LABEL_GAP);
    // chrome = pad + icon + ICON_LABEL_GAP + own label box + bottom pad
    let item_h =
        m.pad + s + vs::ICON_LABEL_GAP + vs::label_box_h(own, block) + vs::LABEL_PAD_BOTTOM;
    item_chrome(
        i,
        container(content)
            .width(Length::Fixed(m.cell_w))
            .height(Length::Fixed(item_h))
            .padding(iced::Padding {
                top: m.pad,
                right: 0.0,
                bottom: vs::LABEL_PAD_BOTTOM,
                left: 0.0,
            }),
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

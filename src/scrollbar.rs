//! Windows-style scrollbar strip: arrow buttons at both ends scrolling
//! one "slight" line step, a track with a draggable thumb, page step on
//! track press. The strip remote-controls the iced scrollable next to
//! it: it publishes an absolute target offset (`Message::FileScrollTo` /
//! `Message::NavScrollTo`), the model issues `operation::scroll_to` and
//! the scrollable reports its position back via on_scroll.

use iced::Widget as _;
use iced::advanced::widget::{Operation, Tree, Widget, tree};
use iced::advanced::{Layout, Renderer as _, Shell, layout, renderer};
use iced::widget::Space;
use iced::{Color, Element, Length, Point, Rectangle, Size, mouse};

use crate::model::Message;
use crate::ui_common::skin;

/// Strip thickness (Win8 scrollbar ~17px at 96 DPI).
pub(crate) const THICKNESS: f32 = 16.0;
/// Arrow-button length along the scroll axis.
const BUTTON: f32 = 16.0;
/// Minimum thumb length along the axis.
const MIN_THUMB: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    Vertical,
    Horizontal,
}

/// Strip zone under the cursor / pointer.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Zone {
    #[default]
    None,
    /// Up / left arrow button.
    Dec,
    /// Down / right arrow button.
    Inc,
    Thumb,
    TrackDec,
    TrackInc,
}

#[derive(Default)]
struct State {
    hover: Zone,
    /// Thumb drag: cursor position along the axis minus the thumb start.
    drag_grab: Option<f32>,
}

impl iced::advanced::widget::Meta for WinScrollbar {}

struct WinScrollbar {
    axis: Axis,
    /// Current scroll offset (px), fed back from the scrollable.
    offset: f32,
    /// Viewport extent along the axis (px).
    viewport: f32,
    /// Content extent along the axis (px).
    content: f32,
    /// Arrow-button line step (px).
    line: f32,
    skin: skin::Skin,
    /// Builds the scroll-request message for an absolute target offset.
    on_scroll: fn(f32) -> Message,
}

pub(crate) fn vertical(
    offset: f32,
    viewport: f32,
    content: f32,
    line: f32,
    skin: skin::Skin,
    on_scroll: fn(f32) -> Message,
) -> Element<'static, Message> {
    WinScrollbar { axis: Axis::Vertical, offset, viewport, content, line, skin, on_scroll }.boxed()
}

pub(crate) fn horizontal(
    offset: f32,
    viewport: f32,
    content: f32,
    line: f32,
    skin: skin::Skin,
    on_scroll: fn(f32) -> Message,
) -> Element<'static, Message> {
    WinScrollbar { axis: Axis::Horizontal, offset, viewport, content, line, skin, on_scroll }.boxed()
}

/// The empty square where the vertical and horizontal strips meet:
/// reserved space belonging to neither (no hover, no press). It stays
/// transparent so the folder view background (the pane backdrop —
/// white in light mode) shows through, like the empty area beside a
/// short list.
pub(crate) fn corner() -> Element<'static, Message> {
    Space::new()
        .width(Length::Fixed(THICKNESS))
        .height(Length::Fixed(THICKNESS))
        .boxed()
}

/// Axis-parallel/-perpendicular decomposition of a point/size.
impl WinScrollbar {
    fn along(&self, p: Point) -> f32 {
        match self.axis {
            Axis::Vertical => p.y,
            Axis::Horizontal => p.x,
        }
    }

    fn max_scroll(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    /// (track start, thumb start, thumb length) relative to the widget
    /// origin, along the axis.
    fn geometry(&self, bounds: Rectangle) -> (f32, f32, f32) {
        let len = match self.axis {
            Axis::Vertical => bounds.height,
            Axis::Horizontal => bounds.width,
        };
        let track_len = (len - 2.0 * BUTTON).max(0.0);
        let max_scroll = self.max_scroll();
        let thumb_len = if max_scroll > 0.0 {
            (track_len * self.viewport / self.content.max(1.0))
                .clamp(MIN_THUMB.min(track_len), track_len)
        } else {
            track_len // disabled: the thumb fills the track
        };
        let free = (track_len - thumb_len).max(0.0);
        let frac = if max_scroll > 0.0 {
            (self.offset / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (BUTTON, BUTTON + frac * free, thumb_len)
    }

    fn zone_at(&self, pos: f32, bounds: Rectangle) -> Zone {
        let len = match self.axis {
            Axis::Vertical => bounds.height,
            Axis::Horizontal => bounds.width,
        };
        let (_, thumb_start, thumb_len) = self.geometry(bounds);
        if pos < BUTTON {
            Zone::Dec
        } else if pos >= len - BUTTON {
            Zone::Inc
        } else if pos < thumb_start {
            Zone::TrackDec
        } else if pos < thumb_start + thumb_len {
            Zone::Thumb
        } else {
            Zone::TrackInc
        }
    }

    /// Scroll offset for a dragged thumb start position (relative).
    fn thumb_to_offset(&self, thumb_start: f32, bounds: Rectangle) -> f32 {
        let (_, _, thumb_len) = self.geometry(bounds);
        let len = match self.axis {
            Axis::Vertical => bounds.height,
            Axis::Horizontal => bounds.width,
        };
        let free = (len - 2.0 * BUTTON - thumb_len).max(0.0);
        if free <= 0.0 {
            return 0.0;
        }
        ((thumb_start - BUTTON) / free).clamp(0.0, 1.0) * self.max_scroll()
    }

    /// Press at a position (mouse or first touch finger).
    fn press_at(
        &self,
        state: &mut State,
        position: Point,
        bounds: Rectangle,
        shell: &mut Shell<'_, Message>,
    ) {
        if !bounds.contains(position) {
            return;
        }
        let zone = self.zone_at(self.along(position) - self.along(bounds.position()), bounds);
        if self.max_scroll() <= 0.0 {
            shell.capture_event();
            return;
        }
        match zone {
            Zone::Dec => shell.publish((self.on_scroll)((self.offset - self.line).max(0.0))),
            Zone::Inc => shell
                .publish((self.on_scroll)((self.offset + self.line).min(self.max_scroll()))),
            Zone::TrackDec => shell
                .publish((self.on_scroll)((self.offset - self.viewport).max(0.0))),
            Zone::TrackInc => shell
                .publish((self.on_scroll)((self.offset + self.viewport).min(self.max_scroll()))),
            Zone::Thumb => {
                let (_, thumb_start, _) = self.geometry(bounds);
                state.drag_grab =
                    Some(self.along(position) - self.along(bounds.position()) - thumb_start);
            }
            Zone::None => {}
        }
        shell.capture_event();
    }

    /// Cursor/finger move: drag the thumb, else track the hover zone.
    fn move_at(
        &self,
        state: &mut State,
        position: Point,
        bounds: Rectangle,
        over: bool,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Some(grab) = state.drag_grab {
            let thumb_start = self.along(position) - self.along(bounds.position()) - grab;
            let target = self.thumb_to_offset(thumb_start, bounds);
            shell.publish((self.on_scroll)(target));
            shell.capture_event();
            return;
        }
        let zone = if over {
            self.zone_at(self.along(position) - self.along(bounds.position()), bounds)
        } else {
            Zone::None
        };
        if zone != state.hover {
            state.hover = zone;
            shell.request_redraw();
        }
    }
}

impl Widget<Message, iced::Theme, iced::Renderer> for WinScrollbar {
    fn size(&self) -> Size<Length> {
        match self.axis {
            Axis::Vertical => Size::new(Length::Fixed(THICKNESS), Length::Fill),
            Axis::Horizontal => Size::new(Length::Fill, Length::Fixed(THICKNESS)),
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) {
        tree.size = limits.resolve(
            match self.axis {
                Axis::Vertical => Length::Fixed(THICKNESS),
                Axis::Horizontal => Length::Fill,
            },
            match self.axis {
                Axis::Vertical => Length::Fill,
                Axis::Horizontal => Length::Fixed(THICKNESS),
            },
            Size::ZERO,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { position })
            | iced::Event::Touch(iced::touch::Event::FingerMoved { position, .. }) => {
                self.move_at(state, *position, bounds, cursor.is_over(bounds), shell);
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position() {
                    self.press_at(state, position, bounds, shell);
                }
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => {
                self.press_at(state, *position, bounds, shell);
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | iced::Event::Touch(iced::touch::Event::FingerLifted { .. })
            | iced::Event::Touch(iced::touch::Event::FingerLost { .. }) => {
                if state.drag_grab.is_some() {
                    state.drag_grab = None;
                    shell.capture_event();
                }
            }
            // wheel over the strip scrolls too (same conversion as iced's
            // scrollable: 60px per line unit). Note: inside the app the
            // WheelBoost wrapper captures wheel events first (it applies
            // the configured multiplier); this arm keeps the strip
            // usable on its own.
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !cursor.is_over(bounds) {
                    return;
                }
                let px = match delta {
                    mouse::ScrollDelta::Lines { x, y } => {
                        60.0 * match self.axis {
                            Axis::Vertical => *y,
                            Axis::Horizontal => {
                                if y.abs() > 0.01 { *y } else { *x }
                            }
                        }
                    }
                    mouse::ScrollDelta::Pixels { x, y } => match self.axis {
                        Axis::Vertical => *y,
                        Axis::Horizontal => {
                            if y.abs() > 0.01 { *y } else { *x }
                        }
                    },
                };
                if px.abs() > 0.01 {
                    let target = (self.offset - px).clamp(0.0, self.max_scroll());
                    shell.publish((self.on_scroll)(target));
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let skin = self.skin;
        let enabled = self.max_scroll() > 0.0;
        let vertical = self.axis == Axis::Vertical;

        let quad = |bounds: Rectangle| renderer::Quad {
            bounds,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        };

        // strip background (no inner-edge separator: the pane boundary
        // strips already frame the scroller)
        renderer.fill_quad(quad(bounds), skin.col_header_bg);

        // arrow buttons at both ends
        let (_, thumb_start, thumb_len) = self.geometry(bounds);
        let end = match self.axis {
            Axis::Vertical => bounds.height,
            Axis::Horizontal => bounds.width,
        } - BUTTON;
        for (zone, start) in [(Zone::Dec, 0.0), (Zone::Inc, end)] {
            let b = if vertical {
                Rectangle { y: bounds.y + start, height: BUTTON, ..bounds }
            } else {
                Rectangle { x: bounds.x + start, width: BUTTON, ..bounds }
            };
            let bg = if state.hover == zone && enabled { skin.hover } else { skin.col_header_bg };
            renderer.fill_quad(quad(b), bg);
            let center = Point::new(b.x + b.width / 2.0, b.y + b.height / 2.0);
            let arrow = if enabled { skin.subtle } else { skin.disabled };
            draw_arrow(renderer, self.axis, zone == Zone::Dec, center, arrow);
        }

        // thumb
        let tb = if vertical {
            Rectangle { y: bounds.y + thumb_start, height: thumb_len, ..bounds }
        } else {
            Rectangle { x: bounds.x + thumb_start, width: thumb_len, ..bounds }
        };
        if enabled {
            let color = if state.drag_grab.is_some() {
                skin.subtle
            } else if state.hover == Zone::Thumb {
                skin.disabled
            } else {
                skin.sep
            };
            renderer.fill_quad(quad(tb.shrink(1.0)), color);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        // Windows keeps the plain arrow cursor over the scrollbar
        mouse::Interaction::default()
    }
}

// ---------------------------------------------------------------------------
// wheel-speed wrapper

/// Wrap a pane so wheel events scroll its scrollable with the configured
/// speed multiplier (iced's built-in 60px per line unit is too slow).
/// The strip protocol is reused: the wrapper publishes an absolute
/// target offset (`FileScrollTo`/`NavScrollTo`), the model issues
/// `operation::scroll_to`. Ctrl+wheel is left alone (the CtrlZoom
/// wrapper around the file view owns it). Transparent wrapper:
/// state/layout/draw delegate to the content.
impl iced::advanced::widget::Meta for WheelBoost<'_> {}

struct WheelBoost<'a> {
    content: Element<'a, Message>,
    /// Current scroll offset along `axis`.
    offset: f32,
    /// Maximum scroll offset (content extent minus viewport).
    max_scroll: f32,
    multiplier: f32,
    axis: Axis,
    on_scroll: fn(f32) -> Message,
}

pub(crate) fn wheel_boost<'a>(
    content: Element<'a, Message>,
    offset: f32,
    max_scroll: f32,
    multiplier: f32,
    axis: Axis,
    on_scroll: fn(f32) -> Message,
) -> Element<'a, Message> {
    WheelBoost { content, offset, max_scroll, multiplier, axis, on_scroll }.boxed()
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for WheelBoost<'a> {
    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn tag(&self) -> tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> tree::State {
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
        if let iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && cursor.is_over(layout.bounds())
        {
            let pick = |x: f32, y: f32| match self.axis {
                Axis::Vertical => y,
                Axis::Horizontal => {
                    if y.abs() > 0.01 { y } else { x }
                }
            };
            // same conversion as iced's scrollable: 60px per line unit
            let px = match delta {
                mouse::ScrollDelta::Lines { x, y } => pick(*x, *y) * 60.0,
                mouse::ScrollDelta::Pixels { x, y } => pick(*x, *y),
            };
            if px.abs() > 0.01 {
                // at the scroll ends the wheel is not ours: pass the
                // event through uncaptured (no useless scroll_to)
                let at_end = px < 0.0 && self.offset >= self.max_scroll - 0.5
                    || px > 0.0 && self.offset <= 0.5;
                if !at_end {
                    shell.publish((self.on_scroll)(self.offset - px * self.multiplier));
                    shell.capture_event();
                    return;
                }
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
        translation: iced::Vector,
        window: iced::Size,
    ) -> Vec<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}

/// 7x4 arrow triangle (scanline quads — the renderer has no triangle
/// primitive), pointing up/left (`dec`) or down/right.
fn draw_arrow(
    renderer: &mut iced::Renderer,
    axis: Axis,
    dec: bool,
    center: Point,
    color: Color,
) {
    for i in 0..4 {
        let w = if dec { 1 + 2 * i } else { 7 - 2 * i } as f32;
        let off = i as f32 - 2.0;
        let bounds = match axis {
            Axis::Vertical => Rectangle {
                x: center.x - w / 2.0,
                y: center.y + off,
                width: w,
                height: crate::theme::border_w(1.0),
            },
            // transposed for left/right: 4 wide, 7 tall
            Axis::Horizontal => Rectangle {
                x: center.x + off,
                y: center.y - w / 2.0,
                width: crate::theme::border_w(1.0),
                height: w,
            },
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Default::default(),
                shadow: Default::default(),
                snap: true,
            },
            color,
        );
    }
}

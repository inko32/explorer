//! Shared building blocks for the file-operation dialogs:
//! push buttons, custom checkbox, icon buttons, D2 command links,
//! the progress bar / speed-graph canvases and the placeholder icons
//! (warning triangle, delete page, file/folder thumbnails).

use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::widget::{button, container, mouse_area, row, text, tooltip, Space};
use iced::{Alignment, Element, Length, Point, Rectangle, Size};

use super::tokens::Tokens;

use iced::widget::Widget as _;
pub const BAR_W: f32 = 395.0;
pub const BAR_H: f32 = 15.0;
pub const GRAPH_W: f32 = 395.0;
pub const GRAPH_H: f32 = 79.0;

// ---------------------------------------------------------------------------
// push button (spec 4.3)
// ---------------------------------------------------------------------------

/// A spec push button: `w`x24, radius 4, accent border when `default`.
pub fn push_btn<'a, M: Clone + 'a>(
    label: &'a str,
    w: f32,
    is_default: bool,
    enabled: bool,
    on_press: M,
    t: Tokens,
) -> Element<'a, M> {
    let btn = button(text(label).size(12).align_x(iced::alignment::Horizontal::Center))
        .width(Length::Fixed(w))
        .height(Length::Fixed(24.0))
        .style(move |_theme, status| {
            let disabled = matches!(status, button::Status::Disabled) || !enabled;
            let (fill, text_color, border) = match status {
                button::Status::Hovered if !disabled => {
                    (t.button_hover_fill, t.text_primary, t.button_border)
                }
                button::Status::Pressed if !disabled => {
                    (t.button_pressed_fill, t.button_pressed_text, t.button_border)
                }
                _ if disabled => (t.button_fill, t.text_disabled, t.button_border),
                _ => (t.button_fill, t.text_primary, t.button_border),
            };
            let border_color = if is_default && !disabled {
                t.accent_border
            } else {
                border
            };
            button::Style {
                background: Some(fill.into()),
                text_color,
                border: iced::Border {
                    color: border_color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            }
        });
    if enabled {
        btn.on_press(on_press).boxed()
    } else {
        btn.boxed()
    }
}

// ---------------------------------------------------------------------------
// custom checkbox (spec 4.4): 13x13 box + label, whole row is the hit area
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Mixed,
}

pub fn check_box<'a, M: Clone + 'a>(
    state: CheckState,
    label: Option<String>,
    enabled: bool,
    on_toggle: M,
    t: Tokens,
) -> Element<'a, M> {
    let box_el: Element<'a, M> = {
        let (fill, border_color, glyph) = match state {
            CheckState::Checked => (t.accent_fill, t.accent_fill, Some("✓")),
            CheckState::Mixed => (t.accent_fill, t.accent_fill, Some("–")),
            CheckState::Unchecked => (t.checkbox_fill, t.checkbox_border, None),
        };
        let mark = glyph.map(|g| {
            text(g)
                .size(11)
                .color(t.text_on_accent)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
        });
        let inner: Element<'a, M> = match mark {
            Some(m) => container(m).width(13.0).height(13.0).center_x(Length::Fixed(13.0)).center_y(Length::Fixed(13.0)).boxed(),
            None => Space::new().width(13.0).height(13.0).boxed(),
        };
        container(inner)
            .width(Length::Fixed(13.0))
            .height(Length::Fixed(13.0))
            .style(move |_| container::Style {
                background: Some(fill.into()),
                border: iced::Border {
                    color: border_color,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..Default::default()
            })
            .boxed()
    };

    let content: Element<'a, M> = match label {
        Some(l) => row![
            box_el,
            text(l).size(12).color(if enabled {
                t.text_primary
            } else {
                t.text_secondary
            })
        ]
        .align_y(Alignment::Center)
        .spacing(6)
        .boxed(),
        None => box_el,
    };

    if enabled {
        mouse_area(container(content)).on_press(on_toggle).boxed()
    } else {
        container(content).boxed()
    }
}

// ---------------------------------------------------------------------------
// icon button (Pause/Resume/Cancel, spec D1.3): 32x32 hit, subtle hover fill
// ---------------------------------------------------------------------------

pub fn icon_btn<'a, M: Clone + 'a>(
    glyph: &'a str,
    enabled: bool,
    tip: &'a str,
    on_press: M,
    t: Tokens,
) -> Element<'a, M> {
    let color = if enabled { t.glyph_primary } else { t.glyph_disabled };
    let label = text(glyph)
        .size(13)
        .color(color)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);
    let cell: Element<'a, M> = container(label)
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .center_x(Length::Fixed(32.0))
        .center_y(Length::Fixed(32.0))
        .style(move |_| container::Style::default())
        .boxed();
    let cell = if enabled {
        mouse_area(cell).on_press(on_press).boxed()
    } else {
        cell
    };
    tooltip(cell, tip).position(tooltip::Position::Top).boxed()
}

// ---------------------------------------------------------------------------
// command link row (spec 4.5)
// ---------------------------------------------------------------------------

pub struct CmdLink<'a, M> {
    pub icon: CmdIcon,
    pub text: std::borrow::Cow<'a, str>,
    pub state: CmdState,
    pub on_press: M,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdState {
    Rest,
    Hovered,
    Selected,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdIcon {
    Check,
    Undo,
    Compare,
    Error,
}

pub fn cmd_link<'a, M: Clone + 'a>(link: CmdLink<'a, M>, t: Tokens) -> Element<'a, M> {
    let CmdLink {
        icon,
        text: label,
        state,
        on_press,
    } = link;
    let glyph = match icon {
        CmdIcon::Check => "✓",
        CmdIcon::Undo => "↶",
        CmdIcon::Compare => "⇄",
        CmdIcon::Error => "✕",
    };
    let icon_color = match icon {
        CmdIcon::Check => t.command_icon_check,
        CmdIcon::Undo => t.command_icon_undo,
        CmdIcon::Compare => t.command_icon_check,
        CmdIcon::Error => t.error_fill,
    };
    let content = row![
        text(glyph).size(16).color(icon_color).width(Length::Fixed(16.0)),
        text(label.into_owned()).size(16).color(match state {
            CmdState::Disabled => t.text_disabled,
            _ => t.accent_command_text,
        })
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let style = move |_: &iced::Theme| container::Style {
        background: match state {
            CmdState::Selected => Some(t.accent_tint_selected.into()),
            CmdState::Hovered => Some(t.accent_tint_hover.into()),
            _ => None,
        },
        border: iced::Border {
            color: match state {
                CmdState::Selected => t.focus_stroke,
                CmdState::Hovered | CmdState::Rest => t.accent_tint_selected,
                _ => t.accent_tint_hover,
            },
            width: match state {
                CmdState::Selected | CmdState::Hovered => 1.0,
                _ => 0.0,
            },
            radius: 0.0.into(),
        },
        ..Default::default()
    };

    let row_el: Element<'a, M> = container(content)
        .width(Length::Fixed(BAR_W))
        .height(Length::Fixed(41.0))
        .padding(iced::Padding {
            left: 10.0,
            right: 10.0,
            top: 0.0,
            bottom: 0.0,
        })
        .style(style)
        .boxed();

    match state {
        CmdState::Disabled => row_el,
        _ => mouse_area(row_el).on_press(on_press).boxed(),
    }
}

// ---------------------------------------------------------------------------
// progress bar canvas (spec D1.3 compact bar)
// ---------------------------------------------------------------------------

pub struct Bar {
    /// `None` = indeterminate (discovery marquee)
    pub frac: Option<f32>,
    /// marquee position 0..1
    pub sweep: f32,
    pub paused: bool,
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for Bar {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let t = &self.t;
        let fill_color = if self.paused { t.paused_fill } else { t.progress_fill };
        // outer border + track (1 px each, inset)
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), t.progress_border);
        frame.fill_rectangle(
            Point::new(1.0, 1.0),
            Size::new(bounds.width - 2.0, bounds.height - 2.0),
            t.progress_track,
        );
        match self.frac {
            Some(f) => {
                let full = bounds.width - 2.0;
                let w = full * f.clamp(0.0, 1.0);
                if w > 0.0 {
                    frame.fill_rectangle(Point::new(1.0, 1.0), Size::new(w, bounds.height - 2.0), fill_color);
                }
            }
            None => {
                // 101 px block sweeping (spec: 2,000 ms + 300 ms pause)
                let span = (bounds.width - 2.0 - 101.0).max(0.0);
                let x = 1.0 + span * self.sweep;
                frame.fill_rectangle(Point::new(x, 1.0), Size::new(101.0, bounds.height - 2.0), fill_color);
            }
        }
        vec![frame.into_geometry()]
    }
}

pub fn bar<M>(b: Bar) -> Canvas<Bar, M, iced::Theme, iced::Renderer> {
    Canvas::new(b)
        .width(Length::Fixed(BAR_W))
        .height(Length::Fixed(BAR_H))
}

// ---------------------------------------------------------------------------
// speed graph canvas (spec D1.3)
// ---------------------------------------------------------------------------

pub struct Graph {
    pub frac: f32,
    /// one sample per column (speed), len == GRAPH_COLS
    pub samples: Vec<f32>,
    pub scale: f32,
    pub speed: f32,
    pub speed_label: String,
    pub paused: bool,
    pub preparing: bool,
    /// shimmer band position 0..1 (discovery)
    pub shimmer: f32,
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for Graph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let t = &self.t;
        // outer border (1 px), track frame (1 px), plot = the rest
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), t.progress_border);
        frame.fill_rectangle(
            Point::new(1.0, 1.0),
            Size::new(bounds.width - 2.0, bounds.height - 2.0),
            t.progress_track,
        );
        let plot = Rectangle {
            x: 2.0,
            y: 2.0,
            width: bounds.width - 4.0,
            height: bounds.height - 4.0,
        };
        frame.fill_rectangle(Point::new(plot.x, plot.y), plot.size(), t.graph_bg);

        let (done_color, grid_color, hist_color) = if self.paused {
            (t.paused_done, t.paused_grid, t.paused_fill)
        } else {
            (t.graph_done, t.graph_grid, t.graph_speed)
        };

        // completed area
        let done_w = plot.width * self.frac.clamp(0.0, 1.0);
        if done_w > 0.0 {
            frame.fill_rectangle(Point::new(plot.x, plot.y), Size::new(done_w, plot.height), done_color);
        }

        // grid: 9 vertical, 4 horizontal (spec D1.3)
        let mut grid = |color: iced::Color| {
            for i in 1..=9 {
                let x = plot.x + plot.width * i as f32 / 10.0;
                frame.stroke(
                    &Path::line(Point::new(x, plot.y), Point::new(x, plot.y + plot.height)),
                    Stroke::default().with_color(color).with_width(1.0),
                );
            }
            for i in 1..=4 {
                let y = plot.y + plot.height * i as f32 / 5.0;
                frame.stroke(
                    &Path::line(Point::new(plot.x, y), Point::new(plot.x + plot.width, y)),
                    Stroke::default().with_color(color).with_width(1.0),
                );
            }
        };
        grid(grid_color);
        // grid over the done area uses the darker done-grid color: redraw
        // clipped to the done strip
        if done_w > 0.0 {
            let done_grid_color = if self.paused { t.paused_grid } else { t.graph_done_grid };
            frame.with_clip(
                Rectangle {
                    width: done_w,
                    ..plot
                },
                |f| {
                    for i in 1..=9 {
                        let x = plot.x + plot.width * i as f32 / 10.0;
                        if x <= plot.x + done_w {
                            f.stroke(
                                &Path::line(Point::new(x, plot.y), Point::new(x, plot.y + plot.height)),
                                Stroke::default().with_color(done_grid_color).with_width(1.0),
                            );
                        }
                    }
                    for i in 1..=4 {
                        let y = plot.y + plot.height * i as f32 / 5.0;
                        f.stroke(
                            &Path::line(Point::new(plot.x, y), Point::new(plot.x + done_w.min(plot.width), y)),
                            Stroke::default().with_color(done_grid_color).with_width(1.0),
                        );
                    }
                },
            );
        }

        if self.preparing {
            // shimmer band: 125 px wide, 3-step gradient approximation
            let band_w = 125.0f32.min(plot.width);
            let x0 = plot.x + (plot.width + band_w) * self.shimmer - band_w;
            let steps = 10;
            for i in 0..steps {
                let f = i as f32 / (steps - 1) as f32;
                // triangle: dark at edges, shimmer color in the middle
                let m = 1.0 - (f * 2.0 - 1.0).abs();
                let c = iced::Color {
                    r: t.graph_bg.r + (t.graph_shimmer.r - t.graph_bg.r) * m,
                    g: t.graph_bg.g + (t.graph_shimmer.g - t.graph_bg.g) * m,
                    b: t.graph_bg.b + (t.graph_shimmer.b - t.graph_bg.b) * m,
                    a: 1.0,
                };
                frame.fill_rectangle(
                    Point::new(x0 + band_w * i as f32 / steps as f32, plot.y),
                    Size::new(band_w / steps as f32 + 1.0, plot.height),
                    c,
                );
            }
        } else {
            // speed history: one filled column per sample
            let cols = self.samples.len().max(1) as f32;
            let col_w = plot.width / cols;
            let scale = self.scale.max(1.0);
            for (i, s) in self.samples.iter().enumerate() {
                if *s <= 0.0 {
                    continue;
                }
                let h = plot.height * (s / scale).clamp(0.0, 1.0);
                frame.fill_rectangle(
                    Point::new(plot.x + i as f32 * col_w, plot.y + plot.height - h),
                    Size::new(col_w + 0.5, h),
                    hist_color,
                );
            }
            if !self.paused {
                // current-speed line + label
                let h = plot.height * (self.speed / scale).clamp(0.0, 1.0);
                let y = plot.y + plot.height - h;
                frame.stroke(
                    &Path::line(Point::new(plot.x, y), Point::new(plot.x + plot.width, y)),
                    Stroke::default().with_color(t.graph_speed_line).with_width(1.0),
                );
                let label = canvas::Text {
                    content: self.speed_label.clone(),
                    position: Point::new(plot.x + plot.width - 6.0, (y - 16.0).max(plot.y)),
                    color: t.graph_speed_line,
                    size: 12.0.into(),
                    align_x: iced::alignment::Horizontal::Right.into(),
                    ..Default::default()
                };
                frame.fill_text(label);
            }
        }
        vec![frame.into_geometry()]
    }
}

pub fn graph<M>(g: Graph) -> Canvas<Graph, M, iced::Theme, iced::Renderer> {
    Canvas::new(g)
        .width(Length::Fixed(GRAPH_W))
        .height(Length::Fixed(GRAPH_H))
}

// ---------------------------------------------------------------------------
// placeholder icons (spec: shell icons at the stated sizes; real icon
// extraction is out of scope at this stage, so these are drawn)
// ---------------------------------------------------------------------------

/// File "page" glyph with folded corner + extension text.
pub struct PageIcon {
    pub ext: String,
    pub size: f32,
    pub folder: bool,
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for PageIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let t = &self.t;
        let s = bounds.width;
        let fold = s * 0.28;
        if self.folder {
            // folder: tab + body
            let body = Path::new(|p| {
                p.move_to(Point::new(s * 0.06, s * 0.30));
                p.line_to(Point::new(s * 0.40, s * 0.30));
                p.line_to(Point::new(s * 0.48, s * 0.20));
                p.line_to(Point::new(s * 0.94, s * 0.20));
                p.line_to(Point::new(s * 0.94, s * 0.88));
                p.line_to(Point::new(s * 0.06, s * 0.88));
                p.close();
            });
            frame.fill(&body, t.graph_shimmer);
            frame.stroke(
                &body,
                Stroke::default().with_color(t.progress_border).with_width(1.0),
            );
        } else {
            // page with folded corner
            let page = Path::new(|p| {
                p.move_to(Point::new(s * 0.18, s * 0.06));
                p.line_to(Point::new(s * 0.62, s * 0.06));
                p.line_to(Point::new(s * 0.82, s * 0.26));
                p.line_to(Point::new(s * 0.82, s * 0.94));
                p.line_to(Point::new(s * 0.18, s * 0.94));
                p.close();
            });
            let page_fill = if t.mode == super::tokens::Mode::Dark {
                iced::Color::from_rgb(0.85, 0.85, 0.85)
            } else {
                iced::Color::WHITE
            };
            frame.fill(&page, page_fill);
            frame.stroke(
                &page,
                Stroke::default().with_color(t.progress_border).with_width(1.0),
            );
            let fold_line = Path::line(
                Point::new(s * 0.62, s * 0.06),
                Point::new(s * 0.62, s * 0.26),
            );
            frame.stroke(
                &Path::new(|p| {
                    p.move_to(Point::new(s * 0.62, s * 0.26));
                    p.line_to(Point::new(s * 0.82, s * 0.26));
                }),
                Stroke::default().with_color(t.progress_border).with_width(1.0),
            );
            let _ = fold;
            let _ = fold_line;
            if !self.ext.is_empty() {
                let label = canvas::Text {
                    content: self.ext.chars().take(4).collect(),
                    position: Point::new(s * 0.5, s * 0.62),
                    color: t.text_secondary,
                    size: (s * 0.22).into(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    ..Default::default()
                };
                frame.fill_text(label);
            }
        }
        vec![frame.into_geometry()]
    }
}

pub fn page_icon<M>(icon: PageIcon) -> Canvas<PageIcon, M, iced::Theme, iced::Renderer> {
    let s = icon.size;
    Canvas::new(icon).width(Length::Fixed(s)).height(Length::Fixed(s))
}

/// Warning triangle (#FFC200 + black "!"), 32x32 (spec D5).
pub struct WarnIcon {
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for WarnIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let s = bounds.width;
        let tri = Path::new(|p| {
            p.move_to(Point::new(s * 0.5, s * 0.04));
            p.line_to(Point::new(s * 0.96, s * 0.92));
            p.line_to(Point::new(s * 0.04, s * 0.92));
            p.close();
        });
        frame.fill(&tri, self.t.warning_icon);
        frame.fill_rectangle(
            Point::new(s * 0.47, s * 0.30),
            Size::new(s * 0.06, s * 0.36),
            iced::Color::BLACK,
        );
        frame.fill_rectangle(
            Point::new(s * 0.47, s * 0.72),
            Size::new(s * 0.06, s * 0.08),
            iced::Color::BLACK,
        );
        vec![frame.into_geometry()]
    }
}

pub fn warn_icon<M>(t: Tokens) -> Canvas<WarnIcon, M, iced::Theme, iced::Renderer> {
    Canvas::new(WarnIcon { t }).width(Length::Fixed(32.0)).height(Length::Fixed(32.0))
}

/// Delete confirmation icon: page + red X (spec D4).
pub struct DeleteIcon {
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for DeleteIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let t = &self.t;
        let s = bounds.width;
        let page = Path::new(|p| {
            p.move_to(Point::new(s * 0.18, s * 0.06));
            p.line_to(Point::new(s * 0.62, s * 0.06));
            p.line_to(Point::new(s * 0.82, s * 0.26));
            p.line_to(Point::new(s * 0.82, s * 0.94));
            p.line_to(Point::new(s * 0.18, s * 0.94));
            p.close();
        });
        let page_fill = if t.mode == super::tokens::Mode::Dark {
            iced::Color::from_rgb(0.85, 0.85, 0.85)
        } else {
            iced::Color::WHITE
        };
        frame.fill(&page, page_fill);
        frame.stroke(
            &page,
            Stroke::default().with_color(t.progress_border).with_width(1.0),
        );
        // red X across the page
        let x = Path::new(|p| {
            p.move_to(Point::new(s * 0.34, s * 0.38));
            p.line_to(Point::new(s * 0.66, s * 0.72));
            p.move_to(Point::new(s * 0.66, s * 0.38));
            p.line_to(Point::new(s * 0.34, s * 0.72));
        });
        frame.stroke(
            &x,
            Stroke::default()
                .with_color(t.delete_icon_x)
                .with_width(2.5),
        );
        vec![frame.into_geometry()]
    }
}

pub fn delete_icon<M>(t: Tokens) -> Canvas<DeleteIcon, M, iced::Theme, iced::Renderer> {
    Canvas::new(DeleteIcon { t }).width(Length::Fixed(32.0)).height(Length::Fixed(32.0))
}

/// Error circle with a white "!" (operation-error message box).
pub struct ErrIcon {
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for ErrIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let s = bounds.width;
        let circle = Path::circle(Point::new(s * 0.5, s * 0.5), s * 0.44);
        frame.fill(&circle, self.t.error_fill);
        frame.fill_rectangle(
            Point::new(s * 0.47, s * 0.24),
            Size::new(s * 0.06, s * 0.34),
            iced::Color::WHITE,
        );
        frame.fill_rectangle(
            Point::new(s * 0.47, s * 0.66),
            Size::new(s * 0.06, s * 0.08),
            iced::Color::WHITE,
        );
        vec![frame.into_geometry()]
    }
}

pub fn err_icon<M>(t: Tokens) -> Canvas<ErrIcon, M, iced::Theme, iced::Renderer> {
    Canvas::new(ErrIcon { t }).width(Length::Fixed(32.0)).height(Length::Fixed(32.0))
}

/// Question circle with a white "?".
pub struct QuestionIcon {
    pub t: Tokens,
}

impl<M> canvas::Program<M, iced::Theme, iced::Renderer> for QuestionIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<iced::Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let s = bounds.width;
        let circle = Path::circle(Point::new(s * 0.5, s * 0.5), s * 0.44);
        frame.fill(&circle, self.t.accent_fill);
        let label = canvas::Text {
            content: "?".to_string(),
            position: Point::new(s * 0.5, s * 0.52),
            color: self.t.text_on_accent,
            size: 22.0.into(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..Default::default()
        };
        frame.fill_text(label);
        vec![frame.into_geometry()]
    }
}

pub fn question_icon<M>(t: Tokens) -> Canvas<QuestionIcon, M, iced::Theme, iced::Renderer> {
    Canvas::new(QuestionIcon { t }).width(Length::Fixed(32.0)).height(Length::Fixed(32.0))
}

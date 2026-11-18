//! Text metrics demo.
//!
//! Given a text, a font, and a size, shows how to:
//!   1. measure the exact width/height the text needs (no wrapping),
//!   2. greedily wrap the text at a fixed right border,
//!   3. truncate it with "..." so the result provably fits a max width,
//!   4. wrap it into at most N lines, ellipsizing the last line when
//!      there is more content than fits,
//!   5. wrap unconditionally at the right border, even inside a word.
//!
//! All numbers come from the same cosmic-text engine (and font database)
//! that iced renders with, so measured widths match drawn pixels.

use iced::advanced::graphics::text::{self as gfx_text, cosmic_text};
use iced::widget::text::LineHeight;
use iced::widget::{column, container, row, scrollable, slider, text, text_input};
use iced::{Border, Color, Element, Font, Length, Theme, Widget as _};

// Must match the LineHeight given to the rendered text widgets below,
// otherwise measured and drawn heights diverge.
const LINE_HEIGHT: LineHeight = LineHeight::Relative(1.25);

fn main() -> iced::Result {
    iced::application(Demo::new, Demo::update, view)
        .title("text metrics demo")
        .run()
}

#[derive(Clone, Debug)]
enum Message {
    Content(String),
    Family(String),
    Size(f32),
    Width(f32),
    Lines(f32),
}

/// Everything derived from the inputs, recomputed only on input change
/// (never inside view()).
struct Analysis {
    /// Exact (width, height) the full text needs with no wrapping.
    natural: (f32, f32),
    /// Greedy-wrapped lines and each line's measured width.
    lines: Vec<(String, f32)>,
    /// Single-line truncation result and its measured width.
    ellipsized: String,
    ellipsis_width: f32,
    truncated: bool,
    /// At most `max_lines` wrapped lines; the last one is ellipsized
    /// when the full text needs more lines.
    clamped: Vec<(String, f32)>,
    clamped_truncated: bool,
    /// Unconditional wrap: every line is filled to the border, breaking
    /// mid-word where necessary.
    hard_lines: Vec<(String, f32)>,
}

struct Demo {
    content: String,
    family: String,
    font: Font,
    size: f32,
    max_width: f32,
    max_lines: f32,
    analysis: Analysis,
}

impl Demo {
    fn new() -> (Self, iced::Task<Message>) {
        let demo = Self {
            content: "The quick brown fox jumps over the lazy dog — packaging \
                      iced applications with precisely measured text is fun."
                .into(),
            family: "Noto Sans".into(),
            font: font_of("Noto Sans"),
            size: 22.0,
            max_width: 320.0,
            max_lines: 4.0,
            analysis: Analysis {
                natural: (0.0, 0.0),
                lines: Vec::new(),
                ellipsized: String::new(),
                ellipsis_width: 0.0,
                truncated: false,
                clamped: Vec::new(),
                clamped_truncated: false,
                hard_lines: Vec::new(),
            },
        };
        let mut demo = demo;
        demo.recompute();
        (demo, iced::Task::none())
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Content(s) => self.content = s,
            Message::Family(s) => {
                self.font = font_of(&s);
                self.family = s;
            }
            Message::Size(v) => self.size = v,
            Message::Width(v) => self.max_width = v,
            Message::Lines(v) => self.max_lines = v,
        }
        self.recompute();
        iced::Task::none()
    }

    fn recompute(&mut self) {
        self.analysis = analyze(
            &self.content,
            self.font,
            self.size,
            self.max_width,
            self.max_lines as usize,
        );
    }
}

fn font_of(family: &str) -> Font {
    let family = family.trim();
    if family.is_empty() {
        Font::default()
    } else {
        // Family::name interns the string globally; no per-edit leak.
        Font::with_family(iced::font::Family::name(family))
    }
}

/// Measures (width, height) of a single unwrapped line with the same
/// font system and metrics iced uses for rendering.
fn measure(fragment: &str, size: f32, font: Font) -> (f32, f32) {
    let system = gfx_text::font_system();
    // Poisoning only happens if a panicking thread held the lock; the
    // guard contents are still usable.
    let mut system = system.write().unwrap_or_else(|e| e.into_inner());

    let mut buffer = cosmic_text::Buffer::new(
        system.raw(),
        cosmic_text::Metrics::new(size, LINE_HEIGHT.to_absolute(size).0),
    );
    buffer.set_wrap(cosmic_text::Wrap::None);
    buffer.set_text(
        fragment,
        &gfx_text::to_attributes(font),
        cosmic_text::Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(system.raw(), false);

    let (size, _) = gfx_text::measure(&buffer);
    (size.width, size.height)
}

/// Greedy word wrap: a word goes on the current line if it still fits;
/// otherwise the line is flushed. Words wider than the box are
/// hard-broken by character.
fn wrap_line(
    line: &str,
    size: f32,
    font: Font,
    max_width: f32,
    out: &mut Vec<(String, f32)>,
) {
    let mut current = String::new();

    for word in line.split(' ') {
        if word.is_empty() {
            continue;
        }

        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };

        if measure(&candidate, size, font).0 <= max_width {
            current = candidate;
            continue;
        }

        if !current.is_empty() {
            let w = measure(&current, size, font).0;
            out.push((std::mem::take(&mut current), w));
        }

        if measure(word, size, font).0 > max_width {
            // Word alone exceeds the box: break it by character.
            let mut piece = String::new();
            for ch in word.chars() {
                let mut candidate = piece.clone();
                candidate.push(ch);
                if piece.is_empty() || measure(&candidate, size, font).0 <= max_width {
                    piece = candidate;
                } else {
                    let w = measure(&piece, size, font).0;
                    out.push((std::mem::take(&mut piece), w));
                    piece.push(ch);
                }
            }
            current = piece;
        } else {
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        let w = measure(&current, size, font).0;
        out.push((current, w));
    }
}

/// Unconditional wrap: appends characters until the next one would
/// cross `max_width`, then breaks — even in the middle of a word.
fn wrap_hard(
    line: &str,
    size: f32,
    font: Font,
    max_width: f32,
    out: &mut Vec<(String, f32)>,
) {
    let mut piece = String::new();
    for ch in line.chars() {
        let mut candidate = piece.clone();
        candidate.push(ch);
        if piece.is_empty() || measure(&candidate, size, font).0 <= max_width {
            piece = candidate;
        } else {
            let w = measure(&piece, size, font).0;
            out.push((std::mem::take(&mut piece), w));
            piece.push(ch);
        }
    }
    if !piece.is_empty() {
        let w = measure(&piece, size, font).0;
        out.push((piece, w));
    }
}

/// Finds the longest prefix + "..." whose width fits `max_width`.
/// Returns the result, its measured width, and whether truncation happened.
fn ellipsize(single_line: &str, size: f32, font: Font, max_width: f32) -> (String, f32, bool) {
    let natural = measure(single_line, size, font).0;
    if natural <= max_width {
        return (single_line.to_string(), natural, false);
    }

    const MARK: &str = "...";
    let budget = max_width - measure(MARK, size, font).0;
    let chars: Vec<char> = single_line.chars().collect();

    // Binary search the longest fitting prefix (prefix width is nearly
    // monotonic, but kerning can make it dip, so verify by walking back).
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let prefix: String = chars[..mid].iter().collect();
        if measure(&prefix, size, font).0 <= budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    while lo > 0 {
        let prefix: String = chars[..lo].iter().collect();
        if measure(&prefix, size, font).0 <= budget {
            break;
        }
        lo -= 1;
    }

    let mut result: String = chars[..lo].iter().collect();
    result.push_str(MARK);
    let width = measure(&result, size, font).0;
    (result, width, true)
}

fn analyze(
    content: &str,
    font: Font,
    size: f32,
    max_width: f32,
    max_lines: usize,
) -> Analysis {
    // 1. exact requirement
    let natural = measure(content, size, font);

    // 2. wrap at the right border
    let mut lines = Vec::new();
    for paragraph in content.split('\n') {
        wrap_line(paragraph, size, font, max_width, &mut lines);
    }

    // 3. ellipsis: single line, newlines flattened
    let single = content.replace('\n', " ");
    let (ellipsized, ellipsis_width, truncated) =
        ellipsize(&single, size, font, max_width);

    // 4. clamp to at most `max_lines` lines: keep the first max_lines-1
    // wrapped lines as-is, join the remainder and ellipsize it onto the
    // last line, so the final line both fills the box and signals that
    // content was cut. Requires max_lines >= 1.
    let (clamped, clamped_truncated) = if lines.len() > max_lines {
        let mut kept = lines[..max_lines - 1].to_vec();
        let tail = lines[max_lines - 1..]
            .iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let (last, w, _) = ellipsize(&tail, size, font, max_width);
        kept.push((last, w));
        (kept, true)
    } else {
        (lines.clone(), false)
    };

    // 5. unconditional wrap: fill each line character by character until
    //    the next glyph would cross the border, then break — even inside
    //    a word.
    let mut hard_lines = Vec::new();
    for paragraph in content.split('\n') {
        wrap_hard(paragraph, size, font, max_width, &mut hard_lines);
    }

    Analysis {
        natural,
        lines,
        ellipsized,
        ellipsis_width,
        truncated,
        clamped,
        clamped_truncated,
        hard_lines,
    }
}

fn boxed<'a, W>(content: W, width: f32, color: Color) -> Element<'a, Message>
where
    W: iced::advanced::widget::Widget<Message, Theme, iced::Renderer> + 'a,
{
    container(content)
        .padding(4)
        .width(Length::Fixed(width + 8.0))
        .style(move |_theme: &Theme| container::Style {
            border: Border {
                color,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .boxed()
}

fn caption(label: String) -> Element<'static, Message> {
    text(label)
        .size(13)
        .color(Color::from_rgb(0.6, 0.6, 0.6))
        .boxed()
}

fn view(demo: &Demo) -> Element<'_, Message> {
    let a = &demo.analysis;
    let size = demo.size;
    let font = demo.font;

    let controls = column![
        row![
            text("text:").width(Length::Fixed(90.0)),
            text_input("type something", &demo.content)
                .on_input(Message::Content),
        ],
        row![
            text("font family:").width(Length::Fixed(90.0)),
            text_input("e.g. Noto Sans / DejaVu Sans / monospace", &demo.family)
                .on_input(Message::Family),
        ],
        row![
            text("size:").width(Length::Fixed(90.0)),
            slider(8.0..=72.0, demo.size, Message::Size).width(Length::Fixed(300.0)),
            text(format!("{size:.1} px")).width(Length::Fixed(70.0)),
        ],
        row![
            text("box width:").width(Length::Fixed(90.0)),
            slider(40.0..=900.0, demo.max_width, Message::Width)
                .width(Length::Fixed(300.0)),
            text(format!("{:.1} px", demo.max_width)).width(Length::Fixed(70.0)),
        ],
        row![
            text("max lines:").width(Length::Fixed(90.0)),
            slider(1.0..=8.0, demo.max_lines, Message::Lines)
                .width(Length::Fixed(300.0)),
            text(format!("{:.0}", demo.max_lines)).width(Length::Fixed(70.0)),
        ],
    ]
    .spacing(8);

    // --- 1. exact width requirement -------------------------------------
    let fit = if a.natural.0 <= demo.max_width {
        format!("fits in the {:.2} px box", demo.max_width)
    } else {
        format!(
            "overflows the {:.2} px box by {:.2} px",
            demo.max_width,
            a.natural.0 - demo.max_width
        )
    };

    let exact_panel = column![
        text("1. exact size requirement (no wrapping)").size(16),
        boxed(
            text(&demo.content)
                .size(size)
                .font(font)
                .line_height(LINE_HEIGHT)
                .wrapping(iced::advanced::text::Wrapping::None),
            a.natural.0,
            Color::from_rgb(0.3, 0.8, 0.4),
        ),
        caption(format!(
            "needs exactly {:.2} x {:.2} px (w x h); {fit}",
            a.natural.0, a.natural.1
        )),
    ]
    .spacing(6);

    // --- 2. wrap at the right border ------------------------------------
    let line_widths = a
        .lines
        .iter()
        .map(|(_, w)| format!("{w:.2}"))
        .collect::<Vec<_>>()
        .join(", ");

    let wrap_panel = column![
        text("2. wrap at the right border").size(16),
        boxed(
            column(
                a.lines
                    .iter()
                    .map(|(l, _)| {
                        text(l.clone())
                            .size(size)
                            .font(font)
                            .line_height(LINE_HEIGHT)
                            .boxed()
                    })
                    .collect::<Vec<Element<Message>>>(),
            ),
            demo.max_width,
            Color::from_rgb(0.4, 0.6, 0.95),
        ),
        caption(format!(
            "{} lines, each <= {:.2} px — widths: [{}]",
            a.lines.len(),
            demo.max_width,
            line_widths
        )),
    ]
    .spacing(6);

    // --- 3. ellipsis ------------------------------------------------------
    let ellipsis_note = if a.truncated { "truncated" } else { "unchanged" };
    let ellipsis_panel = column![
        text("3. ellipsis (single line, must fit the box)").size(16),
        boxed(
            text(&a.ellipsized)
                .size(size)
                .font(font)
                .line_height(LINE_HEIGHT),
            demo.max_width,
            Color::from_rgb(0.95, 0.75, 0.3),
        ),
        caption(format!(
            "\"{}\" -> {:.2} px, {ellipsis_note}; fits {:.2} px box: {}",
            a.ellipsized,
            a.ellipsis_width,
            demo.max_width,
            a.ellipsis_width <= demo.max_width + 0.01
        )),
    ]
    .spacing(6);

    // --- 4. clamp to N lines --------------------------------------------
    let clamp_note = if a.clamped_truncated {
        format!(
            "{} of {} lines shown, last line ellipsized",
            a.clamped.len(),
            a.lines.len()
        )
    } else {
        format!("{} lines, all fit", a.clamped.len())
    };

    let clamp_panel = column![
        text(format!("4. clamp to {:.0} lines (last line ellipsized)", demo.max_lines))
            .size(16),
        boxed(
            column(
                a.clamped
                    .iter()
                    .map(|(l, _)| {
                        text(l.clone())
                            .size(size)
                            .font(font)
                            .line_height(LINE_HEIGHT)
                            .boxed()
                    })
                    .collect::<Vec<Element<Message>>>(),
            ),
            demo.max_width,
            Color::from_rgb(0.8, 0.5, 0.9),
        ),
        caption(clamp_note),
    ]
    .spacing(6);

    // --- 5. unconditional wrap ------------------------------------------
    let hard_widths = a
        .hard_lines
        .iter()
        .map(|(_, w)| format!("{w:.2}"))
        .collect::<Vec<_>>()
        .join(", ");

    let hard_panel = column![
        text("5. unconditional wrap (breaks inside words)").size(16),
        boxed(
            column(
                a.hard_lines
                    .iter()
                    .map(|(l, _)| {
                        text(l.clone())
                            .size(size)
                            .font(font)
                            .line_height(LINE_HEIGHT)
                            .boxed()
                    })
                    .collect::<Vec<Element<Message>>>(),
            ),
            demo.max_width,
            Color::from_rgb(0.9, 0.45, 0.4),
        ),
        caption(format!(
            "{} lines, every line filled to <= {:.2} px — widths: [{}]",
            a.hard_lines.len(),
            demo.max_width,
            hard_widths
        )),
    ]
    .spacing(6);

    scrollable(
        column![
            controls,
            exact_panel,
            wrap_panel,
            ellipsis_panel,
            clamp_panel,
            hard_panel
        ]
        .spacing(24)
        .padding(16),
    )
    .boxed()
}

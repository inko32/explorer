//! Window chrome: self-drawn frame (Win8 two-tone / Win10 1px), resize grab
//! ring, title bar (window icon, QAT, contextual blocks, title), and the
//! caption buttons (_ [] X) overlay.

use iced::Widget as _;
use iced::widget::{button, container, mouse_area, row, stack, text, Space};
use iced::{Color, Element, Length};

use crate::model::{Explorer, Message};
use crate::ui_common::{bg, elide, icon, skin};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeDir {
    N,
    S,
    E,
    W,
    Ne,
    Nw,
    Se,
    Sw,
}

impl ResizeDir {
    pub fn task(self, id: iced::window::Id) -> iced::Task<Message> {
        use iced::window::Direction;
        let d = match self {
            ResizeDir::N => Direction::North,
            ResizeDir::S => Direction::South,
            ResizeDir::E => Direction::East,
            ResizeDir::W => Direction::West,
            ResizeDir::Ne => Direction::NorthEast,
            ResizeDir::Nw => Direction::NorthWest,
            ResizeDir::Se => Direction::SouthEast,
            ResizeDir::Sw => Direction::SouthWest,
        };
        iced::window::drag_resize(id, d)
    }

    fn cursor(self) -> iced::mouse::Interaction {
        use iced::mouse::Interaction;
        match self {
            ResizeDir::E | ResizeDir::W => Interaction::ResizingHorizontally,
            ResizeDir::N | ResizeDir::S => Interaction::ResizingVertically,
            ResizeDir::Ne | ResizeDir::Sw => Interaction::ResizingDiagonallyUp,
            ResizeDir::Nw | ResizeDir::Se => Interaction::ResizingDiagonallyDown,
        }
    }
}


/// An invisible resize zone aligned to one window edge/corner. The outer
/// container spans the whole window (transparent to events); only the inner
/// strip/corner reacts.
pub(crate) fn resize_zone(
    dir: ResizeDir,
    w: Length,
    h: Length,
    ax: Option<iced::Alignment>,
    ay: Option<iced::Alignment>,
) -> Element<'static, Message> {
    let zone = mouse_area(
        container(Space::new().width(w).height(h))
            .style(|_: &iced::Theme| container::Style::default()),
    )
    .interaction(dir.cursor())
    .on_press(Message::StartResize(dir));
    let mut c = container(zone).width(Length::Fill).height(Length::Fill);
    if let Some(x) = ax {
        c = c.align_x(x);
    }
    if let Some(y) = ay {
        c = c.align_y(y);
    }
    c.boxed()
}

/// Window frame (exposed border):
///  - Win8: 3px accent border at the window edge; content is inset 3px
///    (internal calculation follows the exposed outer border).
///  - Win10: single 1px dark border; content inset 1px.
///
/// Frame thickness (top, right, bottom, left). Win8: thin top, THICK
/// sides+bottom (the classic sizing border); Win10: 1px all around.
/// The internal content is inset by exactly these amounts - the "virtual
/// window" shrinks, the frame never covers content.
/// The frame's published insets (top, right, bottom, left) — the border
/// module's message to the internal layout: the available-space origin
/// and size for every non-frame widget (model splitter math, view layout,
/// menu anchors). Maximizer-aware: the maximized frame draws fewer
/// borders, so the available space grows.
pub fn frame_insets_pub(
    self_border: bool,
    style: crate::model::FrameStyle,
    maximized: bool,
) -> (f32, f32, f32, f32) {
    if !self_border {
        return (0.0, 0.0, 0.0, 0.0);
    }
    // the frame border keeps a constant physical width (see
    // SPEC/Border_DPI.md): scale the insets like any 1px border
    let w = crate::theme::border_w;
    match style {
        // Win8: the top stays a 1px hairline, but the thick sizing
        // border on the sides and bottom is ordinary UI geometry and
        // follows the DPI
        crate::model::FrameStyle::Win8 => (w(1.0), 6.0, 6.0, 6.0),
        crate::model::FrameStyle::Win10 => (w(1.0), w(1.0), w(1.0), w(1.0)),
        // Win8.1 (SPEC/win8_border.md): per side 1px outer outline +
        // 6px border + 1px inner outline. The 1px outlines keep a
        // constant physical width (never DPI-scaled, like every 1px
        // border in the app); the 6px border follows the DPI. The top
        // holds only the outer outline — the 29px title bar and the
        // top inner outline are content-level rows.
        crate::model::FrameStyle::Win81 if !maximized => {
            (w(1.0), 6.0 + 2.0 * w(1.0), 6.0 + 2.0 * w(1.0), 6.0 + 2.0 * w(1.0))
        }
        // maximized: the outer outline, the 6px side/bottom border and
        // the side inner outlines are omitted — the available space
        // extends to the window edges on all four sides; only the 1px
        // inner outline under the 29px title bar remains (drawn at the
        // content level, not part of the insets)
        crate::model::FrameStyle::Win81 => (0.0, 0.0, 0.0, 0.0),
    }
}

pub(crate) fn frame_insets(state: &Explorer) -> (f32, f32, f32, f32) {
    frame_insets_pub(state.self_border, state.frame_style, state.maximized)
}

pub(crate) fn frame_color(state: &Explorer, skin: skin::Skin) -> Color {
    match state.frame_style {
        crate::model::FrameStyle::Win8 => skin.accent,
        crate::model::FrameStyle::Win10 => skin.accent_dark,
        crate::model::FrameStyle::Win81 => skin.accent,
    }
}

// ---------------------------------------------------------------------------
// caption glyphs
//
// The ONLY hard-coded icons in the app (the D14 exception): the caption
// buttons are window chrome — they follow the frame style and the
// title-bar colors, not the icon pack (a Breeze user still gets the
// Win8/Win10 frame buttons). Every other icon resolves through the
// icon registry.

/// One caption button in both frame variants. The Win8 glyphs are drawn
/// pixel-per-pixel from a real Win8 capture (ratios against the 21px
/// button height, rendered in a 16-unit box); `{title}` is the title-bar
/// text color (dark on the light-blue Win8 bar). The Win10 glyphs are
/// white on the dark title bar.
struct CaptionGlyph {
    /// Win8 frame; `{title}` = title-bar text color.
    win8: &'static str,
    /// Win10 frame (white on the title bar).
    win10: &'static str,
}

/// `_` `[]` `X` — index-parallel to the caption button row in
/// [`caption_overlay`].
static CAPTION_GLYPHS: [CaptionGlyph; 3] = [
    CaptionGlyph {
        // minimize: 7.3 x 2.1 px low bar
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <rect x='3.5' y='11.4' width='9' height='2.6' fill='{title}'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <line x1='3' y1='8' x2='13' y2='8' stroke='#ffffff' stroke-width='1.4'/></svg>",
    },
    CaptionGlyph {
        // maximize: 10.5 x 8.4 px outline, top bar 2.1 px, sides/bottom
        // 1.05 px (a mini window with a thicker title bar)
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <rect x='1.55' y='2.85' width='12.9' height='2.6' fill='{title}'/>\
               <rect x='1.55' y='2.85' width='1.3' height='10.35' fill='{title}'/>\
               <rect x='13.15' y='2.85' width='1.3' height='10.35' fill='{title}'/>\
               <rect x='1.55' y='11.9' width='12.9' height='1.3' fill='{title}'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <rect x='3' y='3' width='10' height='10' fill='none' stroke='#ffffff' stroke-width='1.4'/></svg>",
    },
    CaptionGlyph {
        // close: 8.4 x 7.3 px, stroke 2.1, flat butt ends cut short of
        // the corners (win10: thinner X)
        win8: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
               <line x1='2.85' y1='3.5' x2='13.15' y2='12.55' stroke='#ffffff' stroke-width='2.6'/>\
               <line x1='13.15' y1='3.5' x2='2.85' y2='12.55' stroke='#ffffff' stroke-width='2.6'/></svg>",
        win10: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'>\
                <line x1='3.5' y1='3.5' x2='12.5' y2='12.5' stroke='#ffffff' stroke-width='1.5'/>\
                <line x1='12.5' y1='3.5' x2='3.5' y2='12.5' stroke='#ffffff' stroke-width='1.5'/></svg>",
    },
];

/// Win8.1 frame chrome (colors + border opacity), owned by
/// win81theme.yaml — a user copy at ~/.config/explorer/win81theme.yaml
/// overrides the embedded one (same pattern as theme.yaml). The
/// defaults are the SPEC/win8_border.md "colour for example" set; every
/// key falls back to its default, so a partial override file works.
pub(crate) mod win81 {
    use iced::Color;

    const THEME_EMBEDDED: &str = include_str!("../assets/win81theme.yaml");

    #[derive(serde::Deserialize)]
    struct Win81Theme {
        /// 0.0-1.0; the frame accent fill, the title bar and the
        /// outlines take this alpha so the content behind the window
        /// shows through them (the window itself is created transparent
        /// at startup when the opacity is below 1.0 — winit only
        /// accepts transparency at window creation). The caption
        /// buttons stay opaque — they must stay readable.
        #[serde(default = "default_opacity")]
        border_opacity: f32,
        /// window accent (border fill, title bar)
        #[serde(default = "default_accent")]
        window_accent: crate::theme::Hex,
        /// darkened accent (the two 1px outlines)
        #[serde(default = "default_outline")]
        outline: crate::theme::Hex,
        /// close button background at rest / hovered / pressed
        #[serde(default = "default_close_idle")]
        close_idle: crate::theme::Hex,
        #[serde(default = "default_close_hover")]
        close_hover: crate::theme::Hex,
        #[serde(default = "default_close_hover")]
        close_pressed: crate::theme::Hex,
        /// close glyph (the X, always white per the spec)
        #[serde(default = "default_white")]
        close_glyph: crate::theme::Hex,
        /// minimize/maximize glyph: black at rest, white on hover
        #[serde(default = "default_black")]
        glyph_idle: crate::theme::Hex,
        #[serde(default = "default_white")]
        glyph_hover: crate::theme::Hex,
        /// minimize/maximize hover background (slight-emphasized accent)
        #[serde(default = "default_hover_bg")]
        hover_bg: crate::theme::Hex,
        /// minimize/maximize pressed background (hover_bg at ~70% alpha)
        #[serde(default = "default_hover_pressed")]
        hover_pressed: crate::theme::Hex,
        /// title-bar caption text size (px at 100% dpi)
        #[serde(default = "default_title_size")]
        title_size: f32,
    }

    fn hex(v: u32) -> crate::theme::Hex {
        crate::theme::Hex(Color::from_rgb8(
            (v >> 16) as u8,
            (v >> 8) as u8,
            v as u8,
        ))
    }

    // defaults: SPEC/win8_border.md "colour for example"
    fn default_opacity() -> f32 {
        0.9
    }
    fn default_accent() -> crate::theme::Hex {
        hex(0x5a83bb)
    }
    fn default_outline() -> crate::theme::Hex {
        hex(0x45648f)
    }
    fn default_close_idle() -> crate::theme::Hex {
        hex(0xc75050)
    }
    fn default_close_hover() -> crate::theme::Hex {
        hex(0xe04343)
    }
    fn default_white() -> crate::theme::Hex {
        hex(0xffffff)
    }
    fn default_black() -> crate::theme::Hex {
        hex(0x000000)
    }
    fn default_hover_bg() -> crate::theme::Hex {
        hex(0x3665b3)
    }
    fn default_hover_pressed() -> crate::theme::Hex {
        // the press shade used to be hover_bg at 70% alpha; keep that
        // derivation for partial overrides that omit the key
        let mut c = default_hover_bg().0;
        c.a *= 0.7;
        crate::theme::Hex(c)
    }
    fn default_title_size() -> f32 {
        // fonts.yaml titlebar.title (12) + 2: the Win8.1 title bar is
        // taller chrome, so its title reads better two points up
        14.0
    }

    fn theme() -> &'static Win81Theme {
        static THEME: std::sync::OnceLock<Win81Theme> = std::sync::OnceLock::new();
        THEME.get_or_init(load)
    }

    fn load() -> Win81Theme {
        let user = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|h| h.join(".config/explorer/win81theme.yaml"));
        if let Some(p) = user
            && let Ok(text) = std::fs::read_to_string(&p)
        {
            match serde_yaml::from_str::<Win81Theme>(&text) {
                Ok(t) => return t,
                Err(e) => eprintln!("win81theme: ignoring {}: {e}", p.display()),
            }
        }
        serde_yaml::from_str(THEME_EMBEDDED).expect("embedded assets/win81theme.yaml must parse")
    }

    /// Border opacity, win81theme.yaml `border_opacity` (0.0-1.0).
    pub fn opacity() -> f32 {
        theme().border_opacity.clamp(0.0, 1.0)
    }

    /// Title-bar caption text size (px at 100% dpi), win81theme.yaml
    /// `title_size`.
    pub fn title_size() -> f32 {
        theme().title_size.max(1.0)
    }

    /// The frame chrome colors from win81theme.yaml (opaque; the frame
    /// fill/outline accessors below apply the border opacity).
    pub(crate) struct Colors {
        pub accent: Color,
        pub close_idle: Color,
        pub close_hover: Color,
        pub close_pressed: Color,
        pub close_glyph: Color,
        pub glyph_idle: Color,
        pub glyph_hover: Color,
        pub hover_bg: Color,
        pub hover_pressed: Color,
    }

    pub(crate) fn colors() -> &'static Colors {
        static COLORS: std::sync::OnceLock<Colors> = std::sync::OnceLock::new();
        COLORS.get_or_init(|| {
            let t = theme();
            Colors {
                accent: t.window_accent.0,
                close_idle: t.close_idle.0,
                close_hover: t.close_hover.0,
                close_pressed: t.close_pressed.0,
                close_glyph: t.close_glyph.0,
                glyph_idle: t.glyph_idle.0,
                glyph_hover: t.glyph_hover.0,
                hover_bg: t.hover_bg.0,
                hover_pressed: t.hover_pressed.0,
            }
        })
    }

    /// The frame accent (border fill, title bar) with the configured
    /// border opacity.
    pub fn accent() -> Color {
        Color { a: opacity(), ..colors().accent }
    }

    /// The outline color (the two 1px outlines) with the configured
    /// border opacity.
    pub fn outline() -> Color {
        let c = theme().outline.0;
        Color { a: opacity(), ..c }
    }

    #[cfg(test)]
    mod tests {
        use super::{Win81Theme, THEME_EMBEDDED};

        // The embedded win81theme.yaml must parse to a valid opacity
        // (the value itself is user-tunable data, not an invariant).
        #[test]
        fn embedded_win81theme_parses() {
            let t: Win81Theme = serde_yaml::from_str(THEME_EMBEDDED).unwrap();
            assert!((0.0..=1.0).contains(&t.border_opacity));
            assert!(t.title_size >= 1.0);
        }

        // The embedded defaults must stay the SPEC/win8_border.md
        // "colour for example" set (a user override may change them,
        // but the shipped theme is the spec's reference).
        #[test]
        fn embedded_defaults_match_spec() {
            use iced::Color;
            let t: Win81Theme = serde_yaml::from_str(THEME_EMBEDDED).unwrap();
            assert_eq!(t.window_accent.0, Color::from_rgb8(0x5a, 0x83, 0xbb));
            assert_eq!(t.outline.0, Color::from_rgb8(0x45, 0x64, 0x8f));
            assert_eq!(t.close_idle.0, Color::from_rgb8(0xc7, 0x50, 0x50));
            assert_eq!(t.close_hover.0, Color::from_rgb8(0xe0, 0x43, 0x43));
            assert_eq!(t.close_glyph.0, Color::WHITE);
            assert_eq!(t.glyph_idle.0, Color::BLACK);
            assert_eq!(t.glyph_hover.0, Color::WHITE);
            assert_eq!(t.hover_bg.0, Color::from_rgb8(0x36, 0x65, 0xb3));
        }
    }
}

/// Win8.1 caption glyphs (SPEC/win8_border.md): proper stroke paths in a
/// viewBox spanning the WHOLE button area (25x20 / 45x20), so the glyphs
/// keep the exact centering the spec measures (e.g. 8+8+9 = 25px for the
/// minimize glyph). `{glyph}` = the minimize/maximize fill color (black at
/// rest, white on hover — the close glyph is always white per the spec).
static CAPTION_GLYPHS_81: [&str; 4] = [
    // minimize: solid 8x2 bar (the bottom two rows of the 8x8 glyph)
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 25 20'>\
     <rect x='8.5' y='12' width='8' height='2' fill='{glyph}'/></svg>",
    // maximize (normal): 10x8 outline — 2px top bar, 1px sides/bottom,
    // one even-odd path (outer rect minus the hollow interior)
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 25 20'>\
     <path fill='{glyph}' fill-rule='evenodd' d='M8 6 H18 V14 H8 Z M9 8 H17 V13 H9 Z'/></svg>",
    // close: 8x7 X, 2px strokes with square caps (solid connecting
    // squares at the corners, no gaps); the stroke takes {glyph} (the
    // close glyph color, white by default)
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 45 20'>\
     <line x1='18.5' y1='6.5' x2='26.5' y2='13.5' stroke='{glyph}' stroke-width='2' stroke-linecap='square'/>\
     <line x1='26.5' y1='6.5' x2='18.5' y2='13.5' stroke='{glyph}' stroke-width='2' stroke-linecap='square'/></svg>",
    // maximize (maximized window): 10x10 restore glyph — the back window
    // as one L-shaped path (top bar + right column), the front window as
    // an even-odd outline
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 25 20'>\
     <path fill='{glyph}' d='M9.5 5 H18.5 V15 H17.5 V6 H9.5 Z'/>\
     <path fill='{glyph}' fill-rule='evenodd' d='M7.5 7 H15.5 V15 H7.5 Z M8.5 9 H14.5 V14 H8.5 Z'/></svg>",
];

/// Render a caption glyph for the frame style: `{title}` takes the
/// title-bar text color (Win8); `{glyph}` takes the button's glyph color
/// (Win8.1 min/max: black at rest, white on hover — the close glyph takes
/// win81theme.yaml `close_glyph`). Templates without tokens (Win10) are
/// white as authored.
fn caption_svg(
    template: &'static str,
    skin: skin::Skin,
    glyph: Color,
) -> iced::widget::svg::Handle {
    let mut doc = template.replace("{glyph}", &crate::icons::hex(glyph));
    if doc.contains("{title}") {
        doc = doc.replace("{title}", &crate::icons::hex(skin.title_text));
    }
    iced::widget::svg::Handle::from_memory(doc.into_bytes())
}

/// Caption buttons (_ [] X) as an overlay at the window's top-right corner:
/// they contact the top edge (Win8: over the frame) and the right edge.
/// Win8: min/max width = 60% of close width, short (20px), thin glyphs.
/// Win10: uniform 46px, nearly full title-bar height.
pub(crate) fn caption_overlay(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    // Pixel ratios from the real Win8 capture (/tmp/user-win8cap.png):
    // buttons hang below the top frame (y=2), height 21 = 69% of the 30px
    // title bar; adjacent, no gaps; min/max w = 1.33x h, close w = 2.24x h;
    // glyph baseline at ~70% of the button height.
    let outline = crate::theme::border_w(1.0);
    let (mw, cw, h, glyph, y, right_pad) = match state.frame_style {
        crate::model::FrameStyle::Win8 => (28.0, 47.0, 21.0, 13.0, 1.0, 6.0),
        crate::model::FrameStyle::Win10 => (46.0, 46.0, 29.0, 15.0, 1.0, 1.0),
        // SPEC/win8_border.md (measured at 100% dpi): 20px buttons just
        // below the 1px outer outline; 6px in front of the right inner
        // outline; 25/25/45 widths with 1px (never DPI-scaled) gaps.
        // The Win8.1 SVGs span the whole button area (their viewBox
        // bakes in the exact glyph centering).
        // maximized: the outer outline is omitted (buttons shift up to
        // y=0) and the buttons shift right 3px (half the side width),
        // leaving 3px + the 1px outline in front of them
        crate::model::FrameStyle::Win81 if state.maximized => {
            (25.0, 45.0, 20.0, 20.0, 0.0, 3.0 + outline)
        }
        crate::model::FrameStyle::Win81 => (25.0, 45.0, 20.0, 20.0, outline, 6.0 + outline),
    };
    let glyphs: [&'static str; 3] = match state.frame_style {
        crate::model::FrameStyle::Win8 => [
            CAPTION_GLYPHS[0].win8,
            CAPTION_GLYPHS[1].win8,
            CAPTION_GLYPHS[2].win8,
        ],
        crate::model::FrameStyle::Win10 => [
            CAPTION_GLYPHS[0].win10,
            CAPTION_GLYPHS[1].win10,
            CAPTION_GLYPHS[2].win10,
        ],
        crate::model::FrameStyle::Win81 => [
            CAPTION_GLYPHS_81[0],
            // the maximize button shows the restore glyph on a
            // maximized window (SPEC/win8_border.md)
            if state.maximized {
                CAPTION_GLYPHS_81[3]
            } else {
                CAPTION_GLYPHS_81[1]
            },
            CAPTION_GLYPHS_81[2],
        ],
    };
    // glyph bottoms share one baseline at ~70% of the button height
    let style = state.frame_style;
    let maximized = state.maximized;
    let one = move |glyph_tpl: &'static str,
                    w: f32,
                    msg: Message,
                    hover_bg: Color,
                    pressed_bg: Color,
                    close_hover: Color,
                    close_pressed: Color,
                    solid: bool,
                    glyph_color: Color,
                    hover_bit: Option<u8>|
          -> Element<'static, Message> {
        // at-rest background: Win8 buttons are opaque (titlebar blue /
        // close red) so the frame's top edge never shows through them;
        // Win10 shows the accent bar through; the Win8.1 close button is
        // red at rest per the spec's color example
        let rest: Option<Color> = match (style, solid) {
            (crate::model::FrameStyle::Win8, true) => Some(skin.close_idle),
            (crate::model::FrameStyle::Win8, false) => Some(skin.accent),
            (crate::model::FrameStyle::Win10, _) => None,
            (crate::model::FrameStyle::Win81, true) => Some(win81::colors().close_idle),
            (crate::model::FrameStyle::Win81, false) => None,
        };
        let blend_white = |c: Color, a: f32| Color {
            r: c.r + (1.0 - c.r) * a,
            g: c.g + (1.0 - c.g) * a,
            b: c.b + (1.0 - c.b) * a,
            a: 1.0,
        };
        // Win8.1: the svg covers the whole button; otherwise a centered
        // square glyph box
        let (gw, gh) = if style == crate::model::FrameStyle::Win81 {
            (w, h)
        } else {
            (glyph, glyph)
        };
        // maximized Win8.1 close button: the space between the
        // button and the window edge (the 3px shift + outline) belongs
        // to it — the hit area extends to the corner, the glyph stays.
        // A maximized window can't be edge-resized, so the corner
        // hotspot is the close hover instead (like real Windows).
        let hit_extra = if style == crate::model::FrameStyle::Win81 && solid && maximized {
            right_pad
        } else {
            0.0
        };
        let hit_w = w + hit_extra;
        let mut content = container(
            iced::widget::svg(caption_svg(glyph_tpl, skin, glyph_color))
                .width(Length::Fixed(gw))
                .height(Length::Fixed(gh)),
        )
        .width(Length::Fixed(hit_w))
        .height(Length::Fixed(h))
        .center_y(Length::Fixed(hit_w));
        content = if hit_extra > 0.0 {
            content.align_x(iced::Alignment::Start)
        } else {
            content.center_x(Length::Fixed(hit_w))
        };
        let btn = button(content)
        .width(Length::Fixed(hit_w))
        .height(Length::Fixed(h))
        .padding(0)
        .style(move |_t, status| button::Style {
            background: match (solid, status) {
                (true, button::Status::Hovered) => Some(close_hover.into()),
                (true, button::Status::Pressed) => Some(close_pressed.into()),
                (true, _) => rest.map(Into::into),
                (false, button::Status::Hovered) => Some(match rest {
                    Some(c) => blend_white(c, 0.18).into(),
                    None => hover_bg.into(),
                }),
                (false, button::Status::Pressed) => Some(match rest {
                    Some(c) => blend_white(c, 0.32).into(),
                    None => pressed_bg.into(),
                }),
                (false, _) => rest.map(Into::into),
            },
            text_color: Color::WHITE,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(msg)
        .boxed();
        // Win8.1 min/max: the glyph turns white on hover, which the
        // button style can't do to an svg — track enter/exit explicitly
        match hover_bit {
            Some(bit) => mouse_area(btn)
                .on_enter(Message::CaptionHover(bit, true))
                .on_exit(Message::CaptionHover(bit, false))
                .boxed(),
            None => btn,
        }
    };
    let ghost = skin.glass_ghost;
    let captions = if style == crate::model::FrameStyle::Win81 {
        // 1px gaps between the buttons, never DPI-scaled; the min/max
        // glyphs are black at rest and white on hover (SPEC color
        // example), all colors from win81theme.yaml
        let w81c = win81::colors();
        let min_glyph = if state.cap_hover & 1 != 0 {
            w81c.glyph_hover
        } else {
            w81c.glyph_idle
        };
        let max_glyph = if state.cap_hover & 2 != 0 {
            w81c.glyph_hover
        } else {
            w81c.glyph_idle
        };
        row![
            one(
                glyphs[0],
                mw,
                Message::Minimize,
                w81c.hover_bg,
                w81c.hover_pressed,
                w81c.close_hover,
                w81c.close_pressed,
                false,
                min_glyph,
                Some(0)
            ),
            Space::new()
                .width(Length::Fixed(outline))
                .height(Length::Fixed(h)),
            one(
                glyphs[1],
                mw,
                Message::Maximize,
                w81c.hover_bg,
                w81c.hover_pressed,
                w81c.close_hover,
                w81c.close_pressed,
                false,
                max_glyph,
                Some(1)
            ),
            Space::new()
                .width(Length::Fixed(outline))
                .height(Length::Fixed(h)),
            one(
                glyphs[2],
                cw,
                Message::Close,
                w81c.close_hover,
                w81c.close_hover,
                w81c.close_hover,
                w81c.close_pressed,
                true,
                w81c.close_glyph,
                None
            ),
        ]
        .height(Length::Fixed(h))
    } else {
        // the Win8 glyphs take the title-bar text color; Win10 white
        let mm_glyph = if style == crate::model::FrameStyle::Win8 {
            skin.title_text
        } else {
            Color::WHITE
        };
        row![
            one(
                glyphs[0],
                mw,
                Message::Minimize,
                ghost,
                ghost.scale_alpha(0.7),
                skin.close_hover,
                skin.close_pressed,
                false,
                mm_glyph,
                None
            ),
            one(
                glyphs[1],
                mw,
                Message::Maximize,
                ghost,
                ghost.scale_alpha(0.7),
                skin.close_hover,
                skin.close_pressed,
                false,
                mm_glyph,
                None
            ),
            one(
                glyphs[2],
                cw,
                Message::Close,
                skin.close_hover,
                skin.close_hover,
                skin.close_hover,
                skin.close_pressed,
                true,
                Color::WHITE,
                None
            ),
        ]
        .height(Length::Fixed(h))
    };
    container(
        captions,
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: y,
        right: right_pad,
        ..Default::default()
    })
    .align_x(iced::Alignment::End)
    .align_y(iced::Alignment::Start)
    .boxed()
}

/// Invisible resize ring around the window edges (custom border mode).
/// `top_zones` false omits the top edge + top corners: a maximized
/// window can't be edge-resized, so the zones would only show a dead
/// resize cursor over the caption corner.
pub(crate) fn resize_frame(margin: f32, top_zones: bool) -> Element<'static, Message> {
    use iced::Alignment;
    let m = Length::Fixed(margin);
    let c = Length::Fixed(margin * 2.0);
    let fill = Length::Fill;
    let mut zones: Vec<Element<'static, Message>> = Vec::new();
    if top_zones {
        zones.push(resize_zone(ResizeDir::N, fill, m, None, None));
        zones.push(resize_zone(ResizeDir::Nw, c, c, None, None));
        zones.push(resize_zone(ResizeDir::Ne, c, c, Some(Alignment::End), None));
    }
    zones.push(resize_zone(ResizeDir::S, fill, m, None, Some(Alignment::End)));
    zones.push(resize_zone(ResizeDir::W, m, fill, None, None));
    zones.push(resize_zone(ResizeDir::E, m, fill, Some(Alignment::End), None));
    // corners on top (they overlap the edge strips)
    zones.push(resize_zone(ResizeDir::Sw, c, c, None, Some(Alignment::End)));
    zones.push(resize_zone(ResizeDir::Se, c, c, Some(Alignment::End), Some(Alignment::End)));
    iced::widget::Stack::with_children(zones).boxed()
}

// ---------------- title bar ----------------

/// Title bar height — the ribbon no longer merges its tab row into the
/// title bar (the rs-ribbon widget owns the tab strip), so the bar is a
/// plain drag band: window icon, centered title, caption space.
pub(crate) const TITLEBAR_H: f32 = 30.0;

/// Height of the Win8.1 title-bar region between the outlines
/// (SPEC/win8_border.md): 20px caption buttons + the space below them —
/// 9px normally, 3px maximized (the bar's bottom line rises when the
/// outer outline is omitted).
pub(crate) fn win81_bar_h(maximized: bool) -> f32 {
    if maximized {
        23.0
    } else {
        29.0
    }
}

/// Height of the title-bar WIDGET; the 1px inner outline beneath the
/// Win8.1 bar is a frame-level row, not part of the bar.
fn bar_h(style: crate::model::FrameStyle, maximized: bool) -> f32 {
    match style {
        crate::model::FrameStyle::Win81 => win81_bar_h(maximized),
        _ => TITLEBAR_H,
    }
}

/// Height the model reserves below the frame's top inset for the title
/// bar: the bar plus, for the Win8.1 frame, the 1px inner-outline row
/// under it (kept at a constant physical width, never DPI-scaled).
pub(crate) fn titlebar_h(style: crate::model::FrameStyle, maximized: bool) -> f32 {
    match style {
        crate::model::FrameStyle::Win81 => win81_bar_h(maximized) + crate::theme::border_w(1.0),
        _ => TITLEBAR_H,
    }
}

/// Pastel tint applied to a contextual block accent (shared with the
/// frame-level tails, see `contextual_extenders`).
fn tint(c: Color, t: f32) -> Color {
    Color::from_rgb(
        c.r + (1.0 - c.r) * t,
        c.g + (1.0 - c.g) * t,
        c.b + (1.0 - c.b) * t,
    )
}

/// 1px "tails" continuing each visible Win8.1 contextual block over
/// the inner outline below the title bar: (x, w, color) in window
/// coordinates, drawn at frame level AFTER the outline so they
/// override it. Empty when the ribbon is sacrificed (no tabs, no
/// blocks) or no contextual set is visible.
pub(crate) fn contextual_extenders(state: &Explorer) -> Vec<(f32, f32, Color)> {
    let (_, fr, _, fl) = frame_insets(state);
    if state.win_w - fl - fr < state.ribbon.sacrifice().0 * state.dpi_multiplier() {
        return Vec::new();
    }
    let selected = state.ribbon.framework().view().selected_tab();
    state
        .ribbon
        .contextual_blocks()
        .iter()
        .map(|b| {
            let active = selected >= b.first_tab && selected <= b.last_tab;
            (fl + b.x, b.w, tint(b.color, if active { 0.45 } else { 0.62 }))
        })
        .collect()
}

/// Contextual tab-set blocks (Win8): a pastel accent block per visible
/// contextual set, bottom-aligned in the title bar, positioned exactly
/// above its ribbon tabs (geometry published by the ribbon widget).
fn contextual_blocks(state: &Explorer) -> Element<'static, Message> {
    let (_, fr, _, fl) = frame_insets(state);
    if state.win_w - fl - fr < state.ribbon.sacrifice().0 * state.dpi_multiplier() {
        // ribbon sacrificed: no tabs, no blocks
        return Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(0.0)).boxed();
    }
    let selected = state.ribbon.framework().view().selected_tab();
    let bar_h = bar_h(state.frame_style, state.maximized);
    let mut r: iced::widget::Row<Element<'_, Message>> = row![].height(Length::Fixed(bar_h));
    let mut prev_end = 0.0f32;
    for b in state.ribbon.contextual_blocks() {
        // the block spans exactly its tab run on the strip below
        let bx = b.x;
        let bw = b.w;
        let gap = (bx - prev_end).max(0.0);
        if gap > 0.0 {
            r = r.push(Space::new().width(Length::Fixed(gap)).boxed());
        }
        prev_end = bx + bw;
        let accent = b.color;
        let active = selected >= b.first_tab && selected <= b.last_tab;
        let label_color = Color::from_rgb(accent.r * 0.6, accent.g * 0.6, accent.b * 0.6);
        // Win8.1: the 26px block continues 1 physical px below the bar
        // as a frame-level "tail" over the inner outline (see
        // contextual_extenders); other frames keep the 24px block.
        // Both cap at the bar height (maximized Win8.1: 23px).
        let block_h = bar_h.min(match state.frame_style {
            crate::model::FrameStyle::Win81 => 26.0,
            _ => 24.0,
        });
        let solid = container(
            container(
                text(b.label.clone()).size(state.fonts.titlebar.block).color(label_color).width(Length::Shrink),
            )
            .width(Length::Fixed(bw))
            .height(Length::Fixed(block_h))
            .center_x(Length::Fixed(bw))
            .center_y(Length::Fixed(block_h)),
        )
        .width(Length::Fixed(bw))
        .height(Length::Fixed(block_h))
        .style(move |_| container::Style {
            background: Some(tint(accent, if active { 0.45 } else { 0.62 }).into()),
            ..Default::default()
        });
        let block = container(solid)
            .width(Length::Fixed(bw))
            .height(Length::Fixed(bar_h))
            .align_y(iced::Alignment::End);
        r = r.push(
            // cursor_seed: block_press arms the caption-drag threshold
            // at Explorer::cursor, which idle moves no longer refresh
            mouse_area(crate::ui_common::cursor_seed(block))
                .on_press(Message::BlockPress(b.first_tab))
                .boxed(),
        );
    }
    r.boxed()
}

/// Title-bar QAT ("show above the ribbon"): icon buttons right of the
/// window icon, framed by thin separators; the chevron docks the QAT
/// back below the command pane.
fn titlebar_qat(state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let sheen = skin.glass_sheen;
    let vline = || {
        container(
            container(Space::new().width(Length::Fixed(crate::theme::border_w(1.0))).height(Length::Fixed(15.0)))
                .style(move |_| container::Style {
                    background: Some(sheen.into()),
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(6.0))
        .center_x(Length::Fixed(6.0))
    };
    let mut qat: iced::widget::Row<Element<'static, Message>> = row![].spacing(2).align_y(iced::Alignment::Center);
    qat = qat.push(vline().boxed());
    for (cmd, icon_name, _enabled) in state.ribbon.qat_buttons() {
        qat = qat.push(
            mouse_area(
                container(crate::icon::view(&icon_name, 16.0, &skin))
                    .width(Length::Fixed(22.0))
                    .height(Length::Fixed(22.0))
                    .center_x(Length::Fixed(22.0))
                    .center_y(Length::Fixed(22.0)),
            )
            .on_press(Message::Ribbon(crate::ribbon::RibbonEvent::Execute {
                command: cmd,
                verb: crate::ribbon::enums::ExecutionVerb::Execute,
                key: None,
                value: None,
            }))
            .boxed(),
        );
    }
    qat = qat.push(
        mouse_area(
            container(crate::icon::view("chevron_down", 10.0, &skin))
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(22.0))
                .center_x(Length::Fixed(14.0))
                .center_y(Length::Fixed(22.0)),
        )
        .on_press(Message::ToggleQatDock)
        .boxed(),
    );
    qat = qat.push(vline().boxed());
    container(qat).height(Length::Fill).center_y(Length::Fill).boxed()
}

pub(crate) fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let icon_zone_w: f32 = 28.0; // window icon zone at the bar's left
    let bar_h = bar_h(state.frame_style, state.maximized);
    // maximized Win8.1: the 6px side border is omitted, so the bar's
    // left-aligned content shifts left by those 6px (only the 1px
    // outline position stays) and the caption buttons shift right by
    // 3px (half the side width) — the reserved captions zone grows 3px
    let (left_pad, captions_w) = match state.frame_style {
        crate::model::FrameStyle::Win8 => (0.0, 28.0 + 28.0 + 47.0 + 6.0),
        crate::model::FrameStyle::Win10 => (0.0, 46.0 * 3.0),
        // 25 + 1 + 25 + 1 + 45 buttons, then 6px + the 1px inner
        // outline in front of the right border (1px widths never
        // DPI-scaled)
        crate::model::FrameStyle::Win81 => {
            let base = 25.0 * 2.0 + 45.0 + 6.0 + 3.0 * crate::theme::border_w(1.0);
            if state.maximized {
                (crate::theme::border_w(1.0), base + 3.0)
            } else {
                (0.0, base)
            }
        }
    };
    let (_, fr, _, fl) = frame_insets(state);
    let content_w = (state.win_w - fr - fl).max(240.0);
    let qat_visible = state.ribbon.qat_docked_top()
        && state.win_w - fl - fr
            >= state.ribbon.sacrifice().0 * state.dpi_multiplier();
    // QAT head width when docked at the top: two separators, the
    // chevron and n 22px buttons with 2px row spacing
    // (mirrors titlebar_qat).
    let head_w = icon_zone_w
        + if qat_visible {
            24.0 * state.ribbon.qat_buttons().len() as f32 + 30.0
        } else {
            0.0
        };
    // the centered title must clear the contextual blocks (they sit
    // above the ribbon tabs, left of the title area) and the QAT head
    let blocks_right = state
        .ribbon
        .contextual_blocks()
        .iter()
        .map(|b| b.x + b.w)
        .fold(0.0f32, f32::max);
    let title_left = (blocks_right + 8.0).max(head_w);
    let title_w = (content_w - title_left - captions_w - 16.0).max(60.0);
    // the Win8.1 bar takes its caption size from win81theme.yaml
    // (taller chrome — see default_title_size); the 8.75 elision width
    // is calibrated for the shared 12px title, so scale it with the size
    let title_size = if state.frame_style == crate::model::FrameStyle::Win81 {
        win81::title_size()
    } else {
        state.fonts.titlebar.title
    };
    let char_w = 8.75 * title_size / state.fonts.titlebar.title.max(1.0);
    let drag_title: Element<Message> = mouse_area(
        container(
            text(elide(&state.title(), title_w, char_w))
                .size(title_size)
                .color(skin.title_text)
                .width(Length::Shrink),
        )
        .width(Length::Fixed(title_w))
        .height(Length::Fill)
        .center_x(Length::Fixed(title_w))
        .center_y(Length::Fill),
    )
    .on_press(Message::TitlebarPress)
    .on_double_click(Message::TitlebarDoubleClick)
    .boxed();
    // the gap between the QAT head and the title (where the contextual
    // blocks float) is title-bar too: dragging there moves the window
    let gap_w = (title_left - head_w).max(0.0);
    let gap_drag = mouse_area(
        Space::new()
            .width(Length::Fixed(gap_w))
            .height(Length::Fixed(bar_h)),
    )
    .on_press(Message::TitlebarPress)
    .on_double_click(Message::TitlebarDoubleClick);
    let mut head = row![
        // the window icon follows the current folder (16x16)
        container(icon(state.location_icon(), 16.0, skin))
            .width(Length::Fixed(icon_zone_w))
            .height(Length::Fill)
            .center_x(Length::Fixed(icon_zone_w))
            .center_y(Length::Fill),
    ];
    if qat_visible {
        head = head.push(titlebar_qat(state, skin));
    }
    // the Win8.1 title bar takes the spec's accent color example
    // (win81theme.yaml `window_accent`)
    let bar_bg = match state.frame_style {
        crate::model::FrameStyle::Win81 => win81::accent(),
        _ => skin.accent,
    };
    let base = container(
        row![
            Space::new().width(Length::Fixed(left_pad)),
            head,
            gap_drag,
            drag_title,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(bar_h)))
                .on_press(Message::TitlebarPress)
                .on_double_click(Message::TitlebarDoubleClick),
            Space::new().width(Length::Fixed(captions_w)),
        ]
        .height(Length::Fixed(bar_h)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(bar_h))
    .style(bg(bar_bg));
    stack![base, contextual_blocks(state)].boxed()
}

#[cfg(test)]
mod tests {
    // Every caption glyph template must be valid SVG once its color
    // tokens are substituted — a document usvg can't parse renders as
    // nothing (blank caption buttons).
    #[test]
    fn caption_glyph_templates_parse() {
        use super::{CAPTION_GLYPHS, CAPTION_GLYPHS_81};
        let opt = resvg::usvg::Options::default();
        let check = |t: &str| {
            let doc = t.replace("{glyph}", "#000000").replace("{title}", "#000000");
            let tree = resvg::usvg::Tree::from_str(&doc, &opt)
                .unwrap_or_else(|e| panic!("caption glyph failed to parse: {e}\n{doc}"));
            assert!(tree.size().width() > 0.0 && tree.size().height() > 0.0);
        };
        for t in CAPTION_GLYPHS_81 {
            check(t);
        }
        for g in CAPTION_GLYPHS.iter() {
            check(g.win8);
            check(g.win10);
        }
    }

}

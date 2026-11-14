//! Precise text metrics for the folder-pane labels. Independent
//! module; ported from the textmetrics demo (the old per-char em
//! estimates in `ui_common` were too inaccurate).
//!
//! Measurements come from the same cosmic-text engine and font
//! database that iced renders with, so a label measured to fit
//! provably fits. The two cuts follow the folder-view scheme:
//!
//!   - [`ellipsize`]: single line, tail "..." — Details plus the
//!     icon-left / text-right families (Content / Tiles / SmallIcon),
//!   - [`wrap_hard`]: unconditional wrap (breaks mid-word) into at
//!     most N lines, the remainder ellipsized onto the last line —
//!     IconTop.
//!
//! Shaping mirrors the text widget's `Shaping::Auto` default (Basic
//! for ASCII, Advanced otherwise), so measured and rendered widths
//! agree.
//!
//! Every measurement is memoized in a bounded global cache: label
//! cuts re-run on every view() redraw and must stay cheap — shaping
//! happens once per distinct (text, size, font); the cache is capped
//! and cleared whole when full so a long session cannot grow it
//! without bound.

use std::collections::HashMap;
use std::sync::Mutex;

use iced::advanced::graphics::text::{self as gfx_text, cosmic_text};
use iced::Font;

/// Measurement cache cap; on overflow the map is cleared (simple
/// bounded eviction — entries are cheap to recompute).
const CACHE_CAP: usize = 16_384;

type Cache = HashMap<(String, u32, Font), (f32, f32)>;

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// The text widget default (`Shaping::Auto`): Basic shaping for
/// ASCII, Advanced otherwise. Measuring with the same mode keeps the
/// numbers identical to what the renderer will draw.
fn shaping_of(s: &str) -> cosmic_text::Shaping {
    if s.is_ascii() {
        cosmic_text::Shaping::Basic
    } else {
        cosmic_text::Shaping::Advanced
    }
}

/// Exact (width, height) of one unwrapped line, shaped with the same
/// font system and metrics the renderer uses.
pub fn measure(fragment: &str, size: f32, font: Font) -> (f32, f32) {
    let key = (fragment.to_string(), size.to_bits(), font);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(v) = cache.get(&key) {
        return *v;
    }
    let v = measure_uncached(fragment, size, font);
    if cache.len() >= CACHE_CAP {
        cache.clear();
    }
    cache.insert(key, v);
    v
}

fn measure_uncached(fragment: &str, size: f32, font: Font) -> (f32, f32) {
    let system = gfx_text::font_system();
    // Poisoning only happens if a panicking thread held the lock; the
    // guard contents are still usable.
    let mut system = system.write().unwrap_or_else(|e| e.into_inner());

    let mut buffer = cosmic_text::Buffer::new(
        system.raw(),
        // Line height is irrelevant for width; a single line of
        // `size` keeps the metrics well-formed.
        cosmic_text::Metrics::new(size, size),
    );
    buffer.set_wrap(cosmic_text::Wrap::None);
    buffer.set_text(
        fragment,
        &gfx_text::to_attributes(font),
        shaping_of(fragment),
        None,
    );
    buffer.shape_until_scroll(system.raw(), false);

    let (size, _) = gfx_text::measure(&buffer);
    (size.width, size.height)
}

/// Exact vertical glyph block (max ascent + max descent) of a line at
/// the given size/font, probed with ascender/descender extremes so the
/// result reflects the font rather than any particular label's
/// characters — row heights must not depend on which glyphs a name
/// happens to contain. Cached (per size/font).
pub fn line_block(size: f32, font: Font) -> f32 {
    let key = (size.to_bits(), font);
    let mut cache = LINE_BLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(v) = cache.get(&key) {
        return *v;
    }
    let v = line_block_uncached(size, font);
    if cache.len() >= 1024 {
        cache.clear();
    }
    cache.insert(key, v);
    v
}

static LINE_BLOCK: Mutex<Option<HashMap<(u32, Font), f32>>> = Mutex::new(None);

fn line_block_uncached(size: f32, font: Font) -> f32 {
    let system = gfx_text::font_system();
    let mut system = system.write().unwrap_or_else(|e| e.into_inner());

    let mut buffer = cosmic_text::Buffer::new(system.raw(), cosmic_text::Metrics::new(size, size));
    buffer.set_wrap(cosmic_text::Wrap::None);
    // Probe with tall/depthy glyphs so ascent and descent both reach
    // the font's real limits.
    buffer.set_text(
        "\u{00c1}gjpq|\u{00c1}",
        &gfx_text::to_attributes(font),
        cosmic_text::Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(system.raw(), false);
    let block = buffer
        .line_layout(system.raw(), 0)
        .map(|ls| ls.iter().fold(0.0f32, |m, l| m.max(l.max_ascent + l.max_descent)))
        .unwrap_or(size);
    block.max(1.0)
}

/// Single-line cut with a tail "..." (folder-view method 3): the
/// result is guaranteed to measure `<= max_w` at the given size/font.
/// Returns the input unchanged when it already fits; the empty string
/// stays empty; a `max_w` too small for any prefix yields "...".
pub fn ellipsize(s: &str, max_w: f32, size: f32, font: Font) -> String {
    if s.is_empty() || measure(s, size, font).0 <= max_w {
        return s.to_string();
    }

    const MARK: &str = "...";
    let budget = max_w - measure(MARK, size, font).0;
    if budget <= 0.0 {
        return MARK.to_string();
    }

    let chars: Vec<char> = s.chars().collect();

    // Binary search the longest prefix within the budget. Prefix width
    // is nearly monotonic, but kerning can make it dip, so walk back
    // to the first prefix that actually fits.
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

    let mut out: String = chars[..lo].iter().collect();
    out.push_str(MARK);
    out
}

/// Unconditional wrap into at most `max_lines` lines (folder-view
/// method 5): every line is filled to `line_w`, breaking mid-word
/// where a word would cross the border. When the content needs more
/// lines than allowed, the remainder is joined and ellipsized onto
/// the last line, so the final line both fills the box and signals
/// the cut. Newlines force a line break.
pub fn wrap_hard(
    s: &str,
    line_w: f32,
    max_lines: usize,
    size: f32,
    font: Font,
) -> Vec<String> {
    let max_lines = max_lines.max(1);
    let mut pieces: Vec<String> = Vec::new();
    for paragraph in s.split('\n') {
        hard_pieces(paragraph, line_w, size, font, &mut pieces);
    }
    if pieces.len() <= max_lines {
        return pieces;
    }
    let mut out: Vec<String> = pieces.drain(..max_lines - 1).collect();
    let tail = pieces.join(" ");
    out.push(ellipsize(&tail, line_w, size, font));
    out
}

/// Displayed line count of [`wrap_hard`], clamped to 1..=max_lines —
/// the icon-top chrome height cache (`it_label_lines`) wants a count,
/// not the lines themselves.
pub fn hard_wrap_count(
    s: &str,
    line_w: f32,
    max_lines: usize,
    size: f32,
    font: Font,
) -> usize {
    wrap_hard(s, line_w, max_lines, size, font).len().max(1)
}

/// Fills `out` with the hard-wrapped pieces of one paragraph: a
/// character appends to the current piece while it still fits,
/// otherwise the piece flushes — even inside a word.
fn hard_pieces(
    paragraph: &str,
    line_w: f32,
    size: f32,
    font: Font,
    out: &mut Vec<String>,
) {
    let mut piece = String::new();
    for ch in paragraph.chars() {
        let mut candidate = piece.clone();
        candidate.push(ch);
        if piece.is_empty() || measure(&candidate, size, font).0 <= line_w {
            piece = candidate;
        } else {
            out.push(std::mem::take(&mut piece));
            piece.push(ch);
        }
    }
    if !piece.is_empty() {
        out.push(piece);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f32 = 12.0;

    fn font() -> Font {
        Font::DEFAULT
    }

    fn w(s: &str) -> f32 {
        measure(s, SIZE, font()).0
    }

    #[test]
    fn ellipsize_keeps_fitting_strings() {
        assert_eq!(ellipsize("", 50.0, SIZE, font()), "");
        assert_eq!(ellipsize("short.txt", 500.0, SIZE, font()), "short.txt");
    }

    #[test]
    fn ellipsize_result_fits_and_ends_with_dots() {
        let name = "a_very_long_file_name_that_cannot_fit.txt";
        let max_w = w("a_very_long_fil"); // fits ~15 chars at 12px
        let out = ellipsize(name, max_w, SIZE, font());
        assert!(out.ends_with("..."), "{out:?}");
        assert!(w(&out) <= max_w + 0.01, "{out:?} = {} > {max_w}", w(&out));
        // nothing more fits: extending by the next source char must overflow
        let prefix_len = out.len() - 3;
        let longer: String = name.chars().take(prefix_len + 1).collect::<String>() + "...";
        assert!(w(&longer) > max_w + 0.01);
    }

    #[test]
    fn ellipsize_tiny_budget_yields_dots() {
        assert_eq!(ellipsize("anything", 2.0, SIZE, font()), "...");
    }

    #[test]
    fn wrap_hard_every_line_fits() {
        let name = "supercalifragilisticexpialidocious_and_some_more_words.txt";
        let line_w = w("supercalifrag"); // ~13 chars at 12px
        let lines = wrap_hard(name, line_w, 4, SIZE, font());
        assert!(lines.len() <= 4);
        for (n, l) in lines.iter().enumerate() {
            assert!(w(l) <= line_w + 0.01, "line {n} {l:?} overflows");
        }
        // unconditional: at least one break lands mid-word (no piece is
        // purely a word from the source when the first word alone wraps)
        assert!(lines.len() > 1);
    }

    #[test]
    fn wrap_hard_clamps_and_ellipsizes_last_line() {
        // 80 chars at ~10 chars/line = ~8 wrapped lines, clamped to 4
        let name = "x".repeat(80);
        let line_w = w(&"x".repeat(10));
        let lines = wrap_hard(&name, line_w, 4, SIZE, font());
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert!(lines[3].ends_with("..."), "{lines:?}");
        for l in &lines {
            assert!(w(l) <= line_w + 0.01, "{l:?} overflows");
        }
    }

    #[test]
    fn wrap_hard_single_line_mode() {
        let lines = wrap_hard("alpha beta gamma delta", 2000.0, 1, SIZE, font());
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn wrap_hard_newline_forces_break() {
        let lines = wrap_hard("abc\ndef", 2000.0, 4, SIZE, font());
        assert_eq!(lines, vec!["abc".to_string(), "def".to_string()]);
    }

    #[test]
    fn hard_wrap_count_is_clamped() {
        let name = "x".repeat(400);
        let n = hard_wrap_count(&name, w(&"x".repeat(10)), 4, SIZE, font());
        assert!((1..=4).contains(&n));
    }
}


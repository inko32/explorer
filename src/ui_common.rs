//! Shared UI plumbing: small element helpers used everywhere. The light/dark palettes live in
//! `assets/theme.yaml`, parsed by `crate::theme`; `skin` below is a
//! re-export so existing call sites stay unchanged.

use iced::widget::container;
use iced::{Color, Element};

use crate::model::Message;

/// Square chrome tool button size (ribbon minimize/help, nav refresh,
/// status-bar view switcher) — one size everywhere.
pub(crate) const CHROME_BTN: f32 = 26.0;


pub mod skin {
    pub use crate::theme::{dark, light, Palette as Skin};
}


pub(crate) fn bg(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: Some(color.into()),
        ..Default::default()
    }
}

pub(crate) fn icon(name: &str, size: f32, skin: skin::Skin) -> Element<'static, Message> {
    crate::icon::view(name, size, &skin)
}

/// Wraps content so that an uncaptured left/right press over it
/// publishes `Message::CursorAt(position)` — and does NOT capture the
/// press: enclosing `mouse_area`/`button` handlers fire unchanged
/// right after. The message bus is FIFO, so the model's cursor is
/// fresh by the time they run.
///
/// Why: idle cursor moves no longer reach the model (main.rs drops
/// them before they become messages — each one would force a full UI
/// rebuild + layout at pointer rate), so `Explorer::cursor` is stale
/// while idle. Press-time readers (drag-initiation thresholds,
/// context-menu and dropdown anchors) get the position from the
/// widget layer instead. Place ONE seed at the outermost interactive
/// container of a region (nested ones would each publish a redundant
/// CursorAt — harmless but wasteful).
pub(crate) fn cursor_seed<'a, W>(content: W) -> Element<'a, Message>
where
    W: iced::advanced::Widget<Message, iced::Theme, iced::Renderer> + 'a,
{
    struct CursorSeed<'a> {
        content: Element<'a, Message>,
    }

    impl iced::advanced::widget::Meta for CursorSeed<'_> {}

    impl<'a> iced::advanced::Widget<Message, iced::Theme, iced::Renderer>
        for CursorSeed<'a>
    {
        fn size(&self) -> iced::Size<iced::Length> {
            self.content.size()
        }
        fn tag(&self) -> iced::advanced::widget::tree::Tag {
            self.content.tag()
        }
        fn state(&self) -> iced::advanced::widget::tree::State {
            self.content.state()
        }
        fn diff(&mut self, tree: &mut iced::advanced::widget::Tree) {
            self.content.diff(tree);
        }
        fn layout(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            renderer: &iced::Renderer,
            limits: &iced::advanced::layout::Limits,
        ) {
            self.content.layout(tree, renderer, limits)
        }
        fn draw(
            &self,
            tree: &iced::advanced::widget::Tree,
            renderer: &mut iced::Renderer,
            theme: &iced::Theme,
            style: &iced::advanced::renderer::Style,
            layout: iced::advanced::Layout,
            cursor: iced::advanced::mouse::Cursor,
            viewport: &iced::Rectangle,
        ) {
            self.content
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        fn operate(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            layout: iced::advanced::Layout,
            viewport: &iced::Rectangle,
            renderer: &iced::Renderer,
            operation: &mut dyn iced::advanced::widget::Operation,
        ) {
            self.content.operate(tree, layout, viewport, renderer, operation);
        }
        fn update(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            event: &iced::Event,
            layout: iced::advanced::Layout,
            cursor: iced::advanced::mouse::Cursor,
            renderer: &iced::Renderer,
            shell: &mut iced::advanced::Shell<'_, Message>,
            viewport: &iced::Rectangle,
        ) {
            self.content
                .update(tree, event, layout, cursor, renderer, shell, viewport);
            if matches!(
                event,
                iced::Event::Mouse(
                    iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Left | iced::mouse::Button::Right
                    )
                )
            ) && !shell.is_event_captured()
                && let Some(p) = cursor.position_over(layout.bounds())
            {
                // no capture: the enclosing mouse_area/button handles
                // the press (and fires its own message) after us
                shell.publish(Message::CursorAt(p));
            }
        }
        fn mouse_interaction(
            &self,
            tree: &iced::advanced::widget::Tree,
            layout: iced::advanced::Layout,
            cursor: iced::advanced::mouse::Cursor,
            viewport: &iced::Rectangle,
            renderer: &iced::Renderer,
        ) -> iced::mouse::Interaction {
            self.content.mouse_interaction(tree, layout, cursor, viewport, renderer)
        }
        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut iced::advanced::widget::Tree,
            layout: iced::advanced::Layout,
            renderer: &iced::Renderer,
            viewport: &iced::Rectangle,
            translation: iced::Vector,
            window: iced::Size,
        ) -> Vec<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>>
        {
            self.content
                .overlay(tree, layout, renderer, viewport, translation, window)
        }
    }

    use iced::Widget as _;
    CursorSeed { content: content._boxed() }.boxed()
}

// ---------------- title bar ----------------
// Name-label cuts (single-line ellipsis and icon-top hard wrap) live
// in `crate::text_metrics` — exact cosmic-text widths, not estimates.

/// List-view name cut: a fixed unit budget (ASCII = 1 unit, any
/// non-ASCII char = 2 units); over-budget names are tail-cut with
/// "...", reserving 3 units for it. No pixel elision in List — the
/// full name is shown on one line.
pub(crate) fn truncate_units(s: &str, budget: usize) -> String {
    let units = |c: char| if c.is_ascii() { 1 } else { 2 };
    if s.chars().map(units).sum::<usize>() <= budget {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let u = units(c);
        if used + u + 3 > budget {
            break;
        }
        used += u;
        out.push(c);
    }
    out.push_str("...");
    out
}

/// Middle-ellipsis text clamp to a pixel width.
pub(crate) fn elide(s: &str, max_w: f32, per_char: f32) -> String {
    let w = s.chars().count() as f32 * per_char;
    if w <= max_w {
        return s.to_string();
    }
    let keep = ((max_w / per_char).max(1.0) as usize).saturating_sub(3);
    let take = keep / 2;
    let start: String = s.chars().take(take).collect();
    let tail: String = s
        .chars()
        .rev()
        .take(keep.saturating_sub(take))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}\u{2026}{}", start, tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_units_counts_non_ascii_double() {
        // 100 ASCII units: shown in full
        let ascii = "a".repeat(100);
        assert_eq!(truncate_units(&ascii, 100), ascii);
        // 101 ASCII: tail-cut, 3 units reserved for "..." (97 + ... = 100)
        assert_eq!(truncate_units(&"a".repeat(101), 100).chars().count(), 100);
        // 60 non-ASCII chars = 120 units: cut at 48 chars (96 units) + "..."
        let out = truncate_units(&"é".repeat(60), 100);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), 51);
    }
}


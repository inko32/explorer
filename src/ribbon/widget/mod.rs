//! The iced ribbon widget.
//!
//! ```rust,ignore
//! let ribbon = rs_ribbon::widget::Ribbon::new(&framework)
//!     .on_event(Message::Ribbon);
//! ```

pub mod layout;
pub mod popup;
pub mod scene;
pub mod state;

use std::time::{Duration, Instant};

use iced::advanced::layout::{self as ilayout, Layout};
use iced::advanced::renderer;
use iced::advanced::svg;
use iced::advanced::text;
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::keyboard;
use iced::mouse;
use iced::overlay;
use iced::{
    Background, Color, Element, Length, Point, Rectangle, Size,
    Theme as IcedTheme, Vector,
};

use crate::ribbon::enums::{EventLocation, EventType, ExecutionVerb};
use crate::ribbon::framework::RibbonFramework;
use crate::ribbon::prop::{PropertyKey, PropValue};
use crate::ribbon::spec::ControlSpec;
use crate::ribbon::theme::Theme;
use crate::ribbon::RibbonEvent;

pub use state::{close_all_popups, close_popup, EditState, Popup, PopupKind, State, SwatchSel, Target};

use layout::Ctx;
use scene::{Cmd, Fill, Scene};
use state::Target::*;

/// The ribbon widget.
pub struct Ribbon<'a, Message, Th = IcedTheme, Renderer = iced::Renderer> {
    fw: &'a RibbonFramework,
    theme: Theme,
    on_event: Option<Box<dyn Fn(RibbonEvent) -> Message + 'a>>,
    _phantom: std::marker::PhantomData<(Th, Renderer)>,
}

impl<'a, Message, Th, Renderer> Ribbon<'a, Message, Th, Renderer> {
    /// A ribbon bound to a loaded [`RibbonFramework`].
    pub fn new(fw: &'a RibbonFramework) -> Self {
        Self {
            fw,
            theme: Theme::windows10(),
            on_event: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Use a custom [`Theme`] instead of the Windows 10 default.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Map ribbon events into the application message type
    /// (the `IUIApplication`/`IUICommandHandler` callbacks).
    pub fn on_event(
        mut self,
        f: impl Fn(RibbonEvent) -> Message + 'a,
    ) -> Self {
        self.on_event = Some(Box::new(f));
        self
    }

}

impl<'a, Message, Th, Renderer> Widget<Message, Th, Renderer>
    for Ribbon<'a, Message, Th, Renderer>
where
    Message: Clone + 'a,
    Renderer: iced::advanced::Renderer
        + text::Renderer
        + svg::Renderer
        + iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &ilayout::Limits,
    ) -> ilayout::Node {
        let state = tree.state.downcast_mut::<State>();
        let width = if limits.max().width.is_finite() {
            limits.max().width
        } else {
            960.0
        };
        let (scene, _) = {
            let cache = &state.measure_cache;
            let renderer_ref = &_renderer;
            let measure = &|s: &str, size: f32| {
                let key = (s.to_string(), (size * 4.0).round() as u32);
                if let Some(w) = cache.borrow().get(&key) {
                    return *w;
                }
                let w = text_measure(*renderer_ref, s, size);
                cache.borrow_mut().insert(key, w);
                w
            };
            let ctx = Ctx {
                fw: self.fw,
                theme: &self.theme,
                state,
                measure,
            };
            layout::ribbon_scene(&ctx, width)
        };
        let size = scene.size;
        state.scene = Some(scene);
        ilayout::Node::new(size)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Th,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        if let Some(scene) = &state.scene {
            draw_scene(
                scene,
                renderer,
                layout.position(),
                viewport,
                &self.theme,
            );
        }
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::default())
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let event = event.clone();
        let state = tree.state.downcast_mut::<State>();
        let origin = layout.position();
        let bounds = layout.bounds();

        // consume IUIContextualUI requests
        if let Some((cmd, x, y)) = self.fw.view().take_pending_popup() {
            state.popup = Some(Popup {
                kind: PopupKind::ContextMenu { cmd, path: Vec::new() },
                origin: Point::new(x, y),
                width_hint: 0.0,
                scroll: 0,
                hover: None,
                size: Size::ZERO,
                parent: None,
            });
            // make the popup visible in this same event cycle
            shell.invalidate_layout();
            shell.request_redraw();
        }

        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                state.hover_pos = position;
                let local = Point::new(position.x - origin.x, position.y - origin.y);
                let hit = state
                    .scene
                    .as_ref()
                    .and_then(|s| s.hit(local).cloned());
                let new_hover = hit.as_ref().and_then(|n| n.target.clone());
                if state.hover != new_hover {
                    state.hover = new_hover.clone();
                    state.hover_since = Some(Instant::now());
                    state.tooltip = None;
                    shell.request_redraw();
                }
                // tooltip
                if let Some(node) = &hit {
                    if let (Some(tip), Some(since)) = (node.tip, state.hover_since) {
                        if since.elapsed() > Duration::from_millis(600)
                            && state.tooltip.is_none()
                        {
                            state.tooltip = Some(tip);
                            self.fw.view_log(
                                EventType::TooltipShown,
                                if matches!(new_hover, Some(Qat(_))) {
                                    EventLocation::Qat
                                } else {
                                    EventLocation::Ribbon
                                },
                                tip,
                            );
                        }
                    }
                }
                // gallery preview from the in-ribbon strip
                handle_strip_preview(self.fw, state, &new_hover, shell, &self.on_event);
                return;
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.keytips = false;
                state.keytip_buffer.clear();
                let Some(position) = cursor.position() else {
                    return;
                };
                // close popups on click-away (the overlay passes through)
                if let Some((pop_origin, pop_size)) = state.popup_bounds {
                    let r = Rectangle::new(pop_origin, pop_size);
                    if pop_size.width > 0.0 && r.contains(position) {
                        return; // overlay handles it
                    }
                    close_all_popups(state);
                    state.popup_bounds = None;
                    shell.request_redraw();
                }
                let local = Point::new(position.x - origin.x, position.y - origin.y);
                let hit = state.scene.as_ref().and_then(|s| s.hit(local).cloned());
                if let Some(node) = &hit {
                    if let Some(t) = &node.target {
                        state.pressed = Some(t.clone());
                        // entering edit mode?
                        match t {
                            SpinnerText(cmd) => {
                                let text = format_spinner(self.fw, *cmd);
                                state.edit = Some(EditState {
                                    cmd: *cmd,
                                    text,
                                    numeric: true,
                                });
                            }
                            ComboMain(cmd) => {
                                let text = self
                                    .fw
                                    .cached(*cmd, PropertyKey::StringValue)
                                    .and_then(|v| v.as_str().map(str::to_owned))
                                    .unwrap_or_default();
                                state.edit = Some(EditState {
                                    cmd: *cmd,
                                    text,
                                    numeric: false,
                                });
                            }
                            _ => {
                                if state.edit.is_some() {
                                    state.edit = None;
                                }
                            }
                        }
                    }
                } else if state.edit.is_some() {
                    state.edit = None;
                }
                return;
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let Some(position) = cursor.position() else {
                    state.pressed = None;
                    return;
                };
                let local = Point::new(position.x - origin.x, position.y - origin.y);
                let hit = state.scene.as_ref().and_then(|s| s.hit(local).cloned());
                let pressed = state.pressed.take();
                if let (Some(node), Some(t)) = (&hit, &pressed) {
                    if node.target.as_ref() == Some(t) {
                        let anchor = translate_rect(node.rect, origin);
                        activate(
                            self.fw,
                            &self.theme,
                            state,
                            t,
                            anchor,
                            origin,
                            shell,
                            self.on_event.as_deref(),
                        );
                        shell.capture_event(); return;
                    }
                }
                return;
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(position) = cursor.position() else {
                    return;
                };
                let local = Point::new(position.x - origin.x, position.y - origin.y);
                let hit = state.scene.as_ref().and_then(|s| s.hit(local));
                if let Some(node) = hit {
                    if let Some(GalleryItem { cmd, .. }) = &node.target {
                        let dy = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => y,
                            mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
                        };
                        if dy.abs() > 0.01 {
                            let entry = state.gallery_scroll.entry(*cmd).or_insert(0);
                            let new = (*entry as i32 - dy.signum() as i32).max(0) as usize;
                            *entry = new;
                            shell.capture_event(); return;
                        }
                    }
                }
                return;
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text: key_text,
                ..
            }) => {
                handle_keyboard(
                    self.fw,
                    &self.theme,
                    state,
                    &key,
                    modifiers,
                    key_text.as_deref(),
                    origin,
                    bounds,
                    shell,
                    self.on_event.as_deref(),
                )
            }
            _ => {},
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if state.edit.is_some() {
            mouse::Interaction::Text
        } else if state.hover.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Th, Renderer>> {
        let state = tree.state.downcast_mut::<State>();

        // context popup requested via IUIContextualUI
        if let Some((cmd, x, y)) = self.fw.view().take_pending_popup() {
            state.popup = Some(Popup {
                kind: PopupKind::ContextMenu { cmd, path: Vec::new() },
                origin: Point::new(x, y),
                width_hint: 0.0,
                scroll: 0,
                hover: None,
                size: Size::ZERO,
                parent: None,
            });
        }

        let popup_clone = state.popup.clone();
        if let Some(popup) = &popup_clone {
            let cache = &state.measure_cache;
            let renderer_ref = &_renderer;
            let measure = &|s: &str, size: f32| {
                let key = (s.to_string(), (size * 4.0).round() as u32);
                if let Some(w) = cache.borrow().get(&key) {
                    return *w;
                }
                let w = text_measure(*renderer_ref, s, size);
                cache.borrow_mut().insert(key, w);
                w
            };
            let (scene, origin) = {
                let ctx = Ctx {
                    fw: self.fw,
                    theme: &self.theme,
                    state,
                    measure,
                };
                let scene = popup::popup_scene(&ctx, popup);
                match (scene, &popup.parent) {
                    // A dropdown opened from a panel popup (collapsed
                    // group, minimized tab) is shown *together with* its
                    // parent — the group stays visible behind the menu,
                    // like the Windows ribbon.
                    (Some(child), Some(parent)) => {
                        let mut merged = popup::popup_scene(&ctx, parent)?;
                        let mut size = merged.size;
                        let dx = popup.origin.x - parent.origin.x;
                        let dy = popup.origin.y - parent.origin.y;
                        for mut node in child.nodes {
                            node.rect.x += dx;
                            node.rect.y += dy;
                            merged.push(node);
                        }
                        size.width = size.width.max(dx + child.size.width);
                        size.height = size.height.max(dy + child.size.height);
                        merged.size = size;
                        (merged, parent.origin)
                    }
                    (Some(scene), None) => (scene, popup.origin),
                    (None, _) => {
                        close_all_popups(state);
                        state.popup_bounds = None;
                        return None;
                    }
                }
            };
            if let Some(p) = &mut state.popup {
                p.size = scene.size;
            }
            state.popup_bounds = Some((origin, scene.size));
            return Some(overlay::Element::new(Box::new(
                popup::RibbonOverlay::<Message, Th, Renderer> {
                    fw: self.fw,
                    theme: &self.theme,
                    state,
                    scene,
                    tooltip: false,
                    origin,
                    mapper: self.on_event.as_deref(),
                    _phantom: std::marker::PhantomData,
                },
            )));
        }

        // tooltip overlay
        if let Some(tip) = state.tooltip {
            if state.popup.is_none() {
                let cache = &state.measure_cache;
                let renderer_ref = &_renderer;
                let measure = &|s: &str, size: f32| {
                    let key = (s.to_string(), (size * 4.0).round() as u32);
                    if let Some(w) = cache.borrow().get(&key) {
                        return *w;
                    }
                    let w = text_measure(*renderer_ref, s, size);
                    cache.borrow_mut().insert(key, w);
                    w
                };
                let scene = {
                    let ctx = Ctx {
                        fw: self.fw,
                        theme: &self.theme,
                        state,
                        measure,
                    };
                    popup::tooltip_scene(&ctx, tip, state.hover_pos)
                };
                return Some(overlay::Element::new(Box::new(
                    popup::RibbonOverlay::<Message, Th, Renderer> {
                        fw: self.fw,
                        theme: &self.theme,
                        state,
                        scene,
                        tooltip: true,
                        origin: Point::ORIGIN,
                        mapper: self.on_event.as_deref(),
                        _phantom: std::marker::PhantomData,
                    },
                )));
            }
        }

        let _ = (layout, translation);
        None
    }
}

impl<'a, Message, Th, Renderer> From<Ribbon<'a, Message, Th, Renderer>>
    for Element<'a, Message, Th, Renderer>
where
    Message: Clone + 'a,
    Th: 'a,
    Renderer: iced::advanced::Renderer
        + text::Renderer
        + svg::Renderer
        + iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>
        + 'a,
{
    fn from(ribbon: Ribbon<'a, Message, Th, Renderer>) -> Self {
        Element::new(ribbon)
    }
}

/// Accurate text width via the renderer's paragraph shaper.
pub(crate) fn text_measure<Renderer>(
    renderer: &Renderer,
    content: &str,
    size: f32,
) -> f32
where
    Renderer: text::Renderer,
{
    use iced::advanced::text::Paragraph as _;
    let paragraph = Renderer::Paragraph::with_text(text::Text {
        content,
        bounds: Size::new(f32::INFINITY, f32::INFINITY),
        size: size.into(),
        line_height: text::LineHeight::Absolute((size * 1.2).into()),
        font: renderer.default_font(),
        align_x: iced::alignment::Horizontal::Left.into(),
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Basic,
        wrapping: text::Wrapping::None,
    });
    paragraph.min_bounds().width
}

// ---------------------------------------------------------------------------
// shared helpers

pub(crate) fn translate_rect(rect: Rectangle, origin: Point) -> Rectangle {
    Rectangle {
        x: rect.x + origin.x,
        y: rect.y + origin.y,
        ..rect
    }
}

pub(crate) fn publish<Message>(
    shell: &mut Shell<'_, Message>,
    mapper: Option<&dyn Fn(RibbonEvent) -> Message>,
    ev: RibbonEvent,
) {
    if let Some(f) = mapper {
        shell.publish(f(ev));
    }
}

fn format_spinner(fw: &RibbonFramework, cmd: u32) -> String {
    let value = fw
        .cached(cmd, PropertyKey::DecimalValue)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let places = fw
        .cached(cmd, PropertyKey::DecimalPlaces)
        .and_then(|v| v.as_u32())
        .unwrap_or(0);
    let format = fw
        .cached(cmd, PropertyKey::FormatString)
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "{0}".into());
    let fmt = format!("{:.*}", places as usize, value);
    let fmt = fmt.trim_end_matches('.').to_string();
    format.replace("{0}", &fmt)
}

fn handle_strip_preview<Message>(
    fw: &RibbonFramework,
    state: &mut State,
    hover: &Option<Target>,
    shell: &mut Shell<'_, Message>,
    mapper: &Option<Box<dyn Fn(RibbonEvent) -> Message + '_>>,
) {
    let gallery_hit = match hover {
        Some(GalleryItem { cmd, index, popup: false }) => Some((*cmd, *index)),
        _ => None,
    };
    if gallery_hit != state.previewed_strip {
        if let Some((cmd, index)) = state.previewed_strip.take() {
            publish(
                shell,
                mapper.as_deref(),
                RibbonEvent::Execute {
                    command: cmd,
                    verb: ExecutionVerb::CancelPreview,
                    key: Some(PropertyKey::SelectedItem),
                    value: Some(PropValue::UInt(index as u32)),
                },
            );
        }
        if let Some((cmd, index)) = gallery_hit {
            state.previewed_strip = Some((cmd, index));
            publish(
                shell,
                mapper.as_deref(),
                RibbonEvent::Execute {
                    command: cmd,
                    verb: ExecutionVerb::Preview,
                    key: Some(PropertyKey::SelectedItem),
                    value: Some(PropValue::UInt(index as u32)),
                },
            );
        }
        let _ = fw;
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_keyboard<Message>(
    fw: &RibbonFramework,
    theme: &Theme,
    state: &mut State,
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    key_text: Option<&str>,
    origin: Point,
    _bounds: Rectangle,
    shell: &mut Shell<'_, Message>,
    mapper: Option<&dyn Fn(RibbonEvent) -> Message>,
) {
    use keyboard::key::{Key, Named};
    match key {
        Key::Named(Named::Alt) | Key::Named(Named::F10) => {
            state.keytips = !state.keytips;
            state.keytips_strip = true;
            state.keytip_buffer.clear();
            shell.capture_event(); return;
        }
        Key::Named(Named::Escape) => {
            if state.edit.take().is_some() {
                shell.capture_event(); return;
            } else if state.keytips {
                if state.keytips_strip {
                    state.keytips = false;
                } else {
                    state.keytips_strip = true;
                }
                state.keytip_buffer.clear();
                shell.capture_event(); return;
            } else {
                return;
            }
        }
        _ => {
            // Ctrl+F1 minimizes the ribbon (Windows behavior)
            if modifiers.control()
                && matches!(key, Key::Named(Named::F1))
            {
                let view = fw.view();
                let minimized = !view.minimized();
                view.set_minimized(minimized);
                close_all_popups(state);
                publish(
                    shell,
                    mapper,
                    RibbonEvent::MinimizedChanged { minimized },
                );
                fw.view_log(
                    if minimized {
                        EventType::RibbonMinimized
                    } else {
                        EventType::RibbonExpanded
                    },
                    EventLocation::Ribbon,
                    0,
                );
                shell.capture_event(); return;
            }

            // inline editing
            if let Some(edit) = state.edit.clone() {
                match key {
                    Key::Named(Named::Enter) => {
                        let value = if edit.numeric {
                            PropValue::Decimal(edit.text.parse().unwrap_or(0.0))
                        } else {
                            PropValue::Str(edit.text.clone())
                        };
                        let key_prop = if edit.numeric {
                            PropertyKey::DecimalValue
                        } else {
                            PropertyKey::StringValue
                        };
                        state.edit = None;
                        publish(
                            shell,
                            mapper,
                            RibbonEvent::Execute {
                                command: edit.cmd,
                                verb: ExecutionVerb::Execute,
                                key: Some(key_prop),
                                value: Some(value),
                            },
                        );
                        shell.capture_event();
                    }
                    Key::Named(Named::Backspace) => {
                        if let Some(e) = state.edit.as_mut() {
                            e.text.pop();
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::Escape) => {
                        state.edit = None;
                        shell.capture_event();
                    }
                    _ => {
                        if let Some(t) = key_text {
                            for ch in t.chars() {
                                if !edit.numeric
                                    || ch.is_ascii_digit()
                                    || (ch == '.' || ch == '-') && !edit.text.contains(ch)
                                {
                                    if let Some(e) = state.edit.as_mut() {
                                        e.text.push(ch);
                                    }
                                }
                            }
                        }
                        shell.capture_event();
                    }
                }
                return;
            }

            // keytips
            if state.keytips {
                if let Some(t) = key_text {
                    for ch in t.chars().filter(|c| !c.is_control()) {
                        state.keytip_buffer.push(ch.to_ascii_uppercase());
                    }
                    if let Some(scene) = &state.scene {
                        let candidates = scene.keytips(state.keytips_strip);
                        let buf = state.keytip_buffer.clone();
                        if let Some((_, target)) =
                            candidates.iter().find(|(k, _)| *k == buf)
                        {
                            if let Some(node) = scene
                                .nodes
                                .iter()
                                .find(|n| n.target.as_ref() == Some(target))
                            {
                                let anchor = translate_rect(node.rect, origin);
                                let target = target.clone();
                                state.keytip_buffer.clear();
                                activate(
                                    fw, theme, state, &target, anchor, origin,
                                    shell, mapper,
                                );
                            }
                            shell.capture_event(); return;
                        }
                        let any_prefix = candidates
                            .iter()
                            .any(|(k, _)| k.starts_with(&buf) && !buf.is_empty());
                        if !any_prefix && buf.len() > 1 {
                            // retry with just the last character
                            let last = buf.chars().last().unwrap().to_string();
                            state.keytip_buffer = last.clone();
                            if let Some((_, target)) =
                                candidates.iter().find(|(k, _)| **k == last)
                            {
                                if let Some(node) = scene
                                    .nodes
                                    .iter()
                                    .find(|n| n.target.as_ref() == Some(target))
                                {
                                    let anchor = translate_rect(node.rect, origin);
                                    let target = target.clone();
                                    state.keytip_buffer.clear();
                                    activate(
                                        fw, theme, state, &target, anchor,
                                        origin, shell, mapper,
                                    );
                                }
                            }
                        } else if !any_prefix {
                            state.keytip_buffer.clear();
                        }
                    }
                    shell.capture_event(); return;
                }
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// activation (one place for widget + overlay)

fn open_popup(
    state: &mut State,
    kind: PopupKind,
    pos: Point,
    width_hint: f32,
) {
    // Opening a dropdown from inside a panel popup (collapsed group,
    // minimized tab) stacks it: closing the dropdown returns to the
    // panel instead of dropping back to the ribbon.
    let parent = match &state.popup {
        Some(p)
            if matches!(
                p.kind,
                PopupKind::OverflowGroup { .. } | PopupKind::MinimizedTab(_)
            ) =>
        {
            // opened from the panel itself
            Some(Box::new(p.clone()))
        }
        // opened from a dropdown inside the panel: keep the panel
        Some(p) => p.parent.clone(),
        None => None,
    };
    state.popup = Some(Popup {
        kind,
        origin: pos,
        width_hint,
        scroll: 0,
        hover: None,
        size: Size::ZERO,
        parent,
    });
    state.keytips = false;
    state.keytip_buffer.clear();
}

/// What kind of dropdown a command opens.
enum DropKind {
    Menu,
    Font,
    Gallery,
    Color,
}

fn drop_kind(fw: &RibbonFramework, cmd: u32) -> DropKind {
    fn find(controls: &[ControlSpec], cmd: u32) -> Option<DropKind> {
        for c in controls {
            match c {
                ControlSpec::FontControl(b)
                    if matches!(&b.command, Some(crate::ribbon::spec::CommandRef::Id(id)) if *id == cmd) =>
                {
                    return Some(DropKind::Font)
                }
                ControlSpec::InRibbonGallery(g)
                    if g.popup
                        && matches!(&g.command, Some(crate::ribbon::spec::CommandRef::Id(id)) if *id == cmd) =>
                {
                    return Some(DropKind::Gallery)
                }
                ControlSpec::ColorPicker(c)
                    if matches!(&c.command, Some(crate::ribbon::spec::CommandRef::Id(id)) if *id == cmd) =>
                {
                    return Some(DropKind::Color)
                }
                ControlSpec::ControlGroup { controls } => {
                    if let Some(k) = find(controls, cmd) {
                        return Some(k);
                    }
                }
                _ => {}
            }
        }
        None
    }
    if let Ok(spec) = fw.spec() {
        for tab in &spec.tabs {
            for group in &tab.groups {
                if let Some(k) = find(&group.controls, cmd) {
                    return k;
                }
            }
        }
    }
    DropKind::Menu
}

/// Execute the semantic action behind a [`Target`].
///
/// `anchor` is the target node's rectangle in window coordinates,
/// `origin` the ribbon widget's top-left in window coordinates.
pub(crate) fn activate<Message>(
    fw: &RibbonFramework,
    theme: &Theme,
    state: &mut State,
    target: &Target,
    anchor: Rectangle,
    origin: Point,
    shell: &mut Shell<'_, Message>,
    mapper: Option<&dyn Fn(RibbonEvent) -> Message>,
) {
    let view = fw.view();
    let below = Point::new(anchor.x, anchor.y + anchor.height + 1.0);
    match target {
        AppMenu => {
            open_popup(state, PopupKind::AppMenu, below, 0.0);
            fw.view_log(EventType::ApplicationMenuOpened, EventLocation::ApplicationMenu, 0);
        }
        Tab(i) => {
            view.select_tab(*i);
            if let Ok(spec) = fw.spec() {
                let id = spec.tabs.get(*i).map(|t| t.id.clone()).unwrap_or_default();
                publish(shell, mapper, RibbonEvent::TabActivated { tab: id });
            }
            fw.view_log(EventType::TabActivated, EventLocation::Ribbon, 0);
            if view.minimized() {
                let width = state
                    .scene
                    .as_ref()
                    .map(|s| s.size.width)
                    .unwrap_or(800.0);
                open_popup(
                    state,
                    PopupKind::MinimizedTab(*i),
                    Point::new(origin.x, anchor.y + anchor.height - 2.0),
                    width,
                );
            }
        }
        Qat(cmd) | Exec(cmd) | SplitMain(cmd) | Help(cmd) => {
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: None,
                    value: None,
                },
            );
            close_all_popups(state);
        }
        Toggle(cmd) => {
            let cur = fw
                .cached(*cmd, PropertyKey::BooleanValue)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::BooleanValue),
                    value: Some(PropValue::Bool(!cur)),
                },
            );
            close_all_popups(state);
        }
        SplitDrop(cmd) | DropDown(cmd) => {
            let kind = match drop_kind(fw, *cmd) {
                DropKind::Menu => PopupKind::Menu { cmd: *cmd, path: Vec::new() },
                DropKind::Font => PopupKind::Font { cmd: *cmd },
                DropKind::Gallery => PopupKind::Gallery { cmd: *cmd },
                DropKind::Color => PopupKind::Color { cmd: *cmd },
            };
            open_popup(state, kind, below, anchor.width);
            fw.view_log(EventType::MenuOpened, EventLocation::Ribbon, *cmd);
        }
        SubMenu(idx, depth) => {
            if let Some(p) = state.popup.as_mut() {
                match &mut p.kind {
                    PopupKind::Menu { path, .. }
                    | PopupKind::ContextMenu { path, .. } => {
                        path.truncate(*depth);
                        if path.last() == Some(idx) {
                            path.pop();
                        } else {
                            path.push(*idx);
                        }
                    }
                    _ => {}
                }
            }
        }
        MenuItem(cmd) => {
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: None,
                    value: None,
                },
            );
            close_all_popups(state);
            fw.view_log(EventType::CommandExecuted, EventLocation::Ribbon, *cmd);
        }
        RecentItem(cmd, i) => {
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::SelectedItem),
                    value: Some(PropValue::UInt(*i as u32)),
                },
            );
            close_all_popups(state);
        }
        RecentPin(cmd, i) => {
            // the handler toggles the pin state of row `i`
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::Pinned),
                    value: Some(PropValue::UInt(*i as u32)),
                },
            );
        }
        ComboMain(cmd) => {
            let text = fw
                .cached(*cmd, PropertyKey::StringValue)
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            state.edit = Some(EditState {
                cmd: *cmd,
                text,
                numeric: false,
            });
        }
        ComboDrop(cmd) => {
            open_popup(state, PopupKind::Combo { cmd: *cmd }, below, anchor.width);
            fw.view_log(EventType::MenuOpened, EventLocation::Ribbon, *cmd);
        }
        ComboItem(cmd, i) => {
            let label = fw
                .cached(*cmd, PropertyKey::ItemsSource)
                .and_then(|v| v.as_collection().cloned())
                .and_then(|c| c.lock().ok()?.label_of(*i))
                .unwrap_or_default();
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::SelectedItem),
                    value: Some(PropValue::UInt(*i as u32)),
                },
            );
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::StringValue),
                    value: Some(PropValue::Str(label)),
                },
            );
            close_all_popups(state);
        }
        SpinnerUp(cmd) | SpinnerDown(cmd) => {
            let sign = if matches!(target, SpinnerUp(_)) { 1.0 } else { -1.0 };
            let value = fw
                .cached(*cmd, PropertyKey::DecimalValue)
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let increment = fw
                .cached(*cmd, PropertyKey::Increment)
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let min = fw
                .cached(*cmd, PropertyKey::MinValue)
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::MIN);
            let max = fw
                .cached(*cmd, PropertyKey::MaxValue)
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::MAX);
            let new = (value + sign * increment).clamp(min, max);
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::DecimalValue),
                    value: Some(PropValue::Decimal(new)),
                },
            );
        }
        SpinnerText(cmd) => {
            let text = format_spinner(fw, *cmd);
            state.edit = Some(EditState {
                cmd: *cmd,
                text,
                numeric: true,
            });
        }
        GalleryItem { cmd, index, popup } => {
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::SelectedItem),
                    value: Some(PropValue::UInt(*index as u32)),
                },
            );
            if *popup {
                close_all_popups(state);
            }
        }
        GalleryScrollH(cmd, delta) => {
            let entry = state.gallery_scroll.entry(*cmd).or_insert(0);
            *entry = (*entry as i32 + delta).max(0) as usize;
        }
        GalleryScrollV(_, delta) => {
            if let Some(p) = state.popup.as_mut() {
                p.scroll = (p.scroll as i32 + delta).max(0) as usize;
            }
        }
        FontFamily(cmd, family) => {
            let mut props = fw
                .cached(*cmd, PropertyKey::FontProperties)
                .and_then(|v| v.as_font().cloned())
                .unwrap_or_default();
            props.family = Some(family.clone());
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::FontProperties),
                    value: Some(PropValue::Font(Box::new(props))),
                },
            );
        }
        FontSize(cmd, size) => {
            let mut props = fw
                .cached(*cmd, PropertyKey::FontProperties)
                .and_then(|v| v.as_font().cloned())
                .unwrap_or_default();
            props.size = size.parse().ok();
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::FontProperties),
                    value: Some(PropValue::Font(Box::new(props))),
                },
            );
        }
        FontToggle(cmd, which) => {
            let mut props = fw
                .cached(*cmd, PropertyKey::FontProperties)
                .and_then(|v| v.as_font().cloned())
                .unwrap_or_default();
            props.toggle(*which);
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::FontProperties),
                    value: Some(PropValue::Font(Box::new(props))),
                },
            );
        }
        FontGrow(cmd) | FontShrink(cmd) => {
            let grow = matches!(target, FontGrow(_));
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::FontPropertiesDeltaSize),
                    value: Some(PropValue::UInt(
                        if grow {
                            crate::ribbon::enums::FontDeltaSize::Grow as u32
                        } else {
                            crate::ribbon::enums::FontDeltaSize::Shrink as u32
                        },
                    )),
                },
            );
        }
        ColorMain(cmd) => {
            let color = fw
                .cached(*cmd, PropertyKey::Color)
                .and_then(|v| v.as_color())
                .unwrap_or(0xC00000);
            publish(
                shell,
                mapper,
                RibbonEvent::Execute {
                    command: *cmd,
                    verb: ExecutionVerb::Execute,
                    key: Some(PropertyKey::Color),
                    value: Some(PropValue::UInt(color)),
                },
            );
            close_all_popups(state);
        }
        ColorDrop(cmd) => {
            open_popup(state, PopupKind::Color { cmd: *cmd }, below, anchor.width);
        }
        ColorSwatch { cmd, sel } => {
            match sel {
                SwatchSel::Automatic => {
                    publish(
                        shell,
                        mapper,
                        RibbonEvent::Execute {
                            command: *cmd,
                            verb: ExecutionVerb::Execute,
                            key: Some(PropertyKey::ColorType),
                            value: Some(PropValue::UInt(
                                crate::ribbon::enums::SwatchColorType::Automatic as u32,
                            )),
                        },
                    );
                }
                SwatchSel::NoColor => {
                    publish(
                        shell,
                        mapper,
                        RibbonEvent::Execute {
                            command: *cmd,
                            verb: ExecutionVerb::Execute,
                            key: Some(PropertyKey::ColorType),
                            value: Some(PropValue::UInt(
                                crate::ribbon::enums::SwatchColorType::NoColor as u32,
                            )),
                        },
                    );
                }
                SwatchSel::MoreColors => {
                    publish(
                        shell,
                        mapper,
                        RibbonEvent::Execute {
                            command: *cmd,
                            verb: ExecutionVerb::Execute,
                            key: None,
                            value: None,
                        },
                    );
                }
                other => {
                    let rgb = resolve_swatch(fw, *cmd, state, other);
                    if let Some(rgb) = rgb {
                        state
                            .color_recent
                            .entry(*cmd)
                            .or_default()
                            .retain(|c| *c != rgb);
                        let recents = state.color_recent.entry(*cmd).or_default();
                        recents.insert(0, rgb);
                        recents.truncate(10);
                        publish(
                            shell,
                            mapper,
                            RibbonEvent::Execute {
                                command: *cmd,
                                verb: ExecutionVerb::Execute,
                                key: Some(PropertyKey::Color),
                                value: Some(PropValue::UInt(rgb)),
                            },
                        );
                    }
                }
            }
            close_all_popups(state);
        }
        Overflow => {
            open_popup(state, PopupKind::Overflow, below, 0.0);
        }
        OverflowGroup(tab, group) => {
            // A popup opened from the overflow « menu flies out to the
            // right of its row; a collapsed (popup-scale) group button
            // drops down below the ribbon like a gallery.
            let is_overflow_row = matches!(
                &state.popup,
                Some(p) if matches!(p.kind, PopupKind::Overflow)
            );
            let pos = if is_overflow_row {
                Point::new(anchor.x + anchor.width + 2.0, anchor.y)
            } else {
                below
            };
            open_popup(
                state,
                PopupKind::OverflowGroup {
                    tab: *tab,
                    group: *group,
                },
                pos,
                0.0,
            );
        }
        QatMenu => {
            open_popup(state, PopupKind::QatMenu, below, 0.0);
        }
        QatMenuItem(cmd) => {
            let mut qat = view.qat_commands();
            if let Some(pos) = qat.iter().position(|c| c == cmd) {
                qat.remove(pos);
            } else {
                qat.push(*cmd);
            }
            view.set_qat_commands(qat.clone());
            publish(shell, mapper, RibbonEvent::QatChanged { commands: qat });
        }
        QatDockToggle => {
            let dock = if view.qat_dock() == crate::ribbon::enums::ControlDock::Top {
                crate::ribbon::enums::ControlDock::Bottom
            } else {
                crate::ribbon::enums::ControlDock::Top
            };
            view.set_qat_dock(dock);
            close_all_popups(state);
        }
        Minimize => {
            let minimized = !view.minimized();
            view.set_minimized(minimized);
            close_all_popups(state);
            publish(shell, mapper, RibbonEvent::MinimizedChanged { minimized });
            fw.view_log(
                if minimized {
                    EventType::RibbonMinimized
                } else {
                    EventType::RibbonExpanded
                },
                EventLocation::Ribbon,
                0,
            );
        }
    }
    // Popup openings and view-state changes (minimize, QAT dock) must be
    // visible in the SAME event cycle: capturing the event drops the
    // cached overlay, so revalidate the layout to rebuild it now, and ask
    // for a frame — nothing else schedules one after a captured click.
    shell.invalidate_layout();
    shell.request_redraw();
    let _ = theme;
}

fn resolve_swatch(
    fw: &RibbonFramework,
    cmd: u32,
    state: &State,
    sel: &SwatchSel,
) -> Option<u32> {
    let theme_colors = fw
        .cached(cmd, PropertyKey::ThemeColors)
        .and_then(|v| match v {
            PropValue::ColorList(c) => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| crate::ribbon::theme::THEME_COLORS.to_vec());
    let standard_colors = fw
        .cached(cmd, PropertyKey::StandardColors)
        .and_then(|v| match v {
            PropValue::ColorList(c) => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| crate::ribbon::theme::STANDARD_COLORS.to_vec());
    match sel {
        SwatchSel::Theme(i) => theme_colors.get(*i).copied(),
        SwatchSel::Standard(i) => standard_colors.get(*i).copied(),
        SwatchSel::Recent(i) => state
            .color_recent
            .get(&cmd)
            .and_then(|c| c.get(*i).copied()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// scene drawing

/// Draw a scene at `origin` (widget position within the window).
pub fn draw_scene<Renderer>(
    scene: &Scene,
    renderer: &mut Renderer,
    origin: Point,
    viewport: &Rectangle,
    theme: &Theme,
) where
    Renderer: iced::advanced::Renderer
        + text::Renderer
        + svg::Renderer
        + iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>,
{
    renderer.with_layer(*viewport, |renderer| {
        for node in &scene.nodes {
            let rect = Rectangle {
                x: node.rect.x + origin.x,
                y: node.rect.y + origin.y,
                ..node.rect
            };
            match &node.cmd {
                Cmd::Quad {
                    fill,
                    border,
                    radius,
                } => {
                    let background: Background = match fill {
                        Fill::Color(c) => Background::Color(*c),
                        Fill::Gradient(top, bottom) => {
                            Background::Gradient(iced::Gradient::Linear(
                                iced::gradient::Linear::new(
                                    std::f32::consts::FRAC_PI_2,
                                )
                                .add_stop(0.0, *top)
                                .add_stop(1.0, *bottom),
                            ))
                        }
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: rect,
                            border: iced::Border {
                                color: border.map(|(c, _)| c).unwrap_or(Color::TRANSPARENT),
                                width: border.map(|(_, w)| w).unwrap_or(0.0).into(),
                                radius: (*radius).into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: true,
                        },
                        background,
                    );
                }
                Cmd::Text {
                    text: content,
                    size,
                    color,
                    halign,
                    valign,
                    bold,
                } => {
                    if content.is_empty() {
                        continue;
                    }
                    let font = renderer.default_font();
                    // never let a label spill outside its box
                    let clip = rect.expand(2.0).intersection(viewport).unwrap_or(rect);
                    // iced's cached-text pipeline treats `position` as the
                    // alignment anchor: for Center/Right (and Center/
                    // Bottom vertical) the point must be the box edge/
                    // center the text should align to.
                    let anchor = Point::new(
                        match halign {
                            iced::alignment::Horizontal::Left => rect.x,
                            iced::alignment::Horizontal::Center => {
                                rect.x + rect.width / 2.0
                            }
                            iced::alignment::Horizontal::Right => rect.x + rect.width,
                        },
                        match valign {
                            iced::alignment::Vertical::Top => rect.y,
                            iced::alignment::Vertical::Center => {
                                rect.y + rect.height / 2.0
                            }
                            iced::alignment::Vertical::Bottom => {
                                rect.y + rect.height
                            }
                        },
                    );
                    let passes: &[f32] = if *bold { &[0.0, 0.4] } else { &[0.0] };
                    for dx in passes {
                        renderer.fill_text(
                            text::Text {
                                content: content.clone(),
                                bounds: rect.size(),
                                size: (*size).into(),
                                line_height: text::LineHeight::Absolute(
                                    (*size * 1.2).into(),
                                ),
                                font,
                                align_x: (*halign).into(),
                                align_y: *valign,
                                shaping: text::Shaping::Basic,
                                wrapping: text::Wrapping::None,
                            },
                            Point::new(anchor.x + dx, anchor.y),
                            *color,
                            clip,
                        );
                    }
                }
                Cmd::Svg {
                    image,
                    tint,
                    opacity,
                } => {
                    let clip = rect.intersection(viewport).unwrap_or(rect);
                    match image.handle(rect.height as u32) {
                        crate::ribbon::image::IconHandle::Svg(handle) => {
                            renderer.draw_svg(
                                svg::Svg {
                                    handle,
                                    color: *tint,
                                    rotation: iced::Radians(0.0),
                                    opacity: *opacity,
                                },
                                rect,
                                clip,
                            );
                        }
                        crate::ribbon::image::IconHandle::Raster(handle) => {
                            renderer.draw_image(
                                iced::advanced::image::Image::new(handle)
                                    .opacity(*opacity)
                                    .snap(true),
                                rect,
                                clip,
                            );
                        }
                    }
                }
                Cmd::KeytipBadge { text: k, anchor } => {
                    let badge = Rectangle::new(
                        Point::new(anchor.x + origin.x, anchor.y + origin.y),
                        Size::new(17.0, 13.0),
                    );
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: badge,
                            border: iced::Border {
                                color: crate::ribbon::theme::shade(theme.keytip_bg, 0.7),
                                width: 1.0_f32.into(),
                                radius: theme.button_radius.into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: true,
                        },
                        Background::Color(theme.keytip_bg),
                    );
                    renderer.fill_text(
                        text::Text {
                            content: k.clone(),
                            bounds: badge.size(),
                            size: 9.0.into(),
                            line_height: text::LineHeight::Absolute(11.0.into()),
                            font: renderer.default_font(),
                            align_x: iced::alignment::Horizontal::Center.into(),
                            align_y: iced::alignment::Vertical::Center,
                            shaping: text::Shaping::Basic,
                            wrapping: text::Wrapping::None,
                        },
                        badge.position(),
                        theme.keytip_text,
                        *viewport,
                    );
                }
                Cmd::Hit => {}
            }
        }
    });
}

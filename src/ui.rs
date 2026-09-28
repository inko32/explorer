//! View composition + Win8 skin. iced widgets, styled to the UI-SPEC.

pub mod skin {
    use iced::color;

    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    pub struct Skin {
        pub accent: iced::Color,
        pub accent_dark: iced::Color,
        pub titlebar_inactive: iced::Color,
        pub window_bg: iced::Color,
        pub ribbon_bg: iced::Color,
        pub nav_bg: iced::Color,
        pub toolbar_bg: iced::Color,
        pub status_bg: iced::Color,
        pub col_header_bg: iced::Color,
        pub hover: iced::Color,
        pub pressed: iced::Color,
        pub sel: iced::Color,
        pub sel_border: iced::Color,
        pub sep: iced::Color,
        pub text: iced::Color,
        pub subtle: iced::Color,
        pub disabled: iced::Color,
        pub folder: iced::Color,
        pub folder_edge: iced::Color,
        pub close_hover: iced::Color,
        pub address_border: iced::Color,
    }

    pub const LIGHT: Skin = Skin {
        accent: color!(0x0078D7),
        accent_dark: color!(0x00559B),
        titlebar_inactive: color!(0xC8C8C8),
        window_bg: color!(0xFFFFFF),
        ribbon_bg: color!(0xFFFFFF),
        nav_bg: color!(0xFFFFFF),
        toolbar_bg: color!(0xFFFFFF),
        status_bg: color!(0xFFFFFF),
        col_header_bg: color!(0xF7F7F7),
        hover: color!(0xE5F3FF),
        pressed: color!(0xA4CEF9),
        sel: color!(0xCCE8FF),
        sel_border: color!(0x0078D7),
        sep: color!(0xDADBDC),
        text: color!(0x1A1A1A),
        subtle: color!(0x828282),
        disabled: color!(0xA0A0A0),
        folder: color!(0xF6C356),
        folder_edge: color!(0xC88E1E),
        close_hover: color!(0xE81123),
        address_border: color!(0xA9A9A9),
    };

    pub const DARK: Skin = Skin {
        accent: color!(0x0078D7),
        accent_dark: color!(0x004A86),
        titlebar_inactive: color!(0x3A3A3A),
        window_bg: color!(0x1E1E1E),
        ribbon_bg: color!(0x2D2D2D),
        nav_bg: color!(0x252526),
        toolbar_bg: color!(0x2D2D2D),
        status_bg: color!(0x252526),
        col_header_bg: color!(0x2B2B2C),
        hover: color!(0x3A4046),
        pressed: color!(0x4A525A),
        sel: color!(0x2F5480),
        sel_border: color!(0x0078D7),
        sep: color!(0x454545),
        text: color!(0xE8E8E8),
        subtle: color!(0xA0A0A0),
        disabled: color!(0x6E6E6E),
        folder: color!(0xC89B3C),
        folder_edge: color!(0x8A6618),
        close_hover: color!(0xE81123),
        address_border: color!(0x555555),
    };

    impl Skin {
        pub fn palette(self) -> iced::theme::Palette {
            iced::theme::Palette {
                background: self.window_bg,
                text: self.text,
                primary: self.accent,
                success: color!(0x1F9E3E),
                warning: color!(0xFFB900),
                danger: self.close_hover,
            }
        }
    }
}

use std::path::{Path, PathBuf};

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space,
};
use iced::{Color, Element, Length};

use crate::fs::{self, Entry};
use crate::model::{self, Explorer, Message, ViewMode};
use crate::ribbon::{self, GroupDef, TabDef};

/// Edge-resize directions for the self-drawn border.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

const MARGIN: f32 = 5.0;

/// An invisible resize zone aligned to one window edge/corner. The outer
/// container spans the whole window (transparent to events); only the inner
/// strip/corner reacts.
fn resize_zone(
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
    c.into()
}

/// Invisible resize ring around the window edges (custom border mode).
fn resize_frame() -> Element<'static, Message> {
    use iced::Alignment;
    let m = Length::Fixed(MARGIN);
    let c = Length::Fixed(MARGIN * 2.0);
    let fill = Length::Fill;
    stack![
        resize_zone(ResizeDir::N, fill, m, None, None),
        resize_zone(ResizeDir::S, fill, m, None, Some(Alignment::End)),
        resize_zone(ResizeDir::W, m, fill, None, None),
        resize_zone(ResizeDir::E, m, fill, Some(Alignment::End), None),
        // corners on top (they overlap the edge strips)
        resize_zone(ResizeDir::Nw, c, c, None, None),
        resize_zone(ResizeDir::Ne, c, c, Some(Alignment::End), None),
        resize_zone(ResizeDir::Sw, c, c, None, Some(Alignment::End)),
        resize_zone(ResizeDir::Se, c, c, Some(Alignment::End), Some(Alignment::End)),
    ]
    .into()
}

pub fn view(state: &Explorer) -> Element<'_, Message> {
    let skin = if state.dark { skin::DARK } else { skin::LIGHT };
    let mut root = column![];
    if state.self_border {
        root = root.push(titlebar(state, skin));
    }
    root = root.push(ribbon(state, skin));
    root = root.push(address_bar(state, skin));
    root = root.push(row![nav_pane(state, skin), file_view(state, skin)].height(Length::Fill));
    root = root.push(statusbar(state, skin));
    if state.self_border {
        stack![root, resize_frame()].into()
    } else {
        root.into()
    }
}

fn bg(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: Some(color.into()),
        ..Default::default()
    }
}

fn icon(name: &str, size: f32, skin: skin::Skin) -> Element<'static, Message> {
    crate::icon::view(name, size, &skin)
}

// ---------------- title bar ----------------

fn titlebar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let caption = |icon_name: &'static str, msg: Message, hover_bg: Color| {
        button(crate::icon::view(icon_name, 15.0, &skin))
            .width(Length::Fixed(46.0))
            .height(Length::Fill)
            .style(move |_t, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(hover_bg.into()),
                    button::Status::Pressed => Some(hover_bg.scale_alpha(0.7).into()),
                    _ => None,
                },
                text_color: Color::WHITE,
                border: Default::default(),
                shadow: Default::default(),
                snap: true,
            })
            .on_press(msg)
    };

    // Win8: contextual tab labels are colored blocks on the title bar,
    // right-aligned before the caption buttons. Title centers in the
    // space left of them (so it shifts when they appear).
    let mut ctx_blocks = row![].height(Length::Fixed(30.0));
    for tab in visible_tabs(state) {
        let Some(accent) = tab.color.as_deref().and_then(ribbon::color_by_name) else { continue };
        let active = state.active_tab == tab.id;
        let width = tab_width(&tab.label);
        let block = container(
            text(tab.label.clone())
                .size(12.0)
                .color(accent.scale_alpha(0.9))
                .width(Length::Shrink),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(30.0))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(30.0))
        .style(move |_| container::Style {
            background: Some(accent.scale_alpha(if active { 0.5 } else { 0.3 }).into()),
            border: iced::Border {
                color: accent.scale_alpha(if active { 1.0 } else { 0.7 }),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
        ctx_blocks = ctx_blocks.push(
            mouse_area(block).on_press(Message::BlockPress(tab.id.clone())),
        );
    }

    // Win8: the contextual blocks sit exactly above their tab-row stubs
    // (which start right after the main tabs); the title centers between
    // the icon and the blocks.
    let title_zone = (main_tabs_width(state) - 28.0).max(60.0);
    let title = container(
        text(state.title())
            .size(12.0)
            .color(Color::WHITE)
            .width(Length::Shrink),
    )
    .width(Length::Fixed(title_zone))
    .height(Length::Fill)
    .center_x(Length::Fixed(title_zone))
    .center_y(Length::Fill);
    let drag_title = mouse_area(title)
        .on_press(Message::TitlebarPress)
        .on_double_click(Message::TitlebarDoubleClick);

    container(
        row![
            container(icon("folder", 16.0, skin))
                .width(Length::Fixed(28.0))
                .height(Length::Fill)
                .center_x(Length::Fixed(28.0))
                .center_y(Length::Fill),
            drag_title,
            ctx_blocks,
            mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(30.0)))
                .on_press(Message::TitlebarPress)
                .on_double_click(Message::TitlebarDoubleClick),
            caption("minimize", Message::Minimize, Color::from_rgba(1.0, 1.0, 1.0, 0.18)),
            caption("maximize", Message::Maximize, Color::from_rgba(1.0, 1.0, 1.0, 0.18)),
            caption("close", Message::Close, skin.close_hover),
        ]
        .height(Length::Fixed(30.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(30.0))
    .style(bg(skin.accent))
    .into()
}

// ---------------- ribbon ----------------

fn ribbon(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    container(if state.ribbon_collapsed {
        column![tab_row(state, skin)]
    } else {
        column![tab_row(state, skin), ribbon_body(state, skin)]
    })
    .width(Length::Fill)
    .height(Length::Fixed(if state.ribbon_collapsed { 28.0 } else { 122.0 }))
    .style(bg(skin.ribbon_bg))
    .into()
}

fn tab_button(
    label: &str,
    width: f32,
    active: bool,
    is_file: bool,
    accent: Option<Color>,
    highlight_bar: bool,
    msg: Message,
    skin: skin::Skin,
) -> Element<'static, Message> {
    // Win8 contextual tabs: colored label + colored highlight bar at the
    // top of the tab, touching the title bar (only with the self-drawn
    // border — native decorations have no title area for the bar).
    let text_color = if is_file {
        Color::WHITE
    } else if active {
        skin.text
    } else {
        skin.subtle
    };
    let text_color = accent.map(|a| if active { a } else { a.scale_alpha(0.75) }).unwrap_or(text_color);
    let font = if active && accent.is_none() {
        iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }
    } else {
        iced::Font::default()
    };
    let btn = button(text(label.to_string()).size(12.0).color(text_color).font(font))
        .width(Length::Fixed(width))
        .height(Length::Fixed(28.0))
        .style(move |_t, status| {
            let bg = if is_file {
                Some(skin.accent)
            } else {
                match status {
                    button::Status::Active if active => Some(skin.ribbon_bg),
                    button::Status::Hovered if !active => Some(skin.hover),
                    _ => None,
                }
            };
            let border_color = match accent {
                Some(a) if active => a,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: bg.map(Into::into),
                text_color,
                border: iced::Border {
                    color: border_color,
                    width: if border_color == Color::TRANSPARENT { 0.0 } else { 1.0 },
                    radius: 0.0.into(),
                },
                shadow: Default::default(),
                snap: true,
            }
        })
        .on_press(msg);
    match (accent, highlight_bar) {
        (Some(a), true) => column![
            container(Space::new().width(Length::Fixed(width)).height(Length::Fill))
                .width(Length::Fixed(width))
                .height(Length::Fixed(if active { 3.0 } else { 2.0 }))
                .style(move |_| container::Style {
                    background: Some(a.scale_alpha(if active { 1.0 } else { 0.45 }).into()),
                    ..Default::default()
                }),
            btn,
        ]
        .width(Length::Fixed(width))
        .into(),
        _ => btn.into(),
    }
}

/// Tabs visible right now: main tabs + contextual tabs whose `when` matches.

fn tab_width(label: &str) -> f32 {
    20.0 + label.chars().count() as f32 * 7.5
}

/// Width of the main tabs region in the tab row — contextual title-bar
/// blocks align directly above their stubs.
fn main_tabs_width(state: &Explorer) -> f32 {
    state.ribbon.tabs.iter().map(|t| tab_width(&t.label)).sum()
}

fn visible_tabs(state: &Explorer) -> Vec<&TabDef> {
    let ctx = state.context();
    state
        .ribbon
        .tabs
        .iter()
        .chain(
            state
                .ribbon
                .contextual_tabs
                .iter()
                .filter(|t| t.when.as_deref().map(|w| ribbon::eval_when(w, ctx)).unwrap_or(true)),
        )
        .collect()
}

fn tab_row(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    // ribbon minimize chevron (Win8: also double-click a tab)
    let collapse = button(icon(if state.ribbon_collapsed { "chevron_down" } else { "up" }, 14.0, skin))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .style(move |_t, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(skin.hover.into()),
                _ => None,
            },
            text_color: skin.text,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(Message::ToggleRibbonCollapse);
    let help = button(icon("help", 16.0, skin))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .style(|_t, _status| button::Style {
            background: None,
            text_color: Color::TRANSPARENT,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        });
    let mut tabs_row = row![].height(Length::Fixed(28.0));
    for tab in visible_tabs(state) {
        let width = tab_width(&tab.label);
        let accent = tab.color.as_deref().and_then(ribbon::color_by_name);
        if let Some(accent) = accent.filter(|_| state.self_border) {
            // Win8: connecting stub under the title-bar block, labeled with
            // the first group's caption.
            let active = state.active_tab == tab.id;
            let stub_label = tab.groups.first().map(|g| g.label.clone()).unwrap_or_default();
            let stub = container(
                text(stub_label)
                    .size(12.0)
                    .color(accent.scale_alpha(0.9))
                    .width(Length::Shrink),
            )
            .width(Length::Fixed(width))
            .height(Length::Fixed(28.0))
            .center_x(Length::Fixed(width))
            .center_y(Length::Fixed(28.0))
            .style(move |_| container::Style {
                background: Some(skin.ribbon_bg.into()),
                border: iced::Border {
                    color: accent.scale_alpha(if active { 1.0 } else { 0.55 }),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            });
            tabs_row = tabs_row.push(
                mouse_area(stub).on_press(Message::SetTab(tab.id.clone())),
            );
        } else {
            tabs_row = tabs_row.push(tab_button(
                &tab.label,
                width,
                state.active_tab == tab.id,
                tab.kind.as_deref() == Some("file"),
                accent,
                state.self_border,
                Message::SetTab(tab.id.clone()),
                skin,
            ));
        }
    }
    container(
        row![
            tabs_row,
            Space::new().width(Length::Fill),
            container(collapse).height(Length::Fixed(28.0)).center_y(Length::Fixed(28.0)),
            container(help).height(Length::Fixed(28.0)).center_y(Length::Fixed(28.0)).padding([0.0, 6.0]),
        ]
        .height(Length::Fixed(28.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(28.0))
    .into()
}

fn cmd_style(
    enabled: bool,
    skin: skin::Skin,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    move |_t, status| {
        let (bgc, tc) = if !enabled {
            (None, skin.disabled)
        } else {
            match status {
                button::Status::Hovered => (Some(skin.hover), skin.text),
                button::Status::Pressed => (Some(skin.pressed), skin.text),
                _ => (None, skin.text),
            }
        };
        button::Style {
            background: bgc.map(Into::into),
            text_color: tc,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        }
    }
}

fn big_cmd(
    icon_el: Element<'static, Message>,
    label: &str,
    enabled: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let content = column![
        icon_el,
        text(label.to_string()).size(11.0).color(if enabled { skin.text } else { skin.disabled }),
    ]
    .spacing(2)
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(56.0));
    let btn = button(content)
        .height(Length::Fixed(77.0))
        .style(cmd_style(enabled, skin));
    match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    }
}


fn small_cmd_active(
    icon_el: Element<'static, Message>,
    label: &str,
    width: f32,
    enabled: bool,
    active: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let content = row![
        icon_el,
        text(label.to_string()).size(11.0).color(if enabled { skin.text } else { skin.disabled }),
    ]
    .spacing(5)
    .align_y(iced::Alignment::Center);
    let btn = button(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(22.0))
        .style(move |_t, status| {
            let mut style = cmd_style(enabled, skin)(_t, status);
            if active {
                style.background = Some(skin.pressed.into());
                style.border = iced::Border {
                    color: skin.accent,
                    width: 1.0,
                    radius: 0.0.into(),
                };
            }
            style
        });
    match (enabled, msg) {
        (true, Some(m)) => btn.on_press(m).into(),
        _ => btn.into(),
    }
}

fn group(caption: &str, width: f32, content: Element<'static, Message>, skin: skin::Skin) -> Element<'static, Message> {
    let sep = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(76.0)))
        .style(bg(skin.sep))
        .height(Length::Fixed(76.0))
        .center_y(Length::Fixed(79.0));
    row![
        sep,
        column![
            container(content).width(Length::Fixed(width)).height(Length::Fixed(79.0)).padding([4.0, 3.0]),
            container(text(caption.to_string()).size(11.0).color(skin.subtle))
                .width(Length::Fixed(width + 4.0))
                .height(Length::Fixed(15.0))
                .center_x(Length::Fixed(width + 4.0)),
        ],
    ]
    .into()
}

fn ribbon_body(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let Some(tab) = visible_tabs(state).into_iter().find(|t| t.id == state.active_tab) else {
        return Space::new().width(Length::Fill).height(Length::Fixed(94.0)).into();
    };
    let mut groups_row = row![];
    for g in &tab.groups {
        groups_row = groups_row.push(group_from_config(g, state, skin));
    }
    groups_row.into()
}

/// Build one group's content from YAML: big commands stand alone, small
/// commands stack into `columns` columns (sequential chunks).
fn group_from_config(g: &GroupDef, state: &Explorer, skin: skin::Skin) -> Element<'static, Message> {
    let has_sel = !state.selected.is_empty();
    let has_clip = state.clipboard.is_some();
    let has_items = !state.entries.is_empty();
    let mut bigs: Vec<Element<Message>> = Vec::new();
    let mut smalls: Vec<Element<Message>> = Vec::new();
    for cmd in &g.commands {
        let enabled = cmd.enabled.unwrap_or(true)
            && ribbon::command_enabled(&cmd.id, has_sel, has_clip, has_items);
        let active = match cmd.id.as_str() {
            "toggle_border" => state.self_border,
            "toggle_dark" => state.dark,
            "toggle_hidden" => state.show_hidden,
            _ => false,
        };
        let icon_el = crate::icon::view(cmd.icon.as_deref().unwrap_or("file"), if cmd.size.as_deref() == Some("big") { 42.0 } else { 16.0 }, &skin);
        let msg = enabled.then(|| Message::RunCommand(cmd.id.clone()));
        let el = if cmd.size.as_deref() == Some("big") {
            big_cmd(icon_el, &cmd.label, enabled, msg, skin)
        } else {
            small_cmd_active(icon_el, &cmd.label, 100.0, enabled, active, msg, skin)
        };
        if cmd.size.as_deref() == Some("big") {
            bigs.push(el);
        } else {
            smalls.push(el);
        }
    }
    let columns = g.columns.unwrap_or(1).max(1);
    let per_col = smalls.len().div_ceil(columns).max(1);
    let mut content = row![].spacing(2);
    for b in bigs {
        content = content.push(b);
    }
    let mut it = smalls.into_iter();
    for _ in 0..columns {
        let mut col = column![].spacing(1);
        for _ in 0..per_col {
            if let Some(s) = it.next() {
                col = col.push(s);
            }
        }
        content = content.push(col);
    }
    group(&g.label.clone(), g.width.unwrap_or(110.0), content.into(), skin)
}

// ---------------- address bar ----------------

fn nav_circle(icon_name: &'static str, enabled: bool, msg: Message, skin: skin::Skin) -> Element<'static, Message> {
    let btn = button(crate::icon::view(icon_name, 16.0, &skin))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(28.0))
        .style(move |_t, status| button::Style {
            background: match (enabled, status) {
                (true, button::Status::Hovered) => Some(skin.hover.into()),
                (true, button::Status::Pressed) => Some(skin.pressed.into()),
                _ => None,
            },
            text_color: if enabled { skin.accent } else { skin.disabled },
            border: iced::Border {
                color: if enabled { skin.accent } else { skin.sep },
                width: 1.0,
                radius: 14.0.into(),
            },
            shadow: Default::default(),
            snap: true,
        });
    if enabled {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

fn address_bar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let refresh = button(icon("refresh", 16.0, skin))
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .style(move |_t, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(skin.hover.into()),
                _ => None,
            },
            text_color: skin.text,
            border: Default::default(),
            shadow: Default::default(),
            snap: true,
        })
        .on_press(Message::Refresh);

    let mut crumbs = row![].spacing(2).align_y(iced::Alignment::Center);
    let segs: Vec<String> = {
        let mut v = vec!["Computer".to_string()];
        for c in state.cwd.components() {
            if let std::path::Component::Normal(c) = c {
                v.push(c.to_string_lossy().into_owned());
            }
        }
        v
    };
    for (i, seg) in segs.iter().enumerate() {
        let is_last = i == segs.len() - 1;
        let seg_text = text(seg.clone()).size(12.0).color(skin.text);
        let cell: Element<Message> = container(seg_text)
            .padding([2.0, 6.0])
            .style(move |_t: &iced::Theme| container::Style {
                background: None,
                ..Default::default()
            })
            .into();
        let cell_el: Element<Message> = if is_last {
            cell
        } else {
            let target = prefix_of(&state.cwd, i);
            button(cell)
                .style(move |_t, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(skin.hover.into()),
                        button::Status::Pressed => Some(skin.pressed.into()),
                        _ => None,
                    },
                    text_color: skin.text,
                    border: Default::default(),
                    shadow: Default::default(),
                    snap: true,
                })
                .on_press(Message::Navigate(target))
                .into()
        };
        crumbs = crumbs.push(cell_el);
        if !is_last {
            crumbs = crumbs.push(icon("chevron_right", 9.0, skin));
        }
    }

    let crumb_bar = container(
        row![crumbs, Space::new().width(Length::Fill), refresh]
            .align_y(iced::Alignment::Center)
            .padding([0.0, 4.0]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(26.0))
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: skin.address_border,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    });

    let search = container(
        row![
            icon("search", 14.0, skin),
            text(format!("Search {}", state.title()))
                .size(12.0)
                .color(skin.disabled),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(26.0))
    .padding([0.0, 8.0])
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.window_bg.into()),
        border: iced::Border {
            color: skin.address_border,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    });

    container(
        row![
            nav_circle("back", !state.back_is_empty(), Message::Back, skin),
            nav_circle("forward", !state.forward_is_empty(), Message::Forward, skin),
            nav_circle("up", state.cwd.parent().is_some(), Message::Up, skin),
            crumb_bar,
            search,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .padding(6),
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .style(bg(skin.toolbar_bg))
    .into()
}

// ---------------- nav pane ----------------

fn nav_item(
    label: String,
    icon_name: &'static str,
    selected: bool,
    msg: Option<Message>,
    skin: skin::Skin,
) -> Element<'static, Message> {
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let item: Element<Message> = container(
        row![
            crate::icon::view(icon_name, 16.0, &skin),
            text(label).size(12.0).color(skin.text),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([2.0, 0.0]),
    )
    .padding([3.0, 8.0])
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(bgc.into()),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into();
    match msg {
        Some(m) => mouse_area(item).on_press(m).into(),
        None => item,
    }
}

fn nav_pane(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut list = column![].spacing(0).padding(4);

    list = list.push(
        row![
            icon("star", 14.0, skin),
            text("Favorites").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([2.0, 6.0]),
    );
    let mut favs: Vec<(&str, &'static str, Option<PathBuf>)> = vec![
        ("Home", "home", home.clone()),
        ("Desktop", "desktop", home.as_ref().map(|h| h.join("Desktop"))),
        ("Documents", "documents", home.as_ref().map(|h| h.join("Documents"))),
        ("Downloads", "downloads", home.as_ref().map(|h| h.join("Downloads"))),
        ("Recent places", "recent", None),
    ];
    for (label, handle, target) in favs.drain(..) {
        let selected = target.as_deref() == Some(state.cwd.as_path());
        let msg = target.map(Message::Navigate);
        list = list.push(nav_item(label.to_string(), handle, selected, msg, skin));
    }

    list = list.push(
        row![
            icon("computer", 14.0, skin),
            text("This PC").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item(
        "Computer".to_string(),
        "computer",
        state.cwd == Path::new("/"),
        Some(Message::Navigate(PathBuf::from("/"))),
        skin,
    ));
    for v in &state.volumes {
        let selected = state.cwd == v.mount;
        let mount = v.mount.clone();
        list = list.push(nav_item(
            v.name.clone(),
            "drive",
            selected,
            Some(Message::Navigate(mount)),
            skin,
        ));
    }

    list = list.push(
        row![
            icon("network", 14.0, skin),
            text("Network").size(12.0).color(skin.subtle),
        ]
        .spacing(6)
        .padding([4.0, 6.0]),
    );
    list = list.push(nav_item("Network".to_string(), "network", false, None, skin));

    container(list)
        .width(Length::Fixed(210.0))
        .height(Length::Fill)
        .style(bg(skin.nav_bg))
        .into()
}

// ---------------- file view ----------------

fn file_view(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let header = container(
        row![
            container(text("Name").size(11.0).color(skin.subtle))
                .width(Length::Fill)
                .padding([4.0, 8.0]),
            container(text("Date modified").size(11.0).color(skin.subtle))
                .width(Length::Fixed(170.0))
                .padding([4.0, 8.0]),
            container(text("Type").size(11.0).color(skin.subtle))
                .width(Length::Fixed(140.0))
                .padding([4.0, 8.0]),
            container(text("Size").size(11.0).color(skin.subtle))
                .width(Length::Fixed(90.0))
                .padding([4.0, 8.0]),
        ]
        .height(Length::Fixed(24.0))
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(move |_t: &iced::Theme| container::Style {
        background: Some(skin.col_header_bg.into()),
        border: iced::Border {
            color: skin.sep,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    let body: Element<Message> = if state.loading {
        container(
            container(text("Loading…").size(12.0).color(skin.subtle))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        match state.view {
        ViewMode::Details => {
            // Virtualized: build only the rows in view (+overscan), with
            // spacers preserving total scroll height (D13).
            const ROW_H: f32 = 22.0;
            let total = state.entries.len();
            let view_rows = (state.viewport_h / ROW_H).ceil() as usize + 4;
            let first = ((state.scroll_y / ROW_H).floor() as usize).saturating_sub(2);
            let last = (first + view_rows).min(total);
            let mut rows = column![].spacing(0);
            if first > 0 {
                rows = rows.push(Space::new().height(Length::Fixed(first as f32 * ROW_H)));
            }
            for i in first..last {
                if let Some(e) = state.entries.get(i) {
                    rows = rows.push(file_row(i, e, state, skin));
                }
            }
            if last < total {
                rows = rows.push(Space::new().height(Length::Fixed((total - last) as f32 * ROW_H)));
            }
            scrollable(rows)
                .height(Length::Fill)
                .on_scroll(|vp| Message::Scrolled {
                    y: vp.absolute_offset().y,
                    height: vp.bounds().height,
                })
                .into()
        }
        ViewMode::Large => {
            let mut grid = row![].spacing(8).padding(8);
            let mut current = column![].spacing(8);
            for (i, e) in state.entries.iter().enumerate() {
                current = current.push(large_cell(i, e, state, skin));
                if (i + 1) % 6 == 0 {
                    grid = grid.push(current);
                    current = column![].spacing(8);
                }
            }
            grid = grid.push(current);
            scrollable(grid).height(Length::Fill).into()
        }
        }
    };

    container(column![header, body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(bg(skin.window_bg))
        .into()
}


fn file_row<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
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

    let name_cell: Element<Message> = if state.renaming.as_ref().map(|(ri, _)| *ri) == Some(i) {
        let buf = state.renaming.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
        text_input("Name", &buf)
            .on_input(Message::RenameInput)
            .on_submit(Message::CommitRename(buf))
            .size(12.0)
            .padding(2)
            .into()
    } else {
        text(e.name.clone()).size(12.0).color(skin.text).into()
    };

    let content = row![
        container(crate::icon::for_entry(e, 16.0, &skin))
            .width(Length::Fixed(26.0))
            .center_y(Length::Fixed(16.0)),
        container(name_cell).width(Length::Fill).padding([2.0, 4.0]),
        container(text(e.modified.map(model::format_date).unwrap_or_default()).size(12.0).color(skin.text))
            .width(Length::Fixed(170.0))
            .padding([2.0, 4.0]),
        container(text(model::type_label(e)).size(12.0).color(skin.text))
            .width(Length::Fixed(140.0))
            .padding([2.0, 4.0]),
        container(
            text(if e.kind == fs::EntryKind::Dir { String::new() } else { model::format_size(e.size) })
                .size(12.0)
                .color(skin.text),
        )
        .width(Length::Fixed(90.0))
        .padding([2.0, 4.0]),
    ]
    .height(Length::Fixed(22.0))
    .align_y(iced::Alignment::Center);

    let styled = container(content)
        .width(Length::Fill)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .on_enter(Message::Hover(Some(i)))
        .on_exit(Message::Hover(None))
        .into()
}

fn large_cell<'a>(i: usize, e: &Entry, state: &Explorer, skin: skin::Skin) -> Element<'a, Message> {
    let selected = state.selected.contains(&i);
    let bgc = if selected { skin.sel } else { Color::TRANSPARENT };
    let border_color = if selected { skin.sel_border } else { Color::TRANSPARENT };
    let content = column![
        crate::icon::for_entry(e, 64.0, &skin),
        text(e.name.clone()).size(11.0).color(skin.text).width(Length::Fixed(96.0)),
    ]
    .spacing(6)
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(104.0))
    .height(Length::Fixed(110.0));
    let styled = container(content)
        .padding(4)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(bgc.into()),
            border: iced::Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    mouse_area(styled)
        .on_press(Message::Select(i))
        .on_double_click(Message::Open(i))
        .into()
}

// ---------------- status bar ----------------

fn statusbar(state: &Explorer, skin: skin::Skin) -> Element<'_, Message> {
    let mut left = format!("{} items", state.entries.len());
    if !state.selected.is_empty() {
        let bytes: u64 = state.selected.iter().filter_map(|&i| state.entries.get(i)).map(|e| e.size).sum();
        left += &format!(
            "    {} selected    {}",
            state.selected.len(),
            model::format_size(bytes)
        );
    }
    if !state.status.is_empty() {
        left += &format!("    {}", state.status);
    }

    let view_btn = |icon_name: &'static str, active: bool, msg: Message| {
        button(crate::icon::view(icon_name, 16.0, &skin))
            .width(Length::Fixed(22.0))
            .height(Length::Fixed(22.0))
            .style(move |_t, status| button::Style {
                background: if active {
                    Some(skin.pressed.into())
                } else {
                    match status {
                        button::Status::Hovered => Some(skin.hover.into()),
                        _ => None,
                    }
                },
                text_color: skin.text,
                border: iced::Border {
                    color: if active { skin.accent } else { Color::TRANSPARENT },
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: Default::default(),
                snap: true,
            })
            .on_press(msg)
    };

    container(
        row![
            text(left).size(11.0).color(skin.subtle),
            Space::new().width(Length::Fill),
            view_btn("view_details", state.view == ViewMode::Details, Message::SetView(ViewMode::Details)),
            view_btn("view_large", state.view == ViewMode::Large, Message::SetView(ViewMode::Large)),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center)
        .padding([0.0, 8.0]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(23.0))
    .style(bg(skin.status_bg))
    .into()
}

fn prefix_of(path: &Path, seg_index: usize) -> PathBuf {
    if seg_index == 0 {
        return PathBuf::from("/");
    }
    let comps: Vec<_> = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .collect();
    let mut out = PathBuf::from("/");
    for c in comps.iter().take(seg_index) {
        out.push(c.as_os_str());
    }
    out
}

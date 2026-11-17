//! Widget runtime state.

use std::collections::HashMap;
use std::time::Instant;

use iced::Point;

use crate::ribbon::prop::FontToggle;

/// Semantic hit target of a scene node.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    AppMenu,
    Tab(usize),
    Qat(u32),
    QatMenu,
    QatMenuItem(u32),
    QatDockToggle,
    Minimize,
    Help(u32),
    /// Plain command button.
    Exec(u32),
    /// Toggle (boolean) command button.
    Toggle(u32),
    /// Split-button main part.
    SplitMain(u32),
    /// Split-button / dropdown arrow part.
    SplitDrop(u32),
    /// Font control / generic dropdown button.
    DropDown(u32),
    /// Leaf row of a menu popup.
    MenuItem(u32),
    /// Row of a menu popup that opens a sub-menu: (row index, depth).
    SubMenu(usize, usize),
    RecentItem(u32, usize),
    RecentPin(u32, usize),
    ComboMain(u32),
    ComboDrop(u32),
    ComboItem(u32, usize),
    SpinnerUp(u32),
    SpinnerDown(u32),
    SpinnerText(u32),
    GalleryItem {
        cmd: u32,
        index: usize,
        popup: bool,
    },
    GalleryScrollH(u32, i32),
    GalleryScrollV(u32, i32),
    FontFamily(u32, String),
    FontSize(u32, String),
    FontToggle(u32, FontToggle),
    /// `UI_PKEY_FontProperties_DeltaSize` = Grow.
    FontGrow(u32),
    /// `UI_PKEY_FontProperties_DeltaSize` = Shrink.
    FontShrink(u32),
    ColorMain(u32),
    ColorDrop(u32),
    ColorSwatch {
        cmd: u32,
        sel: SwatchSel,
    },
    Overflow,
    OverflowGroup(usize, usize),
}

/// Which color a swatch represents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwatchSel {
    Automatic,
    NoColor,
    Rgb(u32),
    Theme(usize),
    Standard(usize),
    Recent(usize),
    MoreColors,
}

/// Active popup.
#[derive(Debug, Clone, PartialEq)]
pub struct Popup {
    pub kind: PopupKind,
    /// Window coordinates of the popup's top-left corner.
    pub origin: Point,
    pub width_hint: f32,
    pub scroll: usize,
    pub hover: Option<Target>,
    /// Last measured popup size (window coordinates).
    pub size: iced::Size,
    /// Panel this popup was opened from (e.g. a collapsed group's
    /// popup): restored when this popup closes, like Windows where a
    /// dropdown opened inside a popup group returns to the group.
    pub parent: Option<Box<Popup>>,
}

/// Armed by a release on a tab (see `State::last_tab_click`).
pub struct TabClickArm {
    pub tab: usize,
    /// Whether `tab` was the selected tab when the click began.
    pub was_selected: bool,
    pub at: Instant,
}

/// Close the current popup, restoring its parent panel (if any).
pub fn close_popup(state: &mut State) {
    if let Some(p) = state.popup.take() {
        state.popup = p.parent.map(|b| *b);
    }
    if state.popup.is_none() {
        state.popup_bounds = None;
    }
}

/// Close the current popup and any panel it was opened from.
pub fn close_all_popups(state: &mut State) {
    state.popup = None;
    state.popup_bounds = None;
}

#[derive(Debug, Clone, PartialEq)]
pub enum PopupKind {
    AppMenu,
    /// Ribbon body of a tab while minimized.
    MinimizedTab(usize),
    /// Dropdown / split-button / context menu; `path` is the open
    /// sub-menu chain.
    Menu { cmd: u32, path: Vec<usize> },
    Combo { cmd: u32 },
    Gallery { cmd: u32 },
    Color { cmd: u32 },
    Font { cmd: u32 },
    Overflow,
    OverflowGroup { tab: usize, group: usize },
    QatMenu,
    ContextMenu { cmd: u32, path: Vec<usize> },
}

/// Inline text editing (spinner / combo text field).
#[derive(Debug, Clone, PartialEq)]
pub struct EditState {
    pub cmd: u32,
    pub text: String,
    pub numeric: bool,
}

/// Mutable widget state stored in the iced widget tree.
pub struct State {
    /// Last laid-out scene (widget-local coordinates).
    pub scene: Option<crate::ribbon::widget::scene::Scene>,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    /// Set when a press closes a tab's own minimized popup: the
    /// matching release is a pure dismiss and must not reactivate the
    /// tab (which would reopen the popup it just closed).
    pub dismiss_tab_on_release: Option<usize>,
    pub hover_since: Option<Instant>,
    /// Armed by a release on a tab. A press on the SAME tab within
    /// the double-click window (no other interaction between) is a
    /// double-click.
    pub last_tab_click: Option<TabClickArm>,
    /// Selected tab when the current tab press began (a release arms
    /// the double-click detector with it, so "was the clicked tab
    /// already active" is judged at click time, not after the first
    /// click switched to it).
    pub cur_tab_press_selected: usize,
    pub hover_pos: Point,
    /// Tooltip currently displayed (command id).
    pub tooltip: Option<u32>,
    /// Rect of the element the tooltip belongs to (window coordinates).
    pub tooltip_anchor: Option<iced::Rectangle>,
    pub popup: Option<Popup>,
    /// Fade-out clock for the popup while the ribbon is inert
    /// (unfocused window; theme.yaml `fade_ms`).
    pub popup_fade_start: Option<Instant>,
    /// Actual on-screen bounds of the open popup overlay (window
    /// coordinates). Differs from `popup.origin/size` when a dropdown
    /// is shown together with its parent panel (collapsed group).
    pub popup_bounds: Option<(Point, iced::Size)>,
    pub edit: Option<EditState>,
    /// First visible item of each in-ribbon gallery strip.
    pub gallery_scroll: HashMap<u32, usize>,
    /// Recent colors per color-picker command (packed 0xRRGGBB).
    pub color_recent: HashMap<u32, Vec<u32>>,
    pub keytips: bool,
    /// `true` = tab-strip stage, `false` = ribbon-body stage.
    pub keytips_strip: bool,
    pub keytip_buffer: String,
    /// Gallery item currently previewed (strip or dropdown popup).
    pub previewed: Option<(u32, usize)>,
    /// When the hover over a gallery item began: the preview arms only
    /// after the double-click window has elapsed.
    pub gallery_dwell_since: Option<Instant>,
    /// Armed by a committed gallery click: previewing stays disabled
    /// for the double-click window.
    pub preview_cooldown: Option<Instant>,
    /// Transient: set while a popup scene is being built — hover queries
    /// then match the popup hover instead of the main-scene hover.
    pub popup_drawing: bool,
    /// Groups hidden into the overflow chevron: (tab, indices).
    pub overflow_hidden: std::cell::RefCell<Option<Vec<usize>>>,
    pub overflow_tab: std::cell::RefCell<Option<usize>>,
    /// Text-measurement cache (string, size*4).
    pub measure_cache: std::cell::RefCell<
        std::collections::HashMap<(String, u32), f32>,
    >,
}

impl State {
    pub fn popup_hover(&self) -> Option<&Target> {
        self.popup.as_ref().and_then(|p| p.hover.as_ref())
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            scene: None,
            hover: None,
            pressed: None,
            hover_since: None,
            last_tab_click: None,
            cur_tab_press_selected: 0,
            dismiss_tab_on_release: None,
            hover_pos: Point::ORIGIN,
            tooltip: None,
            tooltip_anchor: None,
            popup: None,
            popup_fade_start: None,
            popup_bounds: None,
            edit: None,
            gallery_scroll: HashMap::new(),
            color_recent: HashMap::new(),
            keytips: false,
            keytips_strip: true,
            keytip_buffer: String::new(),
            previewed: None,
            gallery_dwell_since: None,
            preview_cooldown: None,
            popup_drawing: false,
            overflow_hidden: std::cell::RefCell::new(None),
            overflow_tab: std::cell::RefCell::new(None),
            measure_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }
}

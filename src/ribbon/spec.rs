//! Ribbon specification — the YAML equivalent of the Windows ribbon
//! markup (`.ribbon` XML resources).
//!
//! ```yaml
//! commands:
//!   cmdSave: 10            # symbolic name -> numeric command id
//! application_menu:
//!   command: cmdAppMenu
//!   items:
//!     - command: cmdNew
//!       image: new
//!     - separator: true
//!     - command: cmdExit
//! quick_access:
//!   commands: [cmdSave]
//!   dock: top
//! # Cascading (Windows ScalingPolicy / SizeDefinitions):
//! # `ideal` sizes are used first; when the window narrows, `scale`
//! # steps are applied in order (one step per `Scale` element).
//! size_definitions:
//!   smallish:
//!     - controls: [cmdCut, cmdCopy]
//!       size: small
//! scaling:
//!   - group: clipboard
//!     ideal: [large]        # sizes tried when everything fits
//!     scale: [medium, small, popup]
//! tabs:
//! help_button: cmdHelp
//! context_popups:
//!   - command: cmdCtxMenu
//!     items: [{ command: cmdCut }, { command: cmdCopy }]
//! tabs:
//!   - id: home
//!     label: Home
//!     keytip: H
//!     groups:
//!       - id: clipboard
//!         label: Clipboard
//!         image: paste          # group icon (overflow popup)
//!         command: cmdClipDlg   # optional dialog launcher
//!         controls:
//!           - type: split_button
//!             command: cmdPaste
//!             size: large
//!             image: paste
//!             items: [{ command: cmdPasteSpecial }]
//!   - id: tableTools
//!     label: Table Tools
//!     contextual: orange
//!     context_command: cmdCtxTable   # UI_PKEY_ContextAvailable governs visibility
//!     modes: [0]                     # visible when SetModes(1 << 0)
//! ```
//!
//! Anywhere a `command:` appears it may be an integer id or a symbolic
//! name declared in the top-level `commands:` map. Names without an
//! entry get a stable id derived via FNV-1a — like the generated `*.h`
//! of the Windows markup compiler.

use serde::Deserialize;

use crate::ribbon::enums::{CommandType, ControlDock, SwatchColorMode};
use crate::ribbon::image::UiImage;
use crate::ribbon::prop::{PropertyKey, PropValue};

type CommandList = Vec<(u32, CommandType, Vec<(PropertyKey, PropValue)>)>;
type SeenMap = std::collections::HashMap<u32, CommandType>;

/// How unavailable (disabled) buttons look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisabledStyle {
    /// Dimmed by opacity only (the classic look).
    #[default]
    Faded,
    /// Desaturated (grayscale icon) plus the opacity dim.
    GrayScaledFaded,
}

/// Full ribbon declaration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RibbonSpec {
    /// How unavailable (disabled) buttons are rendered.
    pub disabled_style: DisabledStyle,
    /// Explorer extension: when the window is unfocused the ribbon
    /// ignores all input and shows no hover feedback (CPU saving).
    #[serde(default = "default_true")]
    pub ignore_input_when_unfocused: bool,
    /// Window-size sacrifices (explorer extension): below
    /// `ribbon_below` content width the ribbon is hidden entirely,
    /// below `nav_below` the navigation pane is hidden.
    pub sacrifice: SacrificeSpec,
    /// UI metrics (explorer extension).
    pub metrics: MetricsSpec,
    /// Symbolic command name -> numeric id.
    pub commands: std::collections::BTreeMap<String, u32>,
    pub application_menu: Option<AppMenuSpec>,
    pub quick_access: Option<QatSpec>,
    /// Command of the "?" help button at the right end of the tab strip.
    pub help_button: Option<CommandRef>,
    /// `IUIContextualUI` popup menus (right-click style).
    pub context_popups: Vec<ContextPopupSpec>,
    /// Named group size templates (Windows `Ribbon.SizeDefinitions` /
    /// `GroupSizeDefinition`): each entry remaps a set of controls to a
    /// different `CtrlSize`. Controls not listed keep their declared
    /// size.
    pub size_definitions: std::collections::BTreeMap<String, Vec<SizeRule>>,
    /// Per-group cascading policy (Windows `ScalingPolicy`): ideal sizes
    /// plus ordered fallback sizes tried as the window narrows.
    pub scaling: Vec<ScalingSpec>,
    pub tabs: Vec<TabSpec>,
}

fn default_true() -> bool {
    true
}

/// UI metrics (explorer extension of the markup).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MetricsSpec {
    /// Height of the navigation band (nav buttons, address bar, search
    /// box): the inner elements are centered in it.
    pub band_height: f32,
    /// Navigation tree row height: the item icon and text sizes derive
    /// from it.
    pub tree_item_height: f32,
    /// Navigation tree per-level indention (px).
    pub tree_indention: f32,
    /// Navigation tree label size as a fraction of the icon size.
    pub tree_font_scale: f32,
    /// Icon size as a fraction of the bar-button (nav circles / refresh /
    /// status view buttons) size.
    pub bar_button_multiplier: f32,
    /// Horizontal gap multiplier for the ribbon (group edge padding,
    /// inter-group and column gaps).
    pub gap_scale: f32,
    /// Horizontal gap between the tabs on the tab strip (px).
    pub tab_gap: f32,
    /// Extra px the group footers (group titles + dialog launchers) sit
    /// below their default position; the ribbon body grows with it, so
    /// the bottom separator line moves down too.
    pub footer_shift: f32,
    /// Functionless spacer between the navigation pane (and its
    /// splitter) and the folder view.
    pub nav_gap: f32,
}

impl Default for MetricsSpec {
    fn default() -> Self {
        Self {
            band_height: 36.0,
            tree_item_height: 22.0,
            tree_indention: 14.0,
            tree_font_scale: 0.75,
            bar_button_multiplier: 2.2,
            gap_scale: 1.0,
            tab_gap: 2.0,
            footer_shift: 3.0,
            nav_gap: 4.0,
        }
    }
}

/// Window-size sacrifice thresholds (explorer extension of the markup).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SacrificeSpec {
    /// Below this content width the ribbon (pane, tabs, title-bar QAT,
    /// contextual blocks) is hidden entirely.
    pub ribbon_below: f32,
    /// Below this content width the navigation pane is hidden (the file
    /// view resizes accordingly).
    pub nav_below: f32,
    /// Below this content width the navigation pane starts shrinking
    /// proportionally (keeping its share of the width) instead of
    /// staying at its full width.
    pub nav_resize_below: f32,
    /// Below this content width the search box is hidden (the address
    /// bar takes the whole band).
    pub search_below: f32,
    /// Below this content width the side (preview/details) pane is
    /// hidden — it sacrifices itself BEFORE the folder view gets too
    /// narrow.
    pub pane_below: f32,
}

impl Default for SacrificeSpec {
    fn default() -> Self {
        Self {
            ribbon_below: 120.0,
            nav_below: 600.0,
            nav_resize_below: 900.0,
            search_below: 400.0,
            pane_below: 800.0,
        }
    }
}

/// One `ControlSizeDefinition` rule: set `size` for the named commands
/// (or all controls when `controls` is empty).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SizeRule {
    /// Commands this rule applies to (symbolic names or ids); empty =
    /// every control in the group.
    pub controls: Vec<CommandRef>,
    pub size: CtrlSize,
}

/// One group size in a cascading policy: a plain control size, a named
/// template from `size_definitions:`, or `popup` (collapse the group to
/// a popup button, Windows `Group` with `Size="Popup"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupScale {
    Large,
    Medium,
    Small,
    Popup,
    Named(String),
}

impl<'de> serde::Deserialize<'de> for GroupScale {
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.to_ascii_lowercase().as_str() {
            "large" => Self::Large,
            "medium" => Self::Medium,
            "small" => Self::Small,
            "popup" => Self::Popup,
            _ => Self::Named(s),
        })
    }
}

/// Windows `ScalingPolicy` entry: ideal size + ordered fallback sizes
/// (`Scale` steps) for one group (`@Group` = group id).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ScalingSpec {
    /// Group id.
    pub group: String,
    /// Sizes used when everything fits (`ScalingPolicy.IdealSizes`),
    /// tried in order, widest first. Empty = the declared control sizes.
    pub ideal: Vec<GroupScale>,
    /// Ordered fallback sizes applied one step at a time when the window
    /// is too narrow. Empty = `[medium, small, popup]` relative to the
    /// current size.
    pub scale: Vec<GroupScale>,
}

/// A command reference: numeric id or symbolic name.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CommandRef {
    Id(u32),
    Name(String),
}

impl CommandRef {
    pub fn resolve(
        &self,
        commands: &std::collections::BTreeMap<String, u32>,
    ) -> Result<u32, String> {
        match self {
            CommandRef::Id(id) => Ok(*id),
            CommandRef::Name(name) => Ok(commands
                .get(name)
                .copied()
                .unwrap_or_else(|| stable_id(name))),
        }
    }
}

/// Deterministic id for undeclared symbolic names (FNV-1a, outside the
/// 1..1000 range reserved for property ids).
fn stable_id(name: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in name.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    1001 + (h % 1_000_000)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppMenuSpec {
    pub command: Option<CommandRef>,
    /// Left column entries.
    pub items: Vec<MenuItemSpec>,
    /// Command exposing `UI_PKEY_RecentItems` for the right column.
    pub recent_items: Option<CommandRef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MenuItemSpec {
    pub command: Option<CommandRef>,
    /// Overrides `UI_PKEY_Label`.
    pub label: Option<String>,
    /// Builtin icon name, `file:<path>` or inline `<svg>…</svg>`.
    pub image: Option<String>,
    pub keytip: Option<String>,
    pub tooltip: Option<String>,
    pub separator_before: bool,
    /// Sub-menu items (`DropDownButton` / cascading menus).
    pub items: Vec<MenuItemSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct QatSpec {
    pub commands: Vec<CommandRef>,
    pub dock: QatDock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QatDock {
    #[default]
    Top,
    Bottom,
}

impl From<QatDock> for ControlDock {
    fn from(d: QatDock) -> Self {
        match d {
            QatDock::Top => ControlDock::Top,
            QatDock::Bottom => ControlDock::Bottom,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContextPopupSpec {
    pub command: Option<CommandRef>,
    pub items: Vec<MenuItemSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TabSpec {
    pub id: String,
    pub label: Option<String>,
    pub keytip: Option<String>,
    /// Contextual group name; its color is derived from the name
    /// (e.g. `orange`, `purple`) like the Windows ribbon.
    /// Contextual group name; its color is derived from the name
    /// (e.g. `orange`, `purple`) like the Windows ribbon. When the name
    /// is not a color (`Table Tools`), `context_color` overrides it.
    pub contextual: Option<String>,
    /// Explicit contextual accent color name (overrides the derivation
    /// from `contextual`).
    pub context_color: Option<String>,
    /// Command whose `UI_PKEY_ContextAvailable` property controls
    /// visibility of this contextual tab set.
    pub context_command: Option<CommandRef>,
    /// Application mode bits (`SetModes`): visible when any listed bit is
    /// set in the current mode mask. Empty = always visible.
    pub modes: Vec<u8>,
    pub groups: Vec<GroupSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GroupSpec {
    pub id: String,
    pub label: Option<String>,
    /// Group icon shown in the overflow popup.
    pub image: Option<String>,
    /// Dialog-launcher command (the tiny arrow in the group caption).
    pub command: Option<CommandRef>,
    pub controls: Vec<ControlSpec>,
}

/// Control sizes (`SizeDefinition`): large = big icon + label (3 lines),
/// medium = small icon + label (2 lines), small = icon + 1-line label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CtrlSize {
    #[default]
    Large,
    Medium,
    Small,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlSpec {
    /// `UI_COMMANDTYPE_ACTION`
    Button(ButtonSpec),
    /// `UI_COMMANDTYPE_BOOLEAN`
    ToggleButton(ButtonSpec),
    /// Button with a dropdown menu (`SplitButton`).
    SplitButton(SplitSpec),
    /// Menu-only button (`DropDownButton`).
    DropDownButton(SplitSpec),
    /// Editable dropdown (`UI_COMMANDTYPE_COLLECTION`).
    ComboBox(BoxSpec),
    /// Numeric up/down (`UI_COMMANDTYPE_DECIMAL`).
    Spinner(SpinnerSpec),
    /// `UI_COMMANDTYPE_FONT`.
    FontControl(BoxSpec),
    /// Color swatch dropdown (`UI_COMMANDTYPE_COLORANCHOR`).
    ColorPicker(ColorSpec),
    /// Horizontally scrolling strip of items (`InRibbonGallery`), or a
    /// dropdown gallery when `popup: true`.
    InRibbonGallery(GallerySpec),
    /// Thin bordered box around a vertical stack of controls.
    ControlGroup { controls: Vec<ControlSpec> },
}

impl ControlSpec {
    pub fn command(&self) -> Option<&CommandRef> {
        match self {
            ControlSpec::Button(b)
            | ControlSpec::ToggleButton(b) => b.command.as_ref(),
            ControlSpec::SplitButton(s)
            | ControlSpec::DropDownButton(s) => s.command.as_ref(),
            ControlSpec::ComboBox(b)
            | ControlSpec::FontControl(b) => b.command.as_ref(),
            ControlSpec::Spinner(s) => s.command.as_ref(),
            ControlSpec::ColorPicker(c) => c.command.as_ref(),
            ControlSpec::InRibbonGallery(g) => g.command.as_ref(),
            ControlSpec::ControlGroup { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ButtonSpec {
    pub command: Option<CommandRef>,
    pub size: CtrlSize,
    pub label: Option<String>,
    pub image: Option<String>,
    pub keytip: Option<String>,
    /// Tooltip description (`UI_PKEY_TooltipDescription`).
    pub tooltip: Option<String>,
    /// Shortcut shown in the tooltip title ("Ctrl+D").
    pub shortcut: Option<String>,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SplitSpec {
    pub command: Option<CommandRef>,
    pub size: CtrlSize,
    pub label: Option<String>,
    pub image: Option<String>,
    pub keytip: Option<String>,
    /// Two-part split chrome (main action + dropdown arrow). Default
    /// off: the whole button opens the menu as one element.
    pub separate: bool,
    pub tooltip: Option<String>,
    pub shortcut: Option<String>,
    pub items: Vec<MenuItemSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BoxSpec {
    pub command: Option<CommandRef>,
    pub label: Option<String>,
    pub keytip: Option<String>,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SpinnerSpec {
    pub command: Option<CommandRef>,
    pub keytip: Option<String>,
    pub width: Option<f32>,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub increment: f64,
    pub decimal_places: u32,
    /// `UI_PKEY_FormatString`, e.g. `"{0}%"`.
    pub format: Option<String>,
}

impl Default for SpinnerSpec {
    fn default() -> Self {
        Self {
            command: None,
            keytip: None,
            width: None,
            value: 0.0,
            min: 0.0,
            max: 100.0,
            increment: 1.0,
            decimal_places: 0,
            format: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColorSpec {
    pub command: Option<CommandRef>,
    pub size: CtrlSize,
    pub label: Option<String>,
    pub keytip: Option<String>,
    pub mode: SwatchColorMode,
    /// Icon used for the anchor (`A` glyph by default).
    pub image: Option<String>,
}

impl Default for ColorSpec {
    fn default() -> Self {
        Self {
            command: None,
            size: CtrlSize::Small,
            label: None,
            keytip: None,
            mode: SwatchColorMode::Normal,
            image: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GallerySpec {
    pub command: Option<CommandRef>,
    pub label: Option<String>,
    pub keytip: Option<String>,
    /// Icon size of one gallery item (square, drawn left of the label).
    pub item_width: f32,
    pub item_height: f32,
    /// Visible columns in the in-ribbon strip.
    pub columns: usize,
    /// Show the item label right of the icon (default on).
    pub show_label: bool,
    /// Show the expand button (opens the dropdown gallery popup).
    pub popup: bool,
}

impl Default for GallerySpec {
    fn default() -> Self {
        Self {
            command: None,
            label: None,
            keytip: None,
            item_width: 16.0,
            item_height: 16.0,
            columns: 3,
            show_label: true,
            popup: false,
        }
    }
}

/// Parse a ribbon YAML document.
pub fn parse(yaml: &str) -> std::result::Result<RibbonSpec, String> {
    serde_yaml::from_str(yaml).map_err(|e| format!("invalid ribbon yaml: {e}"))
}

fn add_command(
    out: &mut CommandList,
    seen: &mut SeenMap,
    cmd: u32,
    ty: CommandType,
    seeds: Vec<(PropertyKey, PropValue)>,
) {
    seen.entry(cmd).or_insert(ty);
    if let Some(existing) = out.iter_mut().find(|(c, _, _)| *c == cmd) {
        for (k, v) in seeds {
            if !existing.2.iter().any(|(ek, _)| *ek == k) {
                existing.2.push((k, v));
            }
        }
    } else {
        out.push((cmd, ty, seeds));
    }
}

/// Resolve every `CommandRef` in the spec to numeric ids.
///
/// Returns `(command id, CommandType, seed properties)` — the seeds are
/// markup-provided initial property values, exactly like the Windows
/// markup compiler bakes defaults into the binary.
pub fn collect_commands(
    spec: &mut RibbonSpec,
) -> std::result::Result<CommandList, String> {
    let commands = &spec.commands;
    let mut out: CommandList = Vec::new();
    let mut seen: SeenMap = std::collections::HashMap::new();

    if let Some(menu) = &mut spec.application_menu {
        if let Some(c) = menu.command.clone() {
            let cmd = c.resolve(commands)?;
            menu.command = Some(CommandRef::Id(cmd));
            add_command(&mut out, &mut seen, cmd, CommandType::Anchor, vec![]);
        }
        for item in &mut menu.items {
            collect_menu_item(commands, item, &mut out, &mut seen)?;
        }
        if let Some(c) = menu.recent_items.clone() {
            let cmd = c.resolve(commands)?;
            menu.recent_items = Some(CommandRef::Id(cmd));
            add_command(
                &mut out,
                &mut seen,
                cmd,
                CommandType::RecentItems,
                vec![(PropertyKey::Label, PropValue::Str("Recent".into()))],
            );
        }
    }

    if let Some(c) = spec.help_button.clone() {
        let cmd = c.resolve(commands)?;
        spec.help_button = Some(CommandRef::Id(cmd));
        add_command(&mut out, &mut seen, cmd, CommandType::Action, vec![]);
    }

    for popup in &mut spec.context_popups {
        if let Some(c) = popup.command.clone() {
            let cmd = c.resolve(commands)?;
            popup.command = Some(CommandRef::Id(cmd));
            add_command(&mut out, &mut seen, cmd, CommandType::Anchor, vec![]);
        }
        for item in &mut popup.items {
            collect_menu_item(commands, item, &mut out, &mut seen)?;
        }
    }

    // `Ribbon.SizeDefinitions`: resolve ControlSizeDefinition command
    // references to numeric ids for the layout engine.
    for rules in spec.size_definitions.values_mut() {
        for rule in rules {
            for c in &mut rule.controls {
                *c = CommandRef::Id(c.resolve(commands)?);
            }
        }
    }

    for tab in &mut spec.tabs {
        if let Some(c) = tab.context_command.clone() {
            let cmd = c.resolve(commands)?;
            tab.context_command = Some(CommandRef::Id(cmd));
            add_command(&mut out, &mut seen, cmd, CommandType::Context, vec![]);
        }
        for group in &mut tab.groups {
            collect_group(commands, group, &mut out, &mut seen)?;
        }
    }

    Ok(out)
}

fn collect_menu_item(
    commands: &std::collections::BTreeMap<String, u32>,
    item: &mut MenuItemSpec,
    out: &mut CommandList,
    seen: &mut SeenMap,
) -> std::result::Result<(), String> {
    if let Some(c) = item.command.clone() {
        let cmd = c.resolve(commands)?;
        item.command = Some(CommandRef::Id(cmd));
        let mut seeds = vec![];
        if let Some(l) = item.label.take() {
            seeds.push((PropertyKey::Label, PropValue::Str(l)));
        }
        if let Some(k) = item.keytip.take() {
            seeds.push((PropertyKey::Keytip, PropValue::Str(k)));
        }
        if let Some(t) = item.tooltip.take() {
            seeds.push((PropertyKey::TooltipTitle, PropValue::Str(t)));
        }
        add_command(out, seen, cmd, CommandType::Action, seeds);
        for sub in &mut item.items {
            collect_menu_item(commands, sub, out, seen)?;
        }
    }
    Ok(())
}

fn collect_group(
    commands: &std::collections::BTreeMap<String, u32>,
    group: &mut GroupSpec,
    out: &mut CommandList,
    seen: &mut SeenMap,
) -> std::result::Result<(), String> {
    if let Some(c) = &group.command {
        let cmd = c.resolve(commands)?;
        add_command(out, seen, cmd, CommandType::Action, vec![]);
    }
    for control in &mut group.controls {
        collect_control(commands, control, out, seen)?;
    }
    Ok(())
}

fn button_seeds(
    label: Option<String>,
    image: Option<String>,
    keytip: Option<String>,
    tooltip: Option<String>,
    shortcut: Option<String>,
) -> Vec<(PropertyKey, PropValue)> {
    let mut seeds = vec![];
    if let Some(l) = label {
        if let Some(sc) = shortcut {
            seeds.push((
                PropertyKey::TooltipTitle,
                PropValue::Str(format!("{l} ({sc})")),
            ));
        }
        seeds.push((PropertyKey::Label, PropValue::Str(l)));
    }
    if let Some(t) = tooltip {
        seeds.push((PropertyKey::TooltipDescription, PropValue::Str(t)));
    }
    if let Some(k) = keytip {
        seeds.push((PropertyKey::Keytip, PropValue::Str(k)));
    }
    if let Some(i) = image {
        let image = parse_image(&i);
        seeds.push((PropertyKey::SmallImage, PropValue::Image(image.clone())));
        seeds.push((PropertyKey::LargeImage, PropValue::Image(image)));
    }
    seeds
}

fn parse_image(image: &str) -> UiImage {
    if let Some(path) = image.strip_prefix("file:") {
        UiImage::File(path.into())
    } else if image.trim_start().starts_with("<svg") {
        UiImage::Svg(image.to_string())
    } else {
        UiImage::Named(image.to_string())
    }
}

// spec-to-command-list walker: every parameter is a distinct input,
// a bag struct would just be unpacking noise here
#[allow(clippy::too_many_arguments)]
fn resolve_ref(
    commands: &std::collections::BTreeMap<String, u32>,
    cmd_slot: &mut Option<CommandRef>,
    out: &mut CommandList,
    seen: &mut SeenMap,
    ty: CommandType,
    label: Option<String>,
    image: Option<String>,
    keytip: Option<String>,
    tooltip: Option<String>,
    shortcut: Option<String>,
    extra: Vec<(PropertyKey, PropValue)>,
) -> std::result::Result<(), String> {
    if let Some(c) = cmd_slot.clone() {
        let cmd = c.resolve(commands)?;
        // write the resolved id back into the spec
        *cmd_slot = Some(CommandRef::Id(cmd));
        let mut seeds = button_seeds(label, image, keytip, tooltip, shortcut);
        seeds.extend(extra);
        add_command(out, seen, cmd, ty, seeds);
    }
    Ok(())
}

fn collect_control(
    commands: &std::collections::BTreeMap<String, u32>,
    control: &mut ControlSpec,
    out: &mut CommandList,
    seen: &mut SeenMap,
) -> std::result::Result<(), String> {
    match control {
        ControlSpec::Button(b) => {
            let (label, image, keytip) = (b.label.take(), b.image.take(), b.keytip.take());
            let (tooltip, shortcut) = (b.tooltip.take(), b.shortcut.take());
            resolve_ref(
                commands,
                &mut b.command,
                out,
                seen,
                CommandType::Action,
                label,
                image,
                keytip,
                tooltip,
                shortcut,
                vec![],
            )?;
        }
        ControlSpec::ToggleButton(b) => {
            let (label, image, keytip) = (b.label.take(), b.image.take(), b.keytip.take());
            let (tooltip, shortcut) = (b.tooltip.take(), b.shortcut.take());
            resolve_ref(
                commands,
                &mut b.command,
                out,
                seen,
                CommandType::Boolean,
                label,
                image,
                keytip,
                tooltip,
                shortcut,
                vec![],
            )?;
        }
        ControlSpec::SplitButton(s) | ControlSpec::DropDownButton(s) => {
            let (label, image, keytip) = (s.label.take(), s.image.take(), s.keytip.take());
            let (tooltip, shortcut) = (s.tooltip.take(), s.shortcut.take());
            resolve_ref(
                commands,
                &mut s.command,
                out,
                seen,
                CommandType::Action,
                label,
                image,
                keytip,
                tooltip,
                shortcut,
                vec![],
            )?;
            for item in &mut s.items {
                collect_menu_item(commands, item, out, seen)?;
            }
        }
        ControlSpec::ComboBox(b) => {
            let (label, keytip) = (b.label.take(), b.keytip.take());
            resolve_ref(
                commands,
                &mut b.command,
                out,
                seen,
                CommandType::Collection,
                label,
                None,
                keytip,
                None,
                None,
                vec![],
            )?;
        }
        ControlSpec::FontControl(b) => {
            let (label, keytip) = (b.label.take(), b.keytip.take());
            resolve_ref(
                commands,
                &mut b.command,
                out,
                seen,
                CommandType::Font,
                label,
                None,
                keytip,
                None,
                None,
                vec![],
            )?;
        }
        ControlSpec::Spinner(s) => {
            let mut extra = vec![
                (PropertyKey::DecimalValue, PropValue::Decimal(s.value)),
                (PropertyKey::MinValue, PropValue::Decimal(s.min)),
                (PropertyKey::MaxValue, PropValue::Decimal(s.max)),
                (PropertyKey::Increment, PropValue::Decimal(s.increment)),
                (PropertyKey::DecimalPlaces, PropValue::UInt(s.decimal_places)),
            ];
            if let Some(f) = &s.format {
                extra.push((PropertyKey::FormatString, PropValue::Str(f.clone())));
            }
            let keytip = s.keytip.take();
            resolve_ref(
                commands,
                &mut s.command,
                out,
                seen,
                CommandType::Decimal,
                None,
                None,
                keytip,
                None,
                None,
                extra,
            )?;
        }
        ControlSpec::ColorPicker(c) => {
            let (label, image, keytip) = (c.label.take(), c.image.take(), c.keytip.take());
            resolve_ref(
                commands,
                &mut c.command,
                out,
                seen,
                CommandType::ColorAnchor,
                label,
                image,
                keytip,
                None,
                None,
                vec![(PropertyKey::ColorMode, PropValue::ColorMode(c.mode))],
            )?;
        }
        ControlSpec::InRibbonGallery(g) => {
            let (label, keytip) = (g.label.take(), g.keytip.take());
            resolve_ref(
                commands,
                &mut g.command,
                out,
                seen,
                CommandType::Collection,
                label,
                None,
                keytip,
                None,
                None,
                vec![],
            )?;
        }
        ControlSpec::ControlGroup { controls } => {
            for c in controls {
                collect_control(commands, c, out, seen)?;
            }
        }
    }
    Ok(())
}

impl RibbonSpec {
    /// Parse a YAML document (used by
    /// [`crate::ribbon::RibbonFramework::load_ui`]).
    pub fn from_yaml(yaml: &str) -> std::result::Result<Self, String> {
        parse(yaml)
    }
}

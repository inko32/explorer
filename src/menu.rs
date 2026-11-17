//! Context menus. ONE YAML file
//! (`assets/menu.yaml`; user override ~/.config/explorer/menu.yaml)
//! defines the menus (selection / background / archive / nav), the
//! file-type groups, the open backend and the Windows-style layout.
//!
//! This module owns: loading + validation (`MenuCfg::load`), file-type
//! detection (`groups_for`), menu construction (`build_*`), the open
//! backend + YAML-defined commands (`open_files` / `run_custom`) and the
//! reusable renderer (`view` + the `OnRightFree` background wrapper).

use std::path::{Path, PathBuf};

use iced::Widget as _;
use iced::advanced::widget::{Operation, Tree, Widget, tree};
use iced::advanced::{Layout, Renderer as _, Shell, layout, overlay, renderer};
use iced::widget::{Space, column, container, mouse_area, row, text};
use iced::{Color, Element, Length, Point, Rectangle, Size, Vector, mouse};
use serde::Deserialize;

use crate::model::Message;
use crate::ui_common::skin;

// ---------------------------------------------------------------------------
// configuration (assets/menu.yaml)

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MenuCfg {
    pub settings: SettingsCfg,
    pub open: OpenCfg,
    pub layout: LayoutCfg,
    /// Built-in command defaults (label/shortcut/icon) by command id.
    pub commands: std::collections::BTreeMap<String, CmdDefaults>,
    pub menus: MenusCfg,
    /// §3.3 file-type groups (a Vec: config order is merge order).
    pub file_types: Vec<TypeGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SettingsCfg {
    /// `extension` | `magic` (magic is reserved: falls back to extension
    /// with a startup warning — no content-inspection dependency).
    pub file_detection: String,
    /// `kde` | `yaml`.
    pub open_backend: String,
    /// Group for extensionless files ("" = none).
    pub extensionless_group: String,
    /// Compound extensions (".tar.gz") matched before the final one.
    pub compound_extensions: Vec<String>,
    /// menu.yaml `menu_click_through` (1/0/true/false): a click that
    /// dismisses an open menu also activates the item beneath it
    /// (Windows behavior) instead of being swallowed by the modal shield.
    pub menu_click_through: ClickThrough,
}

/// A bool that menu.yaml may spell as `1`/`0` or `true`/`false`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickThrough(pub bool);

impl<'de> Deserialize<'de> for ClickThrough {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Flag(bool),
            Num(i64),
        }
        match Repr::deserialize(d)? {
            Repr::Flag(b) => Ok(ClickThrough(b)),
            Repr::Num(n) => Ok(ClickThrough(n != 0)),
        }
    }
}

impl Default for SettingsCfg {
    fn default() -> Self {
        Self {
            file_detection: "extension".into(),
            open_backend: "kde".into(),
            extensionless_group: String::new(),
            compound_extensions: Vec::new(),
            menu_click_through: ClickThrough(true),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OpenCfg {
    pub kde: KdeCfg,
    /// yaml backend: ".png" -> argv; the paths are appended.
    pub handlers: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct KdeCfg {
    /// Tried in order; the first one that launches wins.
    pub commands: Vec<String>,
}

impl Default for KdeCfg {
    fn default() -> Self {
        Self {
            commands: vec!["kde-open".into(), "xdg-open".into()],
        }
    }
}


/// Windows-style menu metrics.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct LayoutCfg {
    pub min_width: f32,
    pub item_height: f32,
    pub gutter: f32,
    pub icon_size: f32,
    pub separator_height: f32,
    pub pad_h: f32,
    pub font_size: f32,
    pub corner_radius: f32,
    /// Render the menu as an independent overlay layer (experimental;
    /// default false = in-window rendering with a modal shield).
    pub popup: bool,
}

impl Default for LayoutCfg {
    fn default() -> Self {
        Self {
            min_width: 220.0,
            item_height: 24.0,
            gutter: 28.0,
            icon_size: 16.0,
            separator_height: 9.0,
            pad_h: 6.0,
            font_size: 12.0,
            corner_radius: 0.0,
            popup: false,
        }
    }
}

/// Defaults for a built-in command id (`commands:` section).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CmdDefaults {
    pub label: Option<String>,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
}

/// One menu item as written in the yaml.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ItemSpec {
    pub command: String,
    pub separator: bool,
    pub label: Option<String>,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
    /// Bold = the default action (Windows menus bold it).
    pub bold: bool,
    /// What the command supports (default: both). An item that cannot
    /// operate on the whole selection is hidden (§8).
    pub files: Option<bool>,
    pub dirs: Option<bool>,
    /// Single-object commands (rename) hide for multi-selections.
    pub single: bool,
    /// File-type group merge: insert after this command
    /// (default: before `properties`).
    pub after: Option<String>,
    /// YAML-defined command: argv; the selected paths are appended
    /// (no shell, §10).
    pub run: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MenusCfg {
    pub general: Vec<ItemSpec>,
    pub background: Vec<ItemSpec>,
    pub archive: Vec<ItemSpec>,
    pub nav: Vec<ItemSpec>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TypeGroup {
    pub name: String,
    pub extensions: Vec<String>,
    pub items: Vec<ItemSpec>,
}

/// Built-in command ids the model dispatches (validation warns on
/// anything else without a `run:` argv).
pub const BUILTIN: &[&str] = &[
    "open", "cut", "copy", "paste", "delete", "rename", "properties",
    "open_with", "print", "preview", "new_folder", "select_all", "refresh",
    "open_new_window", "extract_here",
];

impl MenuCfg {
    /// Load + validate (§9): ~/.config/explorer/menu.yaml overrides the
    /// embedded copy; a broken file falls back to the defaults. Problems
    /// are reported on stderr and the offending entries are skipped.
    pub fn load() -> Self {
        let text = std::env::var_os("HOME")
            .map(PathBuf::from)
            .and_then(|h| std::fs::read_to_string(h.join(".config/explorer/menu.yaml")).ok())
            .unwrap_or_else(|| include_str!("../assets/menu.yaml").to_string());
        let mut cfg: MenuCfg = match serde_yaml::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("menu.yaml: {e} — using built-in defaults");
                MenuCfg::default()
            }
        };
        cfg.validate();
        cfg
    }

    /// §9 validation: unknown commands (dropped unless they carry a
    /// `run:` argv), duplicate extensions (first group wins), reserved
    /// detection modes. Warnings go to stderr.
    fn validate(&mut self) {
        if self.settings.file_detection == "magic" {
            eprintln!("menu.yaml: file_detection=magic is reserved; using extension");
            self.settings.file_detection = "extension".into();
        }
        if self.settings.file_detection != "extension" {
            eprintln!(
                "menu.yaml: unknown file_detection {:?}; using extension",
                self.settings.file_detection
            );
            self.settings.file_detection = "extension".into();
        }
        if self.settings.open_backend != "kde" && self.settings.open_backend != "yaml" {
            eprintln!(
                "menu.yaml: unknown open_backend {:?}; using kde",
                self.settings.open_backend
            );
            self.settings.open_backend = "kde".into();
        }
        let check = |items: &mut Vec<ItemSpec>, menu: &str| {
            items.retain(|i| {
                let bad = !i.separator
                    && !BUILTIN.contains(&i.command.as_str())
                    && i.run.is_none();
                if bad {
                    eprintln!("menu.yaml: {menu}: unknown command {:?} (dropped)", i.command);
                }
                !bad
            });
        };
        check(&mut self.menus.general, "general");
        check(&mut self.menus.background, "background");
        check(&mut self.menus.archive, "archive");
        check(&mut self.menus.nav, "nav");
        let mut seen: std::collections::HashMap<String, String> = Default::default();
        for g in &mut self.file_types {
            g.extensions.retain(|e| {
                let e = e.to_lowercase();
                if let Some(first) = seen.get(&e) {
                    eprintln!(
                        "menu.yaml: extension {e:?} claimed by both {first:?} and {:?} — {first:?} wins",
                        g.name
                    );
                    false
                } else {
                    seen.insert(e, g.name.clone());
                    true
                }
            });
            check(&mut g.items, &g.name);
        }
    }

    fn group(&self, name: &str) -> Option<&TypeGroup> {
        self.file_types.iter().find(|g| g.name == name)
    }
}

// ---------------------------------------------------------------------------
// file-type detection (§4)

/// Extension of `path` for detection: compound extensions first, then
/// the final extension, lowercased (§4.2). No file contents are read.
fn detect_ext(cfg: &MenuCfg, path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    for c in &cfg.settings.compound_extensions {
        if name.ends_with(&c.to_lowercase()) {
            return Some(c.to_lowercase());
        }
    }
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
        .map(|e| format!(".{e}"))
}

/// §3.3/§4: applicable file-type groups for a selection of FILES
/// (directories have no type), each group at most once, in config order.
pub fn groups_for(cfg: &MenuCfg, paths: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths {
        let group = match detect_ext(cfg, p) {
            Some(ext) => cfg
                .file_types
                .iter()
                .find(|g| g.extensions.iter().any(|e| e.to_lowercase() == ext))
                .map(|g| g.name.clone()),
            // §4.2: extensionless files use the configured fallback
            // category; unknown extensions stay generic
            None => cfg
                .group(&cfg.settings.extensionless_group)
                .map(|g| g.name.clone()),
        };
        // §3.4: a group is not duplicated because several selected
        // files belong to it
        if let Some(g) = group
            && !out.contains(&g)
        {
            out.push(g);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// menu building (§3.4 composition)

/// A resolved menu entry (what the renderer draws).
#[derive(Clone, Debug)]
pub enum Entry {
    Sep,
    Item(Item),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub shortcut: String,
    pub icon: Option<String>,
    pub enabled: bool,
    pub bold: bool,
    /// YAML-defined command argv (the selected paths are appended).
    pub run: Option<Vec<String>>,

    files: bool,
    dirs: bool,
    single: bool,
}

impl Item {
    /// A plain enabled command item (no applicability filtering) —
    /// used for menus built in code rather than from menu.yaml
    /// (the drag drop-behavior menu).
    pub fn plain(id: &str, label: String) -> Self {
        Self {
            id: id.into(),
            label,
            shortcut: String::new(),
            icon: None,
            enabled: true,
            bold: false,
            run: None,
            files: true,
            dirs: true,
            single: false,
        }
    }

    /// A plain disabled item (a placeholder entry — the Details column
    /// filter menu uses it until filtering lands).
    pub fn disabled(id: &str, label: String) -> Self {
        let mut item = Self::plain(id, label);
        item.enabled = false;
        item
    }

    /// §8 applicability against the whole selection.
    fn applies(&self, has_files: bool, has_dirs: bool, n: usize) -> bool {
        (self.files || !has_files)
            && (self.dirs || !has_dirs)
            && (!self.single || n <= 1)
    }
}

/// The open menu (model state): resolved entries + position.
#[derive(Clone, Debug)]
pub struct Open {
    pub entries: Vec<Entry>,
    pub x: f32,
    pub y: f32,
    pub hover: Option<usize>,
    /// Nav-pane menu target (the row's path).
    pub nav_target: Option<PathBuf>,
    /// Address-bar dropdown marker (history / subfolders / selector):
    /// picks index straight into `paths`, bypassing the yaml command
    /// dispatch below.
    pub bar: Option<BarMenu>,
}

/// An address-bar dropdown: which one is open,
/// the folder whose children are listed (Subdirs) and the entry paths
/// (parallel to `Open.entries`).
#[derive(Clone, Debug)]
pub struct BarMenu {
    pub kind: BarMenuKind,
    pub prefix: PathBuf,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMenuKind {
    /// Back/forward history (the back_list chevron).
    History,
    /// Subfolders of a breadcrumb segment.
    Subdirs,
    /// Location selector (the address bar right-list).
    Selector,
}

fn resolve(cfg: &MenuCfg, spec: &ItemSpec) -> Entry {
    if spec.separator {
        return Entry::Sep;
    }
    let d = cfg.commands.get(&spec.command).cloned().unwrap_or_default();
    Entry::Item(Item {
        id: spec.command.clone(),
        label: spec
            .label
            .clone()
            .or(d.label)
            .unwrap_or_else(|| spec.command.clone()),
        shortcut: spec.shortcut.clone().or(d.shortcut).unwrap_or_default(),
        icon: spec.icon.clone().or(d.icon),
        enabled: true,
        bold: spec.bold,
        run: spec.run.clone(),
        files: spec.files.unwrap_or(true),
        dirs: spec.dirs.unwrap_or(true),
        single: spec.single,
    })
}

/// Drop leading/trailing and duplicate separators (§3.4 rule 5).
fn cleanup(entries: &mut Vec<Entry>) {
    entries.dedup_by(|b, a| matches!(a, Entry::Sep) && matches!(b, Entry::Sep));
    while matches!(entries.first(), Some(Entry::Sep)) {
        entries.remove(0);
    }
    while matches!(entries.last(), Some(Entry::Sep)) {
        entries.pop();
    }
}

/// §3.4 selection menu: the `general` baseline + applicable file-type
/// group items (deduped by command, inserted after their `after:`
/// command, default before `properties`), filtered against the whole
/// selection.
pub fn build_selection(
    cfg: &MenuCfg,
    n: usize,
    has_files: bool,
    has_dirs: bool,
    groups: &[String],
) -> Vec<Entry> {
    let mut entries: Vec<Entry> =
        cfg.menus.general.iter().map(|s| resolve(cfg, s)).collect();
    for name in groups {
        let Some(g) = cfg.group(name) else { continue };
        for spec in &g.items {
            if spec.separator {
                continue;
            }
            if entries.iter().any(|e| matches!(e, Entry::Item(i) if i.id == spec.command)) {
                continue; // §3.4: no duplicates
            }
            let item = resolve(cfg, spec);
            let pos = spec
                .after
                .as_ref()
                .and_then(|a| entries.iter().position(|e| matches!(e, Entry::Item(i) if &i.id == a)))
                .map(|p| p + 1)
                .or_else(|| {
                    entries.iter().position(|e| matches!(e, Entry::Item(i) if i.id == "properties"))
                })
                .unwrap_or(entries.len());
            entries.insert(pos, item);
        }
    }
    entries.retain(|e| matches!(e, Entry::Sep) || matches!(e, Entry::Item(i) if i.applies(has_files, has_dirs, n)));
    cleanup(&mut entries);
    entries
}

/// Right-click on empty folder space. `can_paste` enables Paste.
pub fn build_background(cfg: &MenuCfg, can_paste: bool) -> Vec<Entry> {
    let mut entries: Vec<Entry> =
        cfg.menus.background.iter().map(|s| resolve(cfg, s)).collect();
    for e in &mut entries {
        if let Entry::Item(i) = e
            && i.id == "paste"
        {
            i.enabled = can_paste;
        }
    }
    cleanup(&mut entries);
    entries
}

/// Right-click inside an archive (selection or background): the archive
/// is read-only, the menu is the configured `archive` one.
pub fn build_archive(cfg: &MenuCfg, n: usize, has_files: bool, has_dirs: bool) -> Vec<Entry> {
    let mut entries: Vec<Entry> =
        cfg.menus.archive.iter().map(|s| resolve(cfg, s)).collect();
    entries.retain(|e| matches!(e, Entry::Sep) || matches!(e, Entry::Item(i) if i.applies(has_files, has_dirs, n)));
    cleanup(&mut entries);
    entries
}

/// Right-click a navigation pane element.
pub fn build_nav(cfg: &MenuCfg) -> Vec<Entry> {
    let mut entries: Vec<Entry> = cfg.menus.nav.iter().map(|s| resolve(cfg, s)).collect();
    cleanup(&mut entries);
    entries
}

// ---------------------------------------------------------------------------
// opening + YAML-defined commands (§5, §10: no shell, paths as argv)

/// Spawn `argv` with `paths` appended as individual arguments.
pub fn run_custom(argv: &[String], paths: &[PathBuf]) -> Result<(), String> {
    let Some((prog, args)) = argv.split_first() else {
        return Err("empty run command".into());
    };
    std::process::Command::new(prog)
        .args(args)
        .args(paths)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{prog}: {e}"))
}

/// §5 open dispatch: kde backend (first configured opener that launches,
/// all paths in one call) or the yaml handlers (per extension, paths
/// grouped per handler — §5.4 per-handler batches).
pub fn open_files(cfg: &MenuCfg, paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("nothing to open".into());
    }
    if cfg.settings.open_backend == "yaml" {
        // resolve a handler per file; group paths sharing one argv
        let mut batches: Vec<(&Vec<String>, Vec<PathBuf>)> = Vec::new();
        for p in paths {
            let ext = detect_ext(cfg, p).unwrap_or_default();
            let Some(argv) = cfg.open.handlers.get(&ext) else {
                return Err(format!(
                    "no open handler for {ext:?} ({}); configure open.handlers",
                    p.display()
                ));
            };
            match batches.iter_mut().find(|(a, _)| *a == argv) {
                Some((_, ps)) => ps.push(p.clone()),
                None => batches.push((argv, vec![p.clone()])),
            }
        }
        for (argv, ps) in batches {
            run_custom(argv, &ps)?;
        }
        return Ok(());
    }
    // kde backend: desktop MIME associations; first available command
    let mut last_err = String::new();
    for cmd in &cfg.open.kde.commands {
        match std::process::Command::new(cmd)
            .args(paths)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) => last_err = format!("{cmd}: {e}"),
        }
    }
    Err(format!(
        "no opener available ({})",
        if last_err.is_empty() { "open.kde.commands is empty".into() } else { last_err }
    ))
}

// ---------------------------------------------------------------------------
// renderer (reusable: every menu kind draws through view())

/// Windows-style context menu. Reusable: pure view of the resolved
/// entries; the caller positions it (ui.rs floats it at the anchor).
pub fn view<'a>(open: &'a Open, cfg: &'a MenuCfg, skin: skin::Skin) -> Element<'a, Message> {
    let l = &cfg.layout;
    // Adaptive width: estimated from the content (text metrics are not
    // available in view(); the estimate matches the ribbon's fallback)
    let est = |s: &str| s.chars().count() as f32 * l.font_size * 0.58;
    let content_w = open
        .entries
        .iter()
        .map(|e| match e {
            Entry::Item(i) => {
                l.gutter
                    + est(&i.label)
                    + if i.shortcut.is_empty() {
                        0.0
                    } else {
                        l.gutter + est(&i.shortcut)
                    }
                    + l.pad_h
                    + 4.0
            }
            Entry::Sep => 0.0,
        })
        .fold(l.min_width, f32::max)
        .ceil();
    let mut col: iced::widget::Column<iced::Element<'_, Message>> = column![].spacing(0);
    for (i, entry) in open.entries.iter().enumerate() {
        match entry {
            Entry::Sep => {
                // 1px line after the icon gutter, vertically centered
                col = col.push(
                    container(
                        row![
                            Space::new().width(Length::Fixed(l.gutter)),
                            container(Space::new().width(Length::Fill).height(Length::Fixed(crate::theme::border_w(1.0))))
                                .width(Length::Fill)
                                .style(move |_t: &iced::Theme| container::Style {
                                    background: Some(skin.menu_section_border.into()),
                                    ..Default::default()
                                }),
                            Space::new().width(Length::Fixed(l.pad_h)),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .width(Length::Fill)
                    .height(Length::Fixed(l.separator_height))
                    .boxed(),
                );
            }
            Entry::Item(item) => col = col.push(item_row(i, item, open.hover == Some(i), l, skin)),
        }
    }
    let m = mouse_area(
        container(col)
            .width(Length::Fixed(content_w))
            .style(move |_t: &iced::Theme| container::Style {
                background: Some(skin.menu_bg.into()),
                border: iced::Border {
                    color: skin.menu_border,
                    width: crate::theme::border_w(1.0),
                    radius: l.corner_radius.into(),
                },
                shadow: iced::Shadow {
                    color: Color { a: 0.25, ..skin.text },
                    offset: iced::Vector::new(2.0, 2.0),
                    blur_radius: 6.0,
                },
                ..Default::default()
            }),
    )
    // Ribbon-style hover: ONE hit-test at the menu level (the row
    // geometry is fixed and known here). Per-item on_enter/on_exit
    // publishes both messages on a single CursorMoved in tree order —
    // moving up ended with the stale on_exit(None), so the highlight
    // only ever worked top-to-bottom.
    .on_move(move |pos| {
        Message::MenuHover(hover_at(&open.entries, pos.y, l))
    })
    .on_exit(Message::MenuHover(None));
    m.boxed()
}

/// The entry index under a menu-local `y`, or None over separators,
/// the border, or below the last item. Mirrors the fixed row geometry
/// of `view` (items of `item_height`, separators of `separator_height`,
/// content starting after the 1px border).
pub(crate) fn hover_at(entries: &[Entry], y: f32, l: &LayoutCfg) -> Option<usize> {
    let mut top = crate::theme::border_w(1.0);
    for (i, e) in entries.iter().enumerate() {
        let h = match e {
            Entry::Item(_) => l.item_height,
            Entry::Sep => l.separator_height,
        };
        if y >= top && y < top + h {
            return match e {
                Entry::Item(_) => Some(i),
                Entry::Sep => None,
            };
        }
        top += h;
    }
    None
}

fn item_row<'a>(
    i: usize,
    item: &'a Item,
    hovered: bool,
    l: &LayoutCfg,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let color = if item.enabled { skin.text } else { skin.disabled };
    // base on the resolved primary family (fonts.yaml priority list),
    // not Font::DEFAULT: an explicit .font() replaces the app's
    // default_font and would drop Selawik for generic sans-serif
    let bold = iced::Font {
        weight: if item.bold { iced::font::Weight::Bold } else { iced::font::Weight::Normal },
        ..crate::fonts::primary()
    };
    let icon_el: Element<Message> = match &item.icon {
        Some(name) => crate::ui_common::icon(name, l.icon_size, skin),
        None => Space::new().width(Length::Fixed(l.icon_size)).boxed(),
    };
    let mut r: iced::widget::Row<iced::Element<'_, Message>> = row![
        container(icon_el).width(Length::Fixed(l.gutter)).align_x(iced::alignment::Horizontal::Center),
        text(item.label.clone()).size(l.font_size).font(bold).color(color),
    ];
    if !item.shortcut.is_empty() {
        r = r
            .push(Space::new().width(Length::Fixed(l.gutter)).boxed())
            .push(text(item.shortcut.clone()).size(l.font_size).color(skin.subtle).boxed());
    }
    let mut cell = container(r.align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(l.item_height))
        .padding(iced::Padding { top: 0.0, right: l.pad_h, bottom: 0.0, left: 0.0 })
        .align_y(iced::alignment::Vertical::Center);
    if item.enabled && hovered {
        cell = cell.style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.hover.into()),
            border: iced::Border {
                color: skin.nav_hover_border,
                width: crate::theme::border_w(1.0),
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    }
    let area = mouse_area(cell);
    if item.enabled {
        area.on_press(Message::MenuPick(i)).boxed()
    } else {
        area.boxed()
    }
}

/// Right-click wrapper for the folder-view background: children first
/// (items capture their own right-click); an uncaptured right press
/// over the area publishes `on_free` (the background menu).
pub fn on_right_free<'a>(
    content: Element<'a, Message>,
    on_free: Message,
) -> Element<'a, Message> {
    struct OnRightFree<'a> {
        content: Element<'a, Message>,
        on_free: Message,
    }

    impl iced::advanced::widget::Meta for OnRightFree<'_> {}

    impl<'a> Widget<Message, iced::Theme, iced::Renderer> for OnRightFree<'a> {
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
            self.content.update(
                tree, event, layout, cursor, renderer, shell, viewport,
            );
            if matches!(
                event,
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            ) && !shell.is_event_captured()
                && let Some(p) = cursor.position_over(layout.bounds())
            {
                // CursorAt first (FIFO bus): the background menu
                // anchors at Explorer::cursor, which idle cursor moves
                // no longer refresh (main.rs drops them)
                shell.publish(Message::CursorAt(p));
                shell.publish(self.on_free.clone());
                shell.capture_event();
            }
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
            translation: Vector,
            window: Size,
        ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
            self.content
                
                .overlay(tree, layout, renderer, viewport, translation, window)
        }
    }

    OnRightFree { content, on_free }.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MenuCfg {
        // the shipped yaml must be valid and match the defaults contract
        serde_yaml::from_str(include_str!("../assets/menu.yaml")).unwrap()
    }

    fn ids(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item(i) => Some(i.id.clone()),
                Entry::Sep => None,
            })
            .collect()
    }

    #[test]
    fn hover_hit_test_matches_row_geometry() {
        let c = cfg();
        let l = &c.layout;
        let item = || Entry::Item(Item::plain("x", "x".into()));
        let entries = vec![item(), Entry::Sep, item()];
        let b = crate::theme::border_w(1.0);
        let h = l.item_height;
        let s = l.separator_height;
        // on the border / over a separator / below the last row: nothing
        assert_eq!(hover_at(&entries, b - 0.5, l), None);
        assert_eq!(hover_at(&entries, b + h + s / 2.0, l), None);
        assert_eq!(hover_at(&entries, b + 2.0 * h + s + 1.0, l), None);
        // the first and last rows resolve in both directions — the
        // hit-test is order-independent, unlike enter/exit pairs
        assert_eq!(hover_at(&entries, b + 0.5, l), Some(0));
        assert_eq!(hover_at(&entries, b + h - 0.5, l), Some(0));
        assert_eq!(hover_at(&entries, b + h + s + 0.5, l), Some(2));
        assert_eq!(hover_at(&entries, b + 2.0 * h + s - 0.5, l), Some(2));
    }

    fn resolve_all(c: &MenuCfg, specs: &[ItemSpec]) -> Vec<Entry> {
        specs.iter().map(|s| resolve(c, s)).collect()
    }

    #[test]
    fn shipped_yaml_loads_and_validates() {
        let mut c = cfg();
        c.validate();
        let m = resolve_all(&c, &c.menus.general);
        assert!(ids(&m).contains(&"open".to_string()));
    }

    #[test]
    fn selection_menu_baseline() {
        let c = cfg();
        let m = build_selection(&c, 1, true, false, &[]);
        let ids = ids(&m);
        // baseline order preserved, open is the bold default action
        assert_eq!(ids.first().map(String::as_str), Some("open"));
        assert!(ids.contains(&"rename".to_string()) && ids.contains(&"properties".to_string()));
        assert!(!matches!(m.first(), Some(Entry::Sep)));
        assert!(!matches!(m.last(), Some(Entry::Sep)));
        match &m[0] {
            Entry::Item(i) => assert!(i.bold),
            _ => panic!(),
        }
    }

    #[test]
    fn group_items_merge_without_duplicates() {
        // §3.4: imagefiles adds Preview/Print after Open (chained via
        // `after:`); selecting two images must not duplicate anything
        let c = cfg();
        let groups = groups_for(&c, &[PathBuf::from("/a/one.PNG"), PathBuf::from("/b/two.jpg")]);
        assert_eq!(groups, ["imagefiles"]);
        let m = build_selection(&c, 2, true, false, &groups);
        let ids = ids(&m);
        assert_eq!(ids.iter().filter(|&i| i == "print").count(), 1);
        assert_eq!(
            ids.iter().take(3).map(String::as_str).collect::<Vec<_>>(),
            ["open", "preview", "print"]
        );
        // rename is single-only: hidden for a multi-selection (§8)
        assert!(!ids.contains(&"rename".to_string()));
    }

    #[test]
    fn applicability_filters_dirs_and_multi() {
        let c = cfg();
        // open_with is files-only: hidden for a pure folder selection
        let m = build_selection(&c, 1, false, true, &[]);
        assert!(!ids(&m).contains(&"open_with".to_string()));
        let m = build_selection(&c, 1, true, true, &[]);
        // mixed: cannot operate on the whole selection
        assert!(!ids(&m).contains(&"open_with".to_string()));
    }

    #[test]
    fn single_folder_offers_open_in_new_window() {
        let c = cfg();
        // single folder: present; single file: hidden (files: false);
        // multiple folders: hidden (single: true)
        let m = build_selection(&c, 1, false, true, &[]);
        assert!(ids(&m).contains(&"open_new_window".to_string()));
        let m = build_selection(&c, 1, true, false, &[]);
        assert!(!ids(&m).contains(&"open_new_window".to_string()));
        let m = build_selection(&c, 2, false, true, &[]);
        assert!(!ids(&m).contains(&"open_new_window".to_string()));
        // images get Preview (type-group distinction)
        let m = build_selection(&c, 1, true, false, &["imagefiles".to_string()]);
        assert!(ids(&m).contains(&"preview".to_string()));
        let m = build_selection(&c, 1, true, false, &["textfiles".to_string()]);
        assert!(!ids(&m).contains(&"preview".to_string()));
    }

    #[test]
    fn extension_detection_is_case_insensitive() {
        let c = cfg();
        assert_eq!(detect_ext(&c, Path::new("Photo.JPEG")), Some(".jpeg".into()));
        assert_eq!(detect_ext(&c, Path::new("noext")), None);
    }

    #[test]
    fn unknown_commands_are_dropped_with_a_warning() {
        let mut c: MenuCfg = serde_yaml::from_str(
            "menus:\n  general:\n    - { command: bogus }\n    - { command: copy }\n    - { command: mine, run: [echo] }\n",
        )
        .unwrap();
        c.validate();
        let ids = ids(&resolve_all(&c, &c.menus.general));
        assert_eq!(ids, ["copy", "mine"]);
    }

    #[test]
    fn background_menu_paste_follows_clipboard() {
        let c = cfg();
        let m = build_background(&c, false);
        let paste = m.iter().find_map(|e| match e {
            Entry::Item(i) if i.id == "paste" => Some(i),
            _ => None,
        });
        assert_eq!(paste.map(|i| i.enabled), Some(false));
        let m = build_background(&c, true);
        let paste = m.iter().find_map(|e| match e {
            Entry::Item(i) if i.id == "paste" => Some(i),
            _ => None,
        });
        assert_eq!(paste.map(|i| i.enabled), Some(true));
    }

    #[test]
    fn menu_click_through_accepts_1_0_true_false() {
        // the shipped default: Windows one-click behavior
        let c = cfg();
        assert!(c.settings.menu_click_through.0);
        for (v, want) in [("0", false), ("false", false), ("1", true), ("true", true)] {
            let c: MenuCfg =
                serde_yaml::from_str(&format!("settings:\n  menu_click_through: {v}\n"))
                    .unwrap_or_else(|e| panic!("menu_click_through: {v} must parse: {e}"));
            assert_eq!(c.settings.menu_click_through.0, want, "menu_click_through: {v}");
        }
    }
}

/// Off-window popup painting (Wayland xdg_popup via the side channel):
/// a pixel sibling of [`view`] with the same geometry constants and
/// colors, but real text metrics (available here, unlike in view()).
/// Returns the premultiplied-RGBA pixels to upload.
pub fn paint_popup(
    open: &Open,
    cfg: &MenuCfg,
    skin: skin::Skin,
    scale: f32,
) -> Option<crate::dnd_wayland::DragVisual> {
    use crate::dnd_wayland::{draw_text_weighted, text_size_weighted, Canvas};

    let l = &cfg.layout;
    let scale = scale.max(1.0);
    let border = crate::theme::border_w(1.0);
    if open.entries.is_empty() {
        return None;
    }

    // --- width: real metrics, same paddings as `view` ---
    let mut content_w = l.min_width;
    for e in &open.entries {
        if let Entry::Item(i) = e {
            let label = text_size_weighted(&i.label, l.font_size * scale, i.bold).0 / scale;
            let w = l.gutter
                + label
                + if i.shortcut.is_empty() {
                    0.0
                } else {
                    l.gutter + text_size_weighted(&i.shortcut, l.font_size * scale, false).0 / scale
                }
                + l.pad_h
                + 4.0;
            content_w = content_w.max(w);
        }
    }
    let w = ((content_w + border * 2.0) * scale).ceil() as u32;

    // --- height: the fixed row geometry of `hover_at` ---
    let rows_h: f32 = open
        .entries
        .iter()
        .map(|e| match e {
            Entry::Item(_) => l.item_height,
            Entry::Sep => l.separator_height,
        })
        .sum();
    let h = ((rows_h + border * 2.0) * scale).ceil() as u32;

    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut cv = Canvas { px: &mut px, w, h };

    // drop shadow (the in-window menu's blur becomes a flat plate here)
    let shadow = [skin.text.r, skin.text.g, skin.text.b]
        .map(|c| (c * 255.0) as u8);
    cv.fill_rect(
        scale,
        border + 2.0,
        border + 2.0,
        content_w + border,
        rows_h + border,
        iced::Color::from_rgba8(shadow[0], shadow[1], shadow[2], 0.25),
        96,
    );

    // frame + background
    cv.fill_rect(1.0, 0.0, 0.0, w as f32, h as f32, skin.menu_bg, 255);
    cv.stroke_rect(0.0, 0.0, w as f32, h as f32, skin.menu_border);

    // --- rows ---
    let mut top = border;
    for (i, entry) in open.entries.iter().enumerate() {
        match entry {
            Entry::Sep => {
                // 1px line after the icon gutter, vertically centered
                let y = (top + l.separator_height / 2.0) * scale;
                let x0 = (border + l.gutter) * scale;
                let x1 = (border + content_w - l.pad_h) * scale;
                let line = crate::dnd_wayland::rgba255(skin.menu_section_border);
                let xx = x0 as i32;
                let yy = y as i32;
                for x in xx..x1 as i32 {
                    cv.blend(x, yy, line, 255);
                }
                top += l.separator_height;
            }
            Entry::Item(item) => {
                let row_top = top;
                if item.enabled && open.hover == Some(i) {
                    // fill and border both go through the same scale
                    cv.fill_rect(scale, border, row_top, content_w, l.item_height, skin.hover, 255);
                    cv.stroke_rect(border * scale, row_top * scale, content_w * scale, l.item_height * scale, skin.nav_hover_border);
                }
                // icon (rasterized from the active pack)
                if let Some(name) = &item.icon {
                    let icon = crate::icon::raster(name, (l.icon_size * scale).round() as u32, &skin);
                    let ix = border + (l.gutter - l.icon_size) / 2.0;
                    let iy = row_top + (l.item_height - l.icon_size) / 2.0;
                    cv.blend_icon(1.0, ix, iy, l.icon_size, &icon, 255);
                }
                let color = if item.enabled { skin.text } else { skin.disabled };
                let c = crate::dnd_wayland::rgba255(color);
                let size = l.font_size * scale;
                let (tw, th) = text_size_weighted(&item.label, size, item.bold);
                let tx = (border + l.gutter) * scale;
                let ty = (row_top + (l.item_height - th / scale) / 2.0) * scale;
                draw_text_weighted(&mut cv, tx, ty, size, &item.label, c, item.bold);
                if !item.shortcut.is_empty() {
                    let sx = (border + l.gutter + tw / scale + l.gutter) * scale;
                    let (sw, sh) = text_size_weighted(&item.shortcut, size, false);
                    let sy = (row_top + (l.item_height - sh / scale) / 2.0) * scale;
                    draw_text_weighted(&mut cv, sx, sy, size, &item.shortcut, crate::dnd_wayland::rgba255(skin.subtle), false);
                    let _ = sw;
                }
                top += l.item_height;
            }
        }
    }

    Some(crate::dnd_wayland::DragVisual {
        width: w,
        height: h,
        pixels: px,
    })
}

/// Menu layer for ui.rs (default in-window rendering): the floated menu
/// over a modal shield that swallows every pointer event — iced stacks
/// stop dispatching at the first captured event, and the LAST child is
/// on top (gets events first), so the menu handles its own hover/clicks
/// and the shield blocks everything behind it. A press outside the menu
/// closes it (`menu_click_through`: it also reaches the items behind).
pub fn layer<'a>(open: &'a Open, cfg: &'a MenuCfg, skin: skin::Skin) -> Element<'a, Message> {
    let (x, y) = (open.x, open.y);
    let menu = iced::widget::float(view(open, cfg, skin)).translate(
        move |bounds, viewport| {
            let tx = if x + bounds.width > viewport.width - 4.0 {
                x - bounds.width // flip: clicked point = right corner
            } else {
                x
            };
            let ty = if y + bounds.height > viewport.height - 4.0 {
                y - bounds.height // flip: clicked point = bottom corner
            } else {
                y
            };
            Vector::new(tx.max(0.0) - bounds.x, ty.max(0.0) - bounds.y)
        },
    );
    iced::widget::stack![
        Shield {
            on_press: Message::CloseMenu,
            click_through: cfg.settings.menu_click_through.0,
        }
        .boxed(),
        menu
    ]
    .boxed()
}

/// Modal shield under an open menu: captures all mouse/touch events so
/// the UI behind the menu stays inert; a press closes the menu. With
/// `menu_click_through` a press is NOT captured: it closes the menu and
/// also reaches the items behind it (one click dismisses AND activates,
/// Windows behavior).
impl iced::advanced::widget::Meta for Shield {}

struct Shield {
    on_press: Message,
    click_through: bool,
}

impl Widget<Message, iced::Theme, iced::Renderer> for Shield {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) {
        tree.size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &iced::Event,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(_))
            | iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                // the menu above runs first and captures its own presses
                if !shell.is_event_captured() {
                    shell.publish(self.on_press.clone());
                }
                // click-through: the press continues to the items behind
                // the menu; otherwise the shield swallows it
                if !self.click_through {
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(_) | iced::Event::Touch(_) => {
                shell.capture_event();
            }
            _ => {}
        }
    }
}

/// Menu host for ui.rs: a zero-size widget at the view root whose
/// `overlay()` floats the menu as an INDEPENDENT LAYER (menu.yaml
/// `layout.popup: true` — experimental; the runtime may not re-query
/// overlays right after the menu opens). Overlays get events before the
/// whole base UI, so the menu is modal by construction.
pub fn host<'a>(open: &'a Open, cfg: &'a MenuCfg, skin: skin::Skin) -> Element<'a, Message> {
    MenuHost { open, cfg, skin }.boxed()
}

struct MenuHost<'a> {
    open: &'a Open,
    cfg: &'a MenuCfg,
    skin: skin::Skin,
}

/// Host state: the widget tree of the menu content.
struct MenuHostState {
    tree: Tree,
}

impl Default for MenuHostState {
    fn default() -> Self {
        Self { tree: Tree::empty() }
    }
}

impl iced::advanced::widget::Meta for MenuHost<'_> {}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for MenuHost<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MenuHostState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MenuHostState::default())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        _limits: &layout::Limits,
    ) {
        tree.size = Size::ZERO;
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        _layout: Layout,
        renderer: &iced::Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let state = tree.state.downcast_mut::<MenuHostState>();
        // lay the menu out now (overlays self-position): the content
        // size is known from its tree after layout
        let mut content = view(self.open, self.cfg, self.skin);
        state.tree.diff(&mut content);
        content.layout(
            &mut state.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, window),
        );
        let size = state.tree.size;
        // anchor = left-top corner; flip to the right and/or bottom
        // corner when the menu would leave the window
        let (x, y) = (self.open.x, self.open.y);
        let tx = if x + size.width > window.width - 4.0 { x - size.width } else { x };
        let ty = if y + size.height > window.height - 4.0 { y - size.height } else { y };
        let menu_layout = Layout::new(size).move_to(Point::new(tx.max(0.0), ty.max(0.0)));
        vec![overlay::Element::new(Box::new(MenuOverlay {
            content,
            tree: &mut state.tree,
            layout: menu_layout,
            window,
            click_through: self.cfg.settings.menu_click_through.0,
        }))]
    }
}

/// The menu overlay: hosts the menu widget tree at the anchor point and
/// captures all pointer events (modal); `click_through` lets a
/// dismiss-press reach the UI behind the menu (see `Shield`).
struct MenuOverlay<'a> {
    content: Element<'a, Message>,
    tree: &'a mut Tree,
    layout: Layout,
    window: Size,
    click_through: bool,
}

impl<'a> overlay::Overlay<Message, iced::Theme, iced::Renderer> for MenuOverlay<'a> {
    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        renderer.with_layer(self.layout.bounds(), |renderer| {
            self.content.draw(
                self.tree,
                renderer,
                theme,
                style,
                self.layout,
                cursor,
                &Rectangle::with_size(self.window),
            );
        });
    }

    fn update(
        &mut self,
        event: &iced::Event,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        self.content.update(
            self.tree,
            event,
            self.layout,
            cursor,
            renderer,
            shell,
            &Rectangle::with_size(self.window),
        );
        // modal: no pointer event reaches the UI behind the menu
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(_))
            | iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if !shell.is_event_captured() {
                    shell.publish(Message::CloseMenu);
                }
                // click-through: the press continues to the UI behind
                if !self.click_through {
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(_) | iced::Event::Touch(_) => {
                shell.capture_event();
            }
            // keyboard passes through (Escape closes via handle_key)
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.mouse_interaction(
            self.tree,
            self.layout,
            cursor,
            &Rectangle::with_size(self.window),
            renderer,
        )
    }
}

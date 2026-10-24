//! Context menus — SPEC/menu_spec.md. ONE YAML file
//! (`assets/menu.yaml`; user override ~/.config/explorer/menu.yaml)
//! defines the menus (selection / background / archive / nav), the
//! file-type groups, the open backend and the Windows-style layout.
//!
//! This module owns: loading + validation (`MenuCfg::load`), file-type
//! detection (`groups_for`), menu construction (`build_*`), the open
//! backend + YAML-defined commands (`open_files` / `run_custom`) and the
//! reusable renderer (`view` + the `OnRightFree` background wrapper).

use std::path::{Path, PathBuf};

use iced::advanced::widget::{Operation, Tree, Widget, tree};
use iced::advanced::{Clipboard, Layout, Shell, layout, overlay, renderer};
use iced::widget::{Space, column, container, mouse_area, row, text};
use iced::{Color, Element, Length, Rectangle, Size, Vector, mouse};
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
}

impl Default for SettingsCfg {
    fn default() -> Self {
        Self {
            file_detection: "extension".into(),
            open_backend: "kde".into(),
            extensionless_group: String::new(),
            compound_extensions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
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

impl Default for OpenCfg {
    fn default() -> Self {
        Self {
            kde: KdeCfg::default(),
            handlers: Default::default(),
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
    "open_with", "print", "new_folder", "select_all", "refresh",
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
        if let Some(g) = group {
            if !out.contains(&g) {
                out.push(g);
            }
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
        if let Entry::Item(i) = e {
            if i.id == "paste" {
                i.enabled = can_paste;
            }
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
/// entries; the caller positions it (ui.rs floats it at the cursor).
pub fn view<'a>(open: &'a Open, cfg: &'a MenuCfg, skin: skin::Skin) -> Element<'a, Message> {
    let l = &cfg.layout;
    let mut col = column![].spacing(0);
    // min-width floor (columns shrink to their widest row)
    col = col.push(Space::new().width(Length::Fixed(l.min_width)).height(Length::Fixed(0.0)));
    for (i, entry) in open.entries.iter().enumerate() {
        match entry {
            Entry::Sep => {
                // 1px line after the icon gutter, vertically centered
                col = col.push(
                    container(
                        row![
                            Space::new().width(Length::Fixed(l.gutter)),
                            container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
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
                    .height(Length::Fixed(l.separator_height)),
                );
            }
            Entry::Item(item) => col = col.push(item_row(i, item, open.hover == Some(i), l, skin)),
        }
    }
    container(col)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(skin.menu_bg.into()),
            border: iced::Border {
                color: skin.menu_border,
                width: 1.0,
                radius: l.corner_radius.into(),
            },
            shadow: iced::Shadow {
                color: Color { a: 0.25, ..skin.text },
                offset: iced::Vector::new(2.0, 2.0),
                blur_radius: 6.0,
            },
            ..Default::default()
        })
        .into()
}

fn item_row<'a>(
    i: usize,
    item: &'a Item,
    hovered: bool,
    l: &LayoutCfg,
    skin: skin::Skin,
) -> Element<'a, Message> {
    let color = if item.enabled { skin.text } else { skin.disabled };
    let bold = iced::Font {
        weight: if item.bold { iced::font::Weight::Bold } else { iced::font::Weight::Normal },
        ..iced::Font::DEFAULT
    };
    let icon_el: Element<Message> = match &item.icon {
        Some(name) => crate::ui_common::icon(name, l.icon_size, skin),
        None => Space::new().width(Length::Fixed(l.icon_size)).into(),
    };
    let mut r = row![
        container(icon_el).width(Length::Fixed(l.gutter)).align_x(iced::alignment::Horizontal::Center),
        text(item.label.clone()).size(l.font_size).font(bold).color(color),
    ];
    if !item.shortcut.is_empty() {
        r = r
            .push(Space::new().width(Length::Fixed(l.gutter)))
            .push(text(item.shortcut.clone()).size(l.font_size).color(skin.subtle));
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
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    }
    let area = mouse_area(cell)
        .on_enter(Message::MenuHover(Some(i)))
        .on_exit(Message::MenuHover(None));
    if item.enabled {
        area.on_press(Message::MenuPick(i)).into()
    } else {
        area.into()
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

    impl<'a> Widget<Message, iced::Theme, iced::Renderer> for OnRightFree<'a> {
        fn size(&self) -> Size<Length> {
            self.content.as_widget().size()
        }
        fn tag(&self) -> tree::Tag {
            self.content.as_widget().tag()
        }
        fn state(&self) -> tree::State {
            self.content.as_widget().state()
        }
        fn children(&self) -> Vec<Tree> {
            self.content.as_widget().children()
        }
        fn diff(&self, tree: &mut Tree) {
            self.content.as_widget().diff(tree);
        }
        fn layout(
            &mut self,
            tree: &mut Tree,
            renderer: &iced::Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            self.content.as_widget_mut().layout(tree, renderer, limits)
        }
        fn draw(
            &self,
            tree: &Tree,
            renderer: &mut iced::Renderer,
            theme: &iced::Theme,
            style: &renderer::Style,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
        ) {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        fn operate(
            &mut self,
            tree: &mut Tree,
            layout: Layout<'_>,
            renderer: &iced::Renderer,
            operation: &mut dyn Operation,
        ) {
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        }
        fn update(
            &mut self,
            tree: &mut Tree,
            event: &iced::Event,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            renderer: &iced::Renderer,
            clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            viewport: &Rectangle,
        ) {
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
            if matches!(
                event,
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            ) && !shell.is_event_captured()
                && cursor.is_over(layout.bounds())
            {
                shell.publish(self.on_free.clone());
                shell.capture_event();
            }
        }
        fn mouse_interaction(
            &self,
            tree: &Tree,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
            renderer: &iced::Renderer,
        ) -> mouse::Interaction {
            self.content
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer)
        }
        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut Tree,
            layout: Layout<'b>,
            renderer: &iced::Renderer,
            viewport: &Rectangle,
            translation: Vector,
        ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
            self.content
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
        }
    }

    Element::new(OnRightFree { content, on_free })
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
        // §3.4: imagefiles adds Print after Open; selecting two images
        // must not duplicate anything
        let c = cfg();
        let groups = groups_for(&c, &[PathBuf::from("/a/one.PNG"), PathBuf::from("/b/two.jpg")]);
        assert_eq!(groups, ["imagefiles"]);
        let m = build_selection(&c, 2, true, false, &groups);
        let ids = ids(&m);
        assert_eq!(ids.iter().filter(|&i| i == "print").count(), 1);
        assert_eq!(
            ids.iter().position(|i| i == "print"),
            ids.iter().position(|i| i == "open").map(|p| p + 1)
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
}

/// Menu layer for ui.rs: a click-away catcher under the floated menu.
/// The menu is clamped inside the viewport.
pub fn layer<'a>(open: &'a Open, cfg: &'a MenuCfg, skin: skin::Skin) -> Element<'a, Message> {
    let (x, y) = (open.x, open.y);
    let menu = iced::widget::float(view(open, cfg, skin)).translate(
        move |bounds, viewport| {
            let tx = x.clamp(0.0, (viewport.width - bounds.width - 4.0).max(0.0));
            let ty = y.clamp(0.0, (viewport.height - bounds.height - 4.0).max(0.0));
            Vector::new(tx - bounds.x, ty - bounds.y)
        },
    );
    iced::widget::stack![
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::CloseMenu)
            .on_right_press(Message::CloseMenu),
        menu,
    ]
    .into()
}

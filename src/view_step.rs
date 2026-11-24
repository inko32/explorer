//! Folder view configuration as data, not code.
//!
//! Everything the spec fixes is configurable: the §3/§4 step→size map,
//! the §3 layout families (per-index ranges), the §5 presets, the §6
//! enabled mask, the §7 input handling and the §12 tunables, plus the
//! §8 layout metrics. Loaded from `assets/folder-view.yaml` by
//! [`ViewCfg::load`]; a user copy at ~/.config/explorer/folder-view.yaml
//! overrides it (same override path as ribbon.yaml). The serde defaults
//! mirror the shipped yaml.

use serde::Deserialize;

/// §3 step space: 48 indices by default (stable, never renumbered).
const DEFAULT_SIZES: [u16; 48] = [
    32, 48, 16, 16, // 0 Content, 1 Tiles, 2 Details, 3 List
    16, 17, 18, 19, 21, 22, 23, 25, 26, 28, 30, // 4..=14 small icons (name right)
    32, 34, 36, 40, 42, 44, 48, 50, 54, 58, 60, // 15..=25 icon-on-top (name below)
    64, 68, 72, 80, 84, 88, 96, 100, 108, 116, 120, // 26..=36
    128, 136, 144, 160, 168, 184, 192, 208, 224, 240, 256, // 37..=47
];

/// Full folder-view configuration (`assets/folder-view.yaml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ViewCfg {
    /// §4 icon edge length per step index (px at 100% scaling).
    pub sizes: Vec<u16>,
    /// §5 presets (View menu entries); preset steps can't be disabled.
    pub presets: PresetsCfg,
    /// §6/§12 EnabledMask: bit i = step reachable by Ctrl+Scroll
    /// (decimal or "0x..." hex).
    pub enabled_mask: MaskRepr,
    /// §7/§12 input tuning.
    pub input: InputCfg,
    /// §8.1 fixed layouts (their `index` in the step space).
    pub layouts: LayoutsCfg,
    /// §8.2 small-icon family (index range + metrics).
    pub small_icon: SmallIconCfg,
    /// §8.3 icon-on-top family (index range + metrics).
    pub icon_top: IconTopCfg,
    /// §8.4 common styling.
    pub style: StyleCfg,
}

impl Default for ViewCfg {
    fn default() -> Self {
        Self {
            sizes: DEFAULT_SIZES.to_vec(),
            presets: PresetsCfg::default(),
            enabled_mask: MaskRepr::default(),
            input: InputCfg::default(),
            layouts: LayoutsCfg::default(),
            small_icon: SmallIconCfg::default(),
            icon_top: IconTopCfg::default(),
            style: StyleCfg::default(),
        }
    }
}

/// §5 presets: the View menu / status bar / ribbon view commands point
/// at these step indices; their bits are forced on in the mask (§6).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct PresetsCfg {
    pub content: u8,
    pub tiles: u8,
    pub details: u8,
    pub list: u8,
    pub small: u8,
    pub medium: u8,
    pub large: u8,
    pub xlarge: u8,
}

impl Default for PresetsCfg {
    fn default() -> Self {
        Self {
            content: 0,
            tiles: 1,
            details: 2,
            list: 3,
            small: 4,
            medium: 21,
            large: 32,
            xlarge: 47,
        }
    }
}

/// §6 enabled mask as written in the yaml (decimal or hex string).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum MaskRepr {
    Int(u64),
    Str(String),
}

impl Default for MaskRepr {
    fn default() -> Self {
        MaskRepr::Int(u64::MAX)
    }
}

impl MaskRepr {
    fn bits(&self) -> u64 {
        match self {
            MaskRepr::Int(v) => *v,
            MaskRepr::Str(s) => {
                let t = s.trim();
                t.strip_prefix("0x")
                    .or_else(|| t.strip_prefix("0X"))
                    .and_then(|h| u64::from_str_radix(h, 16).ok())
                    .or_else(|| t.parse().ok())
                    .unwrap_or(u64::MAX)
            }
        }
    }
}

/// §7/§12 input tuning.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct InputCfg {
    /// NotchDelta: one step per ±this much accumulated wheel delta.
    pub notch_delta: f32,
    /// Rate limit: at most one step per this many ms (extra delta is
    /// dropped, not queued).
    pub step_interval_ms: u64,
    /// ScrollDirection: false = wheel up grows the icons (default).
    pub invert_direction: bool,
    /// §9 zoom animation duration (0 = no animation); parsed, the
    /// animation itself is not implemented yet.
    pub anim_ms: u64,
    /// Wheel scroll speed multiplier (explorer extension): the folder
    /// view and nav pane scroll 60px per wheel line unit times this.
    pub scroll_speed: f32,
}

impl Default for InputCfg {
    fn default() -> Self {
        Self {
            notch_delta: 120.0,
            step_interval_ms: 40,
            invert_direction: false,
            anim_ms: 120,
            scroll_speed: 2.0,
        }
    }
}

/// §8.1 fixed layouts, each pinned to its step index.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LayoutsCfg {
    pub content: ContentCfg,
    pub tiles: TilesCfg,
    pub details: DetailsCfg,
    pub list: ListCfg,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ContentCfg {
    pub index: u8,
    /// Row height: 32px icon + 2x8 padding = 48.
    pub row_h: f32,
    pub icon: f32,
    /// Name (semibold) and property-line font sizes.
    pub name_size: f32,
    pub prop_size: f32,
    pub spacing: f32,
    pub pad_h: f32,
    pub pad_w: f32,
    pub icon_text_gap: f32,
}

impl Default for ContentCfg {
    fn default() -> Self {
        Self {
            index: 0,
            row_h: 48.0,
            icon: 32.0,
            name_size: 12.0,
            prop_size: 12.0,
            spacing: 2.0,
            pad_h: 8.0,
            pad_w: 8.0,
            icon_text_gap: 8.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct TilesCfg {
    pub index: u8,
    pub cell_w: f32,
    pub cell_h: f32,
    pub icon: f32,
    pub name_size: f32,
    pub prop_size: f32,
    pub line_spacing: f32,
    pub icon_text_gap: f32,
    pub pad: f32,
}

impl Default for TilesCfg {
    fn default() -> Self {
        Self {
            index: 1,
            cell_w: 256.0,
            cell_h: 64.0,
            icon: 48.0,
            name_size: 12.0,
            prop_size: 12.0,
            line_spacing: 1.0,
            icon_text_gap: 8.0,
            pad: 8.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DetailsCfg {
    pub index: u8,
    /// DetailsRowHeight: normal and compact (compact is the default).
    pub row_h: f32,
    pub row_h_compact: f32,
    /// Vertical gap between rows, in border pixels (follows the
    /// border DPI unscale like `theme::border_w`): negative overlaps
    /// rows so adjacent item borders coincide (-1 shares a 1px
    /// border); positive is looser.
    pub row_gap: f32,
    /// Column header strip height (25px).
    /// The header metrics themselves live in details-view.yaml.
    pub header_h: f32,
    /// the space below the title bar,
    /// before the file rows (the file area starts below both).
    pub files_gap: f32,
    pub icon: f32,
}

impl Default for DetailsCfg {
    fn default() -> Self {
        Self {
            index: 2,
            row_h: 32.0,
            row_h_compact: 24.0,
            row_gap: -1.0,
            header_h: 25.0,
            files_gap: 6.0,
            icon: 16.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ListCfg {
    pub index: u8,
    pub row_h: f32,
    pub icon: f32,
    pub icon_text_gap: f32,
    /// Column width = col_w_base + longest name * per_char, clamped.
    pub col_w_base: f32,
    pub per_char: f32,
    pub col_w_min: f32,
    /// ListMaxColW.
    pub col_w_max: f32,
    /// Name unit budget (ASCII = 1, non-ASCII = 2); over-budget names
    /// are tail-cut with "...". The full name is shown otherwise.
    pub name_max_units: usize,
    pub pad_h: f32,
    pub pad_w: f32,
}

impl Default for ListCfg {
    fn default() -> Self {
        Self {
            index: 3,
            row_h: 24.0,
            icon: 16.0,
            icon_text_gap: 6.0,
            col_w_base: 30.0,
            per_char: 7.2,
            col_w_min: 48.0,
            col_w_max: 240.0,
            name_max_units: 100,
            pad_h: 0.0,
            pad_w: 4.0,
        }
    }
}

/// §8.2 small-icon family: `from..=to` in the step space (its end is
/// the §12 FlipIndex: the label moves below the icon at `to + 1`).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SmallIconCfg {
    pub from: u8,
    pub to: u8,
    /// Cell height = s + this.
    pub cell_h_extra: f32,
    pub icon_label_gap: f32,
    /// Label width = label_w_base + label_w_factor * (s - 16).
    pub label_w_base: f32,
    pub label_w_factor: f32,
    /// Cell width = s + gap + label + this.
    pub cell_w_tail: f32,
    pub grid_gap: f32,
}

impl Default for SmallIconCfg {
    fn default() -> Self {
        Self {
            from: 4,
            to: 14,
            cell_h_extra: 8.0,
            icon_label_gap: 6.0,
            label_w_base: 160.0,
            label_w_factor: 4.0,
            cell_w_tail: 8.0,
            grid_gap: 2.0,
        }
    }
}

/// §8.3 icon-on-top family.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct IconTopCfg {
    pub from: u8,
    pub to: u8,
    /// ThumbMinSize: smallest icon size that shows thumbnails (parsed;
    /// thumbnails are not implemented yet — the shell icon is shown).
    pub thumb_min: f32,
    /// ThumbBorder (parsed; not implemented yet).
    pub thumb_border: bool,
    /// Wmin: minimum cell width.
    pub w_min: f32,
    /// PadFactor: cell padding P = max(pad_min, floor(pad_factor * s)).
    pub pad_factor: f32,
    pub pad_min: f32,
    /// GapFactor: icon–label gap and grid gap = max(gap_min, floor(gap_factor * s)).
    pub gap_factor: f32,
    pub gap_min: f32,
    /// LabelLines: line count below / at-or-above the switch size.
    pub label_lines_small: u8,
    pub label_lines_switch: f32,
    pub label_lines_large: u8,
    /// Horizontal breathing room for the elided label.
    pub label_pad: f32,
}

impl Default for IconTopCfg {
    fn default() -> Self {
        Self {
            from: 15,
            to: 47,
            thumb_min: 48.0,
            thumb_border: true,
            w_min: 76.0,
            pad_factor: 0.08,
            pad_min: 8.0,
            gap_factor: 0.06,
            gap_min: 4.0,
            label_lines_small: 2,
            label_lines_switch: 96.0,
            label_lines_large: 4,
            label_pad: 8.0,
        }
    }
}

/// §8.4 common styling.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct StyleCfg {
    /// Hover/selection rounded-rectangle radius.
    pub corner_radius: f32,
    /// System UI font (9 pt = 12 px): all item labels.
    pub font_size: f32,
    /// Property lines (Content/Tiles) and the Details header.
    pub prop_size: f32,
    /// Empty strip reserved above the files in every layout without
    /// a header (Content/Tiles/List/SmallIcon/IconTop): the rubber
    /// band can start there. Details reserves its own strip
    /// (details.header_h + details.files_gap).
    pub top_gap: f32,
    /// Empty strip reserved left of the files in every layout without
    /// a header (Content/Tiles/List/SmallIcon/IconTop): the rubber
    /// band can reach or start there. Details reserves its own margin
    /// (details-view.yaml files_left), which its column header
    /// contains.
    pub left_gap: f32,
}

impl Default for StyleCfg {
    fn default() -> Self {
        Self {
            corner_radius: 4.0,
            font_size: 12.0,
            prop_size: 12.0,
            top_gap: 6.0,
            left_gap: 14.0,
        }
    }
}

/// §3 layout families: a group of steps that share one layout and
/// differ only in size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Content,
    Tiles,
    Details,
    List,
    SmallIcon,
    IconTop,
}

/// §8.2 small-icon cell metrics for icon size s.
pub struct SmallIconMetrics {
    pub cell_w: f32,
    pub cell_h: f32,
    pub label_w: f32,
    pub gap: f32,
}

/// §8.3 icon-on-top cell metrics for icon size s.
pub struct IconTopMetrics {
    pub cell_w: f32,
    pub cell_h: f32,
    pub pad: f32,
    pub gap: f32,
    /// Label line cap for this step (configured 2/4, never above 4).
    pub lines: f32,
}

/// Vertical gap between the icon area and the label inside an
/// icon-top cell. Fixed (not `gap_factor`-derived): the label font
/// stays the same size at every icon step.
pub const ICON_LABEL_GAP: f32 = 3.0;
/// Cell padding below the label box (label-box bottom to the chrome
/// edge). Smaller than the top/side pads: the label box already
/// carries its LABEL_LINE_BOTTOM inset, and the visual text-to-edge
/// space is LABEL_PAD_BOTTOM + LABEL_LINE_BOTTOM + the line's own
/// half-leading.
pub const LABEL_PAD_BOTTOM: f32 = 2.0;
/// Vertical gap between two wrapped icon-top label lines (glyph
/// bottom to next glyph top).
pub const LABEL_LINE_GAP: f32 = 1.0;
/// Padding below the last icon-top label line to the item chrome edge.
pub const LABEL_LINE_BOTTOM: f32 = 2.0;
/// Portion of the font descent cropped from each label line box. The
/// descent (~3.5px at 12px) is empty under most characters, and
/// keeping it in the box is what made multi-line labels look loose;
/// real descenders (g/j/p/q/y) dip this many pixels into the gap or
/// the bottom pad below — nothing clips (the container keeps the
/// LABEL_LINE_BOTTOM pad and its own cell padding is empty).
pub const LABEL_DESCENT_SHAVE: f32 = 1.5;

/// Height of the icon-top label box for `lines` lines of `block`
/// pixels each: the line boxes stack flush (each cropped by
/// LABEL_DESCENT_SHAVE), LABEL_LINE_GAP between them,
/// LABEL_LINE_BOTTOM below the last line.
pub fn label_box_h(lines: f32, block: f32) -> f32 {
    let line = (block - LABEL_DESCENT_SHAVE).max(1.0);
    lines * line + (lines - 1.0).max(0.0) * LABEL_LINE_GAP + LABEL_LINE_BOTTOM
}

/// §8.3 icon-top grid with dynamic row heights: the row pitch follows
/// the tallest label in the row, while each item's chrome (border /
/// hover / selection area) is only as tall as its own label needs
/// (icon + 1..=cap lines). Built from the per-entry label line counts
/// (see `Explorer::refresh_it_label_lines`).
pub struct IconTopRows {
    /// grid columns at the pane width
    pub cols: usize,
    /// y offset of each grid row (prefix sums of the row pitches)
    pub starts: Vec<f32>,
    /// total content height (drives the scrollbar extent)
    pub extent: f32,
    /// chrome height of each item: pad + s + ICON_LABEL_GAP +
    /// label_box_h(lines) + LABEL_PAD_BOTTOM
    pub item_h: Vec<f32>,
}

impl ViewCfg {
    /// Load the config: ~/.config/explorer/folder-view.yaml overrides
    /// the embedded `assets/folder-view.yaml`; a broken file falls back
    /// to the defaults.
    pub fn load() -> Self {
        let text = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .and_then(|h| std::fs::read_to_string(h.join(".config/explorer/folder-view.yaml")).ok())
            .unwrap_or_else(|| include_str!("../assets/folder-view.yaml").to_string());
        match serde_yaml::from_str::<ViewCfg>(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("folder-view.yaml: {e} — using defaults");
                ViewCfg::default()
            }
        }
    }

    /// The step space size (indices 0..steps).
    pub fn steps(&self) -> u8 {
        self.sizes.len().clamp(1, 64) as u8
    }

    /// Icon size (px at 100% scaling) for a step index.
    pub fn icon_px(&self, i: u8) -> f32 {
        self.sizes[(i as usize).min(self.sizes.len().saturating_sub(1))] as f32
    }

    /// The layout family of a step index (§3 index map).
    pub fn family(&self, i: u8) -> Family {
        let l = &self.layouts;
        if i == l.content.index {
            Family::Content
        } else if i == l.tiles.index {
            Family::Tiles
        } else if i == l.details.index {
            Family::Details
        } else if i == l.list.index {
            Family::List
        } else if (self.small_icon.from..=self.small_icon.to).contains(&i) {
            Family::SmallIcon
        } else {
            Family::IconTop
        }
    }

    /// §6: preset steps can't be disabled.
    pub fn preset_mask(&self) -> u64 {
        let p = &self.presets;
        [p.content, p.tiles, p.details, p.list, p.small, p.medium, p.large, p.xlarge]
            .iter()
            .fold(0u64, |m, &i| m | (1u64 << i.min(63)))
    }

    /// All bits of the step space set.
    pub fn all_enabled(&self) -> u64 {
        let n = self.steps();
        if n >= 64 {
            u64::MAX
        } else {
            (1u64 << n) - 1
        }
    }

    /// Force the preset bits on and drop bits outside the step space (§6).
    pub fn sanitize(&self, mask: u64) -> u64 {
        (mask & self.all_enabled()) | self.preset_mask()
    }

    /// The configured enabled mask (§6/§12 EnabledMask), sanitized.
    pub fn mask(&self) -> u64 {
        self.sanitize(self.enabled_mask.bits())
    }

    /// §7 scrolling: the next enabled index in the scroll direction.
    /// Bounds: stops at the first/last index — no wrap-around.
    pub fn next_enabled(&self, i: u8, up: bool) -> u8 {
        let mask = self.mask();
        let last = self.steps() - 1;
        let i = i.min(last);
        if up {
            (i + 1..=last).find(|&j| mask >> j & 1 == 1).unwrap_or(i)
        } else {
            (0..i).rev().find(|&j| mask >> j & 1 == 1).unwrap_or(i)
        }
    }

    /// §6 saved-index-disabled rule: snap to the nearest enabled index;
    /// on a tie pick the smaller one.
    pub fn nearest_enabled(&self, i: u8) -> u8 {
        let mask = self.mask();
        let last = self.steps() - 1;
        let i = i.min(last);
        if mask >> i & 1 == 1 {
            return i;
        }
        let lower = (0..i).rev().find(|&j| mask >> j & 1 == 1);
        let upper = (i + 1..=last).find(|&j| mask >> j & 1 == 1);
        match (lower, upper) {
            (Some(l), Some(u)) if u - i < i - l => u,
            (Some(l), _) => l,
            (None, Some(u)) => u,
            (None, None) => self.presets.details.min(last), // unreachable: presets forced on
        }
    }

    /// §10 compatibility: map an external pixel size to the nearest
    /// index by size (ties pick the smaller index).
    #[allow(dead_code)] // spec hook: no external pixel-size setting exists yet
    pub fn nearest_by_size(&self, px: u16) -> u8 {
        (0..self.steps())
            .min_by_key(|&i| self.sizes[i as usize].abs_diff(px))
            .unwrap_or(0)
    }

    /// §8.1 Details row height for the current mode.
    pub fn details_row_h(&self, compact: bool) -> f32 {
        let d = &self.layouts.details;
        if compact { d.row_h_compact } else { d.row_h }
    }

    /// §8.1 Details row pitch: row height plus the effective row gap.
    /// The configured gap is in border pixels, so it goes through the
    /// same DPI unscale as the chrome borders (`theme::border_w`). All
    /// scroll math (extent, anchors, hit tests) must use the pitch,
    /// never the bare row height. Clamped so a hostile gap can't
    /// break the math.
    pub fn details_pitch(&self, compact: bool) -> f32 {
        let gap = crate::theme::border_w(self.layouts.details.row_gap);
        (self.details_row_h(compact) + gap).max(1.0)
    }

    /// §8.2 small-icon cell metrics for icon size s.
    pub fn small_icon(&self, s: f32) -> SmallIconMetrics {
        let c = &self.small_icon;
        let label_w = c.label_w_base + c.label_w_factor * (s - 16.0);
        SmallIconMetrics {
            cell_w: s + c.icon_label_gap + label_w + c.cell_w_tail,
            cell_h: s + c.cell_h_extra,
            label_w,
            gap: c.grid_gap,
        }
    }

    /// §8.3 icon-on-top cell metrics for icon size s.
    pub fn icon_top(&self, s: f32) -> IconTopMetrics {
        let c = &self.icon_top;
        let pad = (c.pad_factor * s).floor().max(c.pad_min);
        let gap = (c.gap_factor * s).floor().max(c.gap_min);
        let lines = (if s >= c.label_lines_switch {
            c.label_lines_large
        } else {
            c.label_lines_small
        } as f32)
            .min(4.0);
        let block = crate::text_metrics::line_block(self.style.font_size, crate::fonts::primary());
        IconTopMetrics {
            cell_w: (s + 2.0 * pad).max(c.w_min),
            // icon area pinned to s (the image widget's height follows
            // the graphic's aspect ratio, not s) — see icon_top_cell
            cell_h: pad + s + ICON_LABEL_GAP + label_box_h(lines, block) + LABEL_PAD_BOTTOM,
            pad,
            gap,
            lines,
        }
    }

    /// §8.3 dynamic row heights from the per-entry label line counts
    /// (one per entry, 1..=cap; shorter/defensive input is truncated).
    pub fn icon_top_rows(
        &self,
        index: u8,
        n_items: usize,
        pane_w: f32,
        label_lines: &[u8],
    ) -> IconTopRows {
        let s = self.icon_px(index);
        let m = self.icon_top(s);
        let n = n_items.min(label_lines.len());
        let cols = self.grid_cols_at(index, m.cell_w, m.gap, pane_w);
        let block = crate::text_metrics::line_block(self.style.font_size, crate::fonts::primary());
        let item_h: Vec<f32> = label_lines[..n]
            .iter()
            .map(|&l| {
                m.pad + s + ICON_LABEL_GAP + label_box_h((l as f32).max(1.0), block)
                    + LABEL_PAD_BOTTOM
            })
            .collect();
        let mut starts = Vec::with_capacity(n.div_ceil(cols));
        // the first row sits below the reserved files_top strip
        let mut y = self.files_top(index);
        for r in 0..n.div_ceil(cols) {
            starts.push(y);
            let row_max = item_h[r * cols..((r + 1) * cols).min(n)]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            y += row_max + m.gap;
        }
        IconTopRows {
            cols,
            starts,
            extent: y,
            item_h,
        }
    }

    /// §8.1 List column width: the longest name, capped at ListMaxColW.
    pub fn list_col_w(&self, longest_name_chars: usize) -> f32 {
        let c = &self.layouts.list;
        (c.col_w_base + longest_name_chars as f32 * c.per_char)
            .clamp(c.col_w_min, c.col_w_max)
    }

    /// Empty strip above the files in content coordinates (px) —
    /// reserved space where the rubber band may start. Details
    /// reserves its own strip (details.header_h + details.files_gap,
    /// see the Details branches below); every other family reserves
    /// style.top_gap.
    pub fn files_top(&self, index: u8) -> f32 {
        if self.family(index) == Family::Details {
            0.0
        } else {
            self.style.top_gap
        }
    }

    /// Empty strip left of the files in content coordinates (px) —
    /// reserved space the rubber band can reach or start in. Details
    /// reserves its own margin (detail_cfg.files_left, contained by
    /// the column header); every other family reserves
    /// style.left_gap.
    pub fn files_left(&self, index: u8) -> f32 {
        if self.family(index) == Family::Details {
            0.0
        } else {
            self.style.left_gap
        }
    }

    /// Grid columns for the file view at a pane width: the headerless
    /// grid layouts lay out right of the reserved files_left strip.
    pub fn grid_cols_at(&self, index: u8, cell_w: f32, gap: f32, pane_w: f32) -> usize {
        grid_cols(cell_w, gap, (pane_w - self.files_left(index)).max(0.0))
    }

    /// §8.1 List rows per column (columns flow top to bottom, below
    /// the reserved files_top strip).
    pub fn list_rows_per_col(&self, viewport_h: f32) -> usize {
        (((viewport_h - self.style.top_gap).max(0.0) / self.layouts.list.row_h).floor() as usize)
            .max(1)
    }

    /// Content extent along the scroll axis (px; y, or x for List) —
    /// drives the scrollbar strip geometry and its collapse rule (the
    /// strip goes away when the content fits the viewport).
    /// `label_lines` are the §8.3 per-entry label line counts (only
    /// read for the icon-top family; pass an empty slice otherwise).
    // metric inputs are independent; the callers (folder view, pane
    // width probe) own them
    #[allow(clippy::too_many_arguments)]
    pub fn content_extent(
        &self,
        index: u8,
        n_items: usize,
        pane_w: f32,
        viewport_h: f32,
        longest_chars: usize,
        label_lines: &[u8],
        details_compact: bool,
    ) -> f32 {
        let l = &self.layouts;
        match self.family(index) {
            // content starts below the reserved files_top strip
            Family::Content => {
                self.files_top(index) + n_items as f32 * l.content.row_h
            }
            Family::Details => {
                l.details.header_h
                    + l.details.files_gap
                    + n_items as f32 * self.details_pitch(details_compact)
            }
            Family::List => {
                // the columns sit right of the reserved files_left strip
                self.files_left(index)
                    + n_items.div_ceil(self.list_rows_per_col(viewport_h)) as f32
                        * self.list_col_w(longest_chars)
            }
            Family::Tiles => {
                self.files_top(index)
                    + n_items
                        .div_ceil(self.grid_cols_at(index, l.tiles.cell_w, 0.0, pane_w))
                        as f32
                        * l.tiles.cell_h
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                self.files_top(index)
                    + n_items
                        .div_ceil(self.grid_cols_at(index, m.cell_w, m.gap, pane_w))
                        as f32
                        * (m.cell_h + m.gap)
            }
            // icon_top_rows folds files_top into the row starts
            Family::IconTop => {
                self.icon_top_rows(index, n_items, pane_w, label_lines).extent
            }
        }
    }

    /// §9 scroll anchor: the item at the viewport's leading edge in the
    /// given layout (`offset` is the scroll y, or x for List).
    // metric inputs are independent; the caller (folder view) owns them
    #[allow(clippy::too_many_arguments)]
    pub fn first_visible(
        &self,
        index: u8,
        offset: f32,
        pane_w: f32,
        viewport_h: f32,
        n_items: usize,
        longest_chars: usize,
        label_lines: &[u8],
        details_compact: bool,
    ) -> usize {
        let l = &self.layouts;
        let item = match self.family(index) {
            Family::Content => {
                ((offset - self.files_top(index)).max(0.0) / l.content.row_h).floor() as usize
            }
            Family::Details => {
                // content starts below the header strip + the §12 gap
                let y = (offset - (l.details.header_h + l.details.files_gap)).max(0.0);
                (y / self.details_pitch(details_compact)).floor() as usize
            }
            Family::Tiles => {
                ((offset - self.files_top(index)).max(0.0) / l.tiles.cell_h).floor() as usize
                    * self.grid_cols_at(index, l.tiles.cell_w, 0.0, pane_w)
            }
            Family::List => {
                ((offset - self.files_left(index)).max(0.0) / self.list_col_w(longest_chars))
                    .floor() as usize
                    * self.list_rows_per_col(viewport_h)
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                ((offset - self.files_top(index)).max(0.0) / (m.cell_h + m.gap)).floor() as usize
                    * self.grid_cols_at(index, m.cell_w, m.gap, pane_w)
            }
            // icon_top_rows folds files_top into the row starts
            Family::IconTop => {
                let rows = self.icon_top_rows(index, n_items, pane_w, label_lines);
                // last row whose start is at or above the offset
                let r = rows
                    .starts
                    .partition_point(|&y| y <= offset)
                    .saturating_sub(1)
                    .min(rows.starts.len().saturating_sub(1));
                r * rows.cols
            }
        };
        item.min(n_items.saturating_sub(1))
    }

    /// §9 scroll anchor: the leading-edge offset of an item in the given
    /// layout (y, or x for List).
    // metric inputs are independent; the caller (scroll anchor) owns them
    #[allow(clippy::too_many_arguments)]
    pub fn item_offset(
        &self,
        index: u8,
        item: usize,
        pane_w: f32,
        viewport_h: f32,
        longest_chars: usize,
        label_lines: &[u8],
        details_compact: bool,
    ) -> f32 {
        let l = &self.layouts;
        match self.family(index) {
            Family::Content => self.files_top(index) + item as f32 * l.content.row_h,
            Family::Details => {
                l.details.header_h
                    + l.details.files_gap
                    + item as f32 * self.details_pitch(details_compact)
            }
            Family::Tiles => {
                self.files_top(index)
                    + (item / self.grid_cols_at(index, l.tiles.cell_w, 0.0, pane_w)) as f32
                        * l.tiles.cell_h
            }
            Family::List => {
                self.files_left(index)
                    + (item / self.list_rows_per_col(viewport_h)) as f32
                        * self.list_col_w(longest_chars)
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                self.files_top(index)
                    + (item / self.grid_cols_at(index, m.cell_w, m.gap, pane_w)) as f32
                        * (m.cell_h + m.gap)
            }
            // icon_top_rows folds files_top into the row starts
            Family::IconTop => {
                let rows = self.icon_top_rows(index, item + 1, pane_w, label_lines);
                let r = (item / rows.cols).min(rows.starts.len().saturating_sub(1));
                rows.starts.get(r).copied().unwrap_or(rows.extent)
            }
        }
    }
}

/// Grid columns at a pane width (Tiles / SmallIcon / IconTop).
pub fn grid_cols(cell_w: f32, gap: f32, pane_w: f32) -> usize {
    (((pane_w + gap) / (cell_w + gap)).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_table_anchors() {
        // §3 anchor rule: the size doubles about every 11 steps
        let cfg = ViewCfg::default();
        assert_eq!(cfg.steps(), 48);
        assert_eq!(cfg.sizes[4], 16);
        assert_eq!(cfg.sizes[15], 32);
        assert_eq!(cfg.sizes[26], 64);
        assert_eq!(cfg.sizes[37], 128);
        assert_eq!(cfg.sizes[47], 256);
    }

    #[test]
    fn full_ladder_stops_at_bounds() {
        // acceptance check 1: from index 0, 47 wheel-up notches reach
        // index 47; one more notch leaves the index unchanged
        let cfg = ViewCfg::default();
        let mut i = 0;
        for _ in 0..47 {
            i = cfg.next_enabled(i, true);
        }
        assert_eq!(i, 47);
        assert_eq!(cfg.next_enabled(47, true), 47);
        assert_eq!(cfg.next_enabled(0, false), 0);
    }

    #[test]
    fn disabled_steps_are_skipped() {
        // acceptance check 3: with steps 5..=7 disabled, one notch up
        // from index 4 goes to index 8 (and back down to 4)
        let mut cfg = ViewCfg::default();
        cfg.enabled_mask = MaskRepr::Int(cfg.all_enabled() & !(0b111 << 5));
        assert_eq!(cfg.next_enabled(4, true), 8);
        assert_eq!(cfg.next_enabled(8, false), 4);
    }

    #[test]
    fn presets_cannot_be_disabled() {
        let mut cfg = ViewCfg::default();
        cfg.enabled_mask = MaskRepr::Int(!cfg.preset_mask());
        let mask = cfg.mask();
        let p = &cfg.presets;
        for i in [p.content, p.tiles, p.details, p.list, p.small, p.medium, p.large, p.xlarge] {
            assert_eq!(mask >> i & 1, 1, "preset {i} forced on");
        }
    }

    #[test]
    fn saved_disabled_index_snaps_to_nearest() {
        let mut cfg = ViewCfg::default();
        cfg.enabled_mask = MaskRepr::Int(cfg.all_enabled() & !(1 << 10));
        assert_eq!(cfg.nearest_enabled(10), 9); // tie: smaller wins
        cfg.enabled_mask = MaskRepr::Int(cfg.all_enabled() & !(1 << 10) & !(1 << 9) & !(1 << 8));
        assert_eq!(cfg.nearest_enabled(10), 11); // 11 nearer than 7
        assert_eq!(cfg.nearest_enabled(4), 4); // enabled: unchanged
    }

    #[test]
    fn pixel_size_maps_to_nearest_index() {
        let cfg = ViewCfg::default();
        assert_eq!(cfg.nearest_by_size(96), cfg.presets.large);
        assert_eq!(cfg.nearest_by_size(95), cfg.presets.large);
        assert_eq!(cfg.nearest_by_size(256), cfg.presets.xlarge);
    }

    #[test]
    fn hex_mask_string_parses() {
        let cfg: ViewCfg = serde_yaml::from_str("enabled_mask: \"0xff\"").unwrap();
        assert_eq!(cfg.mask() & 0xff, 0xff | cfg.preset_mask() & 0xff);
    }

    #[test]
    fn icon_top_rows_follow_the_tallest_label() {
        // §8.3 dynamic row heights: 4 items, 2 columns; item 2 wraps
        // to 3 lines while the rest stay at 1
        let cfg = ViewCfg::default();
        let s = cfg.icon_px(cfg.presets.medium);
        let m = cfg.icon_top(s);
        let block = crate::text_metrics::line_block(cfg.style.font_size, crate::fonts::primary());
        let one = m.pad + s + ICON_LABEL_GAP + label_box_h(1.0, block) + LABEL_PAD_BOTTOM;
        let lines = [1u8, 1, 3, 1];
        // a width that fits exactly 2 columns (right of the reserved
        // files_left strip)
        let w = cfg.files_left(cfg.presets.medium) + 2.0 * (m.cell_w + m.gap) - m.gap;
        let rows = cfg.icon_top_rows(cfg.presets.medium, 4, w, &lines);
        // the first row sits below the reserved files_top strip
        let top = cfg.files_top(cfg.presets.medium);
        assert_eq!(rows.cols, 2);
        // row 0: both 1 line; row 1: tallest is 3 lines
        assert_eq!(rows.starts[0], top);
        assert_eq!(rows.starts[1], top + one + m.gap);
        assert_eq!(
            rows.extent,
            top + 2.0 * (one + m.gap) + (label_box_h(3.0, block) - label_box_h(1.0, block))
        );
        // each item's chrome is only as tall as its own label
        assert_eq!(rows.item_h[0], one);
        assert_eq!(
            rows.item_h[2],
            one + (label_box_h(3.0, block) - label_box_h(1.0, block))
        );
    }

    #[test]
    fn shipped_yaml_matches_the_defaults() {
        // assets/folder-view.yaml documents the defaults — it must parse
        // to exactly them, or the file and the code disagree
        let yaml: ViewCfg =
            serde_yaml::from_str(include_str!("../assets/folder-view.yaml")).unwrap();
        let d = ViewCfg::default();
        assert_eq!(yaml.sizes, d.sizes);
        assert_eq!(yaml.steps(), 48);
        assert_eq!(yaml.layouts.content.row_h, d.layouts.content.row_h);
        assert_eq!(yaml.layouts.details.header_h, d.layouts.details.header_h);
        assert_eq!(yaml.layouts.details.files_gap, d.layouts.details.files_gap);
        assert_eq!(yaml.layouts.list.name_max_units, d.layouts.list.name_max_units);
        assert_eq!(yaml.icon_top.label_lines_switch, d.icon_top.label_lines_switch);
        assert_eq!(yaml.style.top_gap, d.style.top_gap);
        assert_eq!(yaml.style.left_gap, d.style.left_gap);
        assert_eq!(yaml.input.notch_delta, d.input.notch_delta);
    }
}

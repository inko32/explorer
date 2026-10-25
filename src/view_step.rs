//! Folder view configuration — SPEC/FOLDER-SPEC.MD as data, not code.
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
    /// Column header strip (overlays the scrollable's top).
    pub header_h: f32,
    pub icon: f32,
    pub icon_cell_w: f32,
    pub cell_padding_v: f32,
    pub cell_padding_h: f32,
    /// Adjustable column widths (Date modified / Type / Size); the Name
    /// column takes the remaining width.
    pub col_widths: Vec<f32>,
    pub col_min: f32,
    pub col_max: f32,
}

impl Default for DetailsCfg {
    fn default() -> Self {
        Self {
            index: 2,
            row_h: 32.0,
            row_h_compact: 24.0,
            header_h: 24.0,
            icon: 16.0,
            icon_cell_w: 26.0,
            cell_padding_v: 2.0,
            cell_padding_h: 4.0,
            col_widths: vec![170.0, 140.0, 90.0],
            col_min: 40.0,
            col_max: 600.0,
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
    pub label_line_h: f32,
    /// Horizontal breathing room for the elided label.
    pub label_pad: f32,
    /// px per label character (elide estimate).
    pub per_char: f32,
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
            label_lines_large: 3,
            label_line_h: 20.0,
            label_pad: 8.0,
            per_char: 6.5,
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
}

impl Default for StyleCfg {
    fn default() -> Self {
        Self {
            corner_radius: 4.0,
            font_size: 12.0,
            prop_size: 12.0,
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
    pub lines: f32,
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

    /// Initial Details column widths (Date modified / Type / Size),
    /// padded to 3 with the yaml defaults.
    pub fn details_col_widths(&self) -> [f32; 3] {
        let d = DetailsCfg::default();
        let w = &self.layouts.details.col_widths;
        [
            w.first().copied().unwrap_or(d.col_widths[0]),
            w.get(1).copied().unwrap_or(d.col_widths[1]),
            w.get(2).copied().unwrap_or(d.col_widths[2]),
        ]
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
        let lines = if s >= c.label_lines_switch {
            c.label_lines_large
        } else {
            c.label_lines_small
        } as f32;
        IconTopMetrics {
            cell_w: (s + 2.0 * pad).max(c.w_min),
            cell_h: pad + s + gap + lines * c.label_line_h + pad,
            pad,
            gap,
            lines,
        }
    }

    /// §8.1 List column width: the longest name, capped at ListMaxColW.
    pub fn list_col_w(&self, longest_name_chars: usize) -> f32 {
        let c = &self.layouts.list;
        (c.col_w_base + longest_name_chars as f32 * c.per_char)
            .clamp(c.col_w_min, c.col_w_max)
    }

    /// §8.1 List rows per column (columns flow top to bottom).
    fn list_rows_per_col(&self, viewport_h: f32) -> usize {
        ((viewport_h / self.layouts.list.row_h).floor() as usize).max(1)
    }

    /// Content extent along the scroll axis (px; y, or x for List) —
    /// drives the scrollbar strip geometry and its collapse rule (the
    /// strip goes away when the content fits the viewport).
    pub fn content_extent(
        &self,
        index: u8,
        n_items: usize,
        pane_w: f32,
        viewport_h: f32,
        longest_chars: usize,
        details_compact: bool,
    ) -> f32 {
        let l = &self.layouts;
        match self.family(index) {
            Family::Content => n_items as f32 * l.content.row_h,
            Family::Details => {
                l.details.header_h + n_items as f32 * self.details_row_h(details_compact)
            }
            Family::List => {
                n_items.div_ceil(self.list_rows_per_col(viewport_h)) as f32
                    * self.list_col_w(longest_chars)
            }
            Family::Tiles => {
                n_items.div_ceil(grid_cols(l.tiles.cell_w, 0.0, pane_w)) as f32 * l.tiles.cell_h
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                n_items.div_ceil(grid_cols(m.cell_w, m.gap, pane_w)) as f32 * (m.cell_h + m.gap)
            }
            Family::IconTop => {
                let m = self.icon_top(self.icon_px(index));
                n_items.div_ceil(grid_cols(m.cell_w, m.gap, pane_w)) as f32 * (m.cell_h + m.gap)
            }
        }
    }

    /// §9 scroll anchor: the item at the viewport's leading edge in the
    /// given layout (`offset` is the scroll y, or x for List).
    pub fn first_visible(
        &self,
        index: u8,
        offset: f32,
        pane_w: f32,
        viewport_h: f32,
        n_items: usize,
        longest_chars: usize,
        details_compact: bool,
    ) -> usize {
        let l = &self.layouts;
        let item = match self.family(index) {
            Family::Content => (offset / l.content.row_h).floor() as usize,
            Family::Details => {
                // content starts below the header strip
                let y = (offset - l.details.header_h).max(0.0);
                (y / self.details_row_h(details_compact)).floor() as usize
            }
            Family::Tiles => {
                (offset / l.tiles.cell_h).floor() as usize
                    * grid_cols(l.tiles.cell_w, 0.0, pane_w)
            }
            Family::List => {
                (offset / self.list_col_w(longest_chars)).floor() as usize
                    * self.list_rows_per_col(viewport_h)
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                (offset / (m.cell_h + m.gap)).floor() as usize
                    * grid_cols(m.cell_w, m.gap, pane_w)
            }
            Family::IconTop => {
                let m = self.icon_top(self.icon_px(index));
                (offset / (m.cell_h + m.gap)).floor() as usize
                    * grid_cols(m.cell_w, m.gap, pane_w)
            }
        };
        item.min(n_items.saturating_sub(1))
    }

    /// §9 scroll anchor: the leading-edge offset of an item in the given
    /// layout (y, or x for List).
    pub fn item_offset(
        &self,
        index: u8,
        item: usize,
        pane_w: f32,
        viewport_h: f32,
        longest_chars: usize,
        details_compact: bool,
    ) -> f32 {
        let l = &self.layouts;
        match self.family(index) {
            Family::Content => item as f32 * l.content.row_h,
            Family::Details => {
                l.details.header_h + item as f32 * self.details_row_h(details_compact)
            }
            Family::Tiles => {
                (item / grid_cols(l.tiles.cell_w, 0.0, pane_w)) as f32 * l.tiles.cell_h
            }
            Family::List => {
                (item / self.list_rows_per_col(viewport_h)) as f32
                    * self.list_col_w(longest_chars)
            }
            Family::SmallIcon => {
                let m = self.small_icon(self.icon_px(index));
                (item / grid_cols(m.cell_w, m.gap, pane_w)) as f32 * (m.cell_h + m.gap)
            }
            Family::IconTop => {
                let m = self.icon_top(self.icon_px(index));
                (item / grid_cols(m.cell_w, m.gap, pane_w)) as f32 * (m.cell_h + m.gap)
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
    fn shipped_yaml_matches_the_defaults() {
        // assets/folder-view.yaml documents the defaults — it must parse
        // to exactly them, or the file and the code disagree
        let yaml: ViewCfg =
            serde_yaml::from_str(include_str!("../assets/folder-view.yaml")).unwrap();
        let d = ViewCfg::default();
        assert_eq!(yaml.sizes, d.sizes);
        assert_eq!(yaml.steps(), 48);
        assert_eq!(yaml.layouts.content.row_h, d.layouts.content.row_h);
        assert_eq!(yaml.layouts.details.col_widths, d.layouts.details.col_widths);
        assert_eq!(yaml.icon_top.label_lines_switch, d.icon_top.label_lines_switch);
        assert_eq!(yaml.input.notch_delta, d.input.notch_delta);
    }
}

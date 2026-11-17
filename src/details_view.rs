//! Details-view configuration as data, not code.
//! Loaded from `assets/details-view.yaml` by [`DetailsViewCfg::load`];
//! a user copy at ~/.config/explorer/details-view.yaml overrides it
//! (same override path as folder-view.yaml). The serde defaults mirror
//! the shipped yaml.

use serde::Deserialize;

use crate::model::SortBy;

/// The Details column header + the set of available columns (§8/§11).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DetailsViewCfg {
    /// §3 column header font size.
    pub font_size: f32,
    /// §4 space left of the header title text.
    pub text_pad_left: f32,
    /// §5 the column's right dropdown button width.
    pub dropdown_w: f32,
    /// §6 the 1px line left of the dropdown button (border px).
    pub sep_w: f32,
    /// §2 sort arrow, sitting on top of the column title…
    pub arrow_w: f32,
    /// …9px wide, 5px tall, touching the header's top line.
    pub arrow_h: f32,
    /// §13 space left of the file rows.
    pub files_left: f32,
    /// §16 the row chrome keeps this space inside the column block's
    /// right border.
    pub chrome_pad: f32,
    /// Column resizer clamp.
    pub col_min: f32,
    pub col_max: f32,
    /// §8/§11 the available columns, in order: the default columns and
    /// the default width of each.
    pub columns: Vec<ColumnCfg>,
}

/// One available Details column.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ColumnCfg {
    /// Stable identifier: `name` is the icon+name cell, the others map
    /// to an entry property (`size` / `type` / `modified` / …).
    pub id: String,
    /// Header title.
    pub label: String,
    /// Default width; the header resizer adjusts it per session.
    pub width: f32,
    /// The sort key a header click sorts by (`None` = not sortable).
    pub sort: Option<SortBy>,
    /// A default column (visible unless unchecked in the column menu).
    pub on: bool,
}

impl Default for DetailsViewCfg {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            text_pad_left: 16.0,
            dropdown_w: 15.0,
            sep_w: 1.0,
            arrow_w: 9.0,
            arrow_h: 5.0,
            files_left: 14.0,
            chrome_pad: 4.0,
            col_min: 40.0,
            col_max: 600.0,
            columns: vec![
                ColumnCfg {
                    id: "name".into(),
                    label: "Name".into(),
                    width: 300.0,
                    sort: Some(SortBy::Name),
                    on: true,
                },
                ColumnCfg {
                    id: "size".into(),
                    label: "Size".into(),
                    width: 100.0,
                    sort: Some(SortBy::Size),
                    on: true,
                },
                ColumnCfg {
                    id: "type".into(),
                    label: "Type".into(),
                    width: 140.0,
                    sort: Some(SortBy::Type),
                    on: true,
                },
                ColumnCfg {
                    id: "modified".into(),
                    label: "Date modified".into(),
                    width: 170.0,
                    sort: Some(SortBy::Modified),
                    on: true,
                },
            ],
        }
    }
}

impl Default for ColumnCfg {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            width: 100.0,
            sort: None,
            on: false,
        }
    }
}

impl DetailsViewCfg {
    /// Load the config: ~/.config/explorer/details-view.yaml overrides
    /// the embedded `assets/details-view.yaml`; a broken file falls
    /// back to the defaults.
    pub fn load() -> Self {
        let text = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .and_then(|h| {
                std::fs::read_to_string(h.join(".config/explorer/details-view.yaml")).ok()
            })
            .unwrap_or_else(|| include_str!("../assets/details-view.yaml").to_string());
        match serde_yaml::from_str::<DetailsViewCfg>(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("details-view.yaml: {e} — using defaults");
                DetailsViewCfg::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_yaml_matches_the_defaults() {
        // assets/details-view.yaml documents the defaults — it must
        // parse to exactly them, or the file and the code disagree
        let yaml: DetailsViewCfg =
            serde_yaml::from_str(include_str!("../assets/details-view.yaml")).unwrap();
        let d = DetailsViewCfg::default();
        assert_eq!(yaml.font_size, d.font_size);
        assert_eq!(yaml.text_pad_left, d.text_pad_left);
        assert_eq!(yaml.dropdown_w, d.dropdown_w);
        assert_eq!(yaml.arrow_w, d.arrow_w);
        assert_eq!(yaml.arrow_h, d.arrow_h);
        assert_eq!(yaml.files_left, d.files_left);
        assert_eq!(yaml.columns.len(), d.columns.len());
        for (y, e) in yaml.columns.iter().zip(&d.columns) {
            assert_eq!(y.id, e.id);
            assert_eq!(y.label, e.label);
            assert_eq!(y.width, e.width);
            assert_eq!(y.sort, e.sort);
            assert_eq!(y.on, e.on);
        }
    }
}

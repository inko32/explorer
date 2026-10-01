//! `UI_PKEY_*` property keys and values (`IUISimplePropertySet`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ribbon::collection::{SharedPropertySet, UiCollection};
use crate::ribbon::enums::{FontPropsState, FontVerticalPosition, SwatchColorMode};
use crate::ribbon::image::UiImage;

/// Mirrors the `UI_PKEY_*` property keys declared in `UIRibbon.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    Enabled,
    LabelDescription,
    Keytip,
    Label,
    TooltipDescription,
    TooltipTitle,
    LargeImage,
    LargeHighContrastImage,
    SmallImage,
    SmallHighContrastImage,
    CommandId,
    ItemsSource,
    Categories,
    CategoryId,
    SelectedItem,
    CommandType,
    ItemImage,
    BooleanValue,
    DecimalValue,
    StringValue,
    MaxValue,
    MinValue,
    Increment,
    DecimalPlaces,
    FormatString,
    RepresentativeString,
    FontProperties,
    FontPropertiesFamily,
    FontPropertiesSize,
    FontPropertiesBold,
    FontPropertiesItalic,
    FontPropertiesUnderline,
    FontPropertiesStrikethrough,
    FontPropertiesVerticalPositioning,
    FontPropertiesForegroundColor,
    FontPropertiesBackgroundColor,
    FontPropertiesForegroundColorType,
    FontPropertiesBackgroundColorType,
    FontPropertiesChangedProperties,
    FontPropertiesDeltaSize,
    RecentItems,
    Pinned,
    Color,
    ColorType,
    ColorMode,
    ThemeColorsCategoryLabel,
    StandardColorsCategoryLabel,
    RecentColorsCategoryLabel,
    AutomaticColorLabel,
    NoColorLabel,
    MoreColorsLabel,
    ThemeColors,
    StandardColors,
    ThemeColorsTooltips,
    StandardColorsTooltips,
    Viewable,
    Minimized,
    QuickAccessToolbarDock,
    ContextAvailable,
    GlobalBackgroundColor,
    GlobalHighlightColor,
    GlobalTextColor,
}

impl PropertyKey {
    pub const ALL: &'static [PropertyKey] = &[
        PropertyKey::Enabled,
        PropertyKey::LabelDescription,
        PropertyKey::Keytip,
        PropertyKey::Label,
        PropertyKey::TooltipDescription,
        PropertyKey::TooltipTitle,
        PropertyKey::LargeImage,
        PropertyKey::SmallImage,
        PropertyKey::ItemsSource,
        PropertyKey::Categories,
        PropertyKey::SelectedItem,
        PropertyKey::BooleanValue,
        PropertyKey::DecimalValue,
        PropertyKey::StringValue,
        PropertyKey::MaxValue,
        PropertyKey::MinValue,
        PropertyKey::Increment,
        PropertyKey::DecimalPlaces,
        PropertyKey::FormatString,
        PropertyKey::FontProperties,
        PropertyKey::RecentItems,
        PropertyKey::Pinned,
        PropertyKey::Color,
        PropertyKey::ColorType,
        PropertyKey::ColorMode,
        PropertyKey::ThemeColorsCategoryLabel,
        PropertyKey::StandardColorsCategoryLabel,
        PropertyKey::RecentColorsCategoryLabel,
        PropertyKey::AutomaticColorLabel,
        PropertyKey::NoColorLabel,
        PropertyKey::MoreColorsLabel,
        PropertyKey::ThemeColors,
        PropertyKey::StandardColors,
        PropertyKey::Viewable,
        PropertyKey::Minimized,
        PropertyKey::QuickAccessToolbarDock,
        PropertyKey::ContextAvailable,
    ];
}

/// A property value, analogous to the `PROPVARIANT` types used with
/// `UI_PKEY_*` keys. `Color` is a packed `0x00RRGGBB` value.
#[derive(Clone)]
pub enum PropValue {
    // note: `PartialEq` is implemented manually below
    Bool(bool),
    UInt(u32),
    Int(i32),
    Decimal(f64),
    Str(String),
    Image(UiImage),
    Color(u32),
    ColorMode(SwatchColorMode),
    /// `UI_PKEY_ItemsSource` / `UI_PKEY_Categories`.
    Collection(Arc<Mutex<UiCollection>>),
    /// `UI_PKEY_RecentItems`: VT_ARRAY|VT_UNKNOWN of property sets.
    Items(Vec<SharedPropertySet>),
    /// `UI_PKEY_FontProperties`.
    Font(Box<FontProps>),
    /// `UI_PKEY_ThemeColors` / `UI_PKEY_StandardColors`.
    ColorList(Vec<u32>),
    /// `UI_PKEY_ThemeColorsTooltips` / `UI_PKEY_StandardColorsTooltips`.
    StringList(Vec<String>),
}

impl PartialEq for PropValue {
    fn eq(&self, other: &Self) -> bool {
        use PropValue::*;
        match (self, other) {
            (Bool(a), Bool(b)) => a == b,
            (UInt(a), UInt(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Decimal(a), Decimal(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Image(a), Image(b)) => a == b,
            (Color(a), Color(b)) => a == b,
            (ColorMode(a), ColorMode(b)) => a == b,
            (Collection(a), Collection(b)) => Arc::ptr_eq(a, b),
            (Items(a), Items(b)) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| Arc::ptr_eq(x, y))
            }
            (Font(a), Font(b)) => a == b,
            (ColorList(a), ColorList(b)) => a == b,
            (StringList(a), StringList(b)) => a == b,
            _ => false,
        }
    }
}

impl std::fmt::Debug for PropValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropValue::Bool(v) => f.debug_tuple("Bool").field(v).finish(),
            PropValue::UInt(v) => f.debug_tuple("UInt").field(v).finish(),
            PropValue::Int(v) => f.debug_tuple("Int").field(v).finish(),
            PropValue::Decimal(v) => f.debug_tuple("Decimal").field(v).finish(),
            PropValue::Str(v) => f.debug_tuple("Str").field(v).finish(),
            PropValue::Image(v) => f.debug_tuple("Image").field(v).finish(),
            PropValue::Color(v) => f.debug_tuple("Color").field(v).finish(),
            PropValue::ColorMode(v) => f.debug_tuple("ColorMode").field(v).finish(),
            PropValue::Collection(_) => f.write_str("Collection(..)"),
            PropValue::Items(v) => f.debug_tuple("Items").field(&v.len()).finish(),
            PropValue::Font(v) => f.debug_tuple("Font").field(v).finish(),
            PropValue::ColorList(v) => f.debug_tuple("ColorList").field(v).finish(),
            PropValue::StringList(v) => f.debug_tuple("StringList").field(v).finish(),
        }
    }
}

impl PropValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            PropValue::UInt(v) => Some(*v),
            PropValue::Int(v) => u32::try_from(*v).ok(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PropValue::Decimal(v) => Some(*v),
            PropValue::UInt(v) => Some(*v as f64),
            PropValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_color(&self) -> Option<u32> {
        match self {
            PropValue::Color(c) => Some(*c),
            _ => None,
        }
    }
    pub fn as_collection(&self) -> Option<&Arc<Mutex<UiCollection>>> {
        match self {
            PropValue::Collection(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_font(&self) -> Option<&FontProps> {
        match self {
            PropValue::Font(f) => Some(f),
            _ => None,
        }
    }
}

/// `IUISimplePropertySet`.
pub trait PropertySet {
    fn get_value(&self, key: PropertyKey) -> Option<PropValue>;
}

/// Convenience map-backed property set.
#[derive(Clone, Debug, Default)]
pub struct SimplePropertySet {
    pub values: HashMap<PropertyKey, PropValue>,
}

impl SimplePropertySet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, key: PropertyKey, value: PropValue) -> &mut Self {
        self.values.insert(key, value);
        self
    }
}

impl PropertySet for SimplePropertySet {
    fn get_value(&self, key: PropertyKey) -> Option<PropValue> {
        self.values.get(&key).cloned()
    }
}

impl PropertySet for Mutex<SimplePropertySet> {
    fn get_value(&self, key: PropertyKey) -> Option<PropValue> {
        self.lock().ok()?.get_value(key)
    }
}

/// `UI_PKEY_FontProperties` payload.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontProps {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub bold: FontPropsState,
    pub italic: FontPropsState,
    pub underline: FontPropsState,
    pub strikethrough: FontPropsState,
    pub vertical_positioning: FontVerticalPosition,
    pub foreground_color: Option<u32>,
    pub background_color: Option<u32>,
}

impl FontProps {
    /// Bold toggle helper for `UI_FONTDELTASIZE`-style controls.
    pub fn toggle(&mut self, which: FontToggle) {
        let flip = |s: &mut FontPropsState| {
            *s = if *s == FontPropsState::Set {
                FontPropsState::NotSet
            } else {
                FontPropsState::Set
            };
        };
        match which {
            FontToggle::Bold => flip(&mut self.bold),
            FontToggle::Italic => flip(&mut self.italic),
            FontToggle::Underline => flip(&mut self.underline),
            FontToggle::Strikethrough => flip(&mut self.strikethrough),
        }
    }
}

/// The four toggleable font attributes in the font control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontToggle {
    Bold,
    Italic,
    Underline,
    Strikethrough,
}

impl FontToggle {
    pub const ALL: [FontToggle; 4] = [
        FontToggle::Bold,
        FontToggle::Italic,
        FontToggle::Underline,
        FontToggle::Strikethrough,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FontToggle::Bold => "B",
            FontToggle::Italic => "I",
            FontToggle::Underline => "U",
            FontToggle::Strikethrough => "S",
        }
    }
    pub fn keytip(self) -> &'static str {
        match self {
            FontToggle::Bold => "1",
            FontToggle::Italic => "2",
            FontToggle::Underline => "3",
            FontToggle::Strikethrough => "4",
        }
    }
    pub fn is_set(self, props: &FontProps) -> bool {
        let s = match self {
            FontToggle::Bold => props.bold,
            FontToggle::Italic => props.italic,
            FontToggle::Underline => props.underline,
            FontToggle::Strikethrough => props.strikethrough,
        };
        s == FontPropsState::Set
    }
}

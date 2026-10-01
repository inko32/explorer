//! Enumerations from `UIRibbon.h`, translated 1:1.

/// `UI_CONTEXTAVAILABILITY` — availability of a contextual tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextAvailability {
    #[default]
    NotAvailable = 0,
    Available = 1,
    Active = 2,
}

impl From<u32> for ContextAvailability {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Available,
            2 => Self::Active,
            _ => Self::NotAvailable,
        }
    }
}

/// `UI_COMMANDTYPE` — type of a command, reported to
/// `UiApplication::on_create_ui_command`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandType {
    #[default]
    Unknown = 0,
    Group = 1,
    Action = 2,
    Anchor = 3,
    Context = 4,
    Collection = 5,
    CommandCollection = 6,
    Decimal = 7,
    Boolean = 8,
    Font = 9,
    RecentItems = 10,
    ColorAnchor = 11,
    ColorCollection = 12,
}

/// `UI_VIEWTYPE` — only `Ribbon` exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewType {
    #[default]
    Ribbon = 1,
}

/// `UI_VIEWVERB` — lifecycle notification for a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewVerb {
    #[default]
    Create = 0,
    Destroy = 1,
    Size = 2,
    Error = 3,
}

/// `UI_EXECUTIONVERB` — how a command was invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionVerb {
    #[default]
    Execute = 0,
    Preview = 1,
    CancelPreview = 2,
}

/// `UI_INVALIDATIONS` — what to re-query on invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Invalidations(pub u32);

impl Invalidations {
    pub const STATE: Self = Self(0x1);
    pub const VALUE: Self = Self(0x2);
    pub const PROPERTY: Self = Self(0x4);
    pub const ALLPROPERTIES: Self = Self(0x8);
    pub const NONE: Self = Self(0);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Invalidations {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Invalidations {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// `UI_COLLECTIONCHANGE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectionChange {
    #[default]
    Insert = 0,
    Remove = 1,
    Replace = 2,
    Reset = 3,
}

/// `UI_SWATCHCOLORTYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwatchColorType {
    #[default]
    NoColor = 0,
    Automatic = 1,
    Rgb = 2,
}

/// `UI_SWATCHCOLORMODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwatchColorMode {
    #[default]
    Normal = 0,
    Monochrome = 1,
}

/// `UI_EVENTTYPE` — event logger categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventType {
    #[default]
    ApplicationMenuOpened = 0,
    RibbonMinimized = 1,
    RibbonExpanded = 2,
    ApplicationModeSwitched = 3,
    TabActivated = 4,
    MenuOpened = 5,
    CommandExecuted = 6,
    TooltipShown = 7,
}

/// `UI_EVENTLOCATION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventLocation {
    #[default]
    Ribbon = 0,
    Qat = 1,
    ApplicationMenu = 2,
    ContextPopup = 3,
}

/// `UI_CONTROLDOCK` — quick access toolbar dock position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlDock {
    #[default]
    Top = 1,
    Bottom = 3,
}

impl From<u32> for ControlDock {
    fn from(v: u32) -> Self {
        match v {
            3 => Self::Bottom,
            _ => Self::Top,
        }
    }
}

/// `UI_FONTPROPERTIES` / `UI_FONTUNDERLINE` tri-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontPropsState {
    #[default]
    NotAvailable = 0,
    NotSet = 1,
    Set = 2,
}

impl From<u32> for FontPropsState {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::NotSet,
            2 => Self::Set,
            _ => Self::NotAvailable,
        }
    }
}

/// `UI_FONTVERTICALPOSITION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontVerticalPosition {
    #[default]
    NotAvailable = 0,
    NotSet = 1,
    Superscript = 2,
    Subscript = 3,
}

/// `UI_FONTDELTASIZE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontDeltaSize {
    #[default]
    Grow = 0,
    Shrink = 1,
}

/// `UI_HSB` from `UIRibbon.h` — packs hue/saturation/brightness.
pub const fn ui_hsb(hue: u8, saturation: u8, brightness: u8) -> u32 {
    hue as u32 | ((saturation as u32) << 8) | ((brightness as u32) << 16)
}

/// `UI_ALL_COMMANDS` — invalidate every command.
pub const UI_ALL_COMMANDS: u32 = 0;

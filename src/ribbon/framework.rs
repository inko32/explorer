//! `IUIFramework` / `IUIRibbon` / `IUIApplication` / `IUICommandHandler` /
//! `IUIContextualUI`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;

use crate::ribbon::enums::{
    CommandType, ContextAvailability, ControlDock, EventLocation, EventType,
    ExecutionVerb, Invalidations, ViewType, ViewVerb, UI_ALL_COMMANDS,
};
use crate::ribbon::prop::{PropertyKey, PropertySet, PropValue};
use crate::ribbon::spec::{self, RibbonSpec};
use crate::ribbon::Result;

/// `IUIEventingManager` logger callback.
pub(crate) type EventLogger = Rc<RefCell<dyn FnMut(EventType, EventLocation, u32)>>;

/// `HRESULT`-ish error, mirroring the failure codes of the COM API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RibbonError {
    pub code: i32,
    pub message: String,
}

impl RibbonError {
    pub const E_FAIL: i32 = -2147467259; // 0x80004005
    pub const E_INVALIDARG: i32 = -2147024809; // 0x80070057
    pub const E_NOTIMPL: i32 = -2147467263; // 0x80004001
    pub const E_POINTER: i32 = -2147467261; // 0x80004003

    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn fail(message: impl Into<String>) -> Self {
        Self::new(Self::E_FAIL, message)
    }
    pub fn not_loaded() -> Self {
        Self::new(Self::E_FAIL, "IUIFramework: LoadUI has not been called")
    }
    pub fn no_command(command: u32) -> Self {
        Self::new(
            Self::E_INVALIDARG,
            format!("IUIFramework: unknown command id {command}"),
        )
    }
}

impl std::fmt::Display for RibbonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (hr=0x{:08x})", self.message, self.code as u32)
    }
}

impl std::error::Error for RibbonError {}

/// `IUICommandHandler`.
pub trait CommandHandler {
    /// `Execute`: the user invoked `command`.
    fn execute(
        &mut self,
        command: u32,
        verb: ExecutionVerb,
        key: Option<PropertyKey>,
        current_value: Option<PropValue>,
        props: Option<Arc<dyn PropertySet + Send>>,
    ) -> Result<()>;

    /// `UpdateProperty`: return `Some` to set a new value for `key`.
    /// Called by `flush_pending_invalidations` and on demand.
    fn update_property(
        &mut self,
        _command: u32,
        _key: PropertyKey,
        _current_value: Option<PropValue>,
    ) -> Result<Option<PropValue>> {
        Ok(None)
    }
}

/// `IUIApplication`.
pub trait UiApplication {
    /// `OnViewChanged`.
    fn on_view_changed(
        &mut self,
        _view_id: u32,
        _type_id: ViewType,
        _verb: ViewVerb,
        _reason: i32,
    ) {
    }

    /// `OnCreateUICommand` — return the handler for `command`.
    fn on_create_ui_command(
        &mut self,
        command: u32,
        type_id: CommandType,
    ) -> Rc<RefCell<dyn CommandHandler>>;

    /// `OnDestroyUICommand`.
    fn on_destroy_ui_command(
        &mut self,
        _command: u32,
        _type_id: CommandType,
    ) {
    }
}

/// Events the ribbon widget produces for the application
/// (via `UiApplication` in the original, these travel as COM callbacks +
/// `IUICommandHandler::Execute`).
#[derive(Clone, Debug)]
pub enum RibbonEvent {
    /// `IUICommandHandler::Execute`.
    Execute {
        command: u32,
        verb: ExecutionVerb,
        key: Option<PropertyKey>,
        value: Option<PropValue>,
    },
    /// `UI_EVENTTYPE_TabActivated`.
    TabActivated {
        tab: String,
    },
    /// `UI_EVENTTYPE_RibbonMinimized` / `RibbonExpanded`.
    MinimizedChanged {
        minimized: bool,
    },
    /// Quick access toolbar contents changed (persist with
    /// `RibbonView::save_settings_to_stream`).
    QatChanged {
        commands: Vec<u32>,
    },
    /// Explorer extension: a ribbon popup opened or closed. Published
    /// so app views rebuild (open popups gray the folder/nav panes);
    /// the flag itself lives on [`RibbonView::popup_open`].
    PopupVisibility {
        open: bool,
    },
}

/// `IUIContextualUI` — show a context popup menu at a position.
#[derive(Clone)]
pub struct ContextualUi {
    view: RibbonView,
    command: u32,
}

impl ContextualUi {
    /// `ShowAtLocation` (screen coordinates; relative to the window in
    /// this implementation).
    pub fn show_at_location(&self, x: i32, y: i32) {
        self.view
            .show_context_popup(self.command, x as f32, y as f32);
    }
}

/// Runtime view state, shared between the framework, the widget and the
/// application (the `IUIRibbon` object lives in all three places).
pub struct ViewState {
    pub(crate) minimized: bool,
    pub(crate) selected_tab: usize,
    pub(crate) qat_commands: Vec<u32>,
    pub(crate) qat_dock: ControlDock,
    pub(crate) modes: i32,
    /// `IUIContextualUI` request, consumed by the widget.
    pub(crate) pending_popup: Option<(u32, f32, f32)>,
    /// A ribbon popup is currently open, published by the widget (read
    /// by app views: open popups gray the folder/nav panes
    pub(crate) popup_open: bool,
    /// Last laid-out tab rectangles (spec tab index, x, width), written
    /// by the widget on every layout — the window chrome (title bar)
    /// aligns its contextual blocks with these.
    pub(crate) tab_rects: Vec<(usize, f32, f32)>,
    /// yaml `metrics.footer_shift`: the group titles sit this many px
    /// lower and the ribbon body grows with it (set by `load_ui`).
    pub(crate) footer_shift: f32,
    pub(crate) event_logger: Option<EventLogger>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            minimized: false,
            selected_tab: 0,
            qat_commands: Vec::new(),
            qat_dock: ControlDock::Top,
            modes: 1,
            pending_popup: None,
            popup_open: false,
            tab_rects: Vec::new(),
            footer_shift: 0.0,
            event_logger: None,
        }
    }
}

/// `IUIRibbon`.
#[derive(Clone)]
pub struct RibbonView {
    pub(crate) inner: Rc<RefCell<ViewState>>,
}

impl Default for RibbonView {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ViewState::default())),
        }
    }
}

/// Ribbon geometry constants (96 dpi equivalents of the native control).
pub const TAB_STRIP_HEIGHT: f32 = 26.0;
pub const RIBBON_BODY_HEIGHT: f32 = 92.0;
pub const QAT_ROW_HEIGHT: f32 = 22.0;

impl RibbonView {
    /// Body height including the yaml `metrics.footer_shift`.
    pub(crate) fn body_height(&self) -> f32 {
        RIBBON_BODY_HEIGHT + self.inner.borrow().footer_shift
    }

    /// `IUIRibbon::GetHeight`.
    pub fn get_height(&self) -> u32 {
        let s = self.inner.borrow();
        let mut h = TAB_STRIP_HEIGHT;
        // docked "top" lives in the window title bar (no ribbon row)
        if !s.minimized {
            h += RIBBON_BODY_HEIGHT + s.footer_shift;
        }
        if s.qat_dock == ControlDock::Bottom {
            h += QAT_ROW_HEIGHT;
        }
        h as u32
    }

    /// Whether a ribbon popup is currently open (widget-published)
    pub fn popup_open(&self) -> bool {
        self.inner.borrow().popup_open
    }

    pub(crate) fn set_popup_open(&self, open: bool) {
        self.inner.borrow_mut().popup_open = open;
    }

    pub fn minimized(&self) -> bool {
        self.inner.borrow().minimized
    }
    pub fn set_minimized(&self, minimized: bool) {
        self.inner.borrow_mut().minimized = minimized;
    }
    pub fn selected_tab(&self) -> usize {
        self.inner.borrow().selected_tab
    }
    pub fn select_tab(&self, tab: usize) {
        self.inner.borrow_mut().selected_tab = tab;
    }
    pub fn qat_commands(&self) -> Vec<u32> {
        self.inner.borrow().qat_commands.clone()
    }
    pub fn set_qat_commands(&self, commands: Vec<u32>) {
        self.inner.borrow_mut().qat_commands = commands;
    }
    pub fn qat_dock(&self) -> ControlDock {
        self.inner.borrow().qat_dock
    }
    pub fn set_qat_dock(&self, dock: ControlDock) {
        self.inner.borrow_mut().qat_dock = dock;
    }
    pub fn modes(&self) -> i32 {
        self.inner.borrow().modes
    }
    pub(crate) fn set_modes_internal(&self, modes: i32) {
        self.inner.borrow_mut().modes = modes;
    }

    /// Tab rectangles from the last layout (spec tab index, x, width).
    pub fn tab_rects(&self) -> Vec<(usize, f32, f32)> {
        self.inner.borrow().tab_rects.clone()
    }

    pub(crate) fn set_tab_rects(&self, rects: Vec<(usize, f32, f32)>) {
        self.inner.borrow_mut().tab_rects = rects;
    }

    pub(crate) fn show_context_popup(&self, command: u32, x: f32, y: f32) {
        self.inner.borrow_mut().pending_popup = Some((command, x, y));
    }

    /// Consume a pending `IUIContextualUI` request (called by the widget
    /// on the next frame; public for embedding in custom renderers).
    pub fn take_pending_popup(&self) -> Option<(u32, f32, f32)> {
        self.inner.borrow_mut().pending_popup.take()
    }

    /// `IUIRibbon::SaveSettingsToStream` — ribbon state (minimized,
    /// selected tab, QAT contents) in a small YAML document, the moral
    /// equivalent of the binary stream the native control writes.
    pub fn save_settings_to_stream<W: Write>(&self, mut w: W) -> std::io::Result<()> {
        let s = self.inner.borrow();
        writeln!(w, "minimized: {}", s.minimized)?;
        writeln!(w, "selected_tab: {}", s.selected_tab)?;
        writeln!(w, "qat: {:?}", s.qat_commands)?;
        writeln!(w, "qat_dock: {}", s.qat_dock as u32)?;
        writeln!(w, "modes: {}", s.modes)?;
        Ok(())
    }

    /// `IUIRibbon::LoadSettingsFromStream`.
    pub fn load_settings_from_stream<R: Read>(&self, mut r: R) -> std::io::Result<()> {
        let mut text = String::new();
        r.read_to_string(&mut text)?;
        let mut s = self.inner.borrow_mut();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "minimized" => s.minimized = value == "true",
                "selected_tab" => {
                    s.selected_tab = value.parse().unwrap_or(0);
                }
                "qat" => {
                    let inner = value
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    s.qat_commands = inner
                        .split(',')
                        .filter_map(|v| v.trim().parse().ok())
                        .collect();
                }
                "qat_dock" => {
                    s.qat_dock = ControlDock::from(value.parse::<u32>().unwrap_or(1));
                }
                "modes" => s.modes = value.parse().unwrap_or(1),
                _ => {}
            }
        }
        Ok(())
    }
}

/// `IUIFramework` — the ribbon framework object (`CLSID_UIRibbonFramework`).
#[derive(Default)]
pub struct RibbonFramework {
    app: Option<Box<dyn UiApplication>>,
    spec: Option<RibbonSpec>,
    handlers: HashMap<u32, Rc<RefCell<dyn CommandHandler>>>,
    types: HashMap<u32, CommandType>,
    cache: HashMap<(u32, PropertyKey), PropValue>,
    dirty: Vec<(u32, Invalidations, Option<PropertyKey>)>,
    view: RibbonView,
}

impl std::fmt::Debug for RibbonFramework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RibbonFramework")
            .field("loaded", &self.spec.is_some())
            .field("commands", &self.handlers.len())
            .finish()
    }
}

impl RibbonFramework {
    pub fn new() -> Self {
        Self::default()
    }

    /// `IUIFramework::Initialize`.
    pub fn initialize(&mut self, app: impl UiApplication + 'static) {
        self.app = Some(Box::new(app));
    }

    /// `IUIFramework::Destroy`.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.as_mut() {
            app.on_view_changed(0, ViewType::Ribbon, ViewVerb::Destroy, 0);
        }
        let commands: Vec<u32> = self.handlers.keys().copied().collect();
        if let Some(app) = self.app.as_mut() {
            for cmd in commands {
                let ty = self.types.get(&cmd).copied().unwrap_or_default();
                app.on_destroy_ui_command(cmd, ty);
            }
        }
        self.handlers.clear();
        self.cache.clear();
        self.dirty.clear();
    }

    /// `IUIFramework::LoadUI` — takes the YAML ribbon markup instead of
    /// an `HINSTANCE`/`LPCWSTR` resource reference.
    pub fn load_ui(&mut self, yaml: &str) -> Result<()> {
        let mut spec = spec::parse(yaml).map_err(RibbonError::fail)?;
        let commands = spec::collect_commands(&mut spec).map_err(RibbonError::fail)?;

        let app = self
            .app
            .as_mut()
            .ok_or_else(|| RibbonError::fail("IUIFramework::Initialize has not been called"))?;

        self.spec = Some(spec);
        self.view.inner.borrow_mut().footer_shift =
            self.spec.as_ref().map(|s| s.metrics.footer_shift).unwrap_or(0.0);

        // Seed markup-provided property values.
        for (cmd, ty, seeds) in &commands {
            for (key, value) in seeds {
                self.cache.insert((*cmd, *key), value.clone());
            }
            self.types.insert(*cmd, *ty);
        }

        // UI_PKEY_CommandId
        for cmd in commands.iter().map(|(c, _, _)| *c) {
            self.cache
                .insert((cmd, PropertyKey::CommandId), PropValue::UInt(cmd));
        }

        // IUIApplication::OnCreateUICommand for every command.
        for (cmd, ty, _) in &commands {
            let handler = app.on_create_ui_command(*cmd, *ty);
            self.handlers.insert(*cmd, handler);
        }

        // quick access toolbar initial state from the markup
        if let Some(qat) = &self.spec.as_ref().unwrap().quick_access {
            let commands: Vec<u32> = qat
                .commands
                .iter()
                .filter_map(|c| c.resolve(&self.spec.as_ref().unwrap().commands).ok())
                .collect();
            self.view.set_qat_commands(commands);
            self.view.set_qat_dock(qat.dock.into());
        }

        // IUIApplication::OnViewChanged(CREATE)
        app.on_view_changed(0, ViewType::Ribbon, ViewVerb::Create, 0);
        Ok(())
    }

    /// Shortcut to the ribbon view (`GetView(0)` without the `Result`).
    pub fn view(&self) -> RibbonView {
        self.view.clone()
    }

    /// All command ids known to the framework (sorted).
    pub fn commands(&self) -> Vec<u32> {
        let mut cmds: Vec<u32> = self.handlers.keys().copied().collect();
        cmds.sort_unstable();
        cmds
    }

    /// `UI_COMMANDTYPE` of a command.
    pub fn command_type(&self, command: u32) -> Option<CommandType> {
        self.types.get(&command).copied()
    }

    /// `IUIFramework::GetView` — `view_id` must be 0 (`UI_VIEWTYPE_RIBBON`).
    pub fn get_view(&self, view_id: u32) -> Result<RibbonView> {
        if view_id != 0 {
            return Err(RibbonError::new(
                RibbonError::E_INVALIDARG,
                "IUIFramework::GetView: only view 0 (ribbon) exists",
            ));
        }
        if self.spec.is_none() {
            return Err(RibbonError::not_loaded());
        }
        Ok(self.view.clone())
    }

    /// `IUIFramework::GetUICommandProperty`.
    pub fn get_ui_command_property(
        &self,
        command: u32,
        key: PropertyKey,
    ) -> Result<PropValue> {
        self.spec.as_ref().ok_or_else(RibbonError::not_loaded)?;
        if let Some(value) = self.cache.get(&(command, key)) {
            return Ok(value.clone());
        }
        // Query the handler on demand (like the native framework does for
        // never-invalidated keys).
        if let Some(handler) = self.handlers.get(&command) {
            let current = self.cache.get(&(command, key)).cloned();
            if let Ok(Some(value)) =
                handler.borrow_mut().update_property(command, key, current)
            {
                return Ok(value);
            }
        }
        Err(RibbonError::new(
            RibbonError::E_FAIL,
            format!("IUIFramework::GetUICommandProperty({command}, {key:?}): not set"),
        ))
    }

    /// Convenience: property or default.
    pub fn property_or(
        &self,
        command: u32,
        key: PropertyKey,
        default: PropValue,
    ) -> PropValue {
        self.get_ui_command_property(command, key)
            .unwrap_or(default)
    }

    /// `UI_PKEY_Enabled` shortcut.
    pub fn command_enabled(&self, command: u32) -> bool {
        self.property_or(command, PropertyKey::Enabled, PropValue::Bool(true))
            .as_bool()
            .unwrap_or(true)
    }

    /// `IUIFramework::SetUICommandProperty`.
    pub fn set_ui_command_property(
        &mut self,
        command: u32,
        key: PropertyKey,
        value: PropValue,
    ) -> Result<()> {
        self.spec.as_ref().ok_or_else(RibbonError::not_loaded)?;
        self.cache.insert((command, key), value);
        Ok(())
    }

    /// `IUIFramework::InvalidateUICommand`.
    ///
    /// `UI_ALL_COMMANDS` (0) invalidates every command.
    pub fn invalidate_ui_command(
        &mut self,
        command: u32,
        flags: Invalidations,
        key: Option<PropertyKey>,
    ) -> Result<()> {
        self.spec.as_ref().ok_or_else(RibbonError::not_loaded)?;
        self.dirty.push((command, flags, key));
        Ok(())
    }

    /// `IUIFramework::FlushPendingInvalidations` — re-queries the
    /// handlers for all dirty keys (via `UpdateProperty`).
    pub fn flush_pending_invalidations(&mut self) {
        let dirty = std::mem::take(&mut self.dirty);
        for (command, flags, key) in dirty {
            let commands: Vec<u32> = if command == UI_ALL_COMMANDS {
                self.handlers.keys().copied().collect()
            } else {
                vec![command]
            };
            for cmd in commands {
                let keys: Vec<PropertyKey> = if flags.contains(Invalidations::ALLPROPERTIES)
                {
                    PropertyKey::ALL.to_vec()
                } else {
                    let mut keys = vec![];
                    if flags.contains(Invalidations::STATE) {
                        keys.extend([
                            PropertyKey::Enabled,
                            PropertyKey::ContextAvailable,
                            PropertyKey::Viewable,
                            PropertyKey::Minimized,
                            PropertyKey::QuickAccessToolbarDock,
                        ]);
                    }
                    if flags.contains(Invalidations::VALUE) {
                        keys.extend([
                            PropertyKey::BooleanValue,
                            PropertyKey::DecimalValue,
                            PropertyKey::StringValue,
                            PropertyKey::SelectedItem,
                            PropertyKey::Color,
                            PropertyKey::FontProperties,
                            PropertyKey::RecentItems,
                            PropertyKey::ItemsSource,
                        ]);
                    }
                    if flags.contains(Invalidations::PROPERTY)
                        && let Some(k) = key
                    {
                        keys.push(k);
                    }
                    keys
                };
                for k in keys {
                    self.refresh_key(cmd, k);
                }
            }
        }
    }

    fn refresh_key(&mut self, cmd: u32, key: PropertyKey) {
        let Some(handler) = self.handlers.get(&cmd) else {
            return;
        };
        let current = self.cache.get(&(cmd, key)).cloned();
        if let Ok(Some(value)) = handler.borrow_mut().update_property(cmd, key, current)
        {
            self.cache.insert((cmd, key), value);
        }
    }

    /// `IUIFramework::SetModes` — switch application modes; tabs declare
    /// their visible modes in YAML (`modes: [bit, …]`).
    pub fn set_modes(&mut self, modes: i32) -> Result<()> {
        self.spec.as_ref().ok_or_else(RibbonError::not_loaded)?;
        self.view.set_modes_internal(modes);
        if let Some(app) = self.app.as_mut() {
            app.on_view_changed(0, ViewType::Ribbon, ViewVerb::Size, modes);
        }
        self.view_log(EventType::ApplicationModeSwitched, EventLocation::Ribbon, 0);
        Ok(())
    }

    /// `IUIEventingManager::SetEventLogger`.
    pub fn set_event_logger(&mut self, logger: EventLogger) {
        self.view.inner.borrow_mut().event_logger = Some(logger);
    }

    pub(crate) fn view_log(
        &self,
        event: EventType,
        location: EventLocation,
        command: u32,
    ) {
        if let Some(logger) = &self.view.inner.borrow().event_logger {
            (logger.borrow_mut())(event, location, command);
        }
    }

    /// `IUIFramework` analog of a command execution round-trip: routes an
    /// [`RibbonEvent::Execute`] to the handler, then invalidates and
    /// flushes the affected command so the UI reflects the new state.
    pub fn dispatch(&mut self, event: &RibbonEvent) -> Result<()> {
        match event {
            RibbonEvent::Execute {
                command,
                verb,
                key,
                value,
            } => {
                let handler = self
                    .handlers
                    .get(command)
                    .ok_or_else(|| RibbonError::no_command(*command))?;
                handler.borrow_mut().execute(
                    *command,
                    *verb,
                    *key,
                    value.clone(),
                    None,
                )?;
                if matches!(verb, ExecutionVerb::Execute) {
                    self.invalidate_ui_command(
                        *command,
                        Invalidations::VALUE | Invalidations::STATE,
                        None,
                    )?;
                    self.flush_pending_invalidations();
                    self.view_log(
                        EventType::CommandExecuted,
                        EventLocation::Ribbon,
                        *command,
                    );
                }
                Ok(())
            }
            RibbonEvent::QatChanged { commands } => {
                self.view.set_qat_commands(commands.clone());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `IUIFramework::GetView` + `IUIContextualUI`: context popup for a
    /// command declared under `context_popups:` in the YAML.
    pub fn contextual_ui(&self, command: u32) -> ContextualUi {
        ContextualUi {
            view: self.view.clone(),
            command,
        }
    }

    // ---- accessors used by the widget ---------------------------------

    /// The loaded spec (`Err` if `load_ui` was not called).
    pub fn spec(&self) -> Result<&RibbonSpec> {
        self.spec.as_ref().ok_or_else(RibbonError::not_loaded)
    }

    /// Current value of a command property (`cache -> handler`).
    pub(crate) fn cached(&self, command: u32, key: PropertyKey) -> Option<PropValue> {
        self.cache.get(&(command, key)).cloned()
    }

    /// Label helper: `UI_PKEY_Label` fallback to the raw id.
    pub fn command_label(&self, command: u32) -> String {
        self.cached(command, PropertyKey::Label)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("#{command}"))
    }

    /// Contextual availability of a context command. Queries the handler
    /// on demand; contextual UI starts hidden (`NotAvailable`) until the
    /// application publishes availability, like the native framework.
    pub fn context_availability(&self, command: u32) -> ContextAvailability {
        self.get_ui_command_property(command, PropertyKey::ContextAvailable)
            .ok()
            .and_then(|v| v.as_u32())
            .map(ContextAvailability::from)
            .unwrap_or(ContextAvailability::NotAvailable)
    }

    /// Is a tab visible given `UI_PKEY_Viewable` and `SetModes`?
    pub fn tab_visible(&self, tab: &spec::TabSpec) -> bool {
        if let Some(cmd) = &tab.context_command {
            // resolved during load_ui; context_command is None afterwards.
            let _ = cmd;
        }
        if !tab.modes.is_empty() {
            let modes = self.view.modes();
            if !tab.modes.iter().any(|b| modes & (1 << b) != 0) {
                return false;
            }
        }
        true
    }
}

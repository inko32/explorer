//! Framework-level tests: LoadUI, command handlers, invalidation,
//! settings streams, modes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::ribbon::{
    CommandHandler, ContextAvailability, ExecutionVerb, Invalidations,
    PropertyKey, PropertySet, PropValue, RibbonEvent, RibbonFramework,
    SimplePropertySet, UiApplication,
};

const DEMO_YAML: &str = include_str!("framework_tests.yaml");

struct TestApp {
    handler: Rc<RefCell<TestHandler>>,
}

struct TestHandler {
    toggles: HashMap<u32, bool>,
    executed: Vec<(u32, Option<PropertyKey>)>,
    ctx_table: ContextAvailability,
}

impl CommandHandler for TestHandler {
    fn execute(
        &mut self,
        command: u32,
        _verb: ExecutionVerb,
        key: Option<PropertyKey>,
        value: Option<PropValue>,
        _props: Option<Arc<dyn crate::ribbon::PropertySet + Send>>,
    ) -> Result<(), crate::ribbon::RibbonError> {
        self.executed.push((command, key));
        if key == Some(PropertyKey::BooleanValue) {
            if let Some(PropValue::Bool(b)) = value {
                self.toggles.insert(command, b);
            }
        }
        Ok(())
    }

    fn update_property(
        &mut self,
        command: u32,
        key: PropertyKey,
        _current: Option<PropValue>,
    ) -> Result<Option<PropValue>, crate::ribbon::RibbonError> {
        Ok(match key {
            PropertyKey::BooleanValue => {
                Some(PropValue::Bool(self.toggles.get(&command).copied().unwrap_or(false)))
            }
            PropertyKey::ContextAvailable => Some(PropValue::UInt(
                if command == 100 {
                    self.ctx_table as u32
                } else {
                    ContextAvailability::Active as u32
                },
            )),
            PropertyKey::Label => Some(PropValue::Str(format!("cmd{command}"))),
            _ => None,
        })
    }
}

impl UiApplication for TestApp {
    fn on_create_ui_command(
        &mut self,
        _command: u32,
        _type_id: crate::ribbon::CommandType,
    ) -> Rc<RefCell<dyn CommandHandler>> {
        self.handler.clone()
    }
}

fn make_framework() -> (RibbonFramework, Rc<RefCell<TestHandler>>) {
    let handler = Rc::new(RefCell::new(TestHandler {
        toggles: HashMap::new(),
        executed: Vec::new(),
        ctx_table: ContextAvailability::NotAvailable,
    }));
    let mut fw = RibbonFramework::new();
    fw.initialize(TestApp {
        handler: handler.clone(),
    });
    fw.load_ui(DEMO_YAML).unwrap();
    (fw, handler)
}

#[test]
fn load_ui_creates_commands_and_view() {
    let (mut fw, handler) = make_framework();
    // every command got a handler via OnCreateUICommand
    assert!(fw.commands().len() > 40, "expected many commands, got {}", fw.commands().len());
    let view = fw.get_view(0).unwrap();
    assert_eq!(view.get_height(), 26 + 22 + 92); // strip + qat + body
    fw.destroy();
    drop(handler);
}

#[test]
fn toggle_round_trip_through_cache() {
    let (mut fw, handler) = make_framework();
    // simulate the widget: user toggles Bold (33)
    fw.dispatch(&RibbonEvent::Execute {
        command: 33,
        verb: ExecutionVerb::Execute,
        key: Some(PropertyKey::BooleanValue),
        value: Some(PropValue::Bool(true)),
    })
    .unwrap();
    assert_eq!(
        fw.get_ui_command_property(33, PropertyKey::BooleanValue)
            .unwrap(),
        PropValue::Bool(true)
    );
    assert!(handler.borrow().executed.contains(&(33, Some(PropertyKey::BooleanValue))));
}

#[test]
fn invalidate_and_flush_updates_cache() {
    let (mut fw, handler) = make_framework();
    handler.borrow_mut().toggles.insert(33, true);
    fw.invalidate_ui_command(33, Invalidations::VALUE, None)
        .unwrap();
    fw.flush_pending_invalidations();
    assert_eq!(
        fw.get_ui_command_property(33, PropertyKey::BooleanValue)
            .unwrap(),
        PropValue::Bool(true)
    );
}

#[test]
fn set_modes_switches_tabs() {
    let (mut fw, _) = make_framework();
    fw.set_modes(1).unwrap();
    assert_eq!(fw.view().modes(), 1);
    // tab 4 (index 4: print) requires mode bit 1
    let spec = fw.spec().unwrap();
    let print_tab = spec.tabs.iter().position(|t| t.id == "print").unwrap();
    assert!(!spec.tabs[print_tab]
        .modes
        .iter()
        .any(|b| 1 & (1 << b) != 0));
}

#[test]
fn settings_stream_round_trip() {
    let (fw, _) = make_framework();
    let view = fw.get_view(0).unwrap();
    view.set_minimized(true);
    view.select_tab(2);
    view.set_qat_commands(vec![12, 18]);
    let mut buf = Vec::new();
    view.save_settings_to_stream(&mut buf).unwrap();

    let (fw2, _) = make_framework();
    let view2 = fw2.get_view(0).unwrap();
    view2.load_settings_from_stream(&buf[..]).unwrap();
    assert!(view2.minimized());
    assert_eq!(view2.selected_tab(), 2);
    assert_eq!(view2.qat_commands(), vec![12, 18]);
    assert_eq!(view2.get_height(), 26 + 22); // minimized: no body
}

#[test]
fn contextual_availability_property() {
    let (mut fw, handler) = make_framework();
    handler.borrow_mut().ctx_table = ContextAvailability::Active;
    fw.invalidate_ui_command(100, Invalidations::STATE, None)
        .unwrap();
    fw.flush_pending_invalidations();
    assert_eq!(
        fw.get_ui_command_property(100, PropertyKey::ContextAvailable)
            .unwrap(),
        PropValue::UInt(ContextAvailability::Active as u32)
    );
}

#[test]
fn property_set_and_collection() {
    let mut set = SimplePropertySet::new();
    set.set(PropertyKey::Label, PropValue::Str("Hello".into()));
    assert_eq!(
        set.get_value(PropertyKey::Label).unwrap(),
        PropValue::Str("Hello".into())
    );

    let col = crate::ribbon::collection::collection_from_labels(&[("A", None), ("B", None)]);
    assert_eq!(col.lock().unwrap().get_count(), 2);
    assert_eq!(col.lock().unwrap().label_of(1), Some("B".into()));
}

#[test]
fn contextual_ui_request_is_queued() {
    let (fw, _) = make_framework();
    fw.contextual_ui(140).show_at_location(10, 20);
    let pending = fw.view().take_pending_popup();
    assert_eq!(pending, Some((140, 10.0, 20.0)));
}

#[test]
fn spec_commands_are_resolved_in_place() {
    let mut fw = RibbonFramework::new();
    fw.initialize(TestApp {
        handler: Rc::new(RefCell::new(TestHandler {
            toggles: HashMap::new(),
            executed: Vec::new(),
            ctx_table: ContextAvailability::NotAvailable,
        })),
    });
    fw.load_ui(DEMO_YAML).unwrap();
    let spec = fw.spec().unwrap();
    // control command resolved to a numeric id
    let home = spec.tabs.iter().find(|t| t.id == "home").unwrap();
    match &home.groups[0].controls[0] {
        crate::ribbon::ControlSpec::SplitButton(sp) => {
            assert_eq!(
                sp.command,
                Some(crate::ribbon::spec::CommandRef::Id(20))
            );
        }
        other => panic!("expected split button, got {other:?}"),
    }
    // contextual tab command resolved
    let design = spec.tabs.iter().find(|t| t.id == "tableDesign").unwrap();
    assert_eq!(
        design.context_command,
        Some(crate::ribbon::spec::CommandRef::Id(100))
    );
    // and the availability defaults to hidden
    assert_eq!(
        fw.context_availability(100),
        ContextAvailability::NotAvailable
    );
    // QAT seeded from markup
    assert_eq!(fw.view().qat_commands(), vec![12, 18, 19]);
}

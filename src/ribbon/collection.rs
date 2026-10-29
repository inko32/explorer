//! `IUICollection` / `IUICollectionChangedEvent`.

use std::sync::{Arc, Mutex};

use crate::ribbon::enums::CollectionChange;
use crate::ribbon::image::UiImage;
use crate::ribbon::prop::{PropertyKey, PropertySet, PropValue, SimplePropertySet};

/// `IUICollectionChangedEvent::OnChanged`.
pub trait OnCollectionChanged {
    fn on_changed(&mut self, action: CollectionChange, index: u32);
}

/// Shared property set (Send, so it can travel inside iced messages).
pub type SharedPropertySet = Arc<Mutex<dyn PropertySet + Send>>;

/// `IUICollection` of property sets (combo items, gallery items,
/// categories, ...).
#[derive(Default)]
pub struct UiCollection {
    items: Vec<SharedPropertySet>,
    listeners: Vec<Arc<Mutex<dyn OnCollectionChanged + Send>>>,
}

impl std::fmt::Debug for UiCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiCollection")
            .field("len", &self.items.len())
            .finish()
    }
}

impl UiCollection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get_item(&self, index: usize) -> Option<SharedPropertySet> {
        self.items.get(index).cloned()
    }

    pub fn add(&mut self, item: SharedPropertySet) {
        let index = self.items.len() as u32;
        self.items.push(item);
        self.notify(CollectionChange::Insert, index);
    }

    pub fn insert(
        &mut self,
        index: usize,
        item: SharedPropertySet,
    ) {
        let index = index.min(self.items.len());
        self.items.insert(index, item);
        self.notify(CollectionChange::Insert, index as u32);
    }

    pub fn remove_at(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.notify(CollectionChange::Remove, index as u32);
        }
    }

    pub fn replace(
        &mut self,
        index: usize,
        item: SharedPropertySet,
    ) {
        if index < self.items.len() {
            self.items[index] = item;
            self.notify(CollectionChange::Replace, index as u32);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.notify(CollectionChange::Reset, 0);
    }

    pub fn add_listener(
        &mut self,
        listener: Arc<Mutex<dyn OnCollectionChanged + Send>>,
    ) {
        self.listeners.push(listener);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SharedPropertySet> {
        self.items.iter()
    }

    fn notify(&mut self, action: CollectionChange, index: u32) {
        for listener in &self.listeners {
            if let Ok(mut l) = listener.lock() {
                l.on_changed(action, index);
            }
        }
    }

    /// Label of the item at `index` (via `UI_PKEY_Label`).
    pub fn label_of(&self, index: usize) -> Option<String> {
        self.get_item(index)
            .and_then(|item| item.lock().ok()?.get_value(PropertyKey::Label))
            .and_then(|v| v.as_str().map(str::to_owned))
    }
}

/// Build a collection of simple items from `(label, icon)` pairs.
pub fn collection_from_labels(
    items: &[(&'static str, Option<&'static str>)],
) -> Arc<Mutex<UiCollection>> {
    let mut collection = UiCollection::new();
    for (label, icon) in items {
        let mut set = SimplePropertySet::new();
        set.set(PropertyKey::Label, PropValue::Str(label.to_string()));
        if let Some(icon) = icon {
            set.set(
                PropertyKey::ItemImage,
                PropValue::Image(UiImage::Icon(icon)),
            );
        }
        collection.add(Arc::new(Mutex::new(set)));
    }
    Arc::new(Mutex::new(collection))
}

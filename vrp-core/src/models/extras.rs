use rustc_hash::FxHasher;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::Arc;

/// Specifies a type used to store any values regarding problem configuration.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    index: HashMap<TypeId, Arc<dyn Any + Send + Sync>, BuildHasherDefault<FxHasher>>,
}

impl Extras {
    /// Gets the value from extras using the key type provided.
    pub fn get_value<K: 'static, V: Send + Sync + 'static>(&self) -> Option<&V> {
        self.index.get(&TypeId::of::<K>()).and_then(|any| any.downcast_ref::<V>())
    }

    /// Gets a shared reference to the value from extras using the key type provided.
    pub fn get_value_raw<K: 'static, V: Send + Sync + 'static>(&self) -> Option<Arc<V>> {
        self.index.get(&TypeId::of::<K>()).cloned().and_then(|any| any.downcast::<V>().ok())
    }

    /// Sets the value to extras using the key type provided.
    pub fn set_value<K: 'static, V: 'static + Sync + Send>(&mut self, value: V) {
        self.index.insert(TypeId::of::<K>(), Arc::new(value));
    }

    /// Sets the value, passed as shared reference, to extras using key type provided.
    pub(crate) fn set_value_raw<K: 'static, V: 'static + Sync + Send>(&mut self, value: Arc<V>) {
        self.index.insert(TypeId::of::<K>(), value);
    }
}

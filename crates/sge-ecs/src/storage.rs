// Copyright The SimpleGameEngine Contributors

use std::{any::Any, collections::BTreeMap};

use crate::Entity;

pub(crate) trait ErasedStorage {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, entity: Entity);
}

pub(crate) struct SparseStorage<T> {
    pub(crate) values: BTreeMap<Entity, T>,
}

impl<T> Default for SparseStorage<T> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

impl<T: 'static> ErasedStorage for SparseStorage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove_entity(&mut self, entity: Entity) {
        let _ = self.values.remove(&entity);
    }
}

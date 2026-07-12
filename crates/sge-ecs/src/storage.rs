// Copyright The SimpleGameEngine Contributors

use std::{any::Any, collections::BTreeMap};

use crate::Entity;

pub(crate) trait ErasedStorage {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn get_erased(&self, entity: Entity) -> Option<&dyn Any>;
    fn insert_boxed(&mut self, entity: Entity, value: Box<dyn Any>) -> Result<(), Box<dyn Any>>;
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

    fn get_erased(&self, entity: Entity) -> Option<&dyn Any> {
        self.values.get(&entity).map(|value| value as &dyn Any)
    }

    fn insert_boxed(&mut self, entity: Entity, value: Box<dyn Any>) -> Result<(), Box<dyn Any>> {
        let value = value.downcast::<T>()?;
        let _ = self.values.insert(entity, *value);
        Ok(())
    }

    fn remove_entity(&mut self, entity: Entity) {
        let _ = self.values.remove(&entity);
    }
}

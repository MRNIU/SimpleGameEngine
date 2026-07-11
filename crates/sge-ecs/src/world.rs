// Copyright The SimpleGameEngine Contributors

use std::{
    any::{Any, TypeId, type_name},
    collections::{HashMap, hash_map::Entry},
};

use crate::{
    Entity,
    entity::EntityAllocator,
    storage::{ErasedStorage, SparseStorage},
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EcsError {
    #[error("ECS type registration is finished")]
    RegistrationFinished,
    #[error("component type is already registered: {0}")]
    DuplicateComponentType(&'static str),
    #[error("resource type is already registered: {0}")]
    DuplicateResourceType(&'static str),
    #[error("component type is not registered: {0}")]
    UnregisteredComponentType(&'static str),
    #[error("resource type is not registered: {0}")]
    UnregisteredResourceType(&'static str),
    #[error("entity is not alive: {0:?}")]
    EntityNotAlive(Entity),
}

pub struct World {
    entities: EntityAllocator,
    components: HashMap<TypeId, Box<dyn ErasedStorage>>,
    resource_types: HashMap<TypeId, &'static str>,
    resources: HashMap<TypeId, Box<dyn Any>>,
    registration_finished: bool,
}

impl World {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entities: EntityAllocator::default(),
            components: HashMap::new(),
            resource_types: HashMap::new(),
            resources: HashMap::new(),
            registration_finished: false,
        }
    }

    pub fn register_component<T: 'static>(&mut self) -> Result<(), EcsError> {
        if self.registration_finished {
            return Err(EcsError::RegistrationFinished);
        }

        match self.components.entry(TypeId::of::<T>()) {
            Entry::Occupied(_) => Err(EcsError::DuplicateComponentType(type_name::<T>())),
            Entry::Vacant(entry) => {
                entry.insert(Box::<SparseStorage<T>>::default());
                Ok(())
            }
        }
    }

    pub fn register_resource<R: 'static>(&mut self) -> Result<(), EcsError> {
        if self.registration_finished {
            return Err(EcsError::RegistrationFinished);
        }

        match self.resource_types.entry(TypeId::of::<R>()) {
            Entry::Occupied(_) => Err(EcsError::DuplicateResourceType(type_name::<R>())),
            Entry::Vacant(entry) => {
                entry.insert(type_name::<R>());
                Ok(())
            }
        }
    }

    pub fn finish_registration(&mut self) {
        self.registration_finished = true;
    }

    #[must_use]
    pub const fn registration_is_finished(&self) -> bool {
        self.registration_finished
    }

    #[must_use]
    pub fn component_is_registered<T: 'static>(&self) -> bool {
        self.components.contains_key(&TypeId::of::<T>())
    }

    pub fn spawn(&mut self) -> Entity {
        self.entities.spawn()
    }

    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn despawn(&mut self, entity: Entity) -> Result<(), EcsError> {
        if !self.entities.despawn(entity) {
            return Err(EcsError::EntityNotAlive(entity));
        }

        for storage in self.components.values_mut() {
            storage.remove_entity(entity);
        }
        Ok(())
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.entities()
    }

    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) -> Result<Option<T>, EcsError> {
        if !self.is_alive(entity) {
            return Err(EcsError::EntityNotAlive(entity));
        }

        let storage = self
            .storage_mut::<T>()
            .ok_or(EcsError::UnregisteredComponentType(type_name::<T>()))?;
        Ok(storage.values.insert(entity, value))
    }

    #[must_use]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage::<T>()?.values.get(&entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage_mut::<T>()?.values.get_mut(&entity)
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Result<Option<T>, EcsError> {
        if !self.is_alive(entity) {
            return Err(EcsError::EntityNotAlive(entity));
        }

        let storage = self
            .storage_mut::<T>()
            .ok_or(EcsError::UnregisteredComponentType(type_name::<T>()))?;
        Ok(storage.values.remove(&entity))
    }

    #[must_use]
    pub fn contains<T: 'static>(&self, entity: Entity) -> bool {
        self.get::<T>(entity).is_some()
    }

    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> + '_ {
        self.storage::<T>().into_iter().flat_map(|storage| {
            storage
                .values
                .iter()
                .map(|(entity, value)| (*entity, value))
        })
    }

    pub fn query_mut<T: 'static>(&mut self) -> impl Iterator<Item = (Entity, &mut T)> + '_ {
        self.storage_mut::<T>().into_iter().flat_map(|storage| {
            storage
                .values
                .iter_mut()
                .map(|(entity, value)| (*entity, value))
        })
    }

    pub fn insert_resource<R: 'static>(&mut self, value: R) -> Result<Option<R>, EcsError> {
        let type_id = TypeId::of::<R>();
        if !self.resource_types.contains_key(&type_id) {
            return Err(EcsError::UnregisteredResourceType(type_name::<R>()));
        }

        Ok(self
            .resources
            .insert(type_id, Box::new(value))
            .map(|stored| {
                *stored
                    .downcast::<R>()
                    .expect("registered resource TypeId must match stored value type")
            }))
    }

    #[must_use]
    pub fn resource<R: 'static>(&self) -> Option<&R> {
        self.resources.get(&TypeId::of::<R>())?.downcast_ref()
    }

    pub fn resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
        self.resources.get_mut(&TypeId::of::<R>())?.downcast_mut()
    }

    fn storage<T: 'static>(&self) -> Option<&SparseStorage<T>> {
        self.components
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref()
    }

    fn storage_mut<T: 'static>(&mut self) -> Option<&mut SparseStorage<T>> {
        self.components
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

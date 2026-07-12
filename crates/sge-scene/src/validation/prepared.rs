// Copyright The SimpleGameEngine Contributors

use std::any::{Any, TypeId};

use sge_reflect::TypeKey;

use crate::SceneEntityId;

pub struct PreparedScene {
    entities: Vec<PreparedEntity>,
}

pub(crate) struct PreparedEntity {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: Vec<PreparedComponent>,
}

pub(crate) struct PreparedComponent {
    type_key: TypeKey,
    type_id: TypeId,
    value: Box<dyn Any>,
}

impl PreparedScene {
    pub(crate) const fn new(entities: Vec<PreparedEntity>) -> Self {
        Self { entities }
    }

    pub(crate) fn entities(&self) -> impl Iterator<Item = &PreparedEntity> {
        self.entities.iter()
    }

    pub(crate) fn into_entities(self) -> Vec<PreparedEntity> {
        self.entities
    }
}

impl PreparedEntity {
    pub(crate) const fn new(
        id: SceneEntityId,
        parent: Option<SceneEntityId>,
        components: Vec<PreparedComponent>,
    ) -> Self {
        Self {
            id,
            parent,
            components,
        }
    }

    pub(crate) const fn id(&self) -> SceneEntityId {
        self.id
    }

    pub(crate) fn components(&self) -> impl Iterator<Item = &PreparedComponent> {
        self.components.iter()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (SceneEntityId, Option<SceneEntityId>, Vec<PreparedComponent>) {
        (self.id, self.parent, self.components)
    }
}

impl PreparedComponent {
    pub(crate) const fn new(type_key: TypeKey, type_id: TypeId, value: Box<dyn Any>) -> Self {
        Self {
            type_key,
            type_id,
            value,
        }
    }

    pub(crate) const fn type_key(&self) -> &TypeKey {
        &self.type_key
    }

    pub(crate) const fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub(crate) fn into_parts(self) -> (TypeKey, TypeId, Box<dyn Any>) {
        (self.type_key, self.type_id, self.value)
    }
}

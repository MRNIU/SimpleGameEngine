// Copyright The SimpleGameEngine Contributors

use std::{any::TypeId, collections::BTreeMap};

use sge_ecs::{EcsError, Entity, WorldInitializer};
use sge_reflect::{DescriptorError, TypeKey};

use crate::{Parent, PreparedScene, SceneEntityId, parent_descriptor, scene_entity_id_descriptor};

pub struct SceneInstance {
    entities: BTreeMap<SceneEntityId, Entity>,
}

impl SceneInstance {
    #[must_use]
    pub fn entity(&self, id: &SceneEntityId) -> Option<Entity> {
        self.entities.get(id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SceneEntityId, Entity)> + '_ {
        self.entities.iter().map(|(id, entity)| (id, *entity))
    }
}

pub fn instantiate(
    prepared: PreparedScene,
    mut initializer: WorldInitializer<'_>,
) -> Result<SceneInstance, SceneInstantiationError> {
    let identity = scene_entity_id_descriptor()?;
    if !initializer.component_is_registered(TypeId::of::<SceneEntityId>()) {
        return Err(SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component: identity.type_key().clone(),
        });
    }
    let parent = parent_descriptor()?;
    if !initializer.component_is_registered(TypeId::of::<Parent>()) {
        return Err(SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component: parent.type_key().clone(),
        });
    }
    for entity in prepared.entities() {
        for component in entity.components() {
            if matches!(
                component.type_id(),
                type_id if type_id == TypeId::of::<SceneEntityId>()
                    || type_id == TypeId::of::<Parent>()
            ) {
                return Err(SceneInstantiationError::ReservedStructuralComponent {
                    entity: entity.id(),
                    component: component.type_key().clone(),
                });
            }
            if !initializer.component_is_registered(component.type_id()) {
                return Err(SceneInstantiationError::MissingComponentRegistration {
                    entity: Some(entity.id()),
                    component: component.type_key().clone(),
                });
            }
        }
    }

    let mut entities = BTreeMap::new();
    for prepared_entity in prepared.into_entities() {
        let (id, parent_id, components) = prepared_entity.into_parts();
        let runtime_entity = initializer.spawn();
        initializer
            .insert(runtime_entity, id)
            .map_err(|source| SceneInstantiationError::Ecs {
                entity: id,
                component: identity.type_key().clone(),
                source,
            })?;
        if let Some(parent_id) = parent_id {
            initializer
                .insert(runtime_entity, Parent(parent_id))
                .map_err(|source| SceneInstantiationError::Ecs {
                    entity: id,
                    component: parent.type_key().clone(),
                    source,
                })?;
        }
        for component in components {
            let (type_key, type_id, value) = component.into_parts();
            initializer
                .insert_erased(runtime_entity, type_id, value)
                .map_err(|source| SceneInstantiationError::Ecs {
                    entity: id,
                    component: type_key,
                    source,
                })?;
        }
        let _previous = entities.insert(id, runtime_entity);
    }
    Ok(SceneInstance { entities })
}

#[derive(Debug, thiserror::Error)]
pub enum SceneInstantiationError {
    #[error("cannot build structural scene descriptor: {0}")]
    StructuralDescriptor(#[from] DescriptorError),
    #[error("component {component} is not registered for scene entity {entity:?}")]
    MissingComponentRegistration {
        entity: Option<SceneEntityId>,
        component: TypeKey,
    },
    #[error(
        "component {component} on scene entity {entity} collides with a reserved structural component"
    )]
    ReservedStructuralComponent {
        entity: SceneEntityId,
        component: TypeKey,
    },
    #[error("cannot insert component {component} for scene entity {entity}: {source}")]
    Ecs {
        entity: SceneEntityId,
        component: TypeKey,
        #[source]
        source: EcsError,
    },
}

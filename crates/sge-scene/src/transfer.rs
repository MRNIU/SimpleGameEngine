// Copyright The SimpleGameEngine Contributors

use std::{any::TypeId, collections::BTreeMap};

use sge_ecs::{EcsError, Entity, World, WorldInitializer};
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
    preflight_registrations(&prepared, |type_id| {
        initializer.component_is_registered(type_id)
    })?;
    let identity = scene_entity_id_descriptor()?;
    let parent = parent_descriptor()?;

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

pub fn preflight_instantiation(
    prepared: &PreparedScene,
    world: &World,
) -> Result<(), SceneInstantiationError> {
    preflight_registrations(prepared, |type_id| {
        world.component_type_is_registered(type_id)
    })
}

fn preflight_registrations(
    prepared: &PreparedScene,
    is_registered: impl Fn(TypeId) -> bool,
) -> Result<(), SceneInstantiationError> {
    let identity = scene_entity_id_descriptor()?;
    if !is_registered(TypeId::of::<SceneEntityId>()) {
        return Err(SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component: identity.type_key().clone(),
        });
    }
    let parent = parent_descriptor()?;
    if !is_registered(TypeId::of::<Parent>()) {
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
            if !is_registered(component.type_id()) {
                return Err(SceneInstantiationError::MissingComponentRegistration {
                    entity: Some(entity.id()),
                    component: component.type_key().clone(),
                });
            }
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use sge_ecs::World;
    use sge_reflect::TypeKey;

    use super::{SceneInstantiationError, instantiate};
    use crate::{
        Parent, PreparedScene, SceneEntityId,
        validation::{PreparedComponent, PreparedEntity},
    };

    #[test]
    fn instantiate_retains_structural_alias_defense_without_spawning()
    -> Result<(), Box<dyn std::error::Error>> {
        let entity: SceneEntityId = "00000000-0000-0000-0000-000000000001".parse()?;
        let component = TypeKey::new("demo.identity_alias")?;
        let prepared = PreparedScene::new(vec![PreparedEntity::new(
            entity,
            None,
            vec![PreparedComponent::new(
                component.clone(),
                TypeId::of::<SceneEntityId>(),
                Box::new(entity),
            )],
        )]);
        let mut world = World::new();
        world.register_component::<SceneEntityId>()?;
        world.register_component::<Parent>()?;
        world.finish_registration();

        let error = instantiate(prepared, world.initializer())
            .err()
            .ok_or("malformed prepared structural alias was accepted")?;

        assert!(matches!(
            error,
            SceneInstantiationError::ReservedStructuralComponent {
                entity: actual_entity,
                component: actual_component,
            } if actual_entity == entity && actual_component == component
        ));
        assert_eq!(world.entities().count(), 0);
        Ok(())
    }
}

// Copyright The SimpleGameEngine Contributors

use std::{any::TypeId, collections::BTreeMap};

use sge_asset::AssetLookup;
use sge_ecs::{EcsError, Entity, World};
use sge_reflect::{ReflectError, TypeKey, TypeRegistry};

use crate::{
    AuthoringEntity, AuthoringScene, Parent, SceneEntityId, SceneValidationError, prepare,
};

pub fn snapshot(
    world: &World,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<AuthoringScene, SceneSnapshotError> {
    let mut identities = BTreeMap::new();
    for runtime_entity in world.entities() {
        let Some(id) = world.get::<SceneEntityId>(runtime_entity).copied() else {
            return Err(SceneSnapshotError::MissingSceneEntityId { runtime_entity });
        };
        if let Some(first) = identities.insert(id, runtime_entity) {
            return Err(SceneSnapshotError::DuplicateSceneEntityId {
                id,
                first,
                duplicate: runtime_entity,
            });
        }
    }
    let entities = identities
        .into_iter()
        .map(|(id, runtime_entity)| {
            let parent = world.get::<Parent>(runtime_entity).map(|parent| parent.0);
            let components = registry
                .descriptors()
                .filter(|descriptor| {
                    descriptor.scene_saveable()
                        && descriptor.rust_type_id() != TypeId::of::<SceneEntityId>()
                        && descriptor.rust_type_id() != TypeId::of::<Parent>()
                })
                .filter_map(|descriptor| {
                    let component =
                        match world.component_erased(runtime_entity, descriptor.rust_type_id()) {
                            Ok(component) => component,
                            Err(source) => {
                                return Some(Err(SceneSnapshotError::Ecs {
                                    entity: id,
                                    component: descriptor.type_key().clone(),
                                    source,
                                }));
                            }
                        };
                    component.map(|component| {
                        registry
                            .encode(component)
                            .map_err(|source| SceneSnapshotError::Encode {
                                entity: id,
                                component: descriptor.type_key().clone(),
                                source,
                            })
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            AuthoringEntity::new(id, parent, components).map_err(SceneSnapshotError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scene = AuthoringScene::new(entities)?;
    let _prepared = prepare(&scene, registry, assets)?;
    Ok(scene)
}

impl From<SceneValidationError> for SceneSnapshotError {
    fn from(source: SceneValidationError) -> Self {
        Self::Validation(Box::new(source))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SceneSnapshotError {
    #[error("runtime entity {runtime_entity:?} has no SceneEntityId component")]
    MissingSceneEntityId { runtime_entity: Entity },
    #[error("scene entity ID {id} is attached to both runtime entity {first:?} and {duplicate:?}")]
    DuplicateSceneEntityId {
        id: SceneEntityId,
        first: Entity,
        duplicate: Entity,
    },
    #[error("cannot read component {component} for scene entity {entity}: {source}")]
    Ecs {
        entity: SceneEntityId,
        component: TypeKey,
        #[source]
        source: EcsError,
    },
    #[error("cannot encode component {component} for scene entity {entity}: {source}")]
    Encode {
        entity: SceneEntityId,
        component: TypeKey,
        #[source]
        source: ReflectError,
    },
    #[error("snapshot cannot rebuild a valid authoring scene: {0}")]
    Validation(#[source] Box<SceneValidationError>),
}

// Copyright The SimpleGameEngine Contributors

mod component;
mod error;
mod graph;
mod prepared;

use std::collections::BTreeSet;

use sge_asset::AssetLookup;
use sge_reflect::TypeRegistry;

use crate::AuthoringScene;

pub use error::SceneValidationError;
pub use prepared::PreparedScene;
pub(crate) use prepared::{PreparedComponent, PreparedEntity};

use component::prepare_component;
use graph::validate_parent_graph;

pub fn prepare(
    scene: &AuthoringScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<PreparedScene, SceneValidationError> {
    if !registry.is_frozen() {
        return Err(SceneValidationError::RegistryNotFrozen);
    }
    validate_parent_graph(scene)?;
    let entity_ids = scene
        .entities()
        .map(|entity| entity.id())
        .collect::<BTreeSet<_>>();
    let entities = scene
        .entities()
        .map(|entity| {
            let components = entity
                .components()
                .map(|component| {
                    prepare_component(entity.id(), component, registry, &entity_ids, assets)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PreparedEntity::new(
                entity.id(),
                entity.parent(),
                components,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedScene::new(entities))
}

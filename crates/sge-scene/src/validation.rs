// Copyright The SimpleGameEngine Contributors

mod component;
mod error;
mod graph;
mod prepared;

use std::collections::BTreeSet;

use sge_asset::{AssetId, AssetLookup};
use sge_reflect::{ReflectedValue, TypeRegistry};

use crate::{AuthoringScene, SceneEntityId};

pub use error::SceneValidationError;
pub use prepared::PreparedScene;
pub(crate) use prepared::{PreparedComponent, PreparedEntity};

use component::prepare_component;
use graph::validate_parent_graph;

pub(crate) struct SceneEntityView<'scene> {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: &'scene [ReflectedValue],
}

pub(crate) struct ValidationOutput {
    prepared: PreparedScene,
    root_assets: Vec<AssetId>,
}

impl<'scene> SceneEntityView<'scene> {
    pub(crate) const fn new(
        id: SceneEntityId,
        parent: Option<SceneEntityId>,
        components: &'scene [ReflectedValue],
    ) -> Self {
        Self {
            id,
            parent,
            components,
        }
    }
}

impl ValidationOutput {
    pub(crate) fn into_prepared(self) -> PreparedScene {
        self.prepared
    }

    pub(crate) fn into_parts(self) -> (PreparedScene, Vec<AssetId>) {
        (self.prepared, self.root_assets)
    }
}

pub fn prepare(
    scene: &AuthoringScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<PreparedScene, SceneValidationError> {
    let entities = scene
        .entities()
        .map(|entity| SceneEntityView::new(entity.id(), entity.parent(), entity.components_slice()))
        .collect::<Vec<_>>();
    validate_scene(&entities, registry, assets).map(ValidationOutput::into_prepared)
}

pub(crate) fn validate_scene(
    entities: &[SceneEntityView<'_>],
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<ValidationOutput, SceneValidationError> {
    if !registry.is_frozen() {
        return Err(SceneValidationError::RegistryNotFrozen);
    }
    validate_parent_graph(entities.iter().map(|entity| (entity.id, entity.parent)))?;
    let entity_ids = entities
        .iter()
        .map(|entity| entity.id)
        .collect::<BTreeSet<_>>();
    let mut root_assets = BTreeSet::new();
    let prepared = entities
        .iter()
        .map(|entity| {
            let components = entity
                .components
                .iter()
                .map(|component| {
                    prepare_component(
                        entity.id,
                        component,
                        registry,
                        &entity_ids,
                        assets,
                        &mut root_assets,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PreparedEntity::new(entity.id, entity.parent, components))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValidationOutput {
        prepared: PreparedScene::new(prepared),
        root_assets: root_assets.into_iter().collect(),
    })
}

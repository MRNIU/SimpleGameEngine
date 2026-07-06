// Copyright The SimpleGameEngine Contributors
//
//! `.scene.ron` 保存与加载。

use ecs::{EntityRecord, World};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDocument {
    pub entities: Vec<EntityRecord>,
}

impl SceneDocument {
    #[must_use]
    pub fn from_world(world: &World) -> Self {
        Self {
            entities: world.entities().cloned().collect(),
        }
    }
}

pub fn save_scene(world: &World) -> Result<String, SceneError> {
    let document = SceneDocument::from_world(world);
    let config = PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true)
        .enumerate_arrays(true);
    Ok(ron::ser::to_string_pretty(&document, config)?)
}

pub fn load_scene(input: &str) -> Result<World, SceneError> {
    let document: SceneDocument = ron::from_str(input)?;
    Ok(World::from_records(document.entities)?)
}

#[derive(Debug, Error)]
pub enum SceneError {
    #[error("failed to serialize scene: {0}")]
    Serialize(#[from] ron::Error),
    #[error("failed to parse scene: {0}")]
    Deserialize(#[from] ron::error::SpannedError),
    #[error(transparent)]
    Ecs(#[from] ecs::EcsError),
}

#[cfg(test)]
mod tests {
    use super::{load_scene, save_scene};
    use ecs::{EntityId, World};
    use math::Transform;

    #[test]
    fn serialized_scene_omits_runtime_children_cache() {
        let mut world = World::new();
        world.spawn(EntityId::new("root"), "Root", Transform::identity());

        let saved = save_scene(&world).unwrap();
        let loaded = load_scene(&saved).unwrap();

        assert!(!saved.contains("children"));
        assert!(loaded.entity("root").is_some());
    }
}

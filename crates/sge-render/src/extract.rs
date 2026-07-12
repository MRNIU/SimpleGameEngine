// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetId, RuntimeAssetStore};
use sge_ecs::{Entity, World};
use sge_math::Transform;
use sge_reflect::ValidationErrors;

use crate::{
    Camera, Light, Material, MeshRenderer,
    plugin::{validate_camera, validate_light, validate_material, validate_transform},
    snapshot::{RenderCamera, RenderLight, RenderMeshInstance, RenderSnapshot},
};

pub fn extract(
    world: &World,
    assets: &RuntimeAssetStore,
) -> Result<RenderSnapshot, RenderExtractionError> {
    let mut cameras = Vec::new();
    for (entity, camera) in world.query::<Camera>() {
        let transform = required_transform(world, entity, RenderComponentKind::Camera)?;
        validate(entity, RenderComponentKind::Transform, || {
            validate_transform(transform)
        })?;
        validate(entity, RenderComponentKind::Camera, || {
            validate_camera(camera)
        })?;
        cameras.push(RenderCamera::new(entity, *transform, *camera));
    }
    cameras.sort_unstable_by_key(|camera| camera.entity());
    checked_count(RenderItemKind::Camera, cameras.len())?;

    let mut meshes = Vec::new();
    for (entity, renderer) in world.query::<MeshRenderer>() {
        let transform = required_transform(world, entity, RenderComponentKind::MeshRenderer)?;
        let material = world
            .get::<Material>(entity)
            .ok_or(RenderExtractionError::MissingMaterial { entity })?;
        validate(entity, RenderComponentKind::Transform, || {
            validate_transform(transform)
        })?;
        validate(entity, RenderComponentKind::Material, || {
            validate_material(material)
        })?;
        if assets.mesh(renderer.mesh()).is_err() {
            return Err(RenderExtractionError::MissingMeshAsset {
                entity,
                asset: *renderer.mesh().id(),
            });
        }
        meshes.push(RenderMeshInstance::new(
            entity,
            *transform,
            renderer.mesh(),
            *material,
        ));
    }
    meshes.sort_unstable_by_key(|mesh| mesh.entity());
    checked_count(RenderItemKind::Mesh, meshes.len())?;

    let mut lights = Vec::new();
    for (entity, light) in world.query::<Light>() {
        let transform = required_transform(world, entity, RenderComponentKind::Light)?;
        validate(entity, RenderComponentKind::Transform, || {
            validate_transform(transform)
        })?;
        validate(entity, RenderComponentKind::Light, || validate_light(light))?;
        lights.push(RenderLight::new(entity, *transform, *light));
    }
    lights.sort_unstable_by_key(|light| light.entity());
    checked_count(RenderItemKind::Light, lights.len())?;
    if let [first, second, ..] = lights.as_slice() {
        return Err(RenderExtractionError::MultipleLights {
            first: first.entity(),
            second: second.entity(),
        });
    }

    Ok(RenderSnapshot {
        cameras,
        meshes,
        lights,
    })
}

fn required_transform(
    world: &World,
    entity: Entity,
    component: RenderComponentKind,
) -> Result<&Transform, RenderExtractionError> {
    world
        .get::<Transform>(entity)
        .ok_or(RenderExtractionError::MissingTransform { entity, component })
}

fn validate(
    entity: Entity,
    component: RenderComponentKind,
    validate: impl FnOnce() -> Result<(), ValidationErrors>,
) -> Result<(), RenderExtractionError> {
    validate().map_err(|source| RenderExtractionError::InvalidComponent {
        entity,
        component,
        source,
    })
}

fn checked_count(kind: RenderItemKind, count: usize) -> Result<u32, RenderExtractionError> {
    u32::try_from(count).map_err(|_| RenderExtractionError::CountOverflow { kind, count })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderComponentKind {
    Transform,
    Camera,
    MeshRenderer,
    Material,
    Light,
}

impl RenderComponentKind {
    #[must_use]
    pub const fn type_key(self) -> &'static str {
        match self {
            Self::Transform => "sge.transform",
            Self::Camera => "sge.camera",
            Self::MeshRenderer => "sge.mesh_renderer",
            Self::Material => "sge.material",
            Self::Light => "sge.light",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderItemKind {
    Camera,
    Mesh,
    Light,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderExtractionError {
    #[error("entity {entity:?} with {component:?} is missing Transform")]
    MissingTransform {
        entity: Entity,
        component: RenderComponentKind,
    },
    #[error("mesh entity {entity:?} is missing Material")]
    MissingMaterial { entity: Entity },
    #[error("mesh entity {entity:?} references missing mesh asset {asset}")]
    MissingMeshAsset { entity: Entity, asset: AssetId },
    #[error("entity {entity:?} has invalid {component:?}: {source}")]
    InvalidComponent {
        entity: Entity,
        component: RenderComponentKind,
        #[source]
        source: ValidationErrors,
    },
    #[error("render snapshot supports at most one light; found {first:?} and {second:?}")]
    MultipleLights { first: Entity, second: Entity },
    #[error("render {kind:?} count {count} exceeds u32")]
    CountOverflow { kind: RenderItemKind, count: usize },
}

#[cfg(test)]
mod tests {
    use super::{RenderExtractionError, RenderItemKind, checked_count};

    #[test]
    fn checked_count_reports_overflow_without_allocating_items() {
        let count = usize::MAX;
        assert!(matches!(
            checked_count(RenderItemKind::Mesh, count),
            Err(RenderExtractionError::CountOverflow {
                kind: RenderItemKind::Mesh,
                count: found,
            }) if found == count
        ));
    }
}

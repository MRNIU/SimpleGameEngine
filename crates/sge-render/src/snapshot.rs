// Copyright The SimpleGameEngine Contributors

use std::fmt;

use sge_asset::{AssetRef, MeshAsset};
use sge_ecs::Entity;
use sge_math::Transform;

use crate::{Camera, Light, Material};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSnapshot {
    pub(crate) cameras: Vec<RenderCamera>,
    pub(crate) meshes: Vec<RenderMeshInstance>,
    pub(crate) lights: Vec<RenderLight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCamera {
    entity: Entity,
    transform: Transform,
    camera: Camera,
}

#[derive(Clone, Copy, PartialEq)]
pub struct RenderMeshInstance {
    entity: Entity,
    transform: Transform,
    mesh: AssetRef<MeshAsset>,
    material: Material,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderLight {
    entity: Entity,
    transform: Transform,
    light: Light,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderView {
    camera: RenderCamera,
}

impl RenderSnapshot {
    #[must_use]
    pub fn cameras(&self) -> &[RenderCamera] {
        &self.cameras
    }

    #[must_use]
    pub fn meshes(&self) -> &[RenderMeshInstance] {
        &self.meshes
    }

    #[must_use]
    pub fn lights(&self) -> &[RenderLight] {
        &self.lights
    }
}

impl RenderCamera {
    pub(crate) const fn new(entity: Entity, transform: Transform, camera: Camera) -> Self {
        Self {
            entity,
            transform,
            camera,
        }
    }

    #[must_use]
    pub const fn entity(self) -> Entity {
        self.entity
    }

    #[must_use]
    pub const fn transform(self) -> Transform {
        self.transform
    }

    #[must_use]
    pub const fn camera(self) -> Camera {
        self.camera
    }
}

impl RenderMeshInstance {
    pub(crate) const fn new(
        entity: Entity,
        transform: Transform,
        mesh: AssetRef<MeshAsset>,
        material: Material,
    ) -> Self {
        Self {
            entity,
            transform,
            mesh,
            material,
        }
    }

    #[must_use]
    pub const fn entity(self) -> Entity {
        self.entity
    }

    #[must_use]
    pub const fn transform(self) -> Transform {
        self.transform
    }

    #[must_use]
    pub const fn mesh(self) -> AssetRef<MeshAsset> {
        self.mesh
    }

    #[must_use]
    pub const fn material(self) -> Material {
        self.material
    }
}

impl fmt::Debug for RenderMeshInstance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderMeshInstance")
            .field("entity", &self.entity)
            .field("transform", &self.transform)
            .field("mesh", self.mesh.id())
            .field("material", &self.material)
            .finish()
    }
}

impl RenderLight {
    pub(crate) const fn new(entity: Entity, transform: Transform, light: Light) -> Self {
        Self {
            entity,
            transform,
            light,
        }
    }

    #[must_use]
    pub const fn entity(self) -> Entity {
        self.entity
    }

    #[must_use]
    pub const fn transform(self) -> Transform {
        self.transform
    }

    #[must_use]
    pub const fn light(self) -> Light {
        self.light
    }
}

impl RenderView {
    pub fn from_active_camera(snapshot: &RenderSnapshot) -> Result<Self, RenderViewError> {
        let mut active = snapshot
            .cameras
            .iter()
            .copied()
            .filter(|camera| camera.camera.active());
        let camera = active.next().ok_or(RenderViewError::MissingActiveCamera)?;
        if let Some(second) = active.next() {
            return Err(RenderViewError::MultipleActiveCameras {
                first: camera.entity,
                second: second.entity,
            });
        }
        Ok(Self { camera })
    }

    #[must_use]
    pub const fn entity(self) -> Entity {
        self.camera.entity
    }

    #[must_use]
    pub const fn transform(self) -> Transform {
        self.camera.transform
    }

    #[must_use]
    pub const fn camera(self) -> Camera {
        self.camera.camera
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RenderViewError {
    #[error("render snapshot has no active camera")]
    MissingActiveCamera,
    #[error("render snapshot has multiple active cameras: {first:?} and {second:?}")]
    MultipleActiveCameras { first: Entity, second: Entity },
    #[error("active camera {entity:?} has an invalid projection")]
    InvalidProjection { entity: Entity },
}

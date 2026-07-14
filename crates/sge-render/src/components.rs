// Copyright The SimpleGameEngine Contributors

use std::fmt;

use sge_asset::{AssetRef, MeshAsset, OptionalAssetRef, TextureAsset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub(crate) active: bool,
    pub(crate) projection: Projection,
    pub(crate) vertical_fov_radians: f32,
    pub(crate) orthographic_height: f32,
    pub(crate) near: f32,
    pub(crate) far: f32,
}

impl Camera {
    #[must_use]
    pub const fn new(
        active: bool,
        projection: Projection,
        vertical_fov_radians: f32,
        orthographic_height: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            active,
            projection,
            vertical_fov_radians,
            orthographic_height,
            near,
            far,
        }
    }

    #[must_use]
    pub const fn active(self) -> bool {
        self.active
    }

    #[must_use]
    pub const fn projection(self) -> Projection {
        self.projection
    }

    #[must_use]
    pub const fn vertical_fov_radians(self) -> f32 {
        self.vertical_fov_radians
    }

    #[must_use]
    pub const fn orthographic_height(self) -> f32 {
        self.orthographic_height
    }

    #[must_use]
    pub const fn near(self) -> f32 {
        self.near
    }

    #[must_use]
    pub const fn far(self) -> f32 {
        self.far
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(
            false,
            Projection::Perspective,
            std::f32::consts::FRAC_PI_3,
            10.0,
            0.1,
            1000.0,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MeshRenderer {
    pub(crate) mesh: AssetRef<MeshAsset>,
}

impl fmt::Debug for MeshRenderer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeshRenderer")
            .field("mesh", self.mesh.id())
            .finish()
    }
}

impl MeshRenderer {
    #[must_use]
    pub const fn new(mesh: AssetRef<MeshAsset>) -> Self {
        Self { mesh }
    }

    #[must_use]
    pub const fn mesh(self) -> AssetRef<MeshAsset> {
        self.mesh
    }
}

impl Default for MeshRenderer {
    fn default() -> Self {
        Self::new(AssetRef::new(sge_asset::AssetId::nil()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub(crate) base_color: [f32; 4],
    pub(crate) texture: OptionalAssetRef<TextureAsset>,
}

impl Material {
    #[must_use]
    pub const fn new(base_color: [f32; 4]) -> Self {
        Self {
            base_color,
            texture: OptionalAssetRef::none(),
        }
    }

    #[must_use]
    pub const fn with_texture(base_color: [f32; 4], texture: AssetRef<TextureAsset>) -> Self {
        Self {
            base_color,
            texture: OptionalAssetRef::some(texture),
        }
    }

    #[must_use]
    pub const fn base_color(self) -> [f32; 4] {
        self.base_color
    }

    #[must_use]
    pub const fn texture(self) -> Option<AssetRef<TextureAsset>> {
        self.texture.get()
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::new([1.0; 4])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A directional light whose direction comes from its entity Transform rotation.
pub struct Light {
    pub(crate) color: [f32; 4],
    pub(crate) intensity: f32,
}

impl Light {
    #[must_use]
    pub const fn new(color: [f32; 4], intensity: f32) -> Self {
        Self { color, intensity }
    }

    #[must_use]
    pub const fn color(self) -> [f32; 4] {
        self.color
    }

    #[must_use]
    pub const fn intensity(self) -> f32 {
        self.intensity
    }
}

impl Default for Light {
    fn default() -> Self {
        Self::new([1.0; 4], 1.0)
    }
}

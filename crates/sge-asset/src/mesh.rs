// Copyright The SimpleGameEngine Contributors

use serde::{Deserialize, Serialize};

use crate::{AssetType, MESH_ASSET_TYPE_KEY};

pub const MESH_ASSET_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct MeshVertex {
    position: [f32; 3],
    normal: Option<[f32; 3]>,
    texcoord: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshAsset {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshAssetWire {
    format_version: u32,
    vertices: Vec<MeshVertexWire>,
    indices: Vec<u32>,
}

#[derive(Deserialize)]
struct MeshAssetVersionWire {
    format_version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshVertexWire {
    position: [f32; 3],
    normal: Option<[f32; 3]>,
    texcoord: Option<[f32; 2]>,
}

impl MeshVertex {
    pub fn new(
        position: [f32; 3],
        normal: Option<[f32; 3]>,
        texcoord: Option<[f32; 2]>,
    ) -> Result<Self, MeshAssetError> {
        validate_finite("position", &position)?;
        if let Some(normal) = &normal {
            validate_finite("normal", normal)?;
        }
        if let Some(texcoord) = &texcoord {
            validate_finite("texcoord", texcoord)?;
        }
        Ok(Self {
            position,
            normal,
            texcoord,
        })
    }

    #[must_use]
    pub const fn position(&self) -> &[f32; 3] {
        &self.position
    }

    #[must_use]
    pub const fn normal(&self) -> Option<&[f32; 3]> {
        self.normal.as_ref()
    }

    #[must_use]
    pub const fn texcoord(&self) -> Option<&[f32; 2]> {
        self.texcoord.as_ref()
    }
}

impl MeshAsset {
    pub fn new(vertices: Vec<MeshVertex>, indices: Vec<u32>) -> Result<Self, MeshAssetError> {
        if vertices.is_empty() {
            return Err(MeshAssetError::EmptyVertices);
        }
        if indices.is_empty() {
            return Err(MeshAssetError::EmptyIndices);
        }
        if !indices.len().is_multiple_of(3) {
            return Err(MeshAssetError::NonTriangleIndexCount { len: indices.len() });
        }
        if let Some((position, index)) = indices.iter().copied().enumerate().find(|(_, index)| {
            usize::try_from(*index).map_or(true, |index| index >= vertices.len())
        }) {
            return Err(MeshAssetError::IndexOutOfRange {
                position,
                index,
                vertex_count: vertices.len(),
            });
        }
        Ok(Self { vertices, indices })
    }

    pub fn from_ron(input: &str) -> Result<Self, MeshAssetFormatError> {
        let version: MeshAssetVersionWire =
            ron::from_str(input).map_err(|source| MeshAssetFormatError::Parse {
                source: Box::new(source),
            })?;
        if version.format_version != MESH_ASSET_FORMAT_VERSION {
            return Err(MeshAssetFormatError::VersionMismatch {
                expected: MESH_ASSET_FORMAT_VERSION,
                found: version.format_version,
            });
        }
        let wire: MeshAssetWire =
            ron::from_str(input).map_err(|source| MeshAssetFormatError::Parse {
                source: Box::new(source),
            })?;
        let vertices = wire
            .vertices
            .into_iter()
            .enumerate()
            .map(|(index, vertex)| {
                MeshVertex::new(vertex.position, vertex.normal, vertex.texcoord)
                    .map_err(|source| MeshAssetFormatError::InvalidVertex { index, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(vertices, wire.indices).map_err(MeshAssetFormatError::InvalidMesh)
    }

    pub fn to_ron(&self) -> Result<String, MeshAssetFormatError> {
        let wire = MeshAssetWire {
            format_version: MESH_ASSET_FORMAT_VERSION,
            vertices: self
                .vertices
                .iter()
                .map(|vertex| MeshVertexWire {
                    position: vertex.position,
                    normal: vertex.normal,
                    texcoord: vertex.texcoord,
                })
                .collect(),
            indices: self.indices.clone(),
        };
        ron::ser::to_string_pretty(&wire, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|source| MeshAssetFormatError::Serialize { source })
    }

    #[must_use]
    pub fn vertices(&self) -> &[MeshVertex] {
        &self.vertices
    }

    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

impl AssetType for MeshAsset {
    const TYPE_KEY: &'static str = MESH_ASSET_TYPE_KEY;
}

fn validate_finite(attribute: &'static str, values: &[f32]) -> Result<(), MeshAssetError> {
    if let Some(component) = values.iter().position(|value| !value.is_finite()) {
        return Err(MeshAssetError::NonFiniteVertexAttribute {
            attribute,
            component,
        });
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MeshAssetError {
    #[error("mesh must contain at least one vertex")]
    EmptyVertices,
    #[error("mesh must contain at least one index")]
    EmptyIndices,
    #[error("vertex {attribute} component {component} must be finite")]
    NonFiniteVertexAttribute {
        attribute: &'static str,
        component: usize,
    },
    #[error("mesh index count {len} is not divisible by three")]
    NonTriangleIndexCount { len: usize },
    #[error("mesh index {index} at position {position} exceeds vertex count {vertex_count}")]
    IndexOutOfRange {
        position: usize,
        index: u32,
        vertex_count: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum MeshAssetFormatError {
    #[error("cannot parse MeshAsset: {source}")]
    Parse {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize MeshAsset: {source}")]
    Serialize {
        #[source]
        source: ron::Error,
    },
    #[error("unsupported MeshAsset version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("invalid MeshAsset vertex {index}: {source}")]
    InvalidVertex {
        index: usize,
        #[source]
        source: MeshAssetError,
    },
    #[error("invalid MeshAsset: {0}")]
    InvalidMesh(#[source] MeshAssetError),
}

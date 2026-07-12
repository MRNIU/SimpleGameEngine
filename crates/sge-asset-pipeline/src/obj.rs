// Copyright The SimpleGameEngine Contributors

use std::io::Cursor;

use sge_asset::{AssetId, MeshAsset, MeshAssetError, MeshVertex};
use sge_project::{ProjectPath, SourceAssetRecord, SourceImporter};

pub(crate) fn parse_obj(
    record: &SourceAssetRecord,
    raw_bytes: &[u8],
) -> Result<MeshAsset, ObjImportError> {
    let SourceImporter::Obj(settings) = record.importer();
    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let mut reader = Cursor::new(raw_bytes);
    let (models, _materials) = tobj::load_obj_buf(&mut reader, &options, |_| {
        Ok((Vec::new(), Default::default()))
    })
    .map_err(|source| ObjImportError::new(record, ObjImportErrorKind::Parse(source)))?;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (model_index, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        if mesh.positions.is_empty() || mesh.indices.is_empty() {
            return Err(ObjImportError::new(
                record,
                ObjImportErrorKind::EmptyModel { model: model_index },
            ));
        }
        if !mesh.positions.len().is_multiple_of(3) {
            return Err(ObjImportError::new(
                record,
                ObjImportErrorKind::PositionCardinality {
                    model: model_index,
                    actual: mesh.positions.len(),
                },
            ));
        }

        let vertex_count = mesh.positions.len() / 3;
        let expected_normals = vertex_count * 3;
        if !mesh.normals.is_empty() && mesh.normals.len() != expected_normals {
            return Err(ObjImportError::new(
                record,
                ObjImportErrorKind::NormalCardinality {
                    model: model_index,
                    expected: expected_normals,
                    actual: mesh.normals.len(),
                },
            ));
        }
        let expected_texcoords = vertex_count * 2;
        if !mesh.texcoords.is_empty() && mesh.texcoords.len() != expected_texcoords {
            return Err(ObjImportError::new(
                record,
                ObjImportErrorKind::TexcoordCardinality {
                    model: model_index,
                    expected: expected_texcoords,
                    actual: mesh.texcoords.len(),
                },
            ));
        }
        if !mesh.indices.len().is_multiple_of(3) {
            return Err(ObjImportError::new(
                record,
                ObjImportErrorKind::NonTriangleIndexCount {
                    model: model_index,
                    actual: mesh.indices.len(),
                },
            ));
        }

        let base_vertex = vertices.len();
        for vertex_index in 0..vertex_count {
            let position_start = vertex_index * 3;
            let position = [
                mesh.positions[position_start],
                mesh.positions[position_start + 1],
                mesh.positions[position_start + 2],
            ];
            let normal = (!mesh.normals.is_empty()).then(|| {
                let start = vertex_index * 3;
                [
                    mesh.normals[start],
                    mesh.normals[start + 1],
                    mesh.normals[start + 2],
                ]
            });
            let texcoord = (!mesh.texcoords.is_empty()).then(|| {
                let start = vertex_index * 2;
                let v = if settings.flip_texcoord_v() {
                    1.0 - mesh.texcoords[start + 1]
                } else {
                    mesh.texcoords[start + 1]
                };
                [mesh.texcoords[start], v]
            });
            vertices.push(
                MeshVertex::new(position, normal, texcoord).map_err(|source| {
                    ObjImportError::new(
                        record,
                        ObjImportErrorKind::Vertex {
                            model: model_index,
                            vertex: vertex_index,
                            source,
                        },
                    )
                })?,
            );
        }

        for (index_position, local_index) in mesh.indices.iter().copied().enumerate() {
            if usize::try_from(local_index).map_or(true, |index| index >= vertex_count) {
                return Err(ObjImportError::new(
                    record,
                    ObjImportErrorKind::LocalIndexOutOfRange {
                        model: model_index,
                        position: index_position,
                        index: local_index,
                        vertex_count,
                    },
                ));
            }
            let rebased = rebase_index(record, model_index, base_vertex, local_index)?;
            indices.push(rebased);
        }
    }

    MeshAsset::new(vertices, indices)
        .map_err(|source| ObjImportError::new(record, ObjImportErrorKind::Mesh(source)))
}

fn checked_rebase(base: usize, index: u32) -> Option<u32> {
    u32::try_from(base).ok()?.checked_add(index)
}

fn rebase_index(
    record: &SourceAssetRecord,
    model: usize,
    base: usize,
    index: u32,
) -> Result<u32, ObjImportError> {
    checked_rebase(base, index).ok_or_else(|| {
        ObjImportError::new(
            record,
            ObjImportErrorKind::IndexRebaseOverflow { model, base, index },
        )
    })
}

#[derive(Debug, thiserror::Error)]
#[error("cannot import OBJ asset {asset_id} from {source_path}: {kind}")]
pub struct ObjImportError {
    asset_id: AssetId,
    source_path: ProjectPath,
    #[source]
    kind: ObjImportErrorKind,
}

impl ObjImportError {
    fn new(record: &SourceAssetRecord, kind: ObjImportErrorKind) -> Self {
        Self {
            asset_id: record.id(),
            source_path: record.source().clone(),
            kind,
        }
    }

    #[cfg(test)]
    pub(crate) const fn parser_source(&self) -> Option<tobj::LoadError> {
        match &self.kind {
            ObjImportErrorKind::Parse(source) => Some(*source),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ObjImportErrorKind {
    #[error("cannot parse OBJ: {0}")]
    Parse(#[source] tobj::LoadError),
    #[error("model {model} contains no triangle geometry")]
    EmptyModel { model: usize },
    #[error("model {model} position array length {actual} is not divisible by three")]
    PositionCardinality { model: usize, actual: usize },
    #[error("model {model} normal array length must be {expected}, got {actual}")]
    NormalCardinality {
        model: usize,
        expected: usize,
        actual: usize,
    },
    #[error("model {model} texture-coordinate array length must be {expected}, got {actual}")]
    TexcoordCardinality {
        model: usize,
        expected: usize,
        actual: usize,
    },
    #[error("model {model} index array length {actual} is not divisible by three")]
    NonTriangleIndexCount { model: usize, actual: usize },
    #[error(
        "model {model} index {index} at position {position} exceeds vertex count {vertex_count}"
    )]
    LocalIndexOutOfRange {
        model: usize,
        position: usize,
        index: u32,
        vertex_count: usize,
    },
    #[error("model {model} index {index} cannot be rebased from global vertex {base}")]
    IndexRebaseOverflow {
        model: usize,
        base: usize,
        index: u32,
    },
    #[error("model {model} vertex {vertex} is invalid: {source}")]
    Vertex {
        model: usize,
        vertex: usize,
        #[source]
        source: MeshAssetError,
    },
    #[error("imported mesh is invalid: {0}")]
    Mesh(#[source] MeshAssetError),
}

#[cfg(test)]
mod tests;

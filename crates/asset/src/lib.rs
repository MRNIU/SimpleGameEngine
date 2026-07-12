// Copyright The SimpleGameEngine Contributors
//
//! 资源标识、资产清单与导入 mesh 数据。

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANIFEST_RELATIVE_PATH: &str = "assets/asset_manifest.ron";
pub const IMPORTED_RELATIVE_DIR: &str = "assets/imported";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetUuid(uuid::Uuid);

impl AssetUuid {
    #[must_use]
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from_string(value: &str) -> Result<Self, AssetError> {
        Ok(Self(uuid::Uuid::parse_str(value)?))
    }

    pub fn parse_asset_ref(value: &str) -> Result<Self, AssetError> {
        let uuid = value
            .strip_prefix("asset:")
            .ok_or(AssetError::InvalidAssetRef)?;
        Self::from_string(uuid)
    }

    #[must_use]
    pub fn to_asset_ref(&self) -> String {
        format!("asset:{self}")
    }
}

impl std::fmt::Display for AssetUuid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Mesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetImporter {
    Obj,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub uuid: AssetUuid,
    pub name: String,
    pub kind: AssetKind,
    pub path: PathBuf,
    pub importer: AssetImporter,
    pub source_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetManifest {
    pub assets: Vec<AssetRecord>,
}

impl AssetManifest {
    pub fn load_from_project_root(project_root: &Path) -> Result<Self, AssetError> {
        let path = manifest_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(ron::from_str(&fs::read_to_string(path)?)?)
    }

    pub fn save_to_project_root(&self, project_root: &Path) -> Result<(), AssetError> {
        let path = manifest_path(project_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = ron::ser::PrettyConfig::new()
            .depth_limit(4)
            .separate_tuple_members(true)
            .enumerate_arrays(true);
        fs::write(path, ron::ser::to_string_pretty(self, config)?)?;
        Ok(())
    }

    pub fn upsert(&mut self, record: AssetRecord) {
        if let Some(existing) = self
            .assets
            .iter_mut()
            .find(|asset| asset.uuid == record.uuid)
        {
            *existing = record;
        } else {
            self.assets.push(record);
            self.assets
                .sort_by(|left, right| left.name.cmp(&right.name));
        }
    }

    #[must_use]
    pub fn find(&self, uuid: &AssetUuid) -> Option<&AssetRecord> {
        self.assets.iter().find(|asset| &asset.uuid == uuid)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedVertex {
    pub position: [f32; 3],
    pub normal: Option<[f32; 3]>,
    pub uv: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMesh {
    pub vertices: Vec<ImportedVertex>,
    pub indices: Vec<u16>,
}

#[must_use]
pub fn manifest_path(project_root: &Path) -> PathBuf {
    project_root.join(MANIFEST_RELATIVE_PATH)
}

pub fn load_obj_mesh(path: &Path) -> Result<ImportedMesh, AssetError> {
    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _) = tobj::load_obj(path, &options)?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for model in models {
        let mesh = model.mesh;
        if mesh.positions.is_empty() {
            continue;
        }
        let base_vertex = vertices.len();
        for (index, position) in mesh.positions.chunks_exact(3).enumerate() {
            let position = [position[0], position[1], position[2]];
            if !position.iter().all(|value| value.is_finite()) {
                return Err(AssetError::InvalidVertex);
            }
            let normal = read_vec3(&mesh.normals, index)?;
            let uv = read_vec2(&mesh.texcoords, index)?;
            vertices.push(ImportedVertex {
                position,
                normal,
                uv,
            });
        }
        for index in mesh.indices {
            let index = base_vertex
                .checked_add(usize::try_from(index).map_err(|_| AssetError::MeshTooLarge)?)
                .ok_or(AssetError::MeshTooLarge)?;
            indices.push(u16::try_from(index).map_err(|_| AssetError::MeshTooLarge)?);
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        return Err(AssetError::EmptyMesh);
    }

    Ok(ImportedMesh { vertices, indices })
}

pub fn unique_import_path(
    _project_root: &Path,
    source_path: &Path,
    existing: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, AssetError> {
    if source_path.file_name().is_none() {
        return Err(AssetError::MissingFileName);
    }
    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or(AssetError::MissingFileName)?;
    let stem = sanitize_stem(stem);
    let existing = existing.into_iter().collect::<BTreeSet<_>>();

    for index in 0..=u32::MAX {
        let file_name = if index == 0 {
            format!("{stem}.obj")
        } else {
            format!("{stem}_{index}.obj")
        };
        let candidate = PathBuf::from(IMPORTED_RELATIVE_DIR).join(file_name);
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Err(AssetError::MissingFileName)
}

fn read_vec3(values: &[f32], index: usize) -> Result<Option<[f32; 3]>, AssetError> {
    let start = index * 3;
    let Some(values) = values.get(start..start + 3) else {
        return Ok(None);
    };
    let value = [values[0], values[1], values[2]];
    if value.iter().all(|item| item.is_finite()) {
        Ok(Some(value))
    } else {
        Err(AssetError::InvalidVertex)
    }
}

fn read_vec2(values: &[f32], index: usize) -> Result<Option<[f32; 2]>, AssetError> {
    let start = index * 2;
    let Some(values) = values.get(start..start + 2) else {
        return Ok(None);
    };
    let value = [values[0], values[1]];
    if value.iter().all(|item| item.is_finite()) {
        Ok(Some(value))
    } else {
        Err(AssetError::InvalidVertex)
    }
}

fn sanitize_stem(stem: &str) -> String {
    let sanitized: String = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if sanitized.is_empty() {
        "asset".to_owned()
    } else {
        sanitized
    }
}

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("invalid asset uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),
    #[error("invalid asset ref")]
    InvalidAssetRef,
    #[error("failed to read or write asset file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to serialize asset manifest: {0}")]
    Serialize(#[from] ron::Error),
    #[error("failed to parse asset manifest: {0}")]
    Deserialize(#[from] ron::error::SpannedError),
    #[error("failed to parse OBJ: {0}")]
    Obj(#[from] tobj::LoadError),
    #[error("OBJ mesh is empty")]
    EmptyMesh,
    #[error("OBJ vertex is invalid")]
    InvalidVertex,
    #[error("OBJ mesh is too large for current viewport")]
    MeshTooLarge,
    #[error("import path must have a file name")]
    MissingFileName,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::{
        AssetError, AssetImporter, AssetKind, AssetManifest, AssetRecord, AssetUuid, manifest_path,
        unique_import_path,
    };

    #[test]
    fn manifest_roundtrip_uses_project_root_assets_path() {
        let root = temp_project_root("manifest_roundtrip");
        let uuid = AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let mut manifest = AssetManifest::default();
        manifest.upsert(AssetRecord {
            uuid: uuid.clone(),
            name: "crate".to_owned(),
            kind: AssetKind::Mesh,
            path: PathBuf::from("assets/imported/crate.obj"),
            importer: AssetImporter::Obj,
            source_name: "crate.obj".to_owned(),
        });

        manifest.save_to_project_root(&root).unwrap();
        let loaded = AssetManifest::load_from_project_root(&root).unwrap();

        assert_eq!(manifest_path(&root), root.join("assets/asset_manifest.ron"));
        assert_eq!(loaded.find(&uuid).unwrap().name, "crate");
        assert_eq!(
            loaded.find(&uuid).unwrap().path,
            PathBuf::from("assets/imported/crate.obj")
        );
    }

    #[test]
    fn asset_uuid_roundtrips_asset_ref() {
        let uuid = AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();

        assert_eq!(
            AssetUuid::parse_asset_ref(&uuid.to_asset_ref()).unwrap(),
            uuid
        );
        assert!(AssetUuid::parse_asset_ref("primitive:cube").is_err());
    }

    #[test]
    fn obj_loader_triangulates_quad_and_ignores_material() {
        let path = temp_obj_path(
            "quad_with_material",
            "\
mtllib ignored.mtl
o Quad
usemtl Ignored
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1 2 3 4
",
        );

        let mesh = super::load_obj_mesh(&path).unwrap();

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        assert!(mesh.vertices.iter().all(|vertex| vertex.normal.is_none()));
        assert!(mesh.vertices.iter().all(|vertex| vertex.uv.is_none()));
    }

    #[test]
    fn obj_loader_rejects_empty_mesh_and_non_finite_positions() {
        let empty = temp_obj_path("empty_obj", "# no geometry\n");
        let non_finite = temp_obj_path(
            "non_finite_obj",
            "\
v NaN 0 0
v 1 0 0
v 0 1 0
f 1 2 3
",
        );

        assert!(matches!(
            super::load_obj_mesh(&empty),
            Err(AssetError::EmptyMesh)
        ));
        assert!(matches!(
            super::load_obj_mesh(&non_finite),
            Err(AssetError::InvalidVertex)
        ));
    }

    #[test]
    fn unique_import_path_stays_inside_assets_imported() {
        let root = temp_project_root("unique_import_path");
        let source = root.join("../crate.obj");
        let existing = vec![PathBuf::from("assets/imported/crate.obj")];

        let first = unique_import_path(&root, &source, Vec::<PathBuf>::new()).unwrap();
        let second = unique_import_path(&root, &source, existing).unwrap();

        assert_eq!(first, PathBuf::from("assets/imported/crate.obj"));
        assert_eq!(second, PathBuf::from("assets/imported/crate_1.obj"));
        assert!(!first.starts_with(".."));
        assert!(!second.starts_with(".."));
    }

    fn temp_project_root(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/asset_tests")
            .join(format!("{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets/imported")).unwrap();
        root
    }

    fn temp_obj_path(name: &str, content: &str) -> PathBuf {
        let root = temp_project_root(name);
        let path = root.join(format!("{name}.obj"));
        fs::write(&path, content).unwrap();
        path
    }
}

// Copyright The SimpleGameEngine Contributors

use serde::{Deserialize, Serialize};
use sge_asset::{AssetId, AssetIdError, AssetLookup, MESH_ASSET_TYPE_KEY};
use sge_reflect::{KeyError, TypeKey};

use crate::{ProjectIoError, ProjectPath, ProjectPathError, ProjectRoot, canonical_pretty_config};

pub const AUTHORING_ASSET_MANIFEST_FORMAT_VERSION: u32 = 2;
pub const AUTHORING_ASSET_MANIFEST_PATH: &str = "Content/asset_manifest.ron";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjImportSettings {
    flip_texcoord_v: bool,
}

impl ObjImportSettings {
    #[must_use]
    pub const fn new(flip_texcoord_v: bool) -> Self {
        Self { flip_texcoord_v }
    }

    #[must_use]
    pub const fn flip_texcoord_v(&self) -> bool {
        self.flip_texcoord_v
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceImporter {
    Obj(ObjImportSettings),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAssetRecord {
    id: AssetId,
    asset_type: TypeKey,
    source: ProjectPath,
    importer: SourceImporter,
}

impl SourceAssetRecord {
    pub fn new(
        id: AssetId,
        asset_type: TypeKey,
        source: ProjectPath,
        importer: SourceImporter,
    ) -> Result<Self, ManifestError> {
        match &importer {
            SourceImporter::Obj(_) if asset_type.as_str() != MESH_ASSET_TYPE_KEY => {
                let expected = TypeKey::new(MESH_ASSET_TYPE_KEY).map_err(|source| {
                    ManifestError::InvalidAssetType {
                        value: MESH_ASSET_TYPE_KEY.to_owned(),
                        source,
                    }
                })?;
                return Err(ManifestError::ImporterAssetTypeMismatch {
                    expected,
                    actual: asset_type,
                });
            }
            SourceImporter::Obj(_) if !source.as_str().ends_with(".obj") => {
                return Err(ManifestError::InvalidObjSource { path: source });
            }
            SourceImporter::Obj(_) => {}
        }
        Ok(Self {
            id,
            asset_type,
            source,
            importer,
        })
    }

    #[must_use]
    pub const fn id(&self) -> AssetId {
        self.id
    }

    #[must_use]
    pub fn asset_type(&self) -> &TypeKey {
        &self.asset_type
    }

    #[must_use]
    pub fn source(&self) -> &ProjectPath {
        &self.source
    }

    #[must_use]
    pub const fn importer(&self) -> &SourceImporter {
        &self.importer
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringAssetManifest {
    records: Vec<SourceAssetRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoringAssetManifestWire {
    format_version: u32,
    assets: Vec<SourceAssetRecordWire>,
}

#[derive(Deserialize)]
struct ManifestVersionProbe {
    format_version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceAssetRecordWire {
    id: String,
    asset_type: String,
    source: String,
    importer: SourceImporterWire,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum SourceImporterWire {
    Obj { settings: ObjImportSettingsWire },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjImportSettingsWire {
    flip_texcoord_v: bool,
}

impl AuthoringAssetManifest {
    pub fn new(mut records: Vec<SourceAssetRecord>) -> Result<Self, ManifestError> {
        records.sort_unstable_by_key(SourceAssetRecord::id);
        if let Some(pair) = records.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(ManifestError::DuplicateAssetId { id: pair[0].id });
        }
        Ok(Self { records })
    }

    #[must_use]
    pub fn records(&self) -> &[SourceAssetRecord] {
        &self.records
    }

    pub fn from_ron(input: &str) -> Result<Self, ManifestError> {
        Self::from_bytes(input.as_bytes())
    }

    fn from_bytes(input: &[u8]) -> Result<Self, ManifestError> {
        let path = ProjectPath::new(AUTHORING_ASSET_MANIFEST_PATH)?;
        let version: ManifestVersionProbe =
            ron::de::from_bytes(input).map_err(|source| ManifestError::Parse {
                path: path.clone(),
                source: Box::new(source),
            })?;
        if version.format_version != AUTHORING_ASSET_MANIFEST_FORMAT_VERSION {
            return Err(ManifestError::VersionMismatch {
                path,
                expected: AUTHORING_ASSET_MANIFEST_FORMAT_VERSION,
                found: version.format_version,
            });
        }
        let wire: AuthoringAssetManifestWire =
            ron::de::from_bytes(input).map_err(|source| ManifestError::Parse {
                path: path.clone(),
                source: Box::new(source),
            })?;
        let records = wire
            .assets
            .into_iter()
            .map(|record| record.into_domain(&path))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(records).map_err(|source| ManifestError::AtPath {
            path,
            source: Box::new(source),
        })
    }

    pub fn to_ron(&self) -> Result<String, ManifestError> {
        let path = ProjectPath::new(AUTHORING_ASSET_MANIFEST_PATH)?;
        self.validate().map_err(|source| ManifestError::AtPath {
            path: path.clone(),
            source: Box::new(source),
        })?;
        let wire = AuthoringAssetManifestWire {
            format_version: AUTHORING_ASSET_MANIFEST_FORMAT_VERSION,
            assets: self
                .records
                .iter()
                .map(|record| SourceAssetRecordWire {
                    id: record.id.to_string(),
                    asset_type: record.asset_type.to_string(),
                    source: record.source.to_string(),
                    importer: SourceImporterWire::from_domain(&record.importer),
                })
                .collect(),
        };
        ron::ser::to_string_pretty(&wire, canonical_pretty_config().depth_limit(3))
            .map_err(|source| ManifestError::Serialize { path, source })
    }

    pub fn load(root: &ProjectRoot) -> Result<Self, ManifestError> {
        let path = ProjectPath::new(AUTHORING_ASSET_MANIFEST_PATH)?;
        Self::from_bytes(&root.read(&path)?)
    }

    pub fn save(&self, root: &ProjectRoot) -> Result<(), ManifestError> {
        let path = ProjectPath::new(AUTHORING_ASSET_MANIFEST_PATH)?;
        let encoded = self.to_ron()?;
        root.write_atomic(&path, encoded.as_bytes())?;
        Ok(())
    }

    fn validate(&self) -> Result<(), ManifestError> {
        Self::new(self.records.clone()).map(|_| ())
    }
}

impl SourceAssetRecordWire {
    fn into_domain(self, context: &ProjectPath) -> Result<SourceAssetRecord, ManifestError> {
        let id = self.id.parse().map_err(|source| ManifestError::AtPath {
            path: context.clone(),
            source: Box::new(ManifestError::InvalidAssetId {
                value: self.id,
                source,
            }),
        })?;
        let asset_type =
            TypeKey::new(self.asset_type.clone()).map_err(|source| ManifestError::AtPath {
                path: context.clone(),
                source: Box::new(ManifestError::InvalidAssetType {
                    value: self.asset_type,
                    source,
                }),
            })?;
        let source_path =
            ProjectPath::new(&self.source).map_err(|source| ManifestError::AtPath {
                path: context.clone(),
                source: Box::new(ManifestError::InvalidSourcePath {
                    value: self.source,
                    source,
                }),
            })?;
        SourceAssetRecord::new(id, asset_type, source_path, self.importer.into_domain()).map_err(
            |source| ManifestError::AtPath {
                path: context.clone(),
                source: Box::new(source),
            },
        )
    }
}

impl SourceImporterWire {
    fn from_domain(importer: &SourceImporter) -> Self {
        match importer {
            SourceImporter::Obj(settings) => Self::Obj {
                settings: ObjImportSettingsWire {
                    flip_texcoord_v: settings.flip_texcoord_v(),
                },
            },
        }
    }

    const fn into_domain(self) -> SourceImporter {
        match self {
            Self::Obj { settings } => {
                SourceImporter::Obj(ObjImportSettings::new(settings.flip_texcoord_v))
            }
        }
    }
}

impl AssetLookup for AuthoringAssetManifest {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey> {
        self.records
            .iter()
            .find(|record| record.id == *id)
            .map(|record| &record.asset_type)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid built-in manifest format path: {0}")]
    FormatPath(#[from] ProjectPathError),
    #[error("cannot parse authoring asset manifest {path}: {source}")]
    Parse {
        path: ProjectPath,
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize authoring asset manifest {path}: {source}")]
    Serialize {
        path: ProjectPath,
        #[source]
        source: ron::Error,
    },
    #[error(transparent)]
    Io(#[from] ProjectIoError),
    #[error(
        "unsupported authoring asset manifest version at {path}: expected {expected}, found {found}"
    )]
    VersionMismatch {
        path: ProjectPath,
        expected: u32,
        found: u32,
    },
    #[error("invalid authoring asset manifest {path}: {source}")]
    AtPath {
        path: ProjectPath,
        #[source]
        source: Box<ManifestError>,
    },
    #[error("invalid asset ID {value:?}: {source}")]
    InvalidAssetId {
        value: String,
        #[source]
        source: AssetIdError,
    },
    #[error("invalid asset type {value:?}: {source}")]
    InvalidAssetType {
        value: String,
        #[source]
        source: KeyError,
    },
    #[error("invalid source project path {value:?}: {source}")]
    InvalidSourcePath {
        value: String,
        #[source]
        source: ProjectPathError,
    },
    #[error("source importer requires asset type {expected}, got {actual}")]
    ImporterAssetTypeMismatch { expected: TypeKey, actual: TypeKey },
    #[error("OBJ source must use a lowercase .obj suffix: {path}")]
    InvalidObjSource { path: ProjectPath },
    #[error("duplicate asset ID in authoring manifest: {id}")]
    DuplicateAssetId { id: AssetId },
}

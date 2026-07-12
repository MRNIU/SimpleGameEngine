// Copyright The SimpleGameEngine Contributors

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sge_reflect::{KeyError, TypeKey};
use sha2::{Digest, Sha256};

use crate::{
    AssetId, AssetIdError, MESH_ASSET_TYPE_KEY, RuntimeGenerationId, RuntimeGenerationIdError,
    RuntimeProductPath, RuntimeProductPathError,
};

pub const RUNTIME_ASSET_CATALOG_FORMAT_VERSION: u32 = 1;
const ENTRY_SCENE_PATH: &str = "Scenes/entry.runtime-scene.ron";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAssetRecord {
    id: AssetId,
    asset_type: TypeKey,
    product: RuntimeProductPath,
    dependencies: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAssetCatalog {
    game_id: TypeKey,
    generation: RuntimeGenerationId,
    entry_scene: RuntimeProductPath,
    assets: Vec<RuntimeAssetRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAssetCatalogWire {
    format_version: u32,
    game_id: String,
    generation: String,
    entry_scene: String,
    assets: Vec<RuntimeAssetRecordWire>,
}

#[derive(Deserialize)]
struct RuntimeAssetCatalogVersionWire {
    format_version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAssetRecordWire {
    id: String,
    asset_type: String,
    product: String,
    dependencies: Vec<String>,
}

impl RuntimeAssetRecord {
    pub fn new(
        id: AssetId,
        asset_type: TypeKey,
        product: RuntimeProductPath,
        mut dependencies: Vec<AssetId>,
    ) -> Result<Self, RuntimeCatalogError> {
        ensure_assigned_asset_id(id)?;
        for dependency in &dependencies {
            ensure_assigned_asset_id(*dependency)?;
        }
        if !product.as_str().starts_with("Content/") {
            return Err(RuntimeCatalogError::InvalidProductRole { id, path: product });
        }
        if asset_type.as_str() == MESH_ASSET_TYPE_KEY {
            let expected = format!("Content/{id}.mesh.ron");
            if product.as_str() != expected {
                return Err(RuntimeCatalogError::InvalidMeshProductPath { id, path: product });
            }
            if !dependencies.is_empty() {
                return Err(RuntimeCatalogError::MeshHasDependencies { id });
            }
        }
        dependencies.sort_unstable();
        if let Some(dependency) = dependencies
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(RuntimeCatalogError::DuplicateDependency {
                asset: id,
                dependency,
            });
        }
        Ok(Self {
            id,
            asset_type,
            product,
            dependencies,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &AssetId {
        &self.id
    }

    #[must_use]
    pub const fn asset_type(&self) -> &TypeKey {
        &self.asset_type
    }

    #[must_use]
    pub const fn product(&self) -> &RuntimeProductPath {
        &self.product
    }

    #[must_use]
    pub fn dependencies(&self) -> &[AssetId] {
        &self.dependencies
    }
}

fn ensure_assigned_asset_id(id: AssetId) -> Result<(), RuntimeCatalogError> {
    if id.is_nil() {
        Err(RuntimeCatalogError::InvalidAssetId {
            value: id.to_string(),
            source: AssetIdError::NilReserved,
        })
    } else {
        Ok(())
    }
}

impl RuntimeAssetCatalog {
    fn from_parts(
        game_id: TypeKey,
        generation: RuntimeGenerationId,
        entry_scene: RuntimeProductPath,
        assets: Vec<RuntimeAssetRecord>,
    ) -> Result<Self, RuntimeCatalogError> {
        let assets = validated_assets(&entry_scene, assets)?;
        Ok(Self {
            game_id,
            generation,
            entry_scene,
            assets,
        })
    }

    pub fn build(
        game_id: TypeKey,
        entry_scene: RuntimeProductPath,
        assets: Vec<RuntimeAssetRecord>,
        entry_scene_bytes: &[u8],
        product_bytes: &BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<Self, RuntimeCatalogError> {
        let assets = validated_assets(&entry_scene, assets)?;
        let generation = catalog_content_digest(
            &game_id,
            &entry_scene,
            &assets,
            entry_scene_bytes,
            product_bytes,
        )?;
        Ok(Self {
            game_id,
            generation,
            entry_scene,
            assets,
        })
    }

    pub fn verify_generation(
        &self,
        entry_scene_bytes: &[u8],
        product_bytes: &BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<(), RuntimeCatalogError> {
        let actual = catalog_content_digest(
            &self.game_id,
            &self.entry_scene,
            &self.assets,
            entry_scene_bytes,
            product_bytes,
        )?;
        if actual != self.generation {
            return Err(RuntimeCatalogError::GenerationMismatch {
                expected: self.generation.clone(),
                actual,
            });
        }
        Ok(())
    }

    pub fn from_ron(input: &str) -> Result<Self, RuntimeCatalogError> {
        let version: RuntimeAssetCatalogVersionWire =
            ron::from_str(input).map_err(|source| RuntimeCatalogError::Parse {
                source: Box::new(source),
            })?;
        if version.format_version != RUNTIME_ASSET_CATALOG_FORMAT_VERSION {
            return Err(RuntimeCatalogError::VersionMismatch {
                expected: RUNTIME_ASSET_CATALOG_FORMAT_VERSION,
                found: version.format_version,
            });
        }
        let wire: RuntimeAssetCatalogWire =
            ron::from_str(input).map_err(|source| RuntimeCatalogError::Parse {
                source: Box::new(source),
            })?;
        let game_id = TypeKey::new(wire.game_id.clone()).map_err(|source| {
            RuntimeCatalogError::InvalidGameId {
                value: wire.game_id,
                source,
            }
        })?;
        let generation = wire
            .generation
            .parse()
            .map_err(|source| RuntimeCatalogError::InvalidGeneration { source })?;
        let entry_scene = RuntimeProductPath::new(wire.entry_scene.clone()).map_err(|source| {
            RuntimeCatalogError::InvalidProductPath {
                value: wire.entry_scene,
                source,
            }
        })?;
        let assets = wire
            .assets
            .into_iter()
            .map(RuntimeAssetRecord::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_parts(game_id, generation, entry_scene, assets)
    }

    pub fn to_ron(&self) -> Result<String, RuntimeCatalogError> {
        let wire = RuntimeAssetCatalogWire {
            format_version: RUNTIME_ASSET_CATALOG_FORMAT_VERSION,
            game_id: self.game_id.to_string(),
            generation: self.generation.to_string(),
            entry_scene: self.entry_scene.to_string(),
            assets: self
                .assets
                .iter()
                .map(|record| RuntimeAssetRecordWire {
                    id: record.id.to_string(),
                    asset_type: record.asset_type.to_string(),
                    product: record.product.to_string(),
                    dependencies: record
                        .dependencies
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                })
                .collect(),
        };
        ron::ser::to_string_pretty(&wire, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|source| RuntimeCatalogError::Serialize { source })
    }

    #[must_use]
    pub const fn game_id(&self) -> &TypeKey {
        &self.game_id
    }

    #[must_use]
    pub const fn generation(&self) -> &RuntimeGenerationId {
        &self.generation
    }

    #[must_use]
    pub const fn entry_scene(&self) -> &RuntimeProductPath {
        &self.entry_scene
    }

    #[must_use]
    pub fn assets(&self) -> &[RuntimeAssetRecord] {
        &self.assets
    }

    #[must_use]
    pub fn asset(&self, id: &AssetId) -> Option<&RuntimeAssetRecord> {
        self.assets
            .binary_search_by_key(id, |record| record.id)
            .ok()
            .map(|index| &self.assets[index])
    }
}

fn validated_assets(
    entry_scene: &RuntimeProductPath,
    mut assets: Vec<RuntimeAssetRecord>,
) -> Result<Vec<RuntimeAssetRecord>, RuntimeCatalogError> {
    if entry_scene.as_str() != ENTRY_SCENE_PATH {
        return Err(RuntimeCatalogError::InvalidEntryScene {
            path: entry_scene.clone(),
        });
    }
    assets.sort_unstable_by_key(|record| record.id);
    if let Some(id) = assets
        .windows(2)
        .find(|pair| pair[0].id == pair[1].id)
        .map(|pair| pair[0].id)
    {
        return Err(RuntimeCatalogError::DuplicateAssetId { id });
    }
    let mut products = BTreeSet::new();
    for record in &assets {
        if !products.insert(record.product.clone()) {
            return Err(RuntimeCatalogError::DuplicateProductPath {
                path: record.product.clone(),
            });
        }
    }
    let ids = assets
        .iter()
        .map(|record| record.id)
        .collect::<BTreeSet<_>>();
    for record in &assets {
        if let Some(dependency) = record
            .dependencies
            .iter()
            .find(|dependency| !ids.contains(dependency))
        {
            return Err(RuntimeCatalogError::MissingDependency {
                asset: record.id,
                dependency: *dependency,
            });
        }
    }
    Ok(assets)
}

fn catalog_content_digest(
    game_id: &TypeKey,
    entry_scene: &RuntimeProductPath,
    assets: &[RuntimeAssetRecord],
    entry_scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> Result<RuntimeGenerationId, RuntimeCatalogError> {
    for record in assets {
        if !product_bytes.contains_key(&record.id) {
            return Err(RuntimeCatalogError::MissingProductBytes { id: record.id });
        }
    }
    if let Some(id) = product_bytes
        .keys()
        .find(|id| assets.binary_search_by(|record| record.id.cmp(id)).is_err())
    {
        return Err(RuntimeCatalogError::UnexpectedProductBytes { id: *id });
    }

    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, b"sge-runtime-generation-v1");
    hasher.update(RUNTIME_ASSET_CATALOG_FORMAT_VERSION.to_be_bytes());
    hash_frame(&mut hasher, game_id.as_str().as_bytes());
    hash_frame(&mut hasher, entry_scene.as_str().as_bytes());
    hash_frame(&mut hasher, entry_scene_bytes);
    hasher.update((assets.len() as u64).to_be_bytes());
    for record in assets {
        hash_frame(&mut hasher, record.id.to_string().as_bytes());
        hash_frame(&mut hasher, record.asset_type.as_str().as_bytes());
        hash_frame(&mut hasher, record.product.as_str().as_bytes());
        hasher.update((record.dependencies.len() as u64).to_be_bytes());
        for dependency in &record.dependencies {
            hash_frame(&mut hasher, dependency.to_string().as_bytes());
        }
        let bytes = product_bytes
            .get(&record.id)
            .ok_or(RuntimeCatalogError::MissingProductBytes { id: record.id })?;
        hash_frame(&mut hasher, bytes);
    }
    format!("{:x}", hasher.finalize())
        .parse()
        .map_err(|source| RuntimeCatalogError::InvalidGeneration { source })
}

fn hash_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

impl TryFrom<RuntimeAssetRecordWire> for RuntimeAssetRecord {
    type Error = RuntimeCatalogError;

    fn try_from(wire: RuntimeAssetRecordWire) -> Result<Self, Self::Error> {
        let id = wire
            .id
            .parse()
            .map_err(|source| RuntimeCatalogError::InvalidAssetId {
                value: wire.id,
                source,
            })?;
        let asset_type = TypeKey::new(wire.asset_type.clone()).map_err(|source| {
            RuntimeCatalogError::InvalidAssetType {
                value: wire.asset_type,
                source,
            }
        })?;
        let product = RuntimeProductPath::new(wire.product.clone()).map_err(|source| {
            RuntimeCatalogError::InvalidProductPath {
                value: wire.product,
                source,
            }
        })?;
        let dependencies = wire
            .dependencies
            .into_iter()
            .map(|value| {
                value
                    .parse()
                    .map_err(|source| RuntimeCatalogError::InvalidAssetId { value, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(id, asset_type, product, dependencies)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeCatalogError {
    #[error("cannot parse runtime asset catalog: {source}")]
    Parse {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize runtime asset catalog: {source}")]
    Serialize {
        #[source]
        source: ron::Error,
    },
    #[error("unsupported runtime asset catalog version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("invalid runtime game ID {value:?}: {source}")]
    InvalidGameId {
        value: String,
        #[source]
        source: KeyError,
    },
    #[error("invalid runtime generation: {source}")]
    InvalidGeneration {
        #[source]
        source: RuntimeGenerationIdError,
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
    #[error("invalid runtime product path {value:?}: {source}")]
    InvalidProductPath {
        value: String,
        #[source]
        source: RuntimeProductPathError,
    },
    #[error("runtime entry scene must be {ENTRY_SCENE_PATH}, found {path}")]
    InvalidEntryScene { path: RuntimeProductPath },
    #[error("runtime asset {id} product must be below Content, found {path}")]
    InvalidProductRole {
        id: AssetId,
        path: RuntimeProductPath,
    },
    #[error("mesh asset {id} has non-canonical product path {path}")]
    InvalidMeshProductPath {
        id: AssetId,
        path: RuntimeProductPath,
    },
    #[error("mesh asset {id} cannot declare dependencies")]
    MeshHasDependencies { id: AssetId },
    #[error("asset {asset} repeats dependency {dependency}")]
    DuplicateDependency { asset: AssetId, dependency: AssetId },
    #[error("runtime catalog repeats asset ID {id}")]
    DuplicateAssetId { id: AssetId },
    #[error("runtime catalog repeats product path {path}")]
    DuplicateProductPath { path: RuntimeProductPath },
    #[error("runtime asset {asset} depends on missing asset {dependency}")]
    MissingDependency { asset: AssetId, dependency: AssetId },
    #[error("runtime catalog is missing product bytes for asset {id}")]
    MissingProductBytes { id: AssetId },
    #[error("runtime catalog received unexpected product bytes for asset {id}")]
    UnexpectedProductBytes { id: AssetId },
    #[error("runtime generation mismatch: expected {expected}, computed {actual}")]
    GenerationMismatch {
        expected: RuntimeGenerationId,
        actual: RuntimeGenerationId,
    },
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::hash_frame;

    #[test]
    fn adjacent_frames_prevent_split_collisions() {
        let digest = |frames: &[&[u8]]| {
            let mut hasher = Sha256::new();
            for frame in frames {
                hash_frame(&mut hasher, frame);
            }
            hasher.finalize()
        };

        assert_ne!(digest(&[b"ab", b"c"]), digest(&[b"a", b"bc"]));
    }
}

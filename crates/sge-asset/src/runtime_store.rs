// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_reflect::TypeKey;

use crate::{
    AssetId, AssetLookup, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset, MeshAssetFormatError,
    RuntimeGeneration,
};

pub struct RuntimeAssetStore {
    asset_types: BTreeMap<AssetId, TypeKey>,
    meshes: BTreeMap<AssetId, MeshAsset>,
}

impl RuntimeAssetStore {
    pub fn from_meshes(
        meshes: impl IntoIterator<Item = (AssetId, MeshAsset)>,
    ) -> Result<Self, RuntimeAssetStoreError> {
        let mesh_type =
            TypeKey::new(MESH_ASSET_TYPE_KEY).expect("built-in MeshAsset type key must be valid");
        let mut asset_types = BTreeMap::new();
        let mut stored_meshes = BTreeMap::new();
        for (id, mesh) in meshes {
            if stored_meshes.insert(id, mesh).is_some() {
                return Err(RuntimeAssetStoreError::DuplicateAssetId { id });
            }
            asset_types.insert(id, mesh_type.clone());
        }
        Ok(Self {
            asset_types,
            meshes: stored_meshes,
        })
    }

    pub fn load(generation: &RuntimeGeneration) -> Result<Self, RuntimeAssetStoreError> {
        let mut asset_types = BTreeMap::new();
        let mut meshes = BTreeMap::new();
        for record in generation.catalog().assets() {
            if record.asset_type().as_str() != MESH_ASSET_TYPE_KEY {
                return Err(RuntimeAssetStoreError::UnsupportedProductType {
                    id: *record.id(),
                    asset_type: record.asset_type().clone(),
                });
            }
            let bytes = generation
                .product_bytes(record.id())
                .ok_or(RuntimeAssetStoreError::MissingProductBytes { id: *record.id() })?;
            let text = std::str::from_utf8(bytes).map_err(|source| {
                RuntimeAssetStoreError::ProductText {
                    id: *record.id(),
                    source,
                }
            })?;
            let mesh =
                MeshAsset::from_ron(text).map_err(|source| RuntimeAssetStoreError::MeshDecode {
                    id: *record.id(),
                    source,
                })?;
            asset_types.insert(*record.id(), record.asset_type().clone());
            meshes.insert(*record.id(), mesh);
        }
        Ok(Self {
            asset_types,
            meshes,
        })
    }

    pub fn mesh(
        &self,
        reference: AssetRef<MeshAsset>,
    ) -> Result<&MeshAsset, RuntimeAssetStoreError> {
        self.meshes
            .get(reference.id())
            .ok_or(RuntimeAssetStoreError::MissingMesh {
                id: *reference.id(),
            })
    }
}

impl AssetLookup for RuntimeAssetStore {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey> {
        self.asset_types.get(id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeAssetStoreError {
    #[error("duplicate runtime asset ID: {id}")]
    DuplicateAssetId { id: AssetId },
    #[error("runtime asset {id} has unsupported product type {asset_type}")]
    UnsupportedProductType { id: AssetId, asset_type: TypeKey },
    #[error("runtime generation is missing product bytes for asset {id}")]
    MissingProductBytes { id: AssetId },
    #[error("runtime product for asset {id} is not UTF-8: {source}")]
    ProductText {
        id: AssetId,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("cannot decode mesh product for asset {id}: {source}")]
    MeshDecode {
        id: AssetId,
        #[source]
        source: MeshAssetFormatError,
    },
    #[error("runtime mesh asset is missing: {id}")]
    MissingMesh { id: AssetId },
}

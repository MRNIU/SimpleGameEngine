// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_reflect::TypeKey;

use crate::{
    AssetId, AssetLookup, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset, MeshAssetFormatError,
    RuntimeGeneration, TEXTURE_ASSET_TYPE_KEY, TextureAsset, TextureAssetFormatError,
};

pub struct RuntimeAssetStore {
    asset_types: BTreeMap<AssetId, TypeKey>,
    meshes: BTreeMap<AssetId, MeshAsset>,
    textures: BTreeMap<AssetId, TextureAsset>,
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
            if id.is_nil() {
                return Err(RuntimeAssetStoreError::UnassignedAssetId);
            }
            if stored_meshes.insert(id, mesh).is_some() {
                return Err(RuntimeAssetStoreError::DuplicateAssetId { id });
            }
            asset_types.insert(id, mesh_type.clone());
        }
        Ok(Self {
            asset_types,
            meshes: stored_meshes,
            textures: BTreeMap::new(),
        })
    }

    pub fn from_assets(
        meshes: impl IntoIterator<Item = (AssetId, MeshAsset)>,
        textures: impl IntoIterator<Item = (AssetId, TextureAsset)>,
    ) -> Result<Self, RuntimeAssetStoreError> {
        let mut store = Self::from_meshes(meshes)?;
        let texture_type = TypeKey::new(TEXTURE_ASSET_TYPE_KEY)
            .expect("built-in TextureAsset type key must be valid");
        for (id, texture) in textures {
            if id.is_nil() {
                return Err(RuntimeAssetStoreError::UnassignedAssetId);
            }
            if store.asset_types.insert(id, texture_type.clone()).is_some() {
                return Err(RuntimeAssetStoreError::DuplicateAssetId { id });
            }
            store.textures.insert(id, texture);
        }
        Ok(store)
    }

    pub fn load(generation: &RuntimeGeneration) -> Result<Self, RuntimeAssetStoreError> {
        let mut asset_types = BTreeMap::new();
        let mut meshes = BTreeMap::new();
        let mut textures = BTreeMap::new();
        for record in generation.catalog().assets() {
            let bytes = generation
                .product_bytes(record.id())
                .ok_or(RuntimeAssetStoreError::MissingProductBytes { id: *record.id() })?;
            match record.asset_type().as_str() {
                MESH_ASSET_TYPE_KEY => {
                    let text = std::str::from_utf8(bytes).map_err(|source| {
                        RuntimeAssetStoreError::ProductText {
                            id: *record.id(),
                            source,
                        }
                    })?;
                    let mesh = MeshAsset::from_ron(text).map_err(|source| {
                        RuntimeAssetStoreError::MeshDecode {
                            id: *record.id(),
                            source,
                        }
                    })?;
                    meshes.insert(*record.id(), mesh);
                }
                TEXTURE_ASSET_TYPE_KEY => {
                    let texture = TextureAsset::from_bytes(bytes).map_err(|source| {
                        RuntimeAssetStoreError::TextureDecode {
                            id: *record.id(),
                            source,
                        }
                    })?;
                    textures.insert(*record.id(), texture);
                }
                _ => {
                    return Err(RuntimeAssetStoreError::UnsupportedProductType {
                        id: *record.id(),
                        asset_type: record.asset_type().clone(),
                    });
                }
            }
            asset_types.insert(*record.id(), record.asset_type().clone());
        }
        Ok(Self {
            asset_types,
            meshes,
            textures,
        })
    }

    pub fn texture(
        &self,
        reference: AssetRef<TextureAsset>,
    ) -> Result<&TextureAsset, RuntimeAssetStoreError> {
        self.textures
            .get(reference.id())
            .ok_or(RuntimeAssetStoreError::MissingTexture {
                id: *reference.id(),
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
    #[error("nil asset ID cannot be inserted into a runtime asset store")]
    UnassignedAssetId,
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
    #[error("cannot decode texture product for asset {id}: {source}")]
    TextureDecode {
        id: AssetId,
        #[source]
        source: TextureAssetFormatError,
    },
    #[error("runtime mesh asset is missing: {id}")]
    MissingMesh { id: AssetId },
    #[error("runtime texture asset is missing: {id}")]
    MissingTexture { id: AssetId },
}

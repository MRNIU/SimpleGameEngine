// Copyright The SimpleGameEngine Contributors
//
//! 正式 Asset identity、typed reference、canonical runtime products 与只读 lookup 合同。
//!
//! Source import 与 Cook 属于 `sge-asset-pipeline`，不进入本 crate。

mod mesh;
mod runtime_catalog;
mod runtime_content;
mod runtime_path;
mod runtime_store;
mod texture;

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sge_reflect::{KeyError, ReferenceSemantic, ReferenceValue, TypeKey};

pub use mesh::{
    MESH_ASSET_FORMAT_VERSION, MeshAsset, MeshAssetError, MeshAssetFormatError, MeshVertex,
};
pub use runtime_catalog::{
    RUNTIME_ASSET_CATALOG_FORMAT_VERSION, RuntimeAssetCatalog, RuntimeAssetRecord,
    RuntimeCatalogError,
};
pub use runtime_content::{RuntimeContentError, RuntimeContentRoot, RuntimeGeneration};
pub use runtime_path::{
    RuntimeGenerationId, RuntimeGenerationIdError, RuntimeProductPath, RuntimeProductPathError,
};
pub use runtime_store::{RuntimeAssetStore, RuntimeAssetStoreError};
pub use texture::{
    TEXTURE_ASSET_FORMAT_VERSION, TextureAsset, TextureAssetError, TextureAssetFormatError,
};

pub const MESH_ASSET_TYPE_KEY: &str = "sge.mesh";
pub const TEXTURE_ASSET_TYPE_KEY: &str = "sge.texture";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(uuid::Uuid);

pub trait AssetType: 'static {
    const TYPE_KEY: &'static str;
}

pub trait AssetLookup {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey>;
}

pub struct AssetRef<T: AssetType> {
    id: AssetId,
    marker: PhantomData<fn() -> T>,
}

pub struct OptionalAssetRef<T: AssetType>(Option<AssetRef<T>>);

impl<T: AssetType> Clone for OptionalAssetRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: AssetType> Copy for OptionalAssetRef<T> {}

impl<T: AssetType> PartialEq for OptionalAssetRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: AssetType> Eq for OptionalAssetRef<T> {}

impl<T: AssetType> fmt::Debug for OptionalAssetRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("OptionalAssetRef")
            .field(&self.0.map(|value| *value.id()))
            .finish()
    }
}

impl<T: AssetType> OptionalAssetRef<T> {
    #[must_use]
    pub const fn none() -> Self {
        Self(None)
    }

    #[must_use]
    pub const fn some(reference: AssetRef<T>) -> Self {
        Self(Some(reference))
    }

    #[must_use]
    pub const fn get(self) -> Option<AssetRef<T>> {
        self.0
    }
}

impl<T: AssetType> ReferenceValue for OptionalAssetRef<T> {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Ok(ReferenceSemantic::OptionalAsset {
            asset_type: TypeKey::new(T::TYPE_KEY)?,
        })
    }

    fn to_reference(&self) -> String {
        self.0
            .map_or_else(String::new, |reference| reference.to_reference())
    }

    fn from_reference(value: &str) -> Result<Self, String> {
        if value.is_empty() {
            Ok(Self::none())
        } else {
            AssetRef::from_reference(value).map(Self::some)
        }
    }
}

impl<T: AssetType> Clone for AssetRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: AssetType> Copy for AssetRef<T> {}

impl<T: AssetType> PartialEq for AssetRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: AssetType> Eq for AssetRef<T> {}

impl<T: AssetType> PartialOrd for AssetRef<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: AssetType> Ord for AssetRef<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T: AssetType> Hash for AssetRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: AssetType> fmt::Debug for AssetRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AssetRef").field(&self.id).finish()
    }
}

impl<T: AssetType> AssetRef<T> {
    #[must_use]
    pub const fn new(id: AssetId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &AssetId {
        &self.id
    }
}

impl<T: AssetType> ReferenceValue for AssetRef<T> {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Ok(ReferenceSemantic::Asset {
            asset_type: TypeKey::new(T::TYPE_KEY)?,
        })
    }

    fn to_reference(&self) -> String {
        self.id.to_string()
    }

    fn from_reference(value: &str) -> Result<Self, String> {
        value
            .parse()
            .map(Self::new)
            .map_err(|error: AssetIdError| error.to_string())
    }
}

impl AssetId {
    /// Returns the stable nil UUID used only for an unassigned typed reference candidate.
    #[must_use]
    pub const fn nil() -> Self {
        Self(uuid::Uuid::nil())
    }

    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0.is_nil()
    }

    #[must_use]
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl FromStr for AssetId {
    type Err = AssetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = uuid::Uuid::parse_str(value)?;
        if uuid.hyphenated().to_string() != value {
            return Err(AssetIdError::NonCanonical);
        }
        if uuid.is_nil() {
            return Err(AssetIdError::NilReserved);
        }
        Ok(Self(uuid))
    }
}

impl Serialize for AssetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssetIdError {
    #[error("invalid asset UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),
    #[error("asset ID must use canonical lowercase hyphenated UUID form")]
    NonCanonical,
    #[error("nil asset ID is reserved for an unassigned typed reference candidate")]
    NilReserved,
}

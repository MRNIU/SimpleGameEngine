// Copyright The SimpleGameEngine Contributors
//
//! 正式 Asset identity、typed reference 与只读 lookup 合同。
//!
//! Import、Cook 和 runtime product 属于 M3，不在本 crate 中提前建模。

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sge_reflect::{KeyError, ReferenceSemantic, ReferenceValue, TypeKey};

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
}

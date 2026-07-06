// Copyright The SimpleGameEngine Contributors
//
//! 资源标识与项目相对引用。

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(String);

impl AssetId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AssetError::EmptyId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssetError {
    #[error("asset id cannot be empty")]
    EmptyId,
}

#[cfg(test)]
mod tests {
    use super::{AssetError, AssetId};

    #[test]
    fn rejects_empty_asset_ids() {
        assert_eq!(AssetId::new("  "), Err(AssetError::EmptyId));
    }
}

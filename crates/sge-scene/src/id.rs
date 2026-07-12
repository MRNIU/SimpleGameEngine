// Copyright The SimpleGameEngine Contributors

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sge_reflect::{
    DescriptorError, FieldKey, FieldRegistration, KeyError, ReferenceSemantic, ReferenceValue,
    TypeDescriptor, TypeKey,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneEntityId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent(pub SceneEntityId);

impl SceneEntityId {
    #[must_use]
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    const fn placeholder() -> Self {
        Self(uuid::Uuid::nil())
    }
}

pub fn scene_entity_id_descriptor() -> Result<TypeDescriptor, DescriptorError> {
    TypeDescriptor::builder::<SceneEntityId>(
        TypeKey::new("sge.scene_entity_id")?,
        1,
        "Scene Entity ID",
        SceneEntityId::placeholder,
    )
    .field(FieldRegistration::reference(
        FieldKey::new("id")?,
        "ID",
        |value: &SceneEntityId| value,
        |value: &mut SceneEntityId, id| *value = id,
    )?)
    .build()
}

pub fn parent_descriptor() -> Result<TypeDescriptor, DescriptorError> {
    TypeDescriptor::builder::<Parent>(TypeKey::new("sge.parent")?, 1, "Parent", || {
        Parent(SceneEntityId::placeholder())
    })
    .field(FieldRegistration::reference(
        FieldKey::new("parent")?,
        "Parent",
        |value: &Parent| &value.0,
        |value: &mut Parent, parent| value.0 = parent,
    )?)
    .build()
}

impl fmt::Display for SceneEntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl FromStr for SceneEntityId {
    type Err = SceneEntityIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = uuid::Uuid::parse_str(value)?;
        if uuid.hyphenated().to_string() != value {
            return Err(SceneEntityIdError::NonCanonical);
        }
        Ok(Self(uuid))
    }
}

impl Serialize for SceneEntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SceneEntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

impl ReferenceValue for SceneEntityId {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Ok(ReferenceSemantic::Entity)
    }

    fn to_reference(&self) -> String {
        self.to_string()
    }

    fn from_reference(value: &str) -> Result<Self, String> {
        value
            .parse()
            .map_err(|error: SceneEntityIdError| error.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SceneEntityIdError {
    #[error("invalid scene entity UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),
    #[error("scene entity ID must use canonical lowercase hyphenated UUID form")]
    NonCanonical,
}

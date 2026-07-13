// Copyright The SimpleGameEngine Contributors

use std::{borrow::Borrow, fmt};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TypeKey(String);

impl TypeKey {
    pub fn new(value: impl Into<String>) -> Result<Self, KeyError> {
        validate(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TypeKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for TypeKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for TypeKey {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for TypeKey {
    type Error = KeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TypeKey> for String {
    fn from(value: TypeKey) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FieldKey(String);

impl FieldKey {
    pub fn new(value: impl Into<String>) -> Result<Self, KeyError> {
        validate(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FieldKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for FieldKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for FieldKey {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for FieldKey {
    type Error = KeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FieldKey> for String {
    fn from(value: FieldKey) -> Self {
        value.0
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    #[error("reflected key cannot be empty")]
    Empty,
    #[error("invalid reflected key character: {0:?}")]
    InvalidCharacter(char),
}

fn validate(value: String) -> Result<String, KeyError> {
    if value.is_empty() {
        return Err(KeyError::Empty);
    }

    if let Some(character) = value.chars().find(|character| {
        !character.is_ascii_alphanumeric() && !matches!(character, '.' | '_' | '-')
    }) {
        return Err(KeyError::InvalidCharacter(character));
    }

    Ok(value)
}

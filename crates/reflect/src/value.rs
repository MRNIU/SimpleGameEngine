// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{FieldKey, KeyError, TypeKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Bool,
    I64,
    F32,
    String,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Color,
    Enum,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    I64(i64),
    F32(f32),
    String(String),
    Vec2(sge_math::Vec2),
    Vec3(sge_math::Vec3),
    Vec4(sge_math::Vec4),
    Quat(sge_math::Quat),
    Color([f32; 4]),
    Enum(String),
    Reference(String),
}

impl Value {
    #[must_use]
    pub const fn kind(&self) -> ValueKind {
        match self {
            Self::Bool(_) => ValueKind::Bool,
            Self::I64(_) => ValueKind::I64,
            Self::F32(_) => ValueKind::F32,
            Self::String(_) => ValueKind::String,
            Self::Vec2(_) => ValueKind::Vec2,
            Self::Vec3(_) => ValueKind::Vec3,
            Self::Vec4(_) => ValueKind::Vec4,
            Self::Quat(_) => ValueKind::Quat,
            Self::Color(_) => ValueKind::Color,
            Self::Enum(_) => ValueKind::Enum,
            Self::Reference(_) => ValueKind::Reference,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSemantic {
    Entity,
    Asset { asset_type: TypeKey },
}

pub trait ReferenceValue: Sized + 'static {
    fn semantic() -> Result<ReferenceSemantic, KeyError>;
    fn to_reference(&self) -> String;
    fn from_reference(value: &str) -> Result<Self, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Bool,
    I64,
    F32,
    String,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Color,
    Enum { options: Vec<String> },
    Reference(ReferenceSemantic),
}

impl FieldKind {
    #[must_use]
    pub const fn value_kind(&self) -> ValueKind {
        match self {
            Self::Bool => ValueKind::Bool,
            Self::I64 => ValueKind::I64,
            Self::F32 => ValueKind::F32,
            Self::String => ValueKind::String,
            Self::Vec2 => ValueKind::Vec2,
            Self::Vec3 => ValueKind::Vec3,
            Self::Vec4 => ValueKind::Vec4,
            Self::Quat => ValueKind::Quat,
            Self::Color => ValueKind::Color,
            Self::Enum { .. } => ValueKind::Enum,
            Self::Reference(_) => ValueKind::Reference,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMetadata {
    display_name: String,
    kind: FieldKind,
}

impl FieldMetadata {
    pub fn new(display_name: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            display_name: display_name.into(),
            kind,
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn kind(&self) -> &FieldKind {
        &self.kind
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldValues(BTreeMap<FieldKey, Value>);

impl FieldValues {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: FieldKey, value: Value) -> Option<Value> {
        self.0.insert(key, value)
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.0.remove(key)
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&FieldKey, &Value)> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectedValue {
    type_key: TypeKey,
    schema_version: u32,
    fields: FieldValues,
}

impl ReflectedValue {
    pub fn new(type_key: TypeKey, schema_version: u32, fields: FieldValues) -> Self {
        Self {
            type_key,
            schema_version,
            fields,
        }
    }

    #[must_use]
    pub const fn type_key(&self) -> &TypeKey {
        &self.type_key
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn fields(&self) -> &FieldValues {
        &self.fields
    }
}

// Copyright The SimpleGameEngine Contributors

use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap},
};

use crate::{
    FieldKey, FieldKind, FieldMetadata, FieldValues, ReflectedValue, TypeDescriptor, TypeKey,
    ValidationErrors, ValidationIssue, Value, ValueKind,
};

pub struct TypeRegistry {
    by_key: BTreeMap<TypeKey, TypeDescriptor>,
    by_type: HashMap<TypeId, TypeKey>,
    frozen: bool,
}

impl TypeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_key: BTreeMap::new(),
            by_type: HashMap::new(),
            frozen: false,
        }
    }

    pub fn register(&mut self, descriptor: TypeDescriptor) -> Result<(), RegistryError> {
        if self.frozen {
            return Err(RegistryError::Frozen);
        }
        if self.by_key.contains_key(descriptor.type_key()) {
            return Err(RegistryError::DuplicateTypeKey(
                descriptor.type_key().clone(),
            ));
        }
        if self.by_type.contains_key(&descriptor.rust_type_id()) {
            return Err(RegistryError::DuplicateRustType(
                descriptor.rust_type_name(),
            ));
        }

        let type_key = descriptor.type_key().clone();
        let rust_type_id = descriptor.rust_type_id();
        let _previous_type = self.by_type.insert(rust_type_id, type_key.clone());
        let _previous_key = self.by_key.insert(type_key, descriptor);
        Ok(())
    }

    pub fn freeze(&mut self) -> Result<(), RegistryError> {
        if self.frozen {
            return Err(RegistryError::AlreadyFrozen);
        }
        self.frozen = true;
        Ok(())
    }

    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.frozen
    }

    #[must_use]
    pub fn descriptor(&self, key: &str) -> Option<&TypeDescriptor> {
        self.by_key.get(key)
    }

    #[must_use]
    pub fn descriptor_of<T: 'static>(&self) -> Option<&TypeDescriptor> {
        self.by_type
            .get(&TypeId::of::<T>())
            .and_then(|key| self.by_key.get(key))
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &TypeDescriptor> {
        self.by_key.values()
    }

    pub fn encode(&self, value: &dyn Any) -> Result<ReflectedValue, ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_value(value)?;
        let fields = reflected_fields(descriptor, value)?;
        Ok(ReflectedValue::new(
            descriptor.type_key().clone(),
            descriptor.schema_version(),
            fields,
        ))
    }

    pub fn decode(&self, value: &ReflectedValue) -> Result<Box<dyn Any>, ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_key(value.type_key().as_str())?;
        if value.schema_version() != descriptor.schema_version() {
            return Err(ReflectError::SchemaVersionMismatch {
                type_key: descriptor.type_key().clone(),
                expected: descriptor.schema_version(),
                actual: value.schema_version(),
            });
        }
        for field_key in descriptor.fields.keys() {
            if !value.fields().contains_key(field_key.as_str()) {
                return Err(ReflectError::MissingField(field_key.clone()));
            }
        }
        for (field_key, _) in value.fields().iter() {
            if !descriptor.fields.contains_key(field_key) {
                return Err(ReflectError::UnexpectedField(field_key.clone()));
            }
        }
        for (field_key, field) in &descriptor.fields {
            let field_value = value
                .fields()
                .get(field_key.as_str())
                .ok_or_else(|| ReflectError::MissingField(field_key.clone()))?;
            ensure_metadata_value(field_key, &field.metadata, field_value)?;
        }

        let mut decoded = (descriptor.construct)();
        for (field_key, field) in &descriptor.fields {
            let field_value = value
                .fields()
                .get(field_key.as_str())
                .ok_or_else(|| ReflectError::MissingField(field_key.clone()))?;
            (field.set)(decoded.as_mut(), field_value)?;
        }
        reflected_fields(descriptor, decoded.as_ref())?;
        Ok(decoded)
    }

    pub fn clone_value(&self, value: &dyn Any) -> Result<Box<dyn Any>, ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_value(value)?;
        (descriptor.clone_value)(value)
    }

    pub fn validate(&self, value: &dyn Any) -> Result<(), ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_value(value)?;
        reflected_fields(descriptor, value).map(|_| ())
    }

    pub fn field_value(
        &self,
        type_key: &str,
        value: &dyn Any,
        field_key: &FieldKey,
    ) -> Result<Value, ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_key(type_key)?;
        let field = descriptor
            .fields
            .get(field_key)
            .ok_or_else(|| ReflectError::UnknownField(field_key.clone()))?;
        let field_value = (field.get)(value)?;
        ensure_metadata_value(field_key, &field.metadata, &field_value)?;
        Ok(field_value)
    }

    /// Commits a validated field mutation atomically at the root `T` slot.
    ///
    /// The candidate is cloned from `value`, mutated, and fully validated; `value` is replaced
    /// exactly once only after success. Reflectable types must have value-semantic `Clone`
    /// implementations whose getter-visible state does not share mutable storage, and their
    /// setter/validator callbacks must not mutate external or shared state. Without those callback
    /// invariants, root-slot atomicity cannot provide deep transactional isolation.
    pub fn set_field_value(
        &self,
        type_key: &str,
        value: &mut dyn Any,
        field_key: &FieldKey,
        new_value: &Value,
    ) -> Result<(), ReflectError> {
        self.require_frozen()?;
        let descriptor = self.descriptor_for_key(type_key)?;
        let field = descriptor
            .fields
            .get(field_key)
            .ok_or_else(|| ReflectError::UnknownField(field_key.clone()))?;
        ensure_metadata_value(field_key, &field.metadata, new_value)?;

        let mut candidate = (descriptor.clone_value)(value)?;
        (field.set)(candidate.as_mut(), new_value)?;
        reflected_fields(descriptor, candidate.as_ref())?;
        (descriptor.replace)(value, candidate)
    }

    fn require_frozen(&self) -> Result<(), ReflectError> {
        if self.frozen {
            Ok(())
        } else {
            Err(ReflectError::RegistryNotFrozen)
        }
    }

    fn descriptor_for_key(&self, key: &str) -> Result<&TypeDescriptor, ReflectError> {
        self.by_key
            .get(key)
            .ok_or_else(|| ReflectError::UnknownTypeKey(key.to_owned()))
    }

    fn descriptor_for_value(&self, value: &dyn Any) -> Result<&TypeDescriptor, ReflectError> {
        let type_id = value.type_id();
        let type_key = self
            .by_type
            .get(&type_id)
            .ok_or(ReflectError::UnknownRustType(type_id))?;
        self.by_key
            .get(type_key)
            .ok_or_else(|| ReflectError::UnknownTypeKey(type_key.to_string()))
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn reflected_fields(
    descriptor: &TypeDescriptor,
    value: &dyn Any,
) -> Result<FieldValues, ReflectError> {
    let mut values = FieldValues::default();
    let mut issues = Vec::new();
    let mut validation_failed = false;
    for (field_key, field) in &descriptor.fields {
        let field_value = (field.get)(value)?;
        ensure_value_kind(field_key, &field.metadata, &field_value)?;
        if let Some(issue) = enum_membership_issue(field_key, &field.metadata, &field_value) {
            validation_failed = true;
            issues.push(issue);
        }
        if let Some(validate) = field.validate
            && let Err(issue) = validate(&field_value)
        {
            validation_failed = true;
            issues.push(issue);
        }
        let _previous = values.insert(field_key.clone(), field_value);
    }
    if let Err(errors) = (descriptor.validate)(value) {
        validation_failed = true;
        issues.extend_from_slice(errors.issues());
    }
    if validation_failed {
        Err(ReflectError::Validation(ValidationErrors::new(issues)))
    } else {
        Ok(values)
    }
}

fn ensure_metadata_value(
    field_key: &FieldKey,
    metadata: &FieldMetadata,
    value: &Value,
) -> Result<(), ReflectError> {
    ensure_value_kind(field_key, metadata, value)?;
    if let Some(issue) = enum_membership_issue(field_key, metadata, value) {
        return Err(ReflectError::Validation(ValidationErrors::one(issue)));
    }
    Ok(())
}

fn ensure_value_kind(
    field_key: &FieldKey,
    metadata: &FieldMetadata,
    value: &Value,
) -> Result<(), ReflectError> {
    let expected = metadata.kind().value_kind();
    let actual = value.kind();
    if actual == expected {
        Ok(())
    } else {
        Err(ReflectError::value_kind(
            field_key.as_str(),
            format!("{expected:?}"),
            actual,
        ))
    }
}

fn enum_membership_issue(
    field_key: &FieldKey,
    metadata: &FieldMetadata,
    value: &Value,
) -> Option<ValidationIssue> {
    let (FieldKind::Enum { options }, Value::Enum(selected)) = (metadata.kind(), value) else {
        return None;
    };
    if options.iter().any(|option| option == selected) {
        None
    } else {
        Some(ValidationIssue::field(
            field_key.clone(),
            format!("enum value {selected:?} is not a declared option"),
        ))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("type registry is frozen")]
    Frozen,
    #[error("type registry is already frozen")]
    AlreadyFrozen,
    #[error("duplicate reflected type key: {0}")]
    DuplicateTypeKey(TypeKey),
    #[error("duplicate reflected Rust type: {0}")]
    DuplicateRustType(&'static str),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReflectError {
    #[error("type registry must be frozen before use")]
    RegistryNotFrozen,
    #[error("unknown reflected type key: {0}")]
    UnknownTypeKey(String),
    #[error("unknown reflected Rust TypeId: {0:?}")]
    UnknownRustType(TypeId),
    #[error("schema version mismatch for {type_key}: expected {expected}, got {actual}")]
    SchemaVersionMismatch {
        type_key: TypeKey,
        expected: u32,
        actual: u32,
    },
    #[error("reflected Rust type mismatch; expected {expected}")]
    TypeMismatch { expected: &'static str },
    #[error("unknown reflected field: {0}")]
    UnknownField(FieldKey),
    #[error("missing reflected field: {0}")]
    MissingField(FieldKey),
    #[error("unexpected reflected field: {0}")]
    UnexpectedField(FieldKey),
    #[error("value kind mismatch for {field}: expected {expected}, got {actual:?}")]
    ValueKindMismatch {
        field: String,
        expected: String,
        actual: ValueKind,
    },
    #[error("invalid reference payload {value:?}: {reason}")]
    InvalidReferencePayload { value: String, reason: String },
    #[error(transparent)]
    Validation(ValidationErrors),
}

impl ReflectError {
    #[must_use]
    pub fn value_kind(
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: ValueKind,
    ) -> Self {
        Self::ValueKindMismatch {
            field: field.into(),
            expected: expected.into(),
            actual,
        }
    }
}

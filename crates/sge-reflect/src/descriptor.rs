// Copyright The SimpleGameEngine Contributors

use std::{
    any::{Any, TypeId, type_name},
    collections::BTreeMap,
};

use crate::{
    FieldKey, FieldKind, FieldMetadata, ReferenceValue, ReflectError, ValidationErrors,
    ValidationIssue, Value,
};

type FieldValidator = fn(&Value) -> Result<(), ValidationIssue>;
type ComponentValidator<T> = fn(&T) -> Result<(), ValidationErrors>;
type TypedGetter<T> = Box<dyn Fn(&T) -> Value>;
type TypedSetter<T> = Box<dyn Fn(&mut T, &Value) -> Result<(), ReflectError>>;

pub struct FieldRegistration<T> {
    key: FieldKey,
    metadata: FieldMetadata,
    get: TypedGetter<T>,
    set: TypedSetter<T>,
    validate: Option<FieldValidator>,
    reference_bound: bool,
}

impl<T: 'static> FieldRegistration<T> {
    pub fn new(
        key: FieldKey,
        metadata: FieldMetadata,
        get: fn(&T) -> Value,
        set: fn(&mut T, &Value) -> Result<(), ReflectError>,
    ) -> Self {
        Self {
            key,
            metadata,
            get: Box::new(get),
            set: Box::new(set),
            validate: None,
            reference_bound: false,
        }
    }

    pub fn reference<R: ReferenceValue>(
        key: FieldKey,
        display_name: impl Into<String>,
        get: fn(&T) -> &R,
        set: fn(&mut T, R),
    ) -> Result<Self, DescriptorError> {
        let semantic = R::semantic()?;
        let setter_key = key.clone();
        Ok(Self {
            key,
            metadata: FieldMetadata::new(display_name, FieldKind::Reference(semantic)),
            get: Box::new(move |value| Value::Reference(get(value).to_reference())),
            set: Box::new(move |value, field| {
                let Value::Reference(reference) = field else {
                    return Err(ReflectError::value_kind(
                        setter_key.as_str(),
                        "Reference",
                        field.kind(),
                    ));
                };
                let reference = R::from_reference(reference).map_err(|reason| {
                    ReflectError::InvalidReferencePayload {
                        value: reference.clone(),
                        reason,
                    }
                })?;
                set(value, reference);
                Ok(())
            }),
            validate: None,
            reference_bound: true,
        })
    }

    pub fn validator(mut self, validate: fn(&Value) -> Result<(), ValidationIssue>) -> Self {
        self.validate = Some(validate);
        self
    }
}

pub struct TypeDescriptorBuilder<T: Clone + 'static> {
    type_key: crate::TypeKey,
    schema_version: u32,
    display_name: String,
    constructor: fn() -> T,
    fields: Vec<FieldRegistration<T>>,
    validate: Option<ComponentValidator<T>>,
    scene_saveable: bool,
}

impl<T: Clone + 'static> TypeDescriptorBuilder<T> {
    pub fn field(mut self, field: FieldRegistration<T>) -> Self {
        self.fields.push(field);
        self
    }

    pub fn validator(mut self, validate: fn(&T) -> Result<(), ValidationErrors>) -> Self {
        self.validate = Some(validate);
        self
    }

    pub const fn scene_saveable(mut self) -> Self {
        self.scene_saveable = true;
        self
    }

    pub fn build(self) -> Result<TypeDescriptor, DescriptorError> {
        if self.schema_version == 0 {
            return Err(DescriptorError::ZeroSchemaVersion);
        }

        let rust_type_name = type_name::<T>();
        let mut fields = BTreeMap::new();
        for registration in self.fields {
            if fields.contains_key(&registration.key) {
                return Err(DescriptorError::DuplicateFieldKey(registration.key));
            }
            if matches!(registration.metadata.kind(), FieldKind::Reference(_))
                && !registration.reference_bound
            {
                return Err(DescriptorError::UnboundReferenceField(registration.key));
            }

            let get = registration.get;
            let set = registration.set;
            fields.insert(
                registration.key,
                FieldDescriptor {
                    metadata: registration.metadata,
                    get: Box::new(move |value| {
                        let value =
                            value
                                .downcast_ref::<T>()
                                .ok_or(ReflectError::TypeMismatch {
                                    expected: rust_type_name,
                                })?;
                        Ok(get(value))
                    }),
                    set: Box::new(move |value, field| {
                        let value =
                            value
                                .downcast_mut::<T>()
                                .ok_or(ReflectError::TypeMismatch {
                                    expected: rust_type_name,
                                })?;
                        set(value, field)
                    }),
                    validate: registration.validate,
                },
            );
        }

        let constructor = self.constructor;
        let validate = self.validate;
        Ok(TypeDescriptor {
            type_key: self.type_key,
            schema_version: self.schema_version,
            rust_type_id: TypeId::of::<T>(),
            rust_type_name,
            display_name: self.display_name,
            fields,
            scene_saveable: self.scene_saveable,
            construct: Box::new(move || Box::new(constructor())),
            clone_value: Box::new(move |value| {
                let value = value
                    .downcast_ref::<T>()
                    .ok_or(ReflectError::TypeMismatch {
                        expected: rust_type_name,
                    })?;
                Ok(Box::new(value.clone()))
            }),
            validate: Box::new(move |value| {
                let Some(value) = value.downcast_ref::<T>() else {
                    return Err(ValidationErrors::one(ValidationIssue::component(format!(
                        "expected reflected Rust type {rust_type_name}"
                    ))));
                };
                validate.map_or(Ok(()), |validate| validate(value))
            }),
            replace: Box::new(move |value, replacement| {
                let value = value
                    .downcast_mut::<T>()
                    .ok_or(ReflectError::TypeMismatch {
                        expected: rust_type_name,
                    })?;
                let replacement =
                    replacement
                        .downcast::<T>()
                        .map_err(|_| ReflectError::TypeMismatch {
                            expected: rust_type_name,
                        })?;
                *value = *replacement;
                Ok(())
            }),
        })
    }
}

pub(crate) type ErasedGetter = Box<dyn Fn(&dyn Any) -> Result<Value, ReflectError>>;
pub(crate) type ErasedSetter = Box<dyn Fn(&mut dyn Any, &Value) -> Result<(), ReflectError>>;
pub(crate) type ErasedConstructor = Box<dyn Fn() -> Box<dyn Any>>;
pub(crate) type ErasedClone = Box<dyn Fn(&dyn Any) -> Result<Box<dyn Any>, ReflectError>>;
pub(crate) type ErasedValidate = Box<dyn Fn(&dyn Any) -> Result<(), ValidationErrors>>;
pub(crate) type ErasedReplace = Box<dyn Fn(&mut dyn Any, Box<dyn Any>) -> Result<(), ReflectError>>;

pub(crate) struct FieldDescriptor {
    pub(crate) metadata: FieldMetadata,
    pub(crate) get: ErasedGetter,
    pub(crate) set: ErasedSetter,
    pub(crate) validate: Option<FieldValidator>,
}

pub struct TypeDescriptor {
    pub(crate) type_key: crate::TypeKey,
    pub(crate) schema_version: u32,
    pub(crate) rust_type_id: TypeId,
    pub(crate) rust_type_name: &'static str,
    pub(crate) display_name: String,
    pub(crate) fields: BTreeMap<FieldKey, FieldDescriptor>,
    pub(crate) scene_saveable: bool,
    pub(crate) construct: ErasedConstructor,
    pub(crate) clone_value: ErasedClone,
    pub(crate) validate: ErasedValidate,
    pub(crate) replace: ErasedReplace,
}

impl TypeDescriptor {
    /// Starts a descriptor for a reflected value-semantic Rust type.
    ///
    /// `T::clone` must not leave getter-visible mutable state shared with its source. Registered
    /// getters, setters, and validators must not mutate external or shared state. These invariants
    /// let [`crate::TypeRegistry::set_field_value`] provide atomic root-slot commit without claiming
    /// deep transactional isolation for arbitrary interior mutability or callback side effects.
    pub fn builder<T: Clone + 'static>(
        type_key: crate::TypeKey,
        schema_version: u32,
        display_name: impl Into<String>,
        constructor: fn() -> T,
    ) -> TypeDescriptorBuilder<T> {
        TypeDescriptorBuilder {
            type_key,
            schema_version,
            display_name: display_name.into(),
            constructor,
            fields: Vec::new(),
            validate: None,
            scene_saveable: false,
        }
    }

    #[must_use]
    pub const fn type_key(&self) -> &crate::TypeKey {
        &self.type_key
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn rust_type_id(&self) -> TypeId {
        self.rust_type_id
    }

    #[must_use]
    pub const fn rust_type_name(&self) -> &'static str {
        self.rust_type_name
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn fields(&self) -> impl Iterator<Item = (&FieldKey, &FieldMetadata)> {
        self.fields
            .iter()
            .map(|(key, field)| (key, &field.metadata))
    }

    #[must_use]
    pub fn field(&self, key: &str) -> Option<&FieldMetadata> {
        self.fields.get(key).map(|field| &field.metadata)
    }

    #[must_use]
    pub const fn scene_saveable(&self) -> bool {
        self.scene_saveable
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DescriptorError {
    #[error("schema version must be greater than zero")]
    ZeroSchemaVersion,
    #[error("duplicate reflected field: {0}")]
    DuplicateFieldKey(FieldKey),
    #[error("reference field has no typed binding: {0}")]
    UnboundReferenceField(FieldKey),
    #[error("invalid reference semantic: {0}")]
    InvalidReferenceSemantic(#[from] crate::KeyError),
}

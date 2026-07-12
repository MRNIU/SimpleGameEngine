// Copyright The SimpleGameEngine Contributors

use sge_reflect::{FieldKey, FieldKind, ReflectedValue, TypeDescriptor, TypeKey, Value};

use crate::EditError;

#[derive(Debug, Clone, PartialEq)]
pub struct InspectorComponent {
    type_key: TypeKey,
    display_name: String,
    fields: Vec<InspectorField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectorField {
    field_key: FieldKey,
    display_name: String,
    kind: FieldKind,
    value: Value,
}

impl InspectorComponent {
    pub(crate) fn from_reflected(
        descriptor: &TypeDescriptor,
        value: &ReflectedValue,
    ) -> Result<Self, EditError> {
        let fields = descriptor
            .fields()
            .map(|(field_key, metadata)| {
                let value = value
                    .fields()
                    .get(field_key.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        EditError::Reflect(sge_reflect::ReflectError::MissingField(
                            field_key.clone(),
                        ))
                    })?;
                Ok(InspectorField {
                    field_key: field_key.clone(),
                    display_name: metadata.display_name().to_owned(),
                    kind: metadata.kind().clone(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, EditError>>()?;
        Ok(Self {
            type_key: descriptor.type_key().clone(),
            display_name: descriptor.display_name().to_owned(),
            fields,
        })
    }

    #[must_use]
    pub const fn type_key(&self) -> &TypeKey {
        &self.type_key
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn fields(&self) -> &[InspectorField] {
        &self.fields
    }
}

impl InspectorField {
    #[must_use]
    pub const fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn kind(&self) -> &FieldKind {
        &self.kind
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

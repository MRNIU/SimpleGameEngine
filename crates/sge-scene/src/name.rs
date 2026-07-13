// Copyright The SimpleGameEngine Contributors

use sge_reflect::{
    DescriptorError, FieldKey, FieldKind, FieldMetadata, FieldRegistration, ReflectError,
    TypeDescriptor, TypeKey, ValidationErrors, ValidationIssue, Value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneName(String);

impl SceneName {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationErrors> {
        let value = Self(value.into());
        validate_name(&value)?;
        Ok(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SceneName {
    fn default() -> Self {
        Self("Entity".to_owned())
    }
}

pub fn scene_name_descriptor() -> Result<TypeDescriptor, DescriptorError> {
    TypeDescriptor::builder::<SceneName>(TypeKey::new("sge.name")?, 1, "Name", SceneName::default)
        .field(FieldRegistration::new(
            FieldKey::new("value")?,
            FieldMetadata::new("Name", FieldKind::String),
            |name: &SceneName| Value::String(name.0.clone()),
            |name: &mut SceneName, value| {
                let Value::String(value) = value else {
                    return Err(ReflectError::value_kind("value", "String", value.kind()));
                };
                name.0.clone_from(value);
                Ok(())
            },
        ))
        .validator(validate_name)
        .scene_saveable()
        .build()
}

fn validate_name(name: &SceneName) -> Result<(), ValidationErrors> {
    if name.0.trim().is_empty() || name.0.len() > 128 {
        return Err(ValidationErrors::new(vec![ValidationIssue::component(
            "name must contain 1 to 128 UTF-8 bytes",
        )]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_names_reject_empty_and_oversized_values() {
        assert!(SceneName::new(" ").is_err());
        assert!(SceneName::new("x".repeat(129)).is_err());
        assert_eq!(SceneName::new("角色").unwrap().as_str(), "角色");
    }
}

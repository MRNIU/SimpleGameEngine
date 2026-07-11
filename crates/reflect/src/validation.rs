// Copyright The SimpleGameEngine Contributors

use crate::FieldKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    field: Option<FieldKey>,
    message: String,
}

impl ValidationIssue {
    pub fn field(field: FieldKey, message: impl Into<String>) -> Self {
        Self {
            field: Some(field),
            message: message.into(),
        }
    }

    pub fn component(message: impl Into<String>) -> Self {
        Self {
            field: None,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn field_key(&self) -> Option<&FieldKey> {
        self.field.as_ref()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("validation failed: {0:?}")]
pub struct ValidationErrors(Vec<ValidationIssue>);

impl ValidationErrors {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self(issues)
    }

    pub fn one(issue: ValidationIssue) -> Self {
        Self(vec![issue])
    }

    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

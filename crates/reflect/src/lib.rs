// Copyright The SimpleGameEngine Contributors
//
//! SimpleGameEngine 的有限反射值与冻结类型注册表。

mod descriptor;
mod key;
mod registry;
mod validation;
mod value;

pub use descriptor::{DescriptorError, FieldRegistration, TypeDescriptor, TypeDescriptorBuilder};
pub use key::{FieldKey, KeyError, TypeKey};
pub use registry::{ReflectError, RegistryError, TypeRegistry};
pub use validation::{ValidationErrors, ValidationIssue};
pub use value::{
    FieldKind, FieldMetadata, FieldValues, ReferenceSemantic, ReferenceValue, ReflectedValue,
    Value, ValueKind,
};

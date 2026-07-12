// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetId, AssetIdError};
use sge_reflect::{FieldKey, ReflectError, TypeKey, ValueKind};

use crate::{SceneEntityId, SceneEntityIdError};

#[derive(Debug, thiserror::Error)]
pub enum SceneValidationError {
    #[error("type registry must be frozen before preparing a scene")]
    RegistryNotFrozen,
    #[error("duplicate scene entity: {entity}")]
    DuplicateEntity { entity: SceneEntityId },
    #[error("duplicate component {component} on scene entity {entity}")]
    DuplicateComponent {
        entity: SceneEntityId,
        component: TypeKey,
    },
    #[error("scene entity {entity} cannot be its own parent")]
    SelfParent { entity: SceneEntityId },
    #[error("scene entity {entity} references missing parent {parent}")]
    MissingParent {
        entity: SceneEntityId,
        parent: SceneEntityId,
    },
    #[error("parent graph contains a cycle including scene entity {entity}")]
    ParentCycle { entity: SceneEntityId },
    #[error("unknown component {component} on scene entity {entity}")]
    UnknownComponent {
        entity: SceneEntityId,
        component: TypeKey,
    },
    #[error(
        "component {component} on scene entity {entity} collides with a reserved structural component"
    )]
    ReservedStructuralComponent {
        entity: SceneEntityId,
        component: TypeKey,
    },
    #[error("component {component} on scene entity {entity} is not scene-saveable")]
    NonSaveableComponent {
        entity: SceneEntityId,
        component: TypeKey,
    },
    #[error(
        "component {component} on scene entity {entity} has schema {actual}; expected {expected}"
    )]
    ComponentSchemaMismatch {
        entity: SceneEntityId,
        component: TypeKey,
        expected: u32,
        actual: u32,
    },
    #[error("component {component} on scene entity {entity} is missing field {field}")]
    MissingComponentField {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
    },
    #[error("component {component} on scene entity {entity} has unexpected field {field}")]
    UnexpectedComponentField {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} has {actual:?}; expected {expected:?}"
    )]
    ComponentValueKindMismatch {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        expected: ValueKind,
        actual: ValueKind,
    },
    #[error("component {component} on scene entity {entity} failed validation: {message}")]
    ComponentValidation {
        entity: SceneEntityId,
        component: TypeKey,
        field: Option<FieldKey>,
        message: String,
    },
    #[error("component {component} on scene entity {entity} failed to decode: {source}")]
    ComponentDecode {
        entity: SceneEntityId,
        component: TypeKey,
        #[source]
        source: ReflectError,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} has invalid entity reference {value:?}: {source}"
    )]
    InvalidEntityReference {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        value: String,
        #[source]
        source: SceneEntityIdError,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} references missing entity {target}"
    )]
    MissingEntityReference {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        target: SceneEntityId,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} has invalid asset reference {value:?}: {source}"
    )]
    InvalidAssetReference {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        value: String,
        #[source]
        source: AssetIdError,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} references missing asset {asset}"
    )]
    MissingAssetReference {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        asset: AssetId,
    },
    #[error(
        "component {component} field {field} on scene entity {entity} references asset {asset} of type {actual}; expected {expected}"
    )]
    AssetTypeMismatch {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        asset: AssetId,
        expected: TypeKey,
        actual: Box<TypeKey>,
    },
}

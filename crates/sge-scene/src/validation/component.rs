// Copyright The SimpleGameEngine Contributors

use std::{any::TypeId, cmp::Ordering, collections::BTreeSet};

use sge_asset::{AssetId, AssetLookup};
use sge_reflect::{
    FieldKey, FieldKind, ReferenceSemantic, ReflectError, ReflectedValue, TypeDescriptor, TypeKey,
    TypeRegistry, ValidationIssue, Value,
};

use crate::{Parent, SceneEntityId};

use super::{PreparedComponent, SceneValidationError};

pub(super) fn prepare_component(
    entity: SceneEntityId,
    component: &ReflectedValue,
    registry: &TypeRegistry,
    entity_ids: &BTreeSet<SceneEntityId>,
    assets: &impl AssetLookup,
    root_assets: &mut BTreeSet<AssetId>,
) -> Result<PreparedComponent, SceneValidationError> {
    let Some(descriptor) = registry.descriptor(component.type_key().as_str()) else {
        return Err(SceneValidationError::UnknownComponent {
            entity,
            component: component.type_key().clone(),
        });
    };
    if matches!(
        descriptor.rust_type_id(),
        type_id if type_id == TypeId::of::<SceneEntityId>() || type_id == TypeId::of::<Parent>()
    ) {
        return Err(SceneValidationError::ReservedStructuralComponent {
            entity,
            component: component.type_key().clone(),
        });
    }
    if !descriptor.scene_saveable() {
        return Err(SceneValidationError::NonSaveableComponent {
            entity,
            component: component.type_key().clone(),
        });
    }
    if component.schema_version() != descriptor.schema_version() {
        return Err(SceneValidationError::ComponentSchemaMismatch {
            entity,
            component: component.type_key().clone(),
            expected: descriptor.schema_version(),
            actual: component.schema_version(),
        });
    }
    validate_fields(
        entity,
        component,
        descriptor,
        entity_ids,
        assets,
        root_assets,
    )?;
    let value = registry
        .decode(component)
        .map_err(|source| decode_error(entity, component.type_key(), source))?;
    Ok(PreparedComponent::new(
        component.type_key().clone(),
        descriptor.rust_type_id(),
        value,
    ))
}

fn validate_fields(
    entity: SceneEntityId,
    component: &ReflectedValue,
    descriptor: &TypeDescriptor,
    entity_ids: &BTreeSet<SceneEntityId>,
    assets: &impl AssetLookup,
    root_assets: &mut BTreeSet<AssetId>,
) -> Result<(), SceneValidationError> {
    let fields = descriptor
        .fields()
        .map(|(field, _)| field.clone())
        .chain(component.fields().iter().map(|(field, _)| field.clone()))
        .collect::<BTreeSet<_>>();
    for field in fields {
        let Some(metadata) = descriptor.field(field.as_str()) else {
            return Err(SceneValidationError::UnexpectedComponentField {
                entity,
                component: component.type_key().clone(),
                field,
            });
        };
        let Some(value) = component.fields().get(field.as_str()) else {
            return Err(SceneValidationError::MissingComponentField {
                entity,
                component: component.type_key().clone(),
                field,
            });
        };
        let expected = metadata.kind().value_kind();
        let actual = value.kind();
        if expected != actual {
            return Err(SceneValidationError::ComponentValueKindMismatch {
                entity,
                component: component.type_key().clone(),
                field,
                expected,
                actual,
            });
        }
        if let Some(asset) = validate_reference(
            entity,
            component.type_key(),
            &field,
            metadata.kind(),
            value,
            entity_ids,
            assets,
        )? {
            root_assets.insert(asset);
        }
    }
    Ok(())
}

fn validate_reference(
    entity: SceneEntityId,
    component: &TypeKey,
    field: &FieldKey,
    kind: &FieldKind,
    value: &Value,
    entity_ids: &BTreeSet<SceneEntityId>,
    assets: &impl AssetLookup,
) -> Result<Option<AssetId>, SceneValidationError> {
    let (FieldKind::Reference(semantic), Value::Reference(value)) = (kind, value) else {
        return Ok(None);
    };
    match semantic {
        ReferenceSemantic::Entity => {
            let target = value.parse::<SceneEntityId>().map_err(|source| {
                SceneValidationError::InvalidEntityReference {
                    entity,
                    component: component.clone(),
                    field: field.clone(),
                    value: value.clone(),
                    source,
                }
            })?;
            if !entity_ids.contains(&target) {
                return Err(SceneValidationError::MissingEntityReference {
                    entity,
                    component: component.clone(),
                    field: field.clone(),
                    target,
                });
            }
        }
        ReferenceSemantic::Asset {
            asset_type: expected,
        }
        | ReferenceSemantic::OptionalAsset {
            asset_type: expected,
        } => {
            if value.is_empty() && matches!(semantic, ReferenceSemantic::OptionalAsset { .. }) {
                return Ok(None);
            }
            let asset = value.parse::<AssetId>().map_err(|source| {
                SceneValidationError::InvalidAssetReference {
                    entity,
                    component: component.clone(),
                    field: field.clone(),
                    value: value.clone(),
                    source,
                }
            })?;
            let Some(actual) = assets.asset_type(&asset) else {
                return Err(SceneValidationError::MissingAssetReference {
                    entity,
                    component: component.clone(),
                    field: field.clone(),
                    asset,
                });
            };
            if actual != expected {
                return Err(SceneValidationError::AssetTypeMismatch {
                    entity,
                    component: component.clone(),
                    field: field.clone(),
                    asset,
                    expected: expected.clone(),
                    actual: Box::new(actual.clone()),
                });
            }
            return Ok(Some(asset));
        }
    }
    Ok(None)
}

fn decode_error(
    entity: SceneEntityId,
    component: &TypeKey,
    source: ReflectError,
) -> SceneValidationError {
    match source {
        ReflectError::Validation(errors) => {
            if let Some(issue) = errors
                .issues()
                .iter()
                .min_by(|left, right| validation_issue_order(left, right))
            {
                return SceneValidationError::ComponentValidation {
                    entity,
                    component: component.clone(),
                    field: issue.field_key().cloned(),
                    message: issue.message().to_owned(),
                };
            }
            SceneValidationError::ComponentValidation {
                entity,
                component: component.clone(),
                field: None,
                message: "component validation failed without an issue".to_owned(),
            }
        }
        source => SceneValidationError::ComponentDecode {
            entity,
            component: component.clone(),
            source,
        },
    }
}

fn validation_issue_order(left: &ValidationIssue, right: &ValidationIssue) -> Ordering {
    match (left.field_key(), right.field_key()) {
        (Some(left_field), Some(right_field)) => left_field
            .cmp(right_field)
            .then_with(|| left.message().cmp(right.message())),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.message().cmp(right.message()),
    }
}

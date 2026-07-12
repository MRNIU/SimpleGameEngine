// Copyright The SimpleGameEngine Contributors

use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use sge_asset::{AssetId, AssetIdError, AssetLookup};
use sge_reflect::{
    FieldKey, FieldKind, ReferenceSemantic, ReflectError, TypeDescriptor, TypeKey, TypeRegistry,
    ValidationIssue, Value, ValueKind,
};

use crate::{AuthoringScene, SceneEntityId, SceneEntityIdError};

pub struct PreparedScene {
    _entities: Vec<PreparedEntity>,
}

struct PreparedEntity {
    _id: SceneEntityId,
    _parent: Option<SceneEntityId>,
    _components: Vec<PreparedComponent>,
}

struct PreparedComponent {
    _type_key: TypeKey,
    _type_id: TypeId,
    _value: Box<dyn Any>,
}

pub fn prepare(
    scene: &AuthoringScene,
    registry: &sge_reflect::TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<PreparedScene, SceneValidationError> {
    if !registry.is_frozen() {
        return Err(SceneValidationError::RegistryNotFrozen);
    }
    validate_parent_graph(scene)?;
    let entity_ids = scene
        .entities()
        .map(|entity| entity.id())
        .collect::<BTreeSet<_>>();
    let entities = scene
        .entities()
        .map(|entity| {
            let components = entity
                .components()
                .map(|component| {
                    prepare_component(entity.id(), component, registry, &entity_ids, assets)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PreparedEntity {
                _id: entity.id(),
                _parent: entity.parent(),
                _components: components,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedScene {
        _entities: entities,
    })
}

fn prepare_component(
    entity: SceneEntityId,
    component: &sge_reflect::ReflectedValue,
    registry: &TypeRegistry,
    entity_ids: &BTreeSet<SceneEntityId>,
    assets: &impl AssetLookup,
) -> Result<PreparedComponent, SceneValidationError> {
    let Some(descriptor) = registry.descriptor(component.type_key().as_str()) else {
        return Err(SceneValidationError::UnknownComponent {
            entity,
            component: component.type_key().clone(),
        });
    };
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
    validate_fields(entity, component, descriptor, entity_ids, assets)?;
    let value = registry
        .decode(component)
        .map_err(|source| decode_error(entity, component.type_key(), source))?;
    Ok(PreparedComponent {
        _type_key: component.type_key().clone(),
        _type_id: descriptor.rust_type_id(),
        _value: value,
    })
}

fn validate_fields(
    entity: SceneEntityId,
    component: &sge_reflect::ReflectedValue,
    descriptor: &TypeDescriptor,
    entity_ids: &BTreeSet<SceneEntityId>,
    assets: &impl AssetLookup,
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
        validate_reference(
            entity,
            component.type_key(),
            &field,
            metadata.kind(),
            value,
            entity_ids,
            assets,
        )?;
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
) -> Result<(), SceneValidationError> {
    let (FieldKind::Reference(semantic), Value::Reference(value)) = (kind, value) else {
        return Ok(());
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
        } => {
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
        }
    }
    Ok(())
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

fn validate_parent_graph(scene: &AuthoringScene) -> Result<(), SceneValidationError> {
    let parents = scene
        .entities()
        .map(|entity| (entity.id(), entity.parent()))
        .collect::<BTreeMap<_, _>>();

    for (entity, parent) in &parents {
        if parent == &Some(*entity) {
            return Err(SceneValidationError::SelfParent { entity: *entity });
        }
    }
    for (entity, parent) in &parents {
        if let Some(parent) = parent
            && !parents.contains_key(parent)
        {
            return Err(SceneValidationError::MissingParent {
                entity: *entity,
                parent: *parent,
            });
        }
    }

    let mut complete = BTreeSet::new();
    for start in parents.keys().copied() {
        if complete.contains(&start) {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = BTreeMap::new();
        let mut current = start;
        loop {
            if complete.contains(&current) {
                break;
            }
            if let Some(position) = positions.get(&current).copied() {
                let cycle_start = path[position..]
                    .iter()
                    .copied()
                    .fold(current, SceneEntityId::min);
                return Err(SceneValidationError::ParentCycle {
                    entity: cycle_start,
                });
            }
            positions.insert(current, path.len());
            path.push(current);
            let Some(parent) = parents.get(&current).copied().flatten() else {
                break;
            };
            current = parent;
        }
        complete.extend(path);
    }
    Ok(())
}

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

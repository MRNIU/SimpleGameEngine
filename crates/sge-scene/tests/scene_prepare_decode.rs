// Copyright The SimpleGameEngine Contributors

use std::str::FromStr;

use sge_asset::{AssetId, AssetLookup};
use sge_reflect::{
    FieldKey, FieldRegistration, FieldValues, KeyError, ReferenceSemantic, ReferenceValue,
    ReflectError, ReflectedValue, TypeDescriptor, TypeKey, TypeRegistry, Value,
};
use sge_scene::{AuthoringEntity, AuthoringScene, SceneEntityId, SceneValidationError, prepare};

struct EmptyAssets;

impl AssetLookup for EmptyAssets {
    fn asset_type(&self, _id: &AssetId) -> Option<&TypeKey> {
        None
    }
}

fn scene_id() -> Result<SceneEntityId, Box<dyn std::error::Error>> {
    Ok(SceneEntityId::from_str(
        "00000000-0000-0000-0000-000000000001",
    )?)
}

#[derive(Clone)]
struct RejectingEntityReference(SceneEntityId);

impl ReferenceValue for RejectingEntityReference {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Ok(ReferenceSemantic::Entity)
    }

    fn to_reference(&self) -> String {
        self.0.to_string()
    }

    fn from_reference(_value: &str) -> Result<Self, String> {
        Err("rejected by component codec".to_owned())
    }
}

#[derive(Clone)]
struct RejectingComponent {
    target: RejectingEntityReference,
}

fn registry() -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let descriptor = TypeDescriptor::builder::<RejectingComponent>(
        TypeKey::new("demo.rejecting_reference")?,
        1,
        "Rejecting reference",
        || RejectingComponent {
            target: RejectingEntityReference(SceneEntityId::new_v4()),
        },
    )
    .field(FieldRegistration::reference(
        FieldKey::new("target")?,
        "Target",
        |component: &RejectingComponent| &component.target,
        |component: &mut RejectingComponent, target| component.target = target,
    )?)
    .scene_saveable()
    .build()?;
    let mut registry = TypeRegistry::new();
    registry.register(descriptor)?;
    registry.freeze()?;
    Ok(registry)
}

#[test]
fn prepare_wraps_invalid_reflected_payload_with_entity_and_component_context()
-> Result<(), Box<dyn std::error::Error>> {
    let entity = scene_id()?;
    let mut fields = FieldValues::default();
    assert_eq!(
        fields.insert(
            FieldKey::new("target")?,
            Value::Reference(entity.to_string())
        ),
        None
    );
    let component = ReflectedValue::new(TypeKey::new("demo.rejecting_reference")?, 1, fields);
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(entity, None, vec![component])?])?;

    assert!(matches!(
        prepare(&scene, &registry()?, &EmptyAssets),
        Err(SceneValidationError::ComponentDecode {
            entity: found,
            component,
            source: ReflectError::InvalidReferencePayload { reason, .. },
        }) if found == entity
            && component.as_str() == "demo.rejecting_reference"
            && reason == "rejected by component codec"
    ));
    Ok(())
}

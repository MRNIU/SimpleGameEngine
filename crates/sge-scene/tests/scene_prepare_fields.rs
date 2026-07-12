// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetId, AssetRef};
use sge_reflect::{FieldKey, ReflectedValue, Value, ValueKind};
use sge_scene::{AuthoringEntity, AuthoringScene, SceneValidationError, prepare};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

fn encoded_probe(
    count: i64,
) -> Result<(AuthoringScene, sge_reflect::TypeRegistry, Assets, AssetId), Box<dyn std::error::Error>>
{
    let registry = probe_registry()?;
    let entity_id = scene_id(1)?;
    let asset_id = AssetId::new_v4();
    let encoded = registry.encode(&Probe {
        count: 1,
        target: entity_id,
        mesh: AssetRef::<MeshAsset>::new(asset_id),
    })?;
    let mut fields = encoded.fields().clone();
    assert_eq!(
        fields.insert(FieldKey::new("count")?, Value::I64(count)),
        Some(Value::I64(1))
    );
    let component = ReflectedValue::new(encoded.type_key().clone(), 1, fields);
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![component],
    )?])?;
    Ok((
        scene,
        registry,
        Assets::with(asset_id, "asset.mesh")?,
        asset_id,
    ))
}

#[test]
fn prepare_reports_missing_unexpected_and_wrong_kind_fields_with_context()
-> Result<(), Box<dyn std::error::Error>> {
    let (scene, registry, assets, _) = encoded_probe(1)?;
    let original = scene
        .entities()
        .next()
        .and_then(|entity| entity.components().next())
        .ok_or("example scene must contain a component")?;
    let entity_id = scene_id(1)?;

    let mut missing = original.fields().clone();
    assert_eq!(missing.remove("count"), Some(Value::I64(1)));
    let missing_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![sge_reflect::ReflectedValue::new(
            original.type_key().clone(),
            1,
            missing,
        )],
    )?])?;
    assert!(matches!(
        prepare(&missing_scene, &registry, &assets),
        Err(SceneValidationError::MissingComponentField {
            entity,
            component,
            field
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && field.as_str() == "count"
    ));

    let mut unexpected = original.fields().clone();
    assert_eq!(
        unexpected.insert(FieldKey::new("future")?, Value::Bool(true)),
        None
    );
    let unexpected_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![sge_reflect::ReflectedValue::new(
            original.type_key().clone(),
            1,
            unexpected,
        )],
    )?])?;
    assert!(matches!(
        prepare(&unexpected_scene, &registry, &assets),
        Err(SceneValidationError::UnexpectedComponentField {
            entity,
            component,
            field
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && field.as_str() == "future"
    ));

    let mut wrong_kind = original.fields().clone();
    assert_eq!(
        wrong_kind.insert(FieldKey::new("count")?, Value::String("one".to_owned())),
        Some(Value::I64(1))
    );
    let wrong_kind_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![sge_reflect::ReflectedValue::new(
            original.type_key().clone(),
            1,
            wrong_kind,
        )],
    )?])?;
    assert!(matches!(
        prepare(&wrong_kind_scene, &registry, &assets),
        Err(SceneValidationError::ComponentValueKindMismatch {
            entity,
            component,
            field,
            expected: ValueKind::I64,
            actual: ValueKind::String,
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && field.as_str() == "count"
    ));
    Ok(())
}

#[test]
fn prepare_wraps_field_and_component_validation_context() -> Result<(), Box<dyn std::error::Error>>
{
    let entity_id = scene_id(1)?;
    let (invalid_field, registry, assets, _) = encoded_probe(-1)?;
    assert!(matches!(
        prepare(&invalid_field, &registry, &assets),
        Err(SceneValidationError::ComponentValidation {
            entity,
            component,
            field: Some(field),
            message,
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && field.as_str() == "count"
            && message == "count must be positive"
    ));

    let (invalid_component, registry, assets, _) = encoded_probe(13)?;
    assert!(matches!(
        prepare(&invalid_component, &registry, &assets),
        Err(SceneValidationError::ComponentValidation {
            entity,
            component,
            field: None,
            message,
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && message == "count 13 is forbidden"
    ));
    Ok(())
}

#[test]
fn prepared_scene_owns_decoded_values_after_sources_are_dropped()
-> Result<(), Box<dyn std::error::Error>> {
    let (scene, registry, assets, _) = encoded_probe(1)?;
    let prepared = prepare(&scene, &registry, &assets)?;
    drop(scene);
    drop(registry);
    drop(assets);
    drop(prepared);
    Ok(())
}

// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetId, AssetRef};
use sge_reflect::{
    FieldKey, FieldValues, ReflectedValue, TypeDescriptor, TypeKey, TypeRegistry, ValidationErrors,
    ValidationIssue, Value, ValueKind,
};
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

#[test]
fn prepare_selects_the_lowest_field_key_across_shape_and_kind_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let (scene, registry, assets, _) = encoded_probe(1)?;
    let original = scene
        .entities()
        .next()
        .and_then(|entity| entity.components().next())
        .ok_or("example scene must contain a component")?;
    let entity_id = scene_id(1)?;

    let mut unexpected_before_missing = original.fields().clone();
    assert!(matches!(
        unexpected_before_missing.remove("target"),
        Some(Value::Reference(_))
    ));
    assert_eq!(
        unexpected_before_missing.insert(FieldKey::new("alpha")?, Value::Bool(true)),
        None
    );
    let unexpected_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![ReflectedValue::new(
            original.type_key().clone(),
            1,
            unexpected_before_missing,
        )],
    )?])?;
    assert!(matches!(
        prepare(&unexpected_scene, &registry, &assets),
        Err(SceneValidationError::UnexpectedComponentField {
            entity,
            component,
            field,
        }) if entity == entity_id
            && component.as_str() == "demo.probe"
            && field.as_str() == "alpha"
    ));

    let mut wrong_kind_before_missing = original.fields().clone();
    assert!(matches!(
        wrong_kind_before_missing.remove("target"),
        Some(Value::Reference(_))
    ));
    assert_eq!(
        wrong_kind_before_missing.insert(FieldKey::new("count")?, Value::String("one".to_owned()),),
        Some(Value::I64(1))
    );
    let wrong_kind_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        entity_id,
        None,
        vec![ReflectedValue::new(
            original.type_key().clone(),
            1,
            wrong_kind_before_missing,
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

#[derive(Clone)]
struct ForwardIssues;

#[derive(Clone)]
struct ReverseIssues;

fn issue(field: &str, message: &str) -> ValidationIssue {
    match FieldKey::new(field) {
        Ok(field) => ValidationIssue::field(field, message),
        Err(error) => ValidationIssue::component(error.to_string()),
    }
}

fn forward_issues(_value: &ForwardIssues) -> Result<(), ValidationErrors> {
    Err(ValidationErrors::new(vec![
        ValidationIssue::component("component issue"),
        issue("zeta", "zeta issue"),
        issue("alpha", "second alpha issue"),
        issue("alpha", "first alpha issue"),
    ]))
}

fn reverse_issues(_value: &ReverseIssues) -> Result<(), ValidationErrors> {
    Err(ValidationErrors::new(vec![
        issue("alpha", "first alpha issue"),
        issue("alpha", "second alpha issue"),
        issue("zeta", "zeta issue"),
        ValidationIssue::component("component issue"),
    ]))
}

fn issue_registry<T: Clone + 'static>(
    type_key: &str,
    constructor: fn() -> T,
    validator: fn(&T) -> Result<(), ValidationErrors>,
) -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let descriptor =
        TypeDescriptor::builder::<T>(TypeKey::new(type_key)?, 1, "Issue ordering", constructor)
            .validator(validator)
            .scene_saveable()
            .build()?;
    let mut registry = TypeRegistry::new();
    registry.register(descriptor)?;
    registry.freeze()?;
    Ok(registry)
}

fn issue_scene(type_key: &str) -> Result<AuthoringScene, Box<dyn std::error::Error>> {
    let component = ReflectedValue::new(TypeKey::new(type_key)?, 1, FieldValues::default());
    Ok(AuthoringScene::new(vec![AuthoringEntity::new(
        scene_id(1)?,
        None,
        vec![component],
    )?])?)
}

#[test]
fn prepare_canonicalizes_multiple_validation_issues_independent_of_insertion_order()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            issue_scene("demo.forward_issues")?,
            issue_registry("demo.forward_issues", || ForwardIssues, forward_issues)?,
        ),
        (
            issue_scene("demo.reverse_issues")?,
            issue_registry("demo.reverse_issues", || ReverseIssues, reverse_issues)?,
        ),
    ];

    for (scene, registry) in cases {
        assert!(matches!(
            prepare(&scene, &registry, &Assets::default()),
            Err(SceneValidationError::ComponentValidation {
                field: Some(field),
                message,
                ..
            }) if field.as_str() == "alpha" && message == "first alpha issue"
        ));
    }
    Ok(())
}

// Copyright The SimpleGameEngine Contributors

use std::str::FromStr;

use sge_reflect::{FieldKey, FieldValues, ReflectedValue, TypeKey, Value};
use sge_scene::{
    AuthoringEntity, AuthoringScene, SceneEntityId, SceneFormatError, SceneValidationError,
};

const ROOT_ID: &str = "00000000-0000-0000-0000-000000000001";

fn example_scene() -> Result<AuthoringScene, Box<dyn std::error::Error>> {
    let mut fields = FieldValues::default();
    assert_eq!(fields.insert(FieldKey::new("value")?, Value::I64(7)), None);
    let component = ReflectedValue::new(TypeKey::new("demo.probe")?, 1, fields);
    let entity = AuthoringEntity::new(SceneEntityId::from_str(ROOT_ID)?, None, vec![component])?;
    Ok(AuthoringScene::new(vec![entity])?)
}

#[test]
fn authoring_scene_ron_roundtrips_and_has_an_independent_version()
-> Result<(), Box<dyn std::error::Error>> {
    let scene = example_scene()?;
    let encoded = scene.to_ron()?;
    assert_eq!(AuthoringScene::from_ron(&encoded)?, scene);

    let wrong_version = encoded.replacen("format_version: 1", "format_version: 2", 1);
    assert_ne!(wrong_version, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&wrong_version),
        Err(SceneFormatError::VersionMismatch {
            expected: 1,
            found: 2
        })
    ));
    Ok(())
}

#[test]
fn authoring_scene_ron_rejects_unknown_missing_corrupt_and_trailing_data()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = example_scene()?.to_ron()?;

    let mut unknown_top = encoded.clone();
    assert_eq!(unknown_top.pop(), Some(')'));
    unknown_top.push_str(", future: true)");
    assert!(matches!(
        AuthoringScene::from_ron(&unknown_top),
        Err(SceneFormatError::Parse { .. })
    ));

    let unknown_entity = encoded.replacen("parent:", "future: true, parent:", 1);
    assert_ne!(unknown_entity, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&unknown_entity),
        Err(SceneFormatError::Parse { .. })
    ));

    let missing_top = encoded.replacen("entities:", "removed_entities:", 1);
    assert_ne!(missing_top, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&missing_top),
        Err(SceneFormatError::Parse { .. })
    ));
    assert!(matches!(
        AuthoringScene::from_ron(&encoded[..encoded.len() / 2]),
        Err(SceneFormatError::Parse { .. })
    ));
    assert!(matches!(
        AuthoringScene::from_ron(&format!("{encoded}\ntrue")),
        Err(SceneFormatError::Parse { .. })
    ));
    Ok(())
}

#[test]
fn nested_scene_wire_is_strict_and_preserves_structural_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = example_scene()?.to_ron()?;

    let unknown_reflected = encoded.replacen("schema_version:", "future: true, schema_version:", 1);
    assert_ne!(unknown_reflected, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&unknown_reflected),
        Err(SceneFormatError::Parse { .. })
    ));

    let duplicate_field = encoded.replacen('}', ", \"value\": I64(8)}", 1);
    assert_ne!(duplicate_field, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&duplicate_field),
        Err(SceneFormatError::Parse { .. })
    ));

    let duplicate_entity = encoded.replacen(
        "entities: [",
        "entities: [\n        (\n            id: \"00000000-0000-0000-0000-000000000001\",\n            parent: None,\n            components: [],\n        ),",
        1,
    );
    assert_ne!(duplicate_entity, encoded);
    assert!(matches!(
        AuthoringScene::from_ron(&duplicate_entity),
        Err(SceneFormatError::Validation {
            source
        }) if matches!(*source, SceneValidationError::DuplicateEntity { .. })
    ));

    let duplicate_component_source = AuthoringScene::new(vec![AuthoringEntity::new(
        SceneEntityId::from_str(ROOT_ID)?,
        None,
        vec![
            ReflectedValue::new(TypeKey::new("demo.probe")?, 1, FieldValues::default()),
            ReflectedValue::new(TypeKey::new("demo.zeta")?, 1, FieldValues::default()),
        ],
    )?])?
    .to_ron()?;
    let duplicate_component = duplicate_component_source.replacen("demo.zeta", "demo.probe", 1);
    assert_ne!(duplicate_component, duplicate_component_source);
    assert!(matches!(
        AuthoringScene::from_ron(&duplicate_component),
        Err(SceneFormatError::Validation { source })
            if matches!(*source, SceneValidationError::DuplicateComponent { .. })
    ));
    Ok(())
}

#[test]
fn canonical_scene_bytes_ignore_entity_component_and_field_input_order()
-> Result<(), Box<dyn std::error::Error>> {
    let first = SceneEntityId::from_str(ROOT_ID)?;
    let second = SceneEntityId::from_str("00000000-0000-0000-0000-000000000002")?;
    let mut left_fields = FieldValues::default();
    assert_eq!(
        left_fields.insert(FieldKey::new("zeta")?, Value::I64(2)),
        None
    );
    assert_eq!(
        left_fields.insert(FieldKey::new("alpha")?, Value::I64(1)),
        None
    );
    let mut right_fields = FieldValues::default();
    assert_eq!(
        right_fields.insert(FieldKey::new("alpha")?, Value::I64(1)),
        None
    );
    assert_eq!(
        right_fields.insert(FieldKey::new("zeta")?, Value::I64(2)),
        None
    );
    let alpha_left = ReflectedValue::new(TypeKey::new("demo.alpha")?, 1, left_fields);
    let alpha_right = ReflectedValue::new(TypeKey::new("demo.alpha")?, 1, right_fields);
    let zeta = ReflectedValue::new(TypeKey::new("demo.zeta")?, 1, FieldValues::default());

    let left = AuthoringScene::new(vec![
        AuthoringEntity::new(second, Some(first), vec![zeta.clone()])?,
        AuthoringEntity::new(first, None, vec![zeta.clone(), alpha_left])?,
    ])?;
    let right = AuthoringScene::new(vec![
        AuthoringEntity::new(first, None, vec![alpha_right, zeta.clone()])?,
        AuthoringEntity::new(second, Some(first), vec![zeta])?,
    ])?;
    let left_bytes = left.to_ron()?;
    let right_bytes = right.to_ron()?;

    assert_eq!(left_bytes, right_bytes);
    assert!(!left_bytes.contains('\r'));
    assert_eq!(AuthoringScene::from_ron(&left_bytes)?.to_ron()?, left_bytes);
    Ok(())
}

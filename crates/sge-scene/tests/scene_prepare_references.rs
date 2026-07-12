// Copyright The SimpleGameEngine Contributors

mod support;

use std::str::FromStr;

use sge_asset::{AssetId, AssetRef, MESH_ASSET_TYPE_KEY};
use sge_reflect::{FieldKey, ReflectedValue, Value};
use sge_scene::{AuthoringEntity, AuthoringScene, SceneValidationError, prepare};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

const MESH_ID: &str = "10000000-0000-0000-0000-000000000001";

fn probe_component(
    registry: &sge_reflect::TypeRegistry,
    target: &str,
    mesh: &str,
) -> Result<ReflectedValue, Box<dyn std::error::Error>> {
    let source = registry.encode(&Probe {
        count: 1,
        target: scene_id(1)?,
        mesh: AssetRef::<MeshAsset>::new(AssetId::from_str(MESH_ID)?),
    })?;
    let mut fields = source.fields().clone();
    assert!(matches!(
        fields.insert(
            FieldKey::new("target")?,
            Value::Reference(target.to_owned())
        ),
        Some(Value::Reference(_))
    ));
    assert!(matches!(
        fields.insert(FieldKey::new("mesh")?, Value::Reference(mesh.to_owned())),
        Some(Value::Reference(_))
    ));
    Ok(ReflectedValue::new(
        source.type_key().clone(),
        source.schema_version(),
        fields,
    ))
}

fn scene_with_probe(
    target: &str,
    mesh: &str,
    include_second: bool,
) -> Result<AuthoringScene, Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let mut entities = vec![AuthoringEntity::new(
        scene_id(1)?,
        None,
        vec![probe_component(&registry, target, mesh)?],
    )?];
    if include_second {
        entities.push(AuthoringEntity::new(scene_id(2)?, None, Vec::new())?);
    }
    Ok(AuthoringScene::new(entities)?)
}

#[test]
fn prepare_validates_entity_reference_codec_and_target_with_field_context()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let assets = Assets::with(AssetId::from_str(MESH_ID)?, MESH_ASSET_TYPE_KEY)?;
    let entity = scene_id(1)?;

    let invalid = scene_with_probe("not-an-entity-id", MESH_ID, false)?;
    assert!(matches!(
        prepare(&invalid, &registry, &assets),
        Err(SceneValidationError::InvalidEntityReference {
            entity: found,
            component,
            field,
            value,
            ..
        }) if found == entity
            && component.as_str() == "demo.probe"
            && field.as_str() == "target"
            && value == "not-an-entity-id"
    ));

    let missing = scene_with_probe(&scene_id(2)?.to_string(), MESH_ID, false)?;
    assert!(matches!(
        prepare(&missing, &registry, &assets),
        Err(SceneValidationError::MissingEntityReference {
            entity: found,
            component,
            field,
            target,
        }) if found == entity
            && component.as_str() == "demo.probe"
            && field.as_str() == "target"
            && target == scene_id(2)?
    ));

    let self_reference = scene_with_probe(&entity.to_string(), MESH_ID, false)?;
    assert!(prepare(&self_reference, &registry, &assets).is_ok());
    let other_reference = scene_with_probe(&scene_id(2)?.to_string(), MESH_ID, true)?;
    assert!(prepare(&other_reference, &registry, &assets).is_ok());
    Ok(())
}

#[test]
fn prepare_validates_asset_reference_codec_presence_and_type()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let entity = scene_id(1)?;
    let mesh_id = AssetId::from_str(MESH_ID)?;
    let valid_assets = Assets::with(mesh_id, MESH_ASSET_TYPE_KEY)?;

    let invalid = scene_with_probe(&entity.to_string(), "asset:not-an-id", false)?;
    assert!(matches!(
        prepare(&invalid, &registry, &valid_assets),
        Err(SceneValidationError::InvalidAssetReference {
            entity: found,
            component,
            field,
            value,
            ..
        }) if found == entity
            && component.as_str() == "demo.probe"
            && field.as_str() == "mesh"
            && value == "asset:not-an-id"
    ));

    let missing_id = AssetId::from_str("10000000-0000-0000-0000-000000000002")?;
    let missing = scene_with_probe(&entity.to_string(), &missing_id.to_string(), false)?;
    assert!(matches!(
        prepare(&missing, &registry, &valid_assets),
        Err(SceneValidationError::MissingAssetReference {
            entity: found,
            component,
            field,
            asset,
        }) if found == entity
            && component.as_str() == "demo.probe"
            && field.as_str() == "mesh"
            && asset == missing_id
    ));

    let wrong_assets = Assets::with(mesh_id, "asset.texture")?;
    let wrong_type = scene_with_probe(&entity.to_string(), MESH_ID, false)?;
    assert!(matches!(
        prepare(&wrong_type, &registry, &wrong_assets),
        Err(SceneValidationError::AssetTypeMismatch {
            entity: found,
            component,
            field,
            asset,
            expected,
            actual,
        }) if found == entity
            && component.as_str() == "demo.probe"
            && field.as_str() == "mesh"
            && asset == mesh_id
            && expected.as_str() == MESH_ASSET_TYPE_KEY
            && actual.as_str() == "asset.texture"
    ));

    let valid = scene_with_probe(&entity.to_string(), MESH_ID, false)?;
    assert!(prepare(&valid, &registry, &valid_assets).is_ok());
    Ok(())
}

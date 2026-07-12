// Copyright The SimpleGameEngine Contributors

mod support;

use std::{collections::BTreeMap, str::FromStr};

use sge_asset::{AssetId, AssetLookup, AssetRef, MESH_ASSET_TYPE_KEY};
use sge_reflect::{FieldKey, FieldValues, ReflectedValue, TypeKey, Value};
use sge_scene::{
    AuthoringEntity, AuthoringScene, RuntimeScene, RuntimeSceneBuildError, RuntimeSceneFormatError,
    SceneEntityId, SceneValidationError, build_runtime_scene, prepare,
};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

struct RootAssets(BTreeMap<AssetId, TypeKey>);

impl AssetLookup for RootAssets {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey> {
        self.0.get(id)
    }
}

fn empty_runtime_scene() -> Result<RuntimeScene, Box<dyn std::error::Error>> {
    let authoring = AuthoringScene::new(vec![AuthoringEntity::new(
        SceneEntityId::from_str("00000000-0000-0000-0000-000000000001")?,
        None,
        Vec::new(),
    )?])?;
    let assets = Assets::with(AssetId::new_v4(), MESH_ASSET_TYPE_KEY)?;
    Ok(
        build_runtime_scene(&authoring, &probe_registry()?, &assets)?
            .scene()
            .clone(),
    )
}

#[test]
fn runtime_scene_v1_codec_is_distinct_strict_and_canonical()
-> Result<(), Box<dyn std::error::Error>> {
    let scene = empty_runtime_scene()?;
    let encoded = scene.to_ron()?;
    let expected = "(\n    format_version: 1,\n    scene_role: Runtime,\n    entities: [\n        (\n            id: \"00000000-0000-0000-0000-000000000001\",\n            parent: None,\n            components: [],\n        ),\n    ],\n)";
    assert_eq!(encoded, expected);
    assert_eq!(RuntimeScene::from_ron(&encoded)?.to_ron()?, encoded);
    assert!(!encoded.contains('\r'));

    let authoring = encoded.replacen("    scene_role: Runtime,\n", "", 1);
    assert!(matches!(
        RuntimeScene::from_ron(&authoring),
        Err(RuntimeSceneFormatError::Parse { .. })
    ));
    assert!(matches!(
        RuntimeScene::from_ron("(format_version: 2)"),
        Err(RuntimeSceneFormatError::VersionMismatch {
            expected: 1,
            found: 2,
        })
    ));
    assert!(
        RuntimeScene::from_ron(&encoded.replacen("entities:", "future: true, entities:", 1))
            .is_err()
    );
    assert!(
        RuntimeScene::from_ron(&encoded.replacen("entities:", "removed_entities:", 1)).is_err()
    );
    assert!(RuntimeScene::from_ron(&format!("{encoded}\ntrue")).is_err());
    Ok(())
}

#[test]
fn build_runtime_scene_collects_sorted_unique_asset_roots() -> Result<(), Box<dyn std::error::Error>>
{
    let low: AssetId = "10000000-0000-4000-8000-000000000001".parse()?;
    let high: AssetId = "f0000000-0000-4000-8000-000000000002".parse()?;
    let registry = probe_registry()?;
    let root = scene_id(1)?;
    let child = scene_id(2)?;
    let scene = AuthoringScene::new(vec![
        AuthoringEntity::new(
            child,
            Some(root),
            vec![registry.encode(&Probe {
                count: 2,
                target: root,
                mesh: AssetRef::<MeshAsset>::new(low),
            })?],
        )?,
        AuthoringEntity::new(
            root,
            None,
            vec![registry.encode(&Probe {
                count: 1,
                target: child,
                mesh: AssetRef::<MeshAsset>::new(high),
            })?],
        )?,
        AuthoringEntity::new(
            scene_id(3)?,
            None,
            vec![registry.encode(&Probe {
                count: 3,
                target: root,
                mesh: AssetRef::<MeshAsset>::new(low),
            })?],
        )?,
    ])?;
    let asset_type = TypeKey::new(MESH_ASSET_TYPE_KEY)?;
    let assets = RootAssets(BTreeMap::from([
        (low, asset_type.clone()),
        (high, asset_type),
    ]));

    let built = build_runtime_scene(&scene, &registry, &assets)?;

    assert_eq!(built.root_assets(), &[low, high]);
    assert_eq!(
        RuntimeScene::from_ron(&built.scene().to_ron()?)?,
        *built.scene()
    );
    Ok(())
}

#[test]
fn runtime_build_preserves_shared_validation_errors() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let entity = scene_id(1)?;
    let missing_entity = scene_id(9)?;
    let valid_asset = AssetId::new_v4();
    let missing_asset = AssetId::new_v4();
    let assets = Assets::with(valid_asset, MESH_ASSET_TYPE_KEY)?;
    let parent_failure = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        Some(missing_entity),
        Vec::new(),
    )?])?;
    let entity_failure = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        None,
        vec![registry.encode(&Probe {
            count: 1,
            target: missing_entity,
            mesh: AssetRef::<MeshAsset>::new(valid_asset),
        })?],
    )?])?;
    let asset_failure = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        None,
        vec![registry.encode(&Probe {
            count: 1,
            target: entity,
            mesh: AssetRef::<MeshAsset>::new(missing_asset),
        })?],
    )?])?;
    let mut invalid_fields = FieldValues::default();
    assert_eq!(
        invalid_fields.insert(FieldKey::new("count")?, Value::I64(0)),
        None
    );
    assert_eq!(
        invalid_fields.insert(
            FieldKey::new("target")?,
            Value::Reference(entity.to_string())
        ),
        None
    );
    assert_eq!(
        invalid_fields.insert(
            FieldKey::new("mesh")?,
            Value::Reference(valid_asset.to_string())
        ),
        None
    );
    let component_failure = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        None,
        vec![ReflectedValue::new(
            TypeKey::new("demo.probe")?,
            1,
            invalid_fields,
        )],
    )?])?;

    assert!(matches!(
        build_runtime_scene(&asset_failure, &registry, &Assets::default()),
        Err(RuntimeSceneBuildError::Validation(source))
            if matches!(*source, SceneValidationError::MissingAssetReference { asset, .. } if asset == missing_asset)
    ));
    for scene in [
        parent_failure,
        entity_failure,
        asset_failure,
        component_failure,
    ] {
        let authoring_error = prepare(&scene, &registry, &assets)
            .err()
            .ok_or("authoring validation unexpectedly succeeded")?
            .to_string();
        let runtime_error = match build_runtime_scene(&scene, &registry, &assets) {
            Err(RuntimeSceneBuildError::Validation(source)) => source.to_string(),
            Ok(_) => return Err("runtime build validation unexpectedly succeeded".into()),
        };
        assert_eq!(runtime_error, authoring_error);
    }
    Ok(())
}

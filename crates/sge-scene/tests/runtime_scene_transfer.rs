// Copyright The SimpleGameEngine Contributors

mod support;

use std::collections::BTreeMap;

use sge_asset::{
    AssetId, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset as RuntimeMeshAsset, MeshVertex,
    RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeAssetStore, RuntimeGeneration,
    RuntimeProductPath,
};
use sge_ecs::World;
use sge_reflect::{FieldValues, ReflectedValue, TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, RuntimeScene, SceneEntityId, SceneValidationError,
    build_runtime_scene, instantiate, preflight_instantiation, prepare_runtime,
};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

#[test]
fn decoded_runtime_scene_prepares_preflights_and_instantiates_typed_components()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let entity = scene_id(1)?;
    let asset: AssetId = "10000000-0000-4000-8000-000000000001".parse()?;
    let probe = Probe {
        count: 7,
        target: entity,
        mesh: AssetRef::<MeshAsset>::new(asset),
    };
    let authoring = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        None,
        vec![registry.encode(&probe)?],
    )?])?;
    let assets = Assets::with(asset, MESH_ASSET_TYPE_KEY)?;
    let bytes = build_runtime_scene(&authoring, &registry, &assets)?
        .scene()
        .to_ron()?;
    let mesh = RuntimeMeshAsset::new(
        vec![
            MeshVertex::new([0.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([1.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?;
    let product = RuntimeProductPath::new(format!("Content/{asset}.mesh.ron"))?;
    let record =
        RuntimeAssetRecord::new(asset, TypeKey::new(MESH_ASSET_TYPE_KEY)?, product, vec![])?;
    let product_bytes = BTreeMap::from([(asset, mesh.to_ron()?.into_bytes())]);
    let catalog = RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        vec![record],
        bytes.as_bytes(),
        &product_bytes,
    )?;
    let generation =
        RuntimeGeneration::verify_owned(catalog, bytes.clone().into_bytes(), product_bytes)?;
    let store = RuntimeAssetStore::load(&generation)?;
    let runtime = RuntimeScene::from_ron(&bytes)?;
    let prepared = prepare_runtime(&runtime, &registry, &store)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    preflight_instantiation(&prepared, &world)?;
    let instance = instantiate(prepared, world.initializer())?;
    let runtime_entity = instance.entity(&entity).ok_or("runtime entity missing")?;
    let stored = world.get::<Probe>(runtime_entity).ok_or("Probe missing")?;

    assert_eq!(stored.count, probe.count);
    assert_eq!(stored.target, probe.target);
    assert_eq!(stored.mesh.id(), probe.mesh.id());
    Ok(())
}

#[test]
fn runtime_prepare_rejects_reserved_structural_alias_before_decode()
-> Result<(), Box<dyn std::error::Error>> {
    let entity = scene_id(1)?;
    let component_key = TypeKey::new("demo.identity_alias")?;
    let descriptor = TypeDescriptor::builder::<SceneEntityId>(
        component_key.clone(),
        1,
        "Identity alias",
        SceneEntityId::new_v4,
    )
    .scene_saveable()
    .build()?;
    let mut registry = TypeRegistry::new();
    registry.register(descriptor)?;
    registry.freeze()?;
    let authoring = AuthoringScene::new(vec![AuthoringEntity::new(
        entity,
        None,
        vec![ReflectedValue::new(
            component_key.clone(),
            1,
            FieldValues::default(),
        )],
    )?])?;
    let authoring_bytes = authoring.to_ron()?;
    let runtime_bytes = authoring_bytes.replacen(
        "    entities:",
        "    scene_role: Runtime,\n    entities:",
        1,
    );
    let runtime = RuntimeScene::from_ron(&runtime_bytes)?;

    assert!(matches!(
        prepare_runtime(&runtime, &registry, &Assets::default()),
        Err(SceneValidationError::ReservedStructuralComponent { entity: actual, component })
            if actual == entity && component == component_key
    ));
    Ok(())
}

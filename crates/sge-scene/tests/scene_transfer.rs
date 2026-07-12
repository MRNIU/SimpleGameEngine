// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetId, AssetRef, MESH_ASSET_TYPE_KEY};
use sge_ecs::World;
use sge_reflect::{FieldValues, ReflectedValue, TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, SceneEntityId, SceneInstantiationError,
    SceneValidationError, instantiate, preflight_instantiation, prepare,
};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

fn prepared_probe() -> Result<(sge_scene::PreparedScene, Probe), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let entity = scene_id(1)?;
    let asset = AssetId::new_v4();
    let probe = Probe {
        count: 7,
        target: entity,
        mesh: AssetRef::<MeshAsset>::new(asset),
    };
    let reflected = registry.encode(&probe)?;
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(entity, None, vec![reflected])?])?;
    let assets = Assets::with(asset, MESH_ASSET_TYPE_KEY)?;
    Ok((prepare(&scene, &registry, &assets)?, probe))
}

#[test]
fn instantiate_preflights_scene_identity_registration_before_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let mut world = World::new();
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    let error = instantiate(prepared, world.initializer())
        .err()
        .ok_or("missing SceneEntityId registration was accepted")?;

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component,
        } if component.as_str() == "sge.scene_entity_id"
    ));
    assert_eq!(world.entities().count(), 0);
    assert!(!world.component_is_registered::<SceneEntityId>());
    Ok(())
}

#[test]
fn instantiate_preflights_parent_registration_before_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    let error = instantiate(prepared, world.initializer())
        .err()
        .ok_or("missing Parent registration was accepted")?;

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component,
        } if component.as_str() == "sge.parent"
    ));
    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn instantiate_preflights_every_custom_registration_before_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let source_entity = scene_id(1)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.finish_registration();

    let error = instantiate(prepared, world.initializer())
        .err()
        .ok_or("missing custom registration was accepted")?;

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: Some(entity),
            component,
        } if entity == source_entity && component.as_str() == "demo.probe"
    ));
    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn pure_preflight_rejects_missing_scene_identity_without_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let mut world = World::new();
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    let error = preflight_instantiation(&prepared, &world)
        .expect_err("missing SceneEntityId registration was accepted");

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component,
        } if component.as_str() == "sge.scene_entity_id"
    ));
    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn pure_preflight_rejects_missing_parent_without_spawning() -> Result<(), Box<dyn std::error::Error>>
{
    let (prepared, _) = prepared_probe()?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    let error = preflight_instantiation(&prepared, &world)
        .expect_err("missing Parent registration was accepted");

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: None,
            component,
        } if component.as_str() == "sge.parent"
    ));
    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn pure_preflight_rejects_missing_custom_component_without_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let source_entity = scene_id(1)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.finish_registration();

    let error = preflight_instantiation(&prepared, &world)
        .expect_err("missing custom registration was accepted");

    assert!(matches!(
        error,
        SceneInstantiationError::MissingComponentRegistration {
            entity: Some(entity),
            component,
        } if entity == source_entity && component.as_str() == "demo.probe"
    ));
    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn pure_preflight_accepts_complete_registration_without_spawning()
-> Result<(), Box<dyn std::error::Error>> {
    let (prepared, _) = prepared_probe()?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    preflight_instantiation(&prepared, &world)?;

    assert_eq!(world.entities().count(), 0);
    Ok(())
}

#[test]
fn prepare_rejects_reserved_structural_type_collision_before_decode()
-> Result<(), Box<dyn std::error::Error>> {
    let source_entity = scene_id(1)?;
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
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(
        source_entity,
        None,
        vec![ReflectedValue::new(
            component_key.clone(),
            1,
            FieldValues::default(),
        )],
    )?])?;
    let error = prepare(&scene, &registry, &Assets::default())
        .err()
        .ok_or("reserved structural TypeId was accepted during prepare")?;

    assert!(matches!(
        error,
        SceneValidationError::ReservedStructuralComponent { entity, component }
            if entity == source_entity && component == component_key
    ));
    Ok(())
}

#[test]
fn instantiate_moves_structural_and_custom_components_into_canonical_entities()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let root = scene_id(1)?;
    let child = scene_id(2)?;
    let asset = AssetId::new_v4();
    let probe = Probe {
        count: 9,
        target: root,
        mesh: AssetRef::<MeshAsset>::new(asset),
    };
    let scene = AuthoringScene::new(vec![
        AuthoringEntity::new(child, Some(root), vec![registry.encode(&probe)?])?,
        AuthoringEntity::new(root, None, Vec::new())?,
    ])?;
    let prepared = prepare(
        &scene,
        &registry,
        &Assets::with(asset, MESH_ASSET_TYPE_KEY)?,
    )?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();

    let instance = instantiate(prepared, world.initializer())?;

    assert_eq!(world.entities().count(), 2);
    let ids = world
        .query::<SceneEntityId>()
        .map(|(_, id)| *id)
        .collect::<Vec<_>>();
    assert_eq!(ids, [root, child]);
    let root_runtime = world
        .query::<SceneEntityId>()
        .find_map(|(entity, id)| (*id == root).then_some(entity))
        .ok_or("root runtime entity missing")?;
    let child_runtime = world
        .query::<SceneEntityId>()
        .find_map(|(entity, id)| (*id == child).then_some(entity))
        .ok_or("child runtime entity missing")?;
    assert_eq!(world.get::<Parent>(child_runtime), Some(&Parent(root)));
    let stored = world
        .get::<Probe>(child_runtime)
        .ok_or("child Probe missing")?;
    assert_eq!(stored.count, probe.count);
    assert_eq!(stored.target, probe.target);
    assert_eq!(stored.mesh.id(), probe.mesh.id());
    assert_eq!(instance.entity(&root), Some(root_runtime));
    assert_eq!(instance.entity(&child), Some(child_runtime));
    assert_eq!(
        instance.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [root, child]
    );
    Ok(())
}

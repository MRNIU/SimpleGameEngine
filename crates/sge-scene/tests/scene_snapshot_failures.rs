// Copyright The SimpleGameEngine Contributors

mod support;

use std::any::TypeId;

use sge_asset::{AssetId, AssetRef, MESH_ASSET_TYPE_KEY};
use sge_ecs::{EcsError, World};
use sge_reflect::ReflectError;
use sge_scene::{Parent, SceneEntityId, SceneSnapshotError, SceneValidationError, snapshot};

use support::{Assets, MeshAsset, Probe, probe_registry, scene_id};

fn registered_world() -> Result<World, EcsError> {
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();
    Ok(world)
}

#[test]
fn snapshot_preserves_ecs_read_context() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.finish_registration();
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());

    assert!(matches!(
        snapshot(&world, &registry, &Assets::default()),
        Err(SceneSnapshotError::Ecs {
            entity,
            component,
            source: EcsError::UnregisteredComponentTypeId(type_id),
        }) if entity == id
            && component.as_str() == "demo.probe"
            && type_id == TypeId::of::<Probe>()
    ));
    Ok(())
}

#[test]
fn snapshot_preserves_reflect_encode_context() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let asset = AssetId::new_v4();
    let mut world = registered_world()?;
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(
        world
            .insert(
                runtime,
                Probe {
                    count: -1,
                    target: id,
                    mesh: AssetRef::<MeshAsset>::new(asset),
                },
            )?
            .is_none()
    );

    assert!(matches!(
        snapshot(
            &world,
            &registry,
            &Assets::with(asset, MESH_ASSET_TYPE_KEY)?
        ),
        Err(SceneSnapshotError::Encode {
            entity,
            component,
            source: ReflectError::Validation(_),
        }) if entity == id && component.as_str() == "demo.probe"
    ));
    Ok(())
}

#[test]
fn snapshot_reuses_parent_graph_validation() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let missing = scene_id(2)?;
    let mut world = registered_world()?;
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(world.insert(runtime, Parent(missing))?.is_none());

    assert!(matches!(
        snapshot(&world, &registry, &Assets::default()),
        Err(SceneSnapshotError::Validation(source))
            if matches!(*source, SceneValidationError::MissingParent { entity, parent }
                if entity == id && parent == missing)
    ));
    Ok(())
}

#[test]
fn snapshot_reuses_entity_reference_validation() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let missing = scene_id(2)?;
    let asset = AssetId::new_v4();
    let mut world = registered_world()?;
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(
        world
            .insert(
                runtime,
                Probe {
                    count: 1,
                    target: missing,
                    mesh: AssetRef::<MeshAsset>::new(asset),
                },
            )?
            .is_none()
    );

    assert!(matches!(
        snapshot(
            &world,
            &registry,
            &Assets::with(asset, MESH_ASSET_TYPE_KEY)?
        ),
        Err(SceneSnapshotError::Validation(source))
            if matches!(*source, SceneValidationError::MissingEntityReference { entity, target, .. }
                if entity == id && target == missing)
    ));
    Ok(())
}

#[test]
fn snapshot_reuses_typed_asset_reference_validation() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let asset = AssetId::new_v4();
    let mut world = registered_world()?;
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(
        world
            .insert(
                runtime,
                Probe {
                    count: 1,
                    target: id,
                    mesh: AssetRef::<MeshAsset>::new(asset),
                },
            )?
            .is_none()
    );

    assert!(matches!(
        snapshot(&world, &registry, &Assets::with(asset, "asset.texture")?),
        Err(SceneSnapshotError::Validation(source))
            if matches!(*source, SceneValidationError::AssetTypeMismatch {
                entity,
                asset: found,
                ..
            } if entity == id && found == asset)
    ));
    Ok(())
}

#[test]
fn snapshot_rejects_missing_typed_asset_reference() -> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let asset = AssetId::new_v4();
    let mut world = registered_world()?;
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(
        world
            .insert(
                runtime,
                Probe {
                    count: 1,
                    target: id,
                    mesh: AssetRef::<MeshAsset>::new(asset),
                },
            )?
            .is_none()
    );

    assert!(matches!(
        snapshot(&world, &registry, &Assets::default()),
        Err(SceneSnapshotError::Validation(source))
            if matches!(*source, SceneValidationError::MissingAssetReference {
                entity,
                asset: found,
                ..
            } if entity == id && found == asset)
    ));
    Ok(())
}

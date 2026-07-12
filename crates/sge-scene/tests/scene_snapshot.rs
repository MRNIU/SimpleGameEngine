// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetId, AssetRef, MESH_ASSET_TYPE_KEY};
use sge_ecs::World;
use sge_reflect::{TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{
    Parent, SceneEntityId, SceneSnapshotError, instantiate, parent_descriptor, prepare,
    scene_entity_id_descriptor, snapshot,
};

use support::{Assets, MeshAsset, Probe, probe_descriptor, probe_registry, scene_id};

#[test]
fn snapshot_rejects_every_alive_entity_without_scene_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();
    let runtime_entity = world.spawn();

    let error = snapshot(&world, &registry, &Assets::default())
        .err()
        .ok_or("alive entity without SceneEntityId was omitted")?;

    assert!(matches!(
        error,
        SceneSnapshotError::MissingSceneEntityId { runtime_entity: found }
            if found == runtime_entity
    ));
    Ok(())
}

#[test]
fn snapshot_rejects_duplicate_scene_identity_with_both_runtime_entities()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = probe_registry()?;
    let id = scene_id(1)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.finish_registration();
    let first = world.spawn();
    let duplicate = world.spawn();
    assert!(world.insert(first, id)?.is_none());
    assert!(world.insert(duplicate, id)?.is_none());

    let error = snapshot(&world, &registry, &Assets::default())
        .err()
        .ok_or("duplicate SceneEntityId was accepted")?;

    assert!(matches!(
        error,
        SceneSnapshotError::DuplicateSceneEntityId {
            id: found,
            first: found_first,
            duplicate: found_duplicate,
        } if found == id && found_first == first && found_duplicate == duplicate
    ));
    Ok(())
}

#[derive(Clone)]
struct EditorOnly;

#[test]
fn snapshot_retains_structural_only_entities_and_omits_non_saveable_components()
-> Result<(), Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.register(scene_entity_id_descriptor()?)?;
    registry.register(parent_descriptor()?)?;
    registry.register(
        TypeDescriptor::builder::<EditorOnly>(
            TypeKey::new("editor.only")?,
            1,
            "Editor only",
            || EditorOnly,
        )
        .build()?,
    )?;
    registry.freeze()?;
    let id = scene_id(1)?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<EditorOnly>()?;
    world.finish_registration();
    let runtime = world.spawn();
    assert!(world.insert(runtime, id)?.is_none());
    assert!(world.insert(runtime, EditorOnly)?.is_none());

    let scene = snapshot(&world, &registry, &Assets::default())?;

    let entities = scene.entities().collect::<Vec<_>>();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].id(), id);
    assert_eq!(entities[0].parent(), None);
    assert_eq!(entities[0].components().count(), 0);
    Ok(())
}

#[test]
fn snapshot_encodes_saveable_components_in_descriptor_order()
-> Result<(), Box<dyn std::error::Error>> {
    #[derive(Clone)]
    struct Alpha;

    let mut registry = TypeRegistry::new();
    registry.register(probe_descriptor()?)?;
    registry.register(
        TypeDescriptor::builder::<Alpha>(TypeKey::new("demo.alpha")?, 1, "Alpha", || Alpha)
            .scene_saveable()
            .build()?,
    )?;
    registry.freeze()?;
    let root = scene_id(1)?;
    let child = scene_id(2)?;
    let asset = AssetId::new_v4();
    let probe = Probe {
        count: 11,
        target: root,
        mesh: AssetRef::<MeshAsset>::new(asset),
    };
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<Probe>()?;
    world.register_component::<Alpha>()?;
    world.finish_registration();
    let root_runtime = world.spawn();
    let child_runtime = world.spawn();
    assert!(world.insert(root_runtime, root)?.is_none());
    assert!(world.insert(child_runtime, child)?.is_none());
    assert!(world.insert(child_runtime, Parent(root))?.is_none());
    assert!(world.insert(child_runtime, probe.clone())?.is_none());
    assert!(world.insert(child_runtime, Alpha)?.is_none());
    let assets = Assets::with(asset, MESH_ASSET_TYPE_KEY)?;

    let scene = snapshot(&world, &registry, &assets)?;

    let entities = scene.entities().collect::<Vec<_>>();
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].id(), root);
    assert_eq!(entities[0].components().count(), 0);
    assert_eq!(entities[1].id(), child);
    assert_eq!(entities[1].parent(), Some(root));
    let components = entities[1].components().collect::<Vec<_>>();
    assert_eq!(
        components
            .iter()
            .map(|component| component.type_key().as_str())
            .collect::<Vec<_>>(),
        ["demo.alpha", "demo.probe"]
    );
    let encoded = components
        .iter()
        .copied()
        .find(|component| component.type_key().as_str() == "demo.probe")
        .ok_or("saveable Probe was omitted")?;
    assert_eq!(encoded.type_key().as_str(), "demo.probe");
    let decoded = registry.decode(encoded)?;
    let stored = decoded
        .downcast_ref::<Probe>()
        .ok_or("snapshot Probe decoded to the wrong Rust type")?;
    assert_eq!(stored.count, probe.count);
    assert_eq!(stored.target, probe.target);
    assert_eq!(stored.mesh.id(), probe.mesh.id());
    let bytes = scene.to_ron()?;
    let reopened = sge_scene::AuthoringScene::from_ron(&bytes)?;
    assert_eq!(reopened.to_ron()?, bytes);
    let _prepared = sge_scene::prepare(&reopened, &registry, &assets)?;
    Ok(())
}

#[test]
fn snapshot_excludes_saveable_aliases_of_both_structural_rust_types()
-> Result<(), Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDescriptor::builder::<SceneEntityId>(
            TypeKey::new("alias.identity")?,
            1,
            "Identity alias",
            SceneEntityId::new_v4,
        )
        .scene_saveable()
        .build()?,
    )?;
    registry.register(
        TypeDescriptor::builder::<Parent>(TypeKey::new("alias.parent")?, 1, "Parent alias", || {
            Parent(SceneEntityId::new_v4())
        })
        .scene_saveable()
        .build()?,
    )?;
    registry.freeze()?;
    let root = scene_id(1)?;
    let child = scene_id(2)?;
    let mut source = World::new();
    source.register_component::<SceneEntityId>()?;
    source.register_component::<Parent>()?;
    source.finish_registration();
    let root_runtime = source.spawn();
    let child_runtime = source.spawn();
    assert!(source.insert(root_runtime, root)?.is_none());
    assert!(source.insert(child_runtime, child)?.is_none());
    assert!(source.insert(child_runtime, Parent(root))?.is_none());

    let scene = snapshot(&source, &registry, &Assets::default())?;

    assert!(
        scene
            .entities()
            .all(|entity| entity.components().count() == 0)
    );
    let prepared = prepare(&scene, &registry, &Assets::default())?;
    let mut reopened = World::new();
    reopened.register_component::<SceneEntityId>()?;
    reopened.register_component::<Parent>()?;
    reopened.finish_registration();
    let instance = instantiate(prepared, reopened.initializer())?;
    assert_eq!(instance.iter().count(), 2);
    assert_eq!(
        instance
            .entity(&child)
            .and_then(|entity| reopened.get::<Parent>(entity)),
        Some(&Parent(root))
    );
    Ok(())
}

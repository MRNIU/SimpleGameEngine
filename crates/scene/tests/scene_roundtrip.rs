// Copyright The SimpleGameEngine Contributors
//
//! 场景 roundtrip 与 runtime children cache 测试。

use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, MeshRef, Projection, World};
use math::Transform;
use scene::{load_scene, save_scene};

#[test]
fn scene_roundtrip_rebuilds_children_cache_and_render_components() {
    let mut world = World::new();
    world.spawn(EntityId::new("root"), "Root", Transform::identity());
    world.spawn(
        EntityId::new("camera"),
        "Camera",
        Transform::from_translation([0.0, 2.0, 5.0]),
    );
    world.set_parent("camera", "root").unwrap();
    world
        .insert_camera(
            "camera",
            Camera::new(Projection::Perspective {
                fov_y_degrees: 60.0,
            }),
        )
        .unwrap();
    let cube_transform = Transform {
        translation: [1.0, 2.0, 3.0],
        rotation: [0.0, 0.0, 1.0, 0.0],
        scale: [2.0, 1.5, 1.0],
    };
    world.spawn(EntityId::new("cube"), "Player Cube", cube_transform);
    world.set_parent("cube", "root").unwrap();
    world
        .insert_mesh(
            "cube",
            MeshRef::new("primitive:cube", "primitive:default_material"),
        )
        .unwrap();
    world.entity_mut("cube").unwrap().material_override = Some(MaterialOverride {
        base_color: [0.8, 0.2, 0.1, 0.9],
    });
    world.spawn(
        EntityId::new("directional_light"),
        "Directional Light",
        Transform::from_translation([0.0, 4.0, 2.0]),
    );
    world.set_parent("directional_light", "root").unwrap();
    world
        .insert_light(
            "directional_light",
            Light {
                kind: LightKind::Directional,
                color: [1.0, 0.8, 0.6],
                intensity: 1.25,
            },
        )
        .unwrap();

    let serialized = save_scene(&world).unwrap();
    let loaded = load_scene(&serialized).unwrap();

    assert_eq!(
        loaded.children_of("root"),
        vec![
            EntityId::new("camera"),
            EntityId::new("cube"),
            EntityId::new("directional_light"),
        ]
    );
    let camera = loaded.entity("camera").unwrap();
    let cube = loaded.entity("cube").unwrap();
    let light = loaded.entity("directional_light").unwrap();
    assert!(camera.camera.is_some());
    assert_eq!(cube.name, "Player Cube");
    assert_eq!(cube.parent, Some(EntityId::new("root")));
    assert_eq!(cube.transform, cube_transform);
    assert_eq!(cube.mesh.as_ref().unwrap().asset, "primitive:cube");
    assert_eq!(
        cube.mesh.as_ref().unwrap().material,
        "primitive:default_material"
    );
    assert_eq!(
        cube.material_override.as_ref().unwrap().base_color,
        [0.8, 0.2, 0.1, 0.9]
    );
    assert_eq!(light.light.as_ref().unwrap().kind, LightKind::Directional);
    assert_eq!(light.light.as_ref().unwrap().color, [1.0, 0.8, 0.6]);
    assert_eq!(light.light.as_ref().unwrap().intensity, 1.25);
    assert!(!serialized.contains("children"));
}

#[test]
fn scene_roundtrip_preserves_builtin_primitive_refs() {
    let mut world = World::new();
    world.spawn(EntityId::new("sphere"), "Sphere", Transform::identity());
    world
        .insert_mesh(
            "sphere",
            MeshRef::new("primitive:sphere", "primitive:default_material"),
        )
        .unwrap();
    world.spawn(EntityId::new("cone"), "Cone", Transform::identity());
    world
        .insert_mesh(
            "cone",
            MeshRef::new("primitive:cone", "primitive:default_material"),
        )
        .unwrap();

    let serialized = save_scene(&world).unwrap();
    let loaded = load_scene(&serialized).unwrap();

    assert!(serialized.contains("primitive:sphere"));
    assert!(serialized.contains("primitive:cone"));
    assert_eq!(
        loaded
            .entity("sphere")
            .unwrap()
            .mesh
            .as_ref()
            .unwrap()
            .asset,
        "primitive:sphere"
    );
    assert_eq!(
        loaded.entity("cone").unwrap().mesh.as_ref().unwrap().asset,
        "primitive:cone"
    );
}

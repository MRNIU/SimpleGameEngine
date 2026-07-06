// Copyright The SimpleGameEngine Contributors
//
//! 场景 roundtrip 与 runtime children cache 测试。

use ecs::{Camera, EntityId, MeshRef, Projection, World};
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
    world.spawn(EntityId::new("cube"), "Cube", Transform::identity());
    world.set_parent("cube", "root").unwrap();
    world
        .insert_mesh(
            "cube",
            MeshRef::new("primitive:cube", "primitive:default_material"),
        )
        .unwrap();

    let serialized = save_scene(&world).unwrap();
    let loaded = load_scene(&serialized).unwrap();

    assert_eq!(
        loaded.children_of("root"),
        vec![EntityId::new("camera"), EntityId::new("cube")]
    );
    assert!(loaded.entity("camera").unwrap().camera.is_some());
    assert_eq!(
        loaded.entity("cube").unwrap().mesh.as_ref().unwrap().asset,
        "primitive:cube"
    );
    assert!(!serialized.contains("children"));
}

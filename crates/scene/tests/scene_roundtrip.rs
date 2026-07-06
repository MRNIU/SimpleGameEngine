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

    let serialized = save_scene(&world).unwrap();
    let loaded = load_scene(&serialized).unwrap();

    assert_eq!(
        loaded.children_of("root"),
        vec![EntityId::new("camera"), EntityId::new("cube")]
    );
    let camera = loaded.entity("camera").unwrap();
    let cube = loaded.entity("cube").unwrap();
    assert!(camera.camera.is_some());
    assert_eq!(cube.name, "Player Cube");
    assert_eq!(cube.parent, Some(EntityId::new("root")));
    assert_eq!(cube.transform, cube_transform);
    assert_eq!(cube.mesh.as_ref().unwrap().asset, "primitive:cube");
    assert_eq!(
        cube.mesh.as_ref().unwrap().material,
        "primitive:default_material"
    );
    assert!(!serialized.contains("children"));
}

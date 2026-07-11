use super::EditorModel;
use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, Projection};
use sge_math::Transform;

#[test]
fn new_editor_starts_with_camera() {
    let editor = EditorModel::new();

    assert!(
        editor
            .world()
            .entity("camera")
            .and_then(|entity| entity.camera.as_ref())
            .is_some()
    );
}

#[test]
fn new_editor_starts_with_deletable_directional_light() {
    let mut editor = EditorModel::new();
    let light = editor.world().entity("directional_light").unwrap();

    assert_eq!(light.parent, Some(EntityId::new("root")));
    assert_eq!(light.light.as_ref().unwrap().kind, LightKind::Directional);
    assert_eq!(light.light.as_ref().unwrap().color, [1.0, 1.0, 1.0]);
    assert_eq!(light.light.as_ref().unwrap().intensity, 1.0);

    editor
        .delete_entity(&EntityId::new("directional_light"))
        .unwrap();
    assert!(editor.world().entity("directional_light").is_none());
}

#[test]
fn material_override_commit_undo_redo() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    editor.clear_history();
    let before = None;
    let after = Some(MaterialOverride {
        base_color: [0.7, 0.2, 0.1, 1.0],
    });

    editor.preview_material_override(&cube, after).unwrap();
    assert!(!editor.is_dirty());
    assert!(!editor.can_undo());
    assert!(
        editor
            .commit_material_override_edit(&cube, before, after)
            .unwrap()
    );

    assert!(editor.is_dirty());
    assert_eq!(
        editor
            .world()
            .entity(&cube)
            .unwrap()
            .material_override
            .as_ref()
            .unwrap()
            .base_color,
        [0.7, 0.2, 0.1, 1.0]
    );
    assert!(editor.undo().unwrap());
    assert!(
        editor
            .world()
            .entity(&cube)
            .unwrap()
            .material_override
            .is_none()
    );
    assert!(editor.redo().unwrap());
    assert!(
        editor
            .world()
            .entity(&cube)
            .unwrap()
            .material_override
            .is_some()
    );
}

#[test]
fn invalid_light_value_does_not_enter_history() {
    let mut editor = EditorModel::default();
    let light_id = EntityId::new("directional_light");
    editor.mark_saved();
    editor.clear_history();

    let result = editor.set_light(
        &light_id,
        Light {
            kind: LightKind::Directional,
            color: [1.0, f32::NAN, 1.0],
            intensity: 1.0,
        },
    );

    assert_eq!(
        result.unwrap_err(),
        super::EditorError::InvalidSceneContentValue
    );
    assert!(!editor.is_dirty());
    assert!(!editor.can_undo());
}

#[test]
fn camera_commit_uses_projection_in_selected_camera_view() {
    let mut editor = EditorModel::default();
    let camera_id = EntityId::new("camera");
    editor.select(camera_id.clone());
    let before = editor
        .world()
        .entity("camera")
        .unwrap()
        .camera
        .as_ref()
        .unwrap()
        .clone();
    let after = Camera::new(Projection::Perspective {
        fov_y_degrees: 35.0,
    });

    assert!(
        editor
            .commit_camera_edit(&camera_id, before, after)
            .unwrap()
    );

    assert_eq!(
        editor.selected_camera_view().unwrap().projection,
        Projection::Perspective {
            fov_y_degrees: 35.0
        }
    );
}

#[test]
fn transform_command_undo_redo_uses_canonical_rotation() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    editor.clear_history();

    editor
        .set_transform(
            &cube,
            Transform {
                rotation: [0.0, 0.0, 2.0, 0.0],
                ..Transform::identity()
            },
        )
        .unwrap();

    assert!(editor.can_undo());
    assert!(!editor.can_redo());
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.rotation,
        [0.0, 0.0, 1.0, 0.0]
    );

    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.rotation,
        Transform::identity().rotation
    );
    assert!(editor.can_redo());

    assert!(editor.redo().unwrap());
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.rotation,
        [0.0, 0.0, 1.0, 0.0]
    );
}

#[test]
fn canonical_transform_noop_does_not_push_extra_history() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.clear_history();
    editor
        .set_transform(
            &cube,
            Transform {
                rotation: [0.0, 0.0, 2.0, 0.0],
                ..Transform::identity()
            },
        )
        .unwrap();

    editor
        .set_transform(
            &cube,
            Transform {
                rotation: [0.0, 0.0, 4.0, 0.0],
                ..Transform::identity()
            },
        )
        .unwrap();

    assert!(editor.can_undo());
    assert!(!editor.can_redo());
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.rotation,
        Transform::identity().rotation
    );
    assert!(!editor.can_undo());
}

#[test]
fn preview_transform_does_not_touch_history_or_dirty() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    editor.clear_history();
    let before = editor.world().entity(&cube).unwrap().transform;

    editor
        .preview_transform(&cube, Transform::from_translation([3.0, 0.0, 0.0]))
        .unwrap();

    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.translation,
        [3.0, 0.0, 0.0]
    );
    assert!(!editor.is_dirty());
    assert!(!editor.can_undo());

    editor
        .restore_transform_preview(&cube, before, false)
        .unwrap();
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.translation,
        [0.0, 0.0, 0.0]
    );
    assert!(!editor.is_dirty());
}

#[test]
fn commit_transform_edit_pushes_one_history_entry() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    editor.clear_history();
    let before = editor.world().entity(&cube).unwrap().transform;
    let after = Transform::from_translation([2.0, 0.0, 0.0]);

    editor.preview_transform(&cube, after).unwrap();
    assert!(editor.commit_transform_edit(&cube, before, after).unwrap());

    assert!(editor.is_dirty());
    assert!(editor.can_undo());
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.world().entity(&cube).unwrap().transform.translation,
        [0.0, 0.0, 0.0]
    );
}

#[test]
fn create_primitives_use_expected_ids_names_mesh_refs_and_selection() {
    let mut editor = super::EditorModel::default();

    let cube = editor.create_primitive(super::PrimitiveKind::Cube);
    let sphere = editor.create_primitive(super::PrimitiveKind::Sphere);
    let cone = editor.create_primitive(super::PrimitiveKind::Cone);
    let cylinder = editor.create_primitive(super::PrimitiveKind::Cylinder);
    let second_sphere = editor.create_primitive(super::PrimitiveKind::Sphere);

    let cases = [
        (&cube, "cube", "Cube", "primitive:cube"),
        (&sphere, "sphere", "Sphere", "primitive:sphere"),
        (&cone, "cone", "Cone", "primitive:cone"),
        (&cylinder, "cylinder", "Cylinder", "primitive:cylinder"),
        (&second_sphere, "sphere_1", "Sphere 2", "primitive:sphere"),
    ];
    for (id, expected_id, expected_name, expected_asset) in cases {
        let record = editor.world().entity(id.as_str()).unwrap();
        assert_eq!(id.as_str(), expected_id);
        assert_eq!(record.name, expected_name);
        assert_eq!(record.parent, Some(EntityId::new("root")));
        assert_eq!(record.mesh.as_ref().unwrap().asset, expected_asset);
        assert_eq!(
            record.mesh.as_ref().unwrap().material,
            "primitive:default_material"
        );
    }
    assert_eq!(editor.selected(), Some(&second_sphere));
    assert!(editor.is_dirty());
    assert!(editor.can_undo());
}

#[test]
fn primitive_create_undo_redo_restores_entity() {
    let mut editor = super::EditorModel::default();
    let cone = editor.create_primitive(super::PrimitiveKind::Cone);

    assert!(editor.world().entity(cone.as_str()).is_some());
    assert!(editor.undo().unwrap());
    assert!(editor.world().entity(cone.as_str()).is_none());
    assert!(editor.redo().unwrap());
    assert!(editor.world().entity(cone.as_str()).is_some());
    assert_eq!(editor.selected(), Some(&cone));
}

#[test]
fn create_cube_remains_wrapper_for_cube_primitive() {
    let mut editor = super::EditorModel::default();

    let cube = editor.create_cube();

    let record = editor.world().entity(cube.as_str()).unwrap();
    assert_eq!(cube.as_str(), "cube");
    assert_eq!(record.mesh.as_ref().unwrap().asset, "primitive:cube");
}

#[test]
fn imported_mesh_entity_uses_asset_ref_and_default_material() {
    let uuid = asset::AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let mut editor = super::EditorModel::default();

    let entity = editor
        .create_imported_mesh(&uuid, "Crate", Transform::identity())
        .unwrap();

    let record = editor.world().entity(entity.as_str()).unwrap();
    assert_eq!(record.name, "Crate");
    assert_eq!(record.mesh.as_ref().unwrap().asset, uuid.to_asset_ref());
    assert_eq!(
        record.mesh.as_ref().unwrap().material,
        "primitive:default_material"
    );
    assert!(editor.is_dirty());
    assert!(editor.can_undo());
}

#[test]
fn imported_mesh_entity_conflicts_get_suffixes() {
    let first_uuid = asset::AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let second_uuid =
        asset::AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let mut editor = super::EditorModel::default();

    let first = editor
        .create_imported_mesh(&first_uuid, "Crate", Transform::identity())
        .unwrap();
    let second = editor
        .create_imported_mesh(&second_uuid, "Crate", Transform::identity())
        .unwrap();

    assert_eq!(first.as_str(), "asset_crate");
    assert_eq!(second.as_str(), "asset_crate_1");
    assert_eq!(editor.world().entity(first.as_str()).unwrap().name, "Crate");
    assert_eq!(
        editor.world().entity(second.as_str()).unwrap().name,
        "Crate 2"
    );
}

#[test]
fn imported_mesh_create_undo_redo() {
    let uuid = asset::AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let mut editor = super::EditorModel::default();
    let entity = editor
        .create_imported_mesh(&uuid, "Crate", Transform::identity())
        .unwrap();

    assert!(editor.world().entity(entity.as_str()).is_some());
    assert!(editor.undo().unwrap());
    assert!(editor.world().entity(entity.as_str()).is_none());
    assert!(editor.redo().unwrap());
    assert!(editor.world().entity(entity.as_str()).is_some());
}

use super::EditorModel;
use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, Projection};
use math::Transform;

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

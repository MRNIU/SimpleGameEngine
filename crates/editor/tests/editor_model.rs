// Copyright The SimpleGameEngine Contributors
//
//! 编辑器模型 create/edit/save/reopen 测试。

use ecs::EntityId;
use editor::{EditorError, EditorModel};
use math::Transform;
use std::{fs, path::Path};

#[test]
fn editor_model_can_create_save_and_reopen_a_cube_scene() {
    let mut editor = EditorModel::default();

    let cube = editor.create_cube();
    editor.set_translation(&cube, [1.0, 2.0, 3.0]).unwrap();

    let saved = editor.save_scene_to_string().unwrap();
    let reopened = EditorModel::from_scene_str(&saved).unwrap();

    assert_eq!(
        reopened
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [1.0, 2.0, 3.0]
    );
    assert_eq!(reopened.render_scene().meshes.len(), 1);
    assert_eq!(reopened.viewport_draw_call().unwrap().index_count, 36);
}

#[test]
fn editor_model_supports_milestone_one_entity_actions() {
    let mut editor = EditorModel::default();

    let first = editor.create_cube();
    let second = editor.create_cube();
    editor.rename_entity(&second, "Player Cube").unwrap();
    editor
        .set_transform(
            &second,
            Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 2.0, 0.0],
                scale: [2.0, 1.5, 1.0],
            },
        )
        .unwrap();
    let duplicate = editor.duplicate_selected().unwrap();

    assert_eq!(first, EntityId::new("cube"));
    assert_eq!(second, EntityId::new("cube_1"));
    assert_eq!(duplicate, EntityId::new("cube_2"));
    assert_eq!(editor.selected(), Some(&duplicate));
    assert!(editor.is_dirty());

    let copied = editor.world().entity(&duplicate).unwrap();
    assert_eq!(copied.name, "Player Cube Copy");
    assert_eq!(copied.parent, Some(EntityId::new("root")));
    assert_eq!(copied.transform.translation, [1.0, 2.0, 3.0]);
    assert_eq!(copied.transform.rotation, [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(copied.transform.scale, [2.0, 1.5, 1.0]);
    assert_eq!(copied.mesh.as_ref().unwrap().asset, "primitive:cube");
}

#[test]
fn editor_model_rejects_invalid_user_edits() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();

    assert!(matches!(
        editor.rename_entity(&cube, "   "),
        Err(EditorError::InvalidEntityName)
    ));
    assert!(matches!(
        editor.set_transform(
            &cube,
            Transform {
                translation: [f32::NAN, 0.0, 0.0],
                ..Transform::identity()
            },
        ),
        Err(EditorError::InvalidTransformValue)
    ));
    assert!(matches!(
        editor.set_transform(
            &cube,
            Transform {
                scale: [1.0, 0.0, 1.0],
                ..Transform::identity()
            },
        ),
        Err(EditorError::InvalidTransformValue)
    ));
    assert!(matches!(
        editor.set_transform(
            &cube,
            Transform {
                rotation: [0.0, 0.0, 0.0, 0.0],
                ..Transform::identity()
            },
        ),
        Err(EditorError::InvalidTransformValue)
    ));
}

#[test]
fn editor_model_protects_root_and_camera_from_delete_or_duplicate() {
    let mut editor = EditorModel::default();

    editor.select(EntityId::new("root"));
    assert!(matches!(
        editor.delete_selected(),
        Err(EditorError::ProtectedEntity(id)) if id == EntityId::new("root")
    ));

    editor.select(EntityId::new("camera"));
    assert!(matches!(
        editor.duplicate_selected(),
        Err(EditorError::ProtectedEntity(id)) if id == EntityId::new("camera")
    ));
}

#[test]
fn editor_model_delete_falls_back_to_parent_selection() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();

    editor.delete_selected().unwrap();

    assert!(editor.world().entity(&cube).is_none());
    assert_eq!(editor.selected(), Some(&EntityId::new("root")));
}

#[test]
fn editor_model_reopen_preserves_selection_only_when_entity_still_exists() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    assert!(!editor.is_dirty());

    let saved = editor.save_scene_to_string().unwrap();
    editor.reopen_scene_from_str(&saved).unwrap();
    assert_eq!(editor.selected(), Some(&cube));
    assert!(!editor.is_dirty());

    editor.delete_selected().unwrap();
    let without_cube = editor.save_scene_to_string().unwrap();
    editor.select(cube);
    editor.reopen_scene_from_str(&without_cube).unwrap();
    assert_eq!(editor.selected(), None);
}

#[test]
fn editor_smoke_actions_create_save_reopen_and_verify_viewport() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/editor_model_smoke.scene.ron");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let _ = fs::remove_file(&path);

    let report = EditorModel::default().run_smoke_actions(&path).unwrap();

    assert_eq!(report.mesh_count, 3);
    assert!(report.has_camera);
    assert_eq!(report.viewport_index_count, 108);
    assert!(path.exists());
}

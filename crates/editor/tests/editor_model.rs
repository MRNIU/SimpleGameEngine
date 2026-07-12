// Copyright The SimpleGameEngine Contributors
//
//! 编辑器模型 create/edit/save/reopen 测试。

use ecs::EntityId;
use editor::{EditorError, EditorModel};
use render::ViewportView;
use sge_math::Transform;

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
fn scene_content_edits_survive_save_reopen() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor
        .set_material_override(
            &cube,
            Some(ecs::MaterialOverride {
                base_color: [0.4, 0.5, 0.6, 1.0],
            }),
        )
        .unwrap();
    editor
        .set_light(
            &ecs::EntityId::new("directional_light"),
            ecs::Light {
                kind: ecs::LightKind::Directional,
                color: [0.7, 0.8, 0.9],
                intensity: 1.5,
            },
        )
        .unwrap();

    let saved = editor.save_scene_to_string().unwrap();
    let reopened = EditorModel::from_scene_str(&saved).unwrap();

    assert_eq!(
        reopened
            .world()
            .entity(&cube)
            .unwrap()
            .material_override
            .as_ref()
            .unwrap()
            .base_color,
        [0.4, 0.5, 0.6, 1.0]
    );
    assert_eq!(
        reopened
            .world()
            .entity("directional_light")
            .unwrap()
            .light
            .as_ref()
            .unwrap()
            .intensity,
        1.5
    );
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
fn editor_model_can_clear_selection_without_dirtying_scene() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    assert_eq!(editor.selected(), Some(&cube));
    editor.mark_saved();

    editor.clear_selection();

    assert_eq!(editor.selected(), None);
    assert!(!editor.is_dirty());
}

#[test]
fn editor_model_can_build_viewport_draw_for_editor_view() {
    let mut editor = EditorModel::default();
    editor.create_cube();
    let scene_camera_draw = editor.viewport_draw_call().unwrap();
    let editor_view = ViewportView::new(
        EntityId::new("editor_view"),
        Transform::from_translation([1.0, 0.0, 0.0]),
        ecs::Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );

    let editor_draw = editor.viewport_draw_call_for_view(&editor_view).unwrap();

    assert_eq!(editor_draw.camera_entity, EntityId::new("editor_view"));
    assert_eq!(scene_camera_draw.vertices, editor_draw.vertices);
    assert_eq!(editor_draw.mesh_spans.len(), 1);
}

#[test]
fn editor_model_undo_redo_create_cube() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();

    assert!(editor.world().entity(&cube).is_some());
    assert_eq!(editor.selected(), Some(&cube));

    assert!(editor.undo().unwrap());
    assert!(editor.world().entity(&cube).is_none());
    assert_eq!(editor.selected(), None);

    assert!(editor.redo().unwrap());
    assert!(editor.world().entity(&cube).is_some());
    assert_eq!(editor.selected(), Some(&cube));
}

#[test]
fn editor_model_undo_redo_rename_duplicate_and_delete() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();

    editor.rename_entity(&cube, "Renamed Cube").unwrap();
    assert_eq!(editor.world().entity(&cube).unwrap().name, "Renamed Cube");
    editor.undo().unwrap();
    assert_eq!(editor.world().entity(&cube).unwrap().name, "Cube");
    editor.redo().unwrap();
    assert_eq!(editor.world().entity(&cube).unwrap().name, "Renamed Cube");

    let duplicate = editor.duplicate_selected().unwrap();
    assert!(editor.world().entity(&duplicate).is_some());
    editor.undo().unwrap();
    assert!(editor.world().entity(&duplicate).is_none());
    editor.redo().unwrap();
    assert!(editor.world().entity(&duplicate).is_some());

    editor.delete_entity(&duplicate).unwrap();
    assert!(editor.world().entity(&duplicate).is_none());
    editor.undo().unwrap();
    assert!(editor.world().entity(&duplicate).is_some());
    assert_eq!(editor.selected(), Some(&duplicate));
}

#[test]
fn editor_model_new_command_clears_redo_stack() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();

    editor.undo().unwrap();
    assert!(editor.can_redo());
    editor
        .rename_entity(&EntityId::new("camera"), "Scene Camera")
        .unwrap();

    assert!(!editor.can_redo());
    assert!(editor.world().entity(&cube).is_none());
}

#[test]
fn editor_model_rename_noop_does_not_push_history() {
    let mut editor = EditorModel::default();
    let cube = editor.create_cube();
    editor.mark_saved();
    editor.clear_history();

    editor.rename_entity(&cube, "Cube").unwrap();

    assert!(!editor.can_undo());
    assert!(!editor.is_dirty());
}

#[test]
fn editor_model_mark_saved_keeps_history() {
    let mut editor = EditorModel::default();
    editor.create_cube();

    assert!(editor.can_undo());
    editor.mark_saved();

    assert!(!editor.is_dirty());
    assert!(editor.can_undo());
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
    let report = EditorModel::default().run_smoke_actions().unwrap();

    assert_eq!(report.mesh_count, 6);
    assert!(report.has_camera);
    assert_eq!(report.viewport_index_count, 276);
    assert!(!report.transform_undo_redo_ok);
    assert!(!report.content_reopen_ok);
}

#[test]
fn smoke_actions_include_scene_content_edits() {
    let report = EditorModel::default().run_smoke_actions().unwrap();

    assert!(report.mesh_count >= 6);
    assert!(report.has_camera);
    assert!(report.has_light);
    assert!(report.viewport_index_count >= 276);
    assert!(!report.transform_undo_redo_ok);
    assert!(!report.content_reopen_ok);
}

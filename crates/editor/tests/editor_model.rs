// Copyright The SimpleGameEngine Contributors
//
//! 编辑器模型 create/edit/save/reopen 测试。

use editor::EditorModel;
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
    assert_eq!(reopened.viewport_draw_call().unwrap().index_count, 6);
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

    assert_eq!(report.mesh_count, 1);
    assert!(report.has_camera);
    assert_eq!(report.viewport_index_count, 6);
    assert!(path.exists());
}

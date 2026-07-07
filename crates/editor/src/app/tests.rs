// Copyright The SimpleGameEngine Contributors

use super::{EditorLaunchOptions, fonts::cjk_font_candidates, panels::inspector_transform_fields};
use ecs::{Camera, EntityId, EntityRecord, Light, MaterialOverride, Projection};
use math::Transform;
use std::path::PathBuf;

#[test]
fn parses_smoke_argument() {
    let options = EditorLaunchOptions::from_args([
        "editor".to_owned(),
        "--smoke".to_owned(),
        "target/tmp/smoke.scene.ron".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        options.smoke_path,
        Some(PathBuf::from("target/tmp/smoke.scene.ron"))
    );
}

#[test]
fn camera_inspector_hides_scale_field() {
    let mut camera = EntityRecord::new(EntityId::new("camera"), "Camera", Transform::identity());
    camera.camera = Some(Camera::new(Projection::Perspective {
        fov_y_degrees: 60.0,
    }));
    let cube = EntityRecord::new(EntityId::new("cube"), "Cube", Transform::identity());

    assert!(inspector_transform_fields(&camera).show_translation);
    assert!(inspector_transform_fields(&camera).show_rotation);
    assert!(!inspector_transform_fields(&camera).show_scale);
    assert!(inspector_transform_fields(&cube).show_scale);
}

#[test]
fn viewport_preview_transform_does_not_write_history_or_dirty() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();

    app.preview_viewport_transform(cube.clone(), Transform::from_translation([3.0, 0.0, 0.0]));

    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [3.0, 0.0, 0.0]
    );
    assert!(!app.model.is_dirty());
    assert!(!app.model.can_undo());
}

#[test]
fn viewport_commit_transform_writes_one_history_entry() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();
    let before = Transform::identity();
    let after = Transform::from_translation([3.0, 0.0, 0.0]);

    app.preview_viewport_transform(cube.clone(), after);
    app.commit_viewport_transform(cube.clone(), before, after);

    assert!(app.model.is_dirty());
    assert!(app.model.can_undo());
    app.model.undo().unwrap();
    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [0.0, 0.0, 0.0]
    );
}

#[test]
fn viewport_transform_action_drops_stale_target() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.select(EntityId::new("root"));
    app.transform_gizmo.start_drag(crate::viewport::GizmoDrag {
        target: cube.clone(),
        handle: crate::viewport::GizmoHandle::MoveX,
        start_pointer: egui::pos2(0.0, 0.0),
        start_transform: Transform::identity(),
    });

    app.preview_viewport_transform(cube.clone(), Transform::from_translation([3.0, 0.0, 0.0]));

    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [0.0, 0.0, 0.0]
    );
    assert_eq!(app.transform_gizmo.drag(), None);
    assert_eq!(app.status, "Gizmo target changed");
}

#[test]
fn name_edit_session_commits_one_history_entry() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();

    app.begin_name_edit(cube.clone(), "Cube".to_owned());
    app.update_name_edit("Cube A".to_owned());
    app.update_name_edit("Cube B".to_owned());
    assert!(!app.model.can_undo());

    app.finish_name_edit(true);

    assert_eq!(app.model.world().entity(&cube).unwrap().name, "Cube B");
    assert!(app.model.can_undo());
    app.model.undo().unwrap();
    assert_eq!(app.model.world().entity(&cube).unwrap().name, "Cube");
}

#[test]
fn status_bar_selection_uses_entity_name() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.rename_entity(&cube, "Renamed Cube").unwrap();

    assert_eq!(
        super::panels::status_bar_selection_text(&app.model),
        "Renamed Cube"
    );
}

#[test]
fn name_edit_session_cancel_keeps_model_clean() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();

    app.begin_name_edit(cube.clone(), "Cube".to_owned());
    app.update_name_edit("Cube B".to_owned());
    app.finish_name_edit(false);

    assert_eq!(app.model.world().entity(&cube).unwrap().name, "Cube");
    assert!(!app.model.is_dirty());
    assert!(!app.model.can_undo());
}

#[test]
fn bottom_panel_keeps_status_visible_after_greedy_body() {
    egui::__run_test_ui(|ui| {
        ui.set_min_size(egui::vec2(320.0, 240.0));
        let status = egui::Panel::bottom("test_status_bar")
            .show(ui, |ui| ui.label("status"))
            .response
            .rect;
        let body = egui::CentralPanel::default()
            .show(ui, |ui| {
                ui.allocate_exact_size(ui.available_size_before_wrap(), egui::Sense::hover())
                    .0
            })
            .inner;

        assert!(body.bottom() <= status.top());
    });
}

#[test]
fn transform_edit_session_previews_then_commits_one_history_entry() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();
    let before = app.model.world().entity(&cube).unwrap().transform;

    app.begin_transform_edit(cube.clone(), before);
    app.preview_inspector_transform(cube.clone(), Transform::from_translation([1.0, 0.0, 0.0]));
    app.preview_inspector_transform(cube.clone(), Transform::from_translation([2.0, 0.0, 0.0]));
    assert!(!app.model.is_dirty());
    assert!(!app.model.can_undo());

    app.finish_transform_edit(true);

    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [2.0, 0.0, 0.0]
    );
    assert!(app.model.can_undo());
    app.model.undo().unwrap();
    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .transform
            .translation,
        [0.0, 0.0, 0.0]
    );
}

#[test]
fn material_edit_session_previews_then_commits_one_history_entry() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();

    app.begin_material_edit(cube.clone(), None);
    app.preview_material_edit(
        cube.clone(),
        Some(MaterialOverride {
            base_color: [0.2, 0.3, 0.4, 1.0],
        }),
    );
    app.preview_material_edit(
        cube.clone(),
        Some(MaterialOverride {
            base_color: [0.5, 0.6, 0.7, 1.0],
        }),
    );
    assert!(!app.model.is_dirty());
    assert!(!app.model.can_undo());

    app.finish_material_edit(true);

    assert_eq!(
        app.model
            .world()
            .entity(&cube)
            .unwrap()
            .material_override
            .as_ref()
            .unwrap()
            .base_color,
        [0.5, 0.6, 0.7, 1.0]
    );
    assert!(app.model.can_undo());
}

#[test]
fn light_edit_session_cancel_restores_value_and_dirty() {
    let mut app = super::EditorApp::default();
    let light = EntityId::new("directional_light");
    let before = app
        .model
        .world()
        .entity(&light)
        .unwrap()
        .light
        .as_ref()
        .unwrap()
        .clone();
    app.model.mark_saved();
    app.model.clear_history();

    app.begin_light_edit(light.clone(), before.clone());
    app.preview_light_edit(
        light.clone(),
        Light {
            intensity: 3.0,
            ..before.clone()
        },
    );
    app.finish_light_edit(false);

    assert_eq!(
        app.model
            .world()
            .entity(&light)
            .unwrap()
            .light
            .as_ref()
            .unwrap()
            .intensity,
        before.intensity
    );
    assert!(!app.model.is_dirty());
    assert!(!app.model.can_undo());
}

#[test]
fn camera_edit_session_commits_one_history_entry() {
    let mut app = super::EditorApp::default();
    let camera = EntityId::new("camera");
    let before = app
        .model
        .world()
        .entity(&camera)
        .unwrap()
        .camera
        .as_ref()
        .unwrap()
        .clone();
    app.model.mark_saved();
    app.model.clear_history();

    app.begin_camera_edit(camera.clone(), before);
    app.preview_camera_edit(
        camera.clone(),
        Camera::new(Projection::Perspective {
            fov_y_degrees: 45.0,
        }),
    );
    app.preview_camera_edit(
        camera.clone(),
        Camera::new(Projection::Perspective {
            fov_y_degrees: 35.0,
        }),
    );
    app.finish_camera_edit(true);

    assert!(app.model.can_undo());
    assert_eq!(
        app.model
            .world()
            .entity("camera")
            .unwrap()
            .camera
            .as_ref()
            .unwrap()
            .projection,
        Projection::Perspective {
            fov_y_degrees: 35.0
        }
    );
    app.model.undo().unwrap();
    assert_eq!(
        app.model
            .world()
            .entity("camera")
            .unwrap()
            .camera
            .as_ref()
            .unwrap()
            .projection,
        Projection::Perspective {
            fov_y_degrees: 60.0
        }
    );
}

#[test]
fn pilot_camera_exits_when_selection_or_scene_changes() {
    let mut app = super::EditorApp::default();
    app.model.select(EntityId::new("camera"));

    app.toggle_pilot_camera();
    assert!(app.pilot_camera);

    app.model.select(EntityId::new("root"));
    app.sync_pilot_camera_target();
    assert!(!app.pilot_camera);

    app.model.select(EntityId::new("camera"));
    app.toggle_pilot_camera();
    assert!(app.pilot_camera);
    app.replace_with_new_scene();
    assert!(!app.pilot_camera);
}

#[test]
fn editor_body_uses_side_panels_and_central_viewport_contract() {
    let source = include_str!("panels.rs");

    assert!(source.contains("SidePanel::left"));
    assert!(source.contains("SidePanel::right"));
    assert!(source.contains("CentralPanel::default"));
    assert!(!source.contains("ui.columns(3"));
}

#[test]
fn light_inspector_labels_color_and_intensity_controls() {
    let source = include_str!("panels.rs");

    assert!(source.contains("\"Color\""));
    assert!(source.contains("\"Intensity\""));
}

#[test]
fn scene_replace_clears_active_gizmo_drag() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.transform_gizmo.start_drag(crate::viewport::GizmoDrag {
        target: cube,
        handle: crate::viewport::GizmoHandle::MoveX,
        start_pointer: egui::pos2(0.0, 0.0),
        start_transform: Transform::identity(),
    });

    app.replace_with_new_scene();

    assert_eq!(app.transform_gizmo.drag(), None);
}

#[test]
fn cjk_font_candidates_cover_common_desktop_fonts() {
    let candidates = cjk_font_candidates();

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.ends_with("PingFang.ttc"))
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.contains("NotoSansCJK"))
    );
}

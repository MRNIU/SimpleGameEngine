// Copyright The SimpleGameEngine Contributors

use super::{EditorLaunchOptions, fonts::cjk_font_candidates, panels::inspector_transform_fields};
use ecs::{Camera, EntityId, EntityRecord, Light, MaterialOverride, Projection};
use math::Transform;
use std::path::PathBuf;

use crate::viewport::ViewportAction;

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
fn viewport_action_handler_supports_gizmo_preview_commit_and_undo() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    app.model.mark_saved();
    app.model.clear_history();
    let before = app.model.world().entity(&cube).unwrap().transform;
    let after = Transform::from_translation([3.0, 0.0, 0.0]);

    app.handle_viewport_action(ViewportAction::PreviewTransform {
        target: cube.clone(),
        transform: after,
    });

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

    app.handle_viewport_action(ViewportAction::CommitTransform {
        target: cube.clone(),
        before,
        after,
    });

    assert!(app.model.is_dirty());
    assert!(app.model.can_undo());
    assert!(app.model.undo().unwrap());
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
fn ui_action_save_clears_pending_without_running_new() {
    let mut app = super::EditorApp::default();
    let cube = app.model.create_cube();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp")
        .join(format!(
            "ui_action_save_clears_pending_without_running_new_{}.scene.ron",
            std::process::id()
        ));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let _ = std::fs::remove_file(&path);
    app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));
    app.pending_action = Some(super::PendingFileAction::New);

    app.run_ui_action(super::EditorUiAction::SaveScene);

    assert_eq!(app.pending_action, None);
    assert!(app.model.world().entity(cube.as_str()).is_some());
    assert_eq!(app.current_path, Some(path.clone()));
    assert!(!app.model.is_dirty());
    assert_eq!(app.status, "Saved");
    assert!(path.exists());
}

#[test]
fn open_scene_dialog_cancel_does_not_set_pending_action() {
    let mut app = super::EditorApp::default();
    app.model.create_cube();
    app.set_next_open_scene_dialog_path_for_test(None);

    app.run_ui_action(super::EditorUiAction::OpenSceneDialog);

    assert_eq!(app.pending_action, None);
    assert!(app.model.is_dirty());
}

#[test]
fn dirty_open_scene_dialog_sets_pending_open_after_path_selection() {
    let path = temp_scene_path("dirty_open_scene_dialog_sets_pending_open");
    let mut app = super::EditorApp::default();
    app.model.create_cube();
    app.set_next_open_scene_dialog_path_for_test(Some(path.clone()));

    app.run_ui_action(super::EditorUiAction::OpenSceneDialog);

    assert_eq!(
        app.pending_action,
        Some(super::PendingFileAction::Open(path))
    );
    assert_eq!(app.status, "Unsaved changes: save or discard first");
}

#[test]
fn save_without_current_path_uses_save_as_dialog() {
    let path = temp_scene_path("save_without_current_path_uses_save_as_dialog");
    let mut app = super::EditorApp::default();
    app.model.create_cube();
    app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));

    app.run_ui_action(super::EditorUiAction::SaveScene);

    assert_eq!(app.current_path, Some(path.clone()));
    assert!(path.exists());
    assert!(!app.model.is_dirty());
}

#[test]
fn status_bar_source_has_no_path_input_text_edit() {
    let source = include_str!("panels.rs");

    assert!(!source.contains("TextEdit::singleline(&mut self.path_input)"));
}

#[test]
fn import_obj_path_creates_manifest_asset_entity_and_draw_call() {
    let root = temp_project_root("import_obj_path_creates_manifest_asset_entity_and_draw_call");
    let source = root.join("source.obj");
    write_triangle_obj(&source);
    let mut app = super::EditorApp {
        project_root: root.clone(),
        ..Default::default()
    };

    let uuid = app.import_obj_path(&source).unwrap();

    assert!(asset::manifest_path(&root).exists());
    assert!(app.asset_manifest.find(&uuid).is_some());
    assert!(app.imported_meshes.contains_key(&uuid));
    let selected = app.model.selected().cloned().unwrap();
    let record = app.model.world().entity(selected.as_str()).unwrap();
    assert_eq!(record.mesh.as_ref().unwrap().asset, uuid.to_asset_ref());
    let view = app.viewport_camera.to_viewport_view();
    let draw = render::viewport_draw_call_with_view_and_meshes(
        &app.model.render_scene(),
        app.model.selected(),
        &view,
        &app.imported_meshes,
    )
    .unwrap();
    assert_eq!(draw.mesh_spans[0].entity, selected);
}

#[test]
fn import_obj_dialog_cancel_does_not_change_manifest_or_scene() {
    let mut app = super::EditorApp::default();
    app.set_next_import_obj_dialog_path_for_test(None);

    app.run_ui_action(super::EditorUiAction::ImportObjDialog);

    assert!(app.asset_manifest.assets.is_empty());
    assert!(app.model.selected().is_none());
}

#[test]
fn ui_action_create_duplicate_delete_undo_redo_use_model_state() {
    let mut app = super::EditorApp::default();

    app.run_ui_action(super::EditorUiAction::CreateCube);
    let first = app
        .model
        .selected()
        .cloned()
        .expect("created cube selected");
    assert!(app.model.world().entity(first.as_str()).is_some());

    app.run_ui_action(super::EditorUiAction::DuplicateSelection);
    let duplicate = app.model.selected().cloned().expect("duplicate selected");
    assert_ne!(first, duplicate);
    assert!(app.model.world().entity(duplicate.as_str()).is_some());

    app.run_ui_action(super::EditorUiAction::DeleteSelection);
    assert!(app.model.world().entity(duplicate.as_str()).is_none());

    app.run_ui_action(super::EditorUiAction::Undo);
    assert!(app.model.world().entity(duplicate.as_str()).is_some());

    app.run_ui_action(super::EditorUiAction::Redo);
    assert!(app.model.world().entity(duplicate.as_str()).is_none());
}

#[test]
fn ui_action_fit_view_sets_one_shot_request() {
    let mut app = super::EditorApp::default();

    assert!(!app.fit_view_requested);

    app.run_ui_action(super::EditorUiAction::FitView);

    assert!(app.fit_view_requested);
    assert_eq!(app.status, "Fit view requested");
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

#[test]
fn keyboard_shortcuts_allowed_is_false_when_widget_has_keyboard_focus() {
    let context = egui::Context::default();

    assert!(super::EditorApp::keyboard_shortcuts_allowed(&context));

    context.memory_mut(|memory| memory.request_focus(egui::Id::new("name_edit")));

    assert!(!super::EditorApp::keyboard_shortcuts_allowed(&context));
}

#[test]
fn app_source_keeps_global_modified_shortcuts_outside_focus_guard() {
    let source = include_str!("../app.rs");

    assert!(source.contains("fn handle_keyboard_shortcuts"));
    assert!(source.contains("EditorUiAction::SaveScene"));
    assert!(source.contains("EditorUiAction::Undo"));
    assert!(source.contains("EditorUiAction::Redo"));
    assert!(source.contains("keyboard_shortcuts_allowed(context)"));
}

#[test]
fn modified_shortcuts_are_checked_before_plain_shortcuts() {
    let source = include_str!("../app.rs");
    let shortcut_source = &source[source
        .find("fn handle_keyboard_shortcuts")
        .expect("shortcut helper present")..];
    let save_as = shortcut_source
        .find("EditorUiAction::SaveSceneAsDialog")
        .expect("Save As shortcut present");
    let save = shortcut_source
        .find("EditorUiAction::SaveScene)")
        .expect("Save shortcut present");
    let redo = shortcut_source
        .find("EditorUiAction::Redo")
        .expect("Redo shortcut present");
    let undo = shortcut_source
        .find("EditorUiAction::Undo")
        .expect("Undo shortcut present");

    assert!(save_as < save);
    assert!(redo < undo);
}

#[test]
fn menu_bar_source_contains_expected_top_level_menus() {
    let source = include_str!("panels.rs");

    assert!(source.contains("draw_menu_bar"));
    assert!(source.contains("\"File\""));
    assert!(source.contains("\"Edit\""));
    assert!(source.contains("\"Create\""));
    assert!(source.contains("\"View\""));
    assert!(source.contains("EditorUiAction::NewScene"));
    assert!(source.contains("EditorUiAction::FitView"));
}

#[test]
fn editor_app_draws_menu_before_toolbar() {
    let source = include_str!("../app.rs");
    let menu_index = source.find("editor_menu_bar").expect("menu panel present");
    let toolbar_index = source
        .find("editor_toolbar")
        .expect("toolbar panel present");

    assert!(menu_index < toolbar_index);
}

#[test]
fn toolbar_source_uses_polish_groups_and_no_toolbar_path_label() {
    let source = include_str!("panels.rs");

    assert!(source.contains("\"File\""));
    assert!(source.contains("\"Edit\""));
    assert!(source.contains("\"Create\""));
    assert!(source.contains("\"Transform\""));
    assert!(source.contains("\"View\""));
    assert!(source.contains("\"State\""));
    assert!(!source.contains("ui.label(\"Path\")"));
}

#[test]
fn status_bar_contains_read_only_file_status() {
    let source = include_str!("panels.rs");

    assert!(source.contains("\"No file\""));
    assert!(!source.contains("TextEdit::singleline(&mut self.path_input)"));
    assert!(source.contains("status_bar_selection_text"));
}

#[test]
fn editor_body_uses_resizable_side_panels_with_polish_widths() {
    let source = include_str!("panels.rs");

    assert!(source.contains("side_panel_layout(ui.available_width())"));
    assert!(!source.contains("default_size(240.0)"));
    assert!(!source.contains("default_size(340.0)"));
    assert!(source.contains(".resizable(true)"));
}

#[test]
fn side_panel_resize_ranges_are_not_fixed_pixels() {
    let source = include_str!("panels.rs");

    assert!(!source.contains("size_range(160.0..=520.0)"));
    assert!(!source.contains("size_range(240.0..=720.0)"));
}

#[test]
fn proportional_side_panel_layout_keeps_viewport_minimum() {
    for width in [640.0, 760.0, 960.0, 1280.0, 2560.0] {
        let layout = super::panels::side_panel_layout(width);

        assert!(layout.hierarchy_min <= layout.hierarchy_default);
        assert!(layout.hierarchy_default <= layout.hierarchy_max);
        assert!(layout.inspector_min <= layout.inspector_default);
        assert!(layout.inspector_default <= layout.inspector_max);
        assert!(layout.hierarchy_default + layout.inspector_default + layout.viewport_min <= width);

        let remaining_after_hierarchy_max = width - layout.hierarchy_max;
        let inspector_max_after_hierarchy = layout
            .inspector_max
            .min((remaining_after_hierarchy_max - layout.viewport_min).max(0.0));
        assert!(
            layout.hierarchy_max + inspector_max_after_hierarchy + layout.viewport_min <= width
        );
    }

    let compact = super::panels::side_panel_layout(960.0);
    let wide = super::panels::side_panel_layout(1920.0);

    assert!(compact.hierarchy_default < wide.hierarchy_default);
    assert!(compact.inspector_default < wide.inspector_default);
}

#[test]
fn resizable_side_panel_contents_take_available_width() {
    let source = include_str!("panels.rs");

    assert_eq!(source.matches("ui.take_available_width();").count(), 2);
}

#[test]
fn app_installs_compact_dark_tool_style() {
    let source = include_str!("../app.rs");

    assert!(source.contains("install_editor_style"));
    assert!(source.contains("egui::Visuals::dark()"));
    assert!(source.contains("panel_fill"));
}

fn temp_scene_path(name: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp")
        .join(format!("{name}_{}.scene.ron", std::process::id()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let _ = std::fs::remove_file(&path);
    path
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/editor_asset_tests")
        .join(format!("{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/imported")).unwrap();
    root
}

fn write_triangle_obj(path: &std::path::Path) {
    std::fs::write(path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
}

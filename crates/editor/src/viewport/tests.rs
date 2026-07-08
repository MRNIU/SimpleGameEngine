// Copyright The SimpleGameEngine Contributors

use super::{
    GizmoDrag, GizmoHandle, GizmoHandleRect, GizmoMode, TransformGizmoState, ViewCamera,
    ViewMoveInput, ViewportAction, ViewportWgpuProbe, hit_test_viewport_draw,
    screen_position_for_vertex,
};
use ecs::EntityId;
use math::{Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportMeshSpan, ViewportVertex};

fn draw_with_two_mesh_spans() -> ViewportDrawCall {
    ViewportDrawCall {
        label: "primitive:cube".to_owned(),
        camera_entity: EntityId::new("editor_view"),
        vertex_count: 8,
        index_count: 12,
        vertices: vec![
            ViewportVertex {
                position: [-0.8, -0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [-0.4, -0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [-0.4, 0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [-0.8, 0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.4, -0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.8, -0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.8, 0.2, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.4, 0.2, 0.0],
                color: [1.0; 4],
            },
        ],
        indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        mesh_spans: vec![
            ViewportMeshSpan {
                entity: EntityId::new("cube"),
                vertex_range: 0..4,
                index_range: 0..6,
            },
            ViewportMeshSpan {
                entity: EntityId::new("cube_1"),
                vertex_range: 4..8,
                index_range: 6..12,
            },
        ],
    }
}

#[test]
fn viewport_wgpu_probe_requires_prepare_and_paint() {
    let probe = ViewportWgpuProbe::default();

    assert!(!probe.report().completed);

    probe.mark_prepared();
    assert!(!probe.report().completed);

    probe.mark_painted();
    let report = probe.report();

    assert!(report.completed);
    assert_eq!(report.prepare_count, 1);
    assert_eq!(report.paint_count, 1);
}

#[test]
fn viewport_canvas_keeps_nonzero_paint_area() {
    assert_eq!(
        super::viewport_canvas_size(egui::vec2(0.0, 0.0)),
        egui::vec2(240.0, 180.0)
    );
    assert_eq!(
        super::viewport_canvas_size(egui::vec2(320.0, 240.0)),
        egui::vec2(320.0, 240.0)
    );
}

#[test]
fn view_camera_clamps_pitch_and_speed() {
    let mut camera = ViewCamera::default();

    camera.adjust_speed(10.0);
    assert!(camera.speed() >= 1.5);

    camera.look(egui::vec2(0.0, 20_000.0));
    camera.adjust_speed(-10_000.0);
    assert!(camera.pitch().is_finite());
    assert!(camera.pitch() >= ViewCamera::MIN_PITCH);
    assert_eq!(camera.speed(), ViewCamera::MIN_SPEED);

    camera.look(egui::vec2(0.0, -20_000.0));
    camera.adjust_speed(10_000.0);
    assert!(camera.pitch() <= ViewCamera::MAX_PITCH);
    assert_eq!(camera.speed(), ViewCamera::MAX_SPEED);
}

#[test]
fn view_camera_movement_changes_editor_only_view() {
    let mut camera = ViewCamera::default();
    let before = camera.to_viewport_view();

    camera.move_local(
        ViewMoveInput {
            forward: true,
            right: true,
            ..ViewMoveInput::default()
        },
        1.0,
    );
    let after = camera.to_viewport_view();
    let movement = Vec3::from_array(after.transform.translation)
        - Vec3::from_array(before.transform.translation);

    assert_ne!(before.transform.translation, after.transform.translation);
    assert!(movement.length() >= 1.0);
    assert_eq!(after.entity, EntityId::new("editor_view"));
}

#[test]
fn hit_test_uses_entity_span_metadata() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let hit = screen_position_for_vertex(rect, draw.vertices[5].position);

    let action = hit_test_viewport_draw(&draw, rect, hit);

    assert_eq!(action, ViewportAction::Select(EntityId::new("cube_1")));
}

#[test]
fn hit_test_empty_space_clears_selection() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));

    let action = hit_test_viewport_draw(&draw, rect, egui::pos2(100.0, 100.0));

    assert_eq!(action, ViewportAction::ClearSelection);
}

#[test]
fn gizmo_layout_uses_fitted_draw_and_selected_span() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));

    let handles = super::gizmo_layout(&draw, rect, Some(&EntityId::new("cube_1")), GizmoMode::Move);

    assert_eq!(handles.len(), 3);
    assert!(
        handles
            .iter()
            .any(|handle| handle.handle == GizmoHandle::MoveX)
    );
    assert!(
        handles
            .iter()
            .any(|handle| handle.handle == GizmoHandle::MoveY)
    );
    assert!(
        handles
            .iter()
            .any(|handle| handle.handle == GizmoHandle::MoveZ)
    );
    assert!(
        handles
            .iter()
            .all(|handle| rect.expand(64.0).contains(handle.center))
    );
}

#[test]
fn gizmo_hit_test_prefers_nearest_handle() {
    let handles = vec![
        GizmoHandleRect::new(
            GizmoHandle::MoveX,
            egui::pos2(100.0, 100.0),
            egui::Vec2::X,
            20.0,
        ),
        GizmoHandleRect::new(
            GizmoHandle::MoveY,
            egui::pos2(104.0, 100.0),
            -egui::Vec2::Y,
            20.0,
        ),
    ];

    let hit = super::hit_test_gizmo(&handles, egui::pos2(103.0, 100.0));

    assert_eq!(hit, Some(GizmoHandle::MoveY));
}

#[test]
fn move_gizmo_drag_changes_only_selected_axis() {
    let start = Transform {
        translation: [1.0, 2.0, 3.0],
        ..Transform::identity()
    };
    let start_pointer = egui::pos2(10.0, 10.0);

    let moved_x = super::transform_for_gizmo_drag(
        GizmoHandle::MoveX,
        start,
        start_pointer,
        egui::pos2(60.0, 10.0),
    );
    let moved_y = super::transform_for_gizmo_drag(
        GizmoHandle::MoveY,
        start,
        start_pointer,
        egui::pos2(10.0, -40.0),
    );
    let moved_z = super::transform_for_gizmo_drag(
        GizmoHandle::MoveZ,
        start,
        start_pointer,
        egui::pos2(60.0, -40.0),
    );

    assert_eq!(moved_x.translation, [1.5, 2.0, 3.0]);
    assert_eq!(moved_y.translation, [1.0, 2.5, 3.0]);
    assert_eq!(moved_z.translation, [1.0, 2.0, 3.707_106_8]);
}

fn assert_quat_close(actual: [f32; 4], expected: [f32; 4]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 0.000_01,
            "actual {actual} did not match expected {expected}"
        );
    }
}

#[test]
fn rotate_gizmo_layout_uses_fixed_screen_axes() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));

    let handles = super::gizmo_layout(
        &draw,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Rotate,
    );

    assert_eq!(handles.len(), 3);
    let rotate_x = handles
        .iter()
        .find(|handle| handle.handle == GizmoHandle::RotateX)
        .unwrap();
    let rotate_y = handles
        .iter()
        .find(|handle| handle.handle == GizmoHandle::RotateY)
        .unwrap();
    let rotate_z = handles
        .iter()
        .find(|handle| handle.handle == GizmoHandle::RotateZ)
        .unwrap();
    assert_eq!(rotate_x.axis, egui::Vec2::X);
    assert_eq!(rotate_y.axis, -egui::Vec2::Y);
    let expected_z_axis =
        (egui::Vec2::X - egui::Vec2::Y) / (egui::Vec2::X - egui::Vec2::Y).length();
    assert_eq!(rotate_z.axis, expected_z_axis);
    assert_eq!(rotate_x.rect.size(), egui::vec2(10.0, 10.0));
}

#[test]
fn rotate_gizmo_drag_changes_only_rotation_with_fixed_signs() {
    let start = Transform {
        translation: [1.0, 2.0, 3.0],
        scale: [2.0, 3.0, 4.0],
        ..Transform::identity()
    };
    let start_pointer = egui::pos2(10.0, 10.0);

    let rotated_x = super::transform_for_gizmo_drag(
        GizmoHandle::RotateX,
        start,
        start_pointer,
        egui::pos2(60.0, 10.0),
    );
    let rotated_y = super::transform_for_gizmo_drag(
        GizmoHandle::RotateY,
        start,
        start_pointer,
        egui::pos2(10.0, -40.0),
    );
    let rotated_z = super::transform_for_gizmo_drag(
        GizmoHandle::RotateZ,
        start,
        start_pointer,
        egui::pos2(60.0, -40.0),
    );
    let reverse_z = super::transform_for_gizmo_drag(
        GizmoHandle::RotateZ,
        start,
        start_pointer,
        egui::pos2(-40.0, 60.0),
    );

    assert_eq!(rotated_x.translation, start.translation);
    assert_eq!(rotated_x.scale, start.scale);
    assert_quat_close(rotated_x.rotation, Quat::from_rotation_x(0.5).to_array());
    assert_quat_close(rotated_y.rotation, Quat::from_rotation_y(0.5).to_array());
    assert_quat_close(
        rotated_z.rotation,
        Quat::from_rotation_z(0.707_106_77).to_array(),
    );
    assert_quat_close(
        reverse_z.rotation,
        Quat::from_rotation_z(-0.707_106_77).to_array(),
    );
}

#[test]
fn rotate_gizmo_drag_ignores_non_finite_pointer_delta() {
    let start = Transform::identity();

    let rotated = super::transform_for_gizmo_drag(
        GizmoHandle::RotateZ,
        start,
        egui::pos2(0.0, 0.0),
        egui::pos2(f32::NAN, 10.0),
    );

    assert_eq!(rotated, start);
}

#[test]
fn uniform_scale_drag_changes_all_scale_axes_and_clamps_minimum() {
    let start = Transform {
        scale: [1.0, 2.0, 3.0],
        ..Transform::identity()
    };
    let start_pointer = egui::pos2(10.0, 10.0);

    let grown = super::transform_for_gizmo_drag(
        GizmoHandle::UniformScale,
        start,
        start_pointer,
        egui::pos2(60.0, -40.0),
    );
    let clamped = super::transform_for_gizmo_drag(
        GizmoHandle::UniformScale,
        start,
        start_pointer,
        egui::pos2(-200.0, 220.0),
    );

    assert_eq!(grown.scale, [1.707_106_8, 2.707_106_8, 3.707_106_8]);
    assert_eq!(clamped.scale, [0.01, 0.01, 0.01]);
}

#[test]
fn gizmo_state_stores_and_clears_drag_target() {
    let mut state = TransformGizmoState::default();
    let drag = GizmoDrag {
        target: EntityId::new("cube"),
        handle: GizmoHandle::MoveX,
        start_pointer: egui::pos2(10.0, 10.0),
        start_transform: Transform::identity(),
    };

    state.start_drag(drag.clone());
    assert_eq!(state.drag(), Some(&drag));

    state.clear_drag();
    assert_eq!(state.drag(), None);
}

#[test]
fn gizmo_state_tracks_hovered_and_active_handle() {
    let mut state = TransformGizmoState::default();
    assert_eq!(state.hovered(), None);
    assert_eq!(state.active(), None);

    state.set_hovered(Some(GizmoHandle::MoveX));
    state.start_drag(GizmoDrag {
        target: EntityId::new("cube"),
        handle: GizmoHandle::MoveY,
        start_pointer: egui::pos2(0.0, 0.0),
        start_transform: Transform::identity(),
    });
    state.sync_active_from_drag();

    assert_eq!(state.hovered(), Some(GizmoHandle::MoveX));
    assert_eq!(state.active(), Some(GizmoHandle::MoveY));
}

#[test]
fn gizmo_drag_starts_from_press_origin_before_drag_threshold() {
    let target = EntityId::new("cube");
    let start_transform = Transform::from_translation([1.0, 2.0, 3.0]);
    let handles = vec![GizmoHandleRect::new(
        GizmoHandle::MoveX,
        egui::pos2(100.0, 100.0),
        egui::Vec2::X,
        10.0,
    )];

    let drag = super::gizmo_drag_from_press_origin(
        &handles,
        Some(egui::pos2(100.0, 100.0)),
        Some(&target),
        Some(start_transform),
    );

    assert_eq!(
        drag,
        Some(GizmoDrag {
            target,
            handle: GizmoHandle::MoveX,
            start_pointer: egui::pos2(100.0, 100.0),
            start_transform,
        })
    );
}

#[test]
fn viewport_transform_actions_distinguish_preview_commit_and_restore() {
    let target = EntityId::new("cube");
    let before = Transform::identity();
    let after = Transform::from_translation([1.0, 2.0, 3.0]);

    assert_eq!(
        ViewportAction::PreviewTransform {
            target: target.clone(),
            transform: after,
        },
        ViewportAction::PreviewTransform {
            target: EntityId::new("cube"),
            transform: Transform::from_translation([1.0, 2.0, 3.0]),
        }
    );
    assert_eq!(
        ViewportAction::CommitTransform {
            target: target.clone(),
            before,
            after,
        },
        ViewportAction::CommitTransform {
            target: EntityId::new("cube"),
            before: Transform::identity(),
            after: Transform::from_translation([1.0, 2.0, 3.0]),
        }
    );
    assert_eq!(
        ViewportAction::RestoreTransform {
            target,
            transform: before,
        },
        ViewportAction::RestoreTransform {
            target: EntityId::new("cube"),
            transform: Transform::identity(),
        }
    );
}

#[test]
fn fit_visible_draw_keeps_camera_finite() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube"))));
    let view = camera.to_viewport_view();

    assert!(view.transform.translation.into_iter().all(f32::is_finite));
    assert!(view.transform.rotation.into_iter().all(f32::is_finite));
}

#[test]
fn fit_visible_draw_pans_edge_selection_toward_center() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let view = camera.to_viewport_view();

    assert!((view.transform.translation[0] - 5.0).abs() < 0.1);
}

#[test]
fn fit_visible_draw_without_selection_centers_all_visible_cubes() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, None));
    let view = camera.to_viewport_view();

    assert!(view.transform.translation[0].abs() < 0.1);
}

#[test]
fn draw_viewport_signature_accepts_keyboard_and_fit_guards() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("keyboard_shortcuts_allowed: bool"));
    assert!(source.contains("fit_view_requested: bool"));
    assert!(source.contains("let fit_requested = fit_view_requested || keyboard_fit_requested"));
}

#[test]
fn viewport_keyboard_fit_uses_shortcut_guard_without_hover_gate() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("keyboard_shortcuts_allowed && f_pressed"));
    assert!(!source.contains("response.hovered() && keyboard_shortcuts_allowed && f_pressed"));
}

#[test]
fn viewport_right_button_navigation_does_not_capture_keyboard_focus() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("right_down && response.hovered()"));
    assert!(!source.contains("response.request_focus()"));
}

#[test]
fn viewport_source_draws_camera_mode_overlay() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("view_mode_label"));
    assert!(source.contains("Editor Camera"));
    assert!(source.contains("Pilot Camera"));
}

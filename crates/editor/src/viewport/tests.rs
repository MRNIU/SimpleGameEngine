// Copyright The SimpleGameEngine Contributors

use super::{
    GizmoDrag, GizmoHandle, GizmoHandleRect, GizmoMode, TransformGizmoState, ViewCamera,
    ViewMoveInput, ViewportAction, ViewportWgpuProbe, hit_test_viewport_draw,
    screen_position_for_vertex,
};
use ecs::EntityId;
use math::{Quat, Transform, Vec3};
use render::{
    ViewportClipPlanes, ViewportDrawCall, ViewportMeshSpan, ViewportProjection, ViewportSize,
    ViewportVertex, ViewportView,
};

use super::grid::{GridPlane, adaptive_grid_lines, grid_plane_for_preset, grid_step_for_spacing};

fn test_projection() -> ViewportProjection {
    let view = ViewportView::new(
        EntityId::new("test_camera"),
        Transform::from_translation([0.0, 0.0, -4.0]),
        ecs::Projection::Perspective {
            fov_y_degrees: 90.0,
        },
    );
    ViewportProjection::from_view(
        &view,
        ViewportSize::new(200.0, 200.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
}

#[test]
fn adaptive_grid_uses_decimal_steps_and_hysteresis() {
    assert_eq!(grid_step_for_spacing(15.0, 1.0), 10.0);
    assert_eq!(grid_step_for_spacing(20.0, 10.0), 10.0);
    assert_eq!(grid_step_for_spacing(180.0, 10.0), 1.0);
}

#[test]
fn perspective_grid_lines_are_projectable_after_clipping() {
    let projection = test_projection();
    let frame = adaptive_grid_lines(&projection, GridPlane::XY, 1.0).unwrap();

    assert!(frame.lines.len() <= 512);
    assert!(frame.lines.iter().all(|line| {
        projection
            .project_world_segment(line.start, line.end)
            .is_some()
    }));
}

#[test]
fn orthographic_side_view_selects_matching_grid_plane() {
    assert_eq!(
        grid_plane_for_preset(super::ViewPreset::Front),
        GridPlane::YZ
    );
    assert_eq!(
        grid_plane_for_preset(super::ViewPreset::Right),
        GridPlane::XZ
    );
}

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
                world_bounds_min: [-0.8, -0.2, 0.0],
                world_bounds_max: [-0.4, 0.2, 0.0],
                world_center: [-0.6, 0.0, 0.0],
            },
            ViewportMeshSpan {
                entity: EntityId::new("cube_1"),
                vertex_range: 4..8,
                index_range: 6..12,
                world_bounds_min: [0.4, -0.2, 0.0],
                world_bounds_max: [0.8, 0.2, 0.0],
                world_center: [0.6, 0.0, 0.0],
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
fn viewport_canvas_does_not_exceed_positive_available_space() {
    assert_eq!(
        super::viewport_canvas_size(egui::vec2(120.0, 90.0)),
        egui::vec2(120.0, 90.0)
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

fn translation_delta_after_move(input: ViewMoveInput) -> Vec3 {
    let mut camera = ViewCamera::default();
    let before = Vec3::from_array(camera.to_viewport_view().transform.translation);

    camera.move_local(input, 1.0);
    let after = Vec3::from_array(camera.to_viewport_view().transform.translation);

    after - before
}

#[test]
fn fly_navigation_maps_wasd_to_viewport_axes() {
    let camera = ViewCamera::default();
    let (forward, right, _) = camera.basis();
    let forward = Vec3::from_array(forward);
    let right = Vec3::from_array(right);
    let right_movement = translation_delta_after_move(ViewMoveInput {
        right: true,
        ..ViewMoveInput::default()
    });
    let forward_movement = translation_delta_after_move(ViewMoveInput {
        forward: true,
        ..ViewMoveInput::default()
    });

    assert_vec3_finite(right_movement);
    assert_vec3_finite(forward_movement);
    assert!(
        right_movement.normalize().dot(right.normalize()) > 0.99,
        "D should move along the viewport right axis, got {right_movement:?}"
    );
    assert!(
        forward_movement.normalize().dot(forward.normalize()) > 0.99,
        "W should move along the viewport forward axis, got {forward_movement:?}"
    );
}

fn projected_origin_delta_after_look(delta: egui::Vec2) -> egui::Vec2 {
    let mut camera = ViewCamera::default();
    let before_view = camera.to_viewport_view();
    let before = ViewportProjection::from_view(
        &before_view,
        ViewportSize::new(800.0, 600.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
    .project_world_point([0.0, 0.0, 0.0])
    .unwrap();

    camera.look(delta);
    let after_view = camera.to_viewport_view();
    let after = ViewportProjection::from_view(
        &after_view,
        ViewportSize::new(800.0, 600.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
    .project_world_point([0.0, 0.0, 0.0])
    .unwrap();

    egui::vec2(after[0] - before[0], after[1] - before[1])
}

fn vec3_from_array(value: [f32; 3]) -> Vec3 {
    Vec3::from_array(value)
}

fn assert_vec3_finite(value: Vec3) {
    assert!(value.is_finite(), "vector must be finite: {value:?}");
}

fn assert_not_collinear(left: Vec3, right: Vec3) {
    assert!(
        left.normalize().cross(right.normalize()).length() > 0.000_1,
        "vectors must not be collinear: left={left:?}, right={right:?}"
    );
}

#[test]
fn right_mouse_look_maps_pointer_axes_to_screen_axes() {
    let horizontal = projected_origin_delta_after_look(egui::vec2(40.0, 0.0));
    let vertical = projected_origin_delta_after_look(egui::vec2(0.0, 40.0));

    assert!(
        horizontal.x.abs() > horizontal.y.abs() * 2.0,
        "horizontal RMB drag should mostly move the view horizontally, got {horizontal:?}"
    );
    assert!(
        vertical.y.abs() > vertical.x.abs() * 2.0,
        "vertical RMB drag should mostly move the view vertically, got {vertical:?}"
    );
}

#[test]
fn look_and_orbit_pointer_axes_have_fixed_signs() {
    let mut look = ViewCamera::default();
    let yaw = look.yaw();
    let pitch = look.pitch();

    look.look(egui::vec2(25.0, 0.0));
    assert!(look.yaw() > yaw, "drag right should increase yaw");
    assert_eq!(look.pitch(), pitch);

    let yaw = look.yaw();
    look.look(egui::vec2(-25.0, 0.0));
    assert!(look.yaw() < yaw, "drag left should decrease yaw");

    let pitch = look.pitch();
    look.look(egui::vec2(0.0, -25.0));
    assert!(look.pitch() > pitch, "drag up should increase pitch");

    let pitch = look.pitch();
    look.look(egui::vec2(0.0, 25.0));
    assert!(look.pitch() < pitch, "drag down should decrease pitch");

    let mut orbit = ViewCamera::default();
    let draw = draw_with_two_mesh_spans();
    assert!(orbit.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let pivot = orbit.orbit_pivot();
    let yaw = orbit.yaw();
    let pitch = orbit.pitch();

    orbit.orbit(egui::vec2(25.0, -25.0));
    assert!(orbit.yaw() > yaw);
    assert!(orbit.pitch() > pitch);
    assert_eq!(orbit.orbit_pivot(), pivot);
}

#[test]
fn view_camera_basis_is_z_up_and_non_degenerate() {
    let camera = ViewCamera::default();
    let (forward, right, up) = camera.basis();
    let forward = vec3_from_array(forward);
    let right = vec3_from_array(right);
    let up = vec3_from_array(up);

    assert_vec3_finite(forward);
    assert_vec3_finite(right);
    assert_vec3_finite(up);
    assert_not_collinear(forward, right);
    assert_not_collinear(forward, up);
    assert_not_collinear(right, up);
    assert!(
        forward.dot(Vec3::X) > 0.0,
        "default forward should face world +X: {forward:?}"
    );
    assert!(
        right.dot(Vec3::Y) > 0.0,
        "default right should face world +Y: {right:?}"
    );
    assert!(
        up.dot(Vec3::Z) > 0.0,
        "default up should face world +Z: {up:?}"
    );
}

#[test]
fn orthographic_presets_are_finite_and_named() {
    let mut camera = ViewCamera::default();

    for preset in [
        super::ViewPreset::Top,
        super::ViewPreset::Bottom,
        super::ViewPreset::Front,
        super::ViewPreset::Back,
        super::ViewPreset::Right,
        super::ViewPreset::Left,
    ] {
        camera.set_preset(preset);
        let view = camera.to_viewport_view();
        assert!(view.transform.translation.into_iter().all(f32::is_finite));
        assert!(view.transform.rotation.into_iter().all(f32::is_finite));
        assert!(camera.view_mode_label().contains("Orthographic"));
    }
}

#[test]
fn right_mouse_navigation_from_orthographic_returns_to_perspective() {
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    let before = camera.to_viewport_view();

    camera.look(egui::vec2(20.0, 0.0));
    camera.move_local(
        ViewMoveInput {
            forward: true,
            ..ViewMoveInput::default()
        },
        1.0,
    );
    let after = camera.to_viewport_view();

    assert_eq!(camera.view_mode_label(), "Perspective");
    assert_ne!(before.transform.translation, after.transform.translation);
}

#[test]
fn camera_hint_uses_world_metrics_for_distance() {
    let mut draw = draw_with_two_mesh_spans();
    draw.mesh_spans[0].world_center = [0.0, 0.0, 0.0];
    draw.mesh_spans[1].world_center = [0.0, 3.0, 4.0];
    let camera = ViewCamera::default();

    let hint = camera.hint_text(Some(&draw), Some(&EntityId::new("cube_1")));

    assert!(hint.contains("Camera Speed"));
    assert!(hint.contains("Distance"));
    assert!(hint.contains("9.43"));
}

#[test]
fn pilot_camera_hint_uses_scene_view_data_not_editor_camera_speed() {
    let draw = draw_with_two_mesh_spans();
    let view = render::ViewportView::new(
        EntityId::new("camera"),
        Transform::from_translation([0.0, 0.0, 10.0]),
        ecs::Projection::Perspective {
            fov_y_degrees: 42.0,
        },
    );

    let hint = super::pilot_camera_hint_text(&view, Some(&draw), Some(&EntityId::new("cube_1")));

    assert!(hint.contains("Pilot Camera"));
    assert!(hint.contains("Perspective FOV 42.0"));
    assert!(hint.contains("Distance"));
    assert!(hint.contains("Meshes 2"));
    assert!(!hint.contains("Camera Speed"));
}

#[test]
fn orthographic_fit_keeps_selected_mesh_from_filling_viewport() {
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    let mut draw = draw_with_two_mesh_spans();
    draw.mesh_spans[1].world_bounds_min = [0.0, 0.0, 0.0];
    draw.mesh_spans[1].world_bounds_max = [1.0, 1.0, 1.0];
    draw.mesh_spans[1].world_center = [0.5, 0.5, 0.5];

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let view = camera.to_viewport_view();
    let ecs::Projection::Orthographic { vertical_size } = view.projection else {
        panic!("expected orthographic projection");
    };

    assert!(vertical_size >= 2.0);
}

#[test]
fn frame_selected_mesh_updates_perspective_pivot_and_distance() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let view = camera.to_viewport_view();
    let pivot = Vec3::from_array(camera.orbit_pivot());
    let position = Vec3::from_array(view.transform.translation);

    assert_eq!(camera.orbit_pivot(), [0.6, 0.0, 0.0]);
    assert!(camera.orbit_distance().is_finite());
    assert!(camera.orbit_distance() >= ViewCamera::MIN_ORBIT_DISTANCE);
    assert!((pivot.distance(position) - camera.orbit_distance()).abs() < 0.000_1);
}

#[test]
fn frame_empty_scene_uses_origin_and_default_distance() {
    let mut draw = draw_with_two_mesh_spans();
    draw.mesh_spans.clear();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("missing"))));

    assert_eq!(camera.orbit_pivot(), [0.0, 0.0, 0.0]);
    assert!(camera.orbit_distance().is_finite());
    assert!(camera.orbit_distance() >= ViewCamera::MIN_ORBIT_DISTANCE);
}

#[test]
fn dolly_changes_distance_without_changing_fov() {
    let draw = draw_with_two_mesh_spans();
    let entity = EntityId::new("cube_1");
    let mut camera = ViewCamera::default();
    assert!(camera.fit_draw(&draw, Some(&entity)));
    let before_distance = camera.orbit_distance();
    let before_fov = camera.fov_y_degrees();

    camera.dolly(-20.0);

    assert!(camera.orbit_distance() < before_distance);
    assert_eq!(camera.fov_y_degrees(), before_fov);
}

#[test]
fn orbit_pan_and_dolly_keep_pivot_contract() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();
    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let pivot = camera.orbit_pivot();
    let position = camera.to_viewport_view().transform.translation;

    camera.orbit(egui::vec2(80.0, -20.0));
    let orbit_position = camera.to_viewport_view().transform.translation;
    assert_eq!(camera.orbit_pivot(), pivot);
    assert_ne!(orbit_position, position);

    camera.pan(egui::vec2(12.0, -6.0));
    let panned_pivot = camera.orbit_pivot();
    let panned_position = camera.to_viewport_view().transform.translation;
    assert_ne!(panned_pivot, pivot);
    assert_ne!(panned_position, orbit_position);

    let before_dolly_distance = camera.orbit_distance();
    camera.dolly(-20.0);
    assert!(camera.orbit_distance() < before_dolly_distance);
    assert!(camera.orbit_distance() >= ViewCamera::MIN_ORBIT_DISTANCE);
}

#[test]
fn orthographic_fit_uses_world_metrics_for_center_and_scale() {
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    let mut draw = draw_with_two_mesh_spans();
    draw.mesh_spans[1].world_bounds_min = [10.0, 20.0, 0.0];
    draw.mesh_spans[1].world_bounds_max = [14.0, 26.0, 2.0];
    draw.mesh_spans[1].world_center = [12.0, 23.0, 1.0];

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let view = camera.to_viewport_view();

    assert!(view.transform.translation.into_iter().all(f32::is_finite));
    assert!(
        camera
            .hint_text(Some(&draw), Some(&EntityId::new("cube_1")))
            .contains("Ortho Scale")
    );
}

#[test]
fn hit_test_uses_entity_span_metadata() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let projection = test_projection();
    let projected = projection
        .project_world_point(draw.mesh_spans[1].world_center)
        .unwrap();
    let hit = screen_position_for_vertex(rect, [projected[0], projected[1], 0.0]);

    let action = hit_test_viewport_draw(&draw, &projection, rect, hit);

    assert_eq!(action, ViewportAction::Select(EntityId::new("cube_1")));
}

#[test]
fn hit_test_empty_space_clears_selection() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let projection = test_projection();

    let action = hit_test_viewport_draw(&draw, &projection, rect, egui::pos2(100.0, 100.0));

    assert_eq!(action, ViewportAction::ClearSelection);
}

#[test]
fn hit_test_selects_nearest_world_triangle() {
    let draw = ViewportDrawCall {
        label: "overlap".to_owned(),
        camera_entity: EntityId::new("test_camera"),
        vertex_count: 6,
        index_count: 6,
        vertices: vec![
            ViewportVertex {
                position: [-1.0, -1.0, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [1.0, -1.0, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.0, 1.0, 0.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [-1.0, -1.0, 2.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [1.0, -1.0, 2.0],
                color: [1.0; 4],
            },
            ViewportVertex {
                position: [0.0, 1.0, 2.0],
                color: [1.0; 4],
            },
        ],
        indices: vec![0, 1, 2, 3, 4, 5],
        mesh_spans: vec![
            ViewportMeshSpan {
                entity: EntityId::new("near"),
                vertex_range: 0..3,
                index_range: 0..3,
                world_bounds_min: [-1.0, -1.0, 0.0],
                world_bounds_max: [1.0, 1.0, 0.0],
                world_center: [0.0, 0.0, 0.0],
            },
            ViewportMeshSpan {
                entity: EntityId::new("far"),
                vertex_range: 3..6,
                index_range: 3..6,
                world_bounds_min: [-1.0, -1.0, 2.0],
                world_bounds_max: [1.0, 1.0, 2.0],
                world_center: [0.0, 0.0, 2.0],
            },
        ],
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 200.0));

    assert_eq!(
        hit_test_viewport_draw(&draw, &test_projection(), rect, rect.center()),
        ViewportAction::Select(EntityId::new("near"))
    );
}

#[test]
fn reference_grid_uses_xy_plane_and_z_axis_marker() {
    let frame = adaptive_grid_lines(&test_projection(), GridPlane::XY, 1.0).unwrap();
    let lines = frame.lines;

    assert!(lines.iter().any(|line| {
        line.start[2] == 0.0
            && line.end[2] == 0.0
            && line.color == egui::Color32::from_rgb(160, 60, 60)
    }));
    assert!(lines.iter().any(|line| {
        line.start == [0.0, 0.0, 0.0]
            && line.end[2] > 0.0
            && line.color == egui::Color32::from_rgb(80, 130, 240)
    }));
}

#[test]
fn orientation_cube_hit_test_returns_presets_and_perspective() {
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let layout = super::orientation_cube_layout(rect);

    assert_eq!(
        super::orientation_cube_hit_test(&layout, layout.top.center()),
        Some(ViewportAction::SetViewPreset(super::ViewPreset::Top))
    );
    assert_eq!(
        super::orientation_cube_hit_test(&layout, layout.perspective.center()),
        Some(ViewportAction::ReturnToPerspective)
    );
}

#[test]
fn orientation_overlay_consumes_before_scene_selection() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let layout = super::orientation_cube_layout(rect);
    let pointer = layout.top.center();
    let projection = test_projection();

    let overlay = super::orientation_cube_hit_test(&layout, pointer);
    let selection = hit_test_viewport_draw(&draw, &projection, rect, pointer);

    assert!(matches!(overlay, Some(ViewportAction::SetViewPreset(_))));
    assert_ne!(overlay, Some(selection));
}

#[test]
fn move_z_gizmo_uses_screen_up_axis() {
    let start = Transform {
        translation: [1.0, 2.0, 3.0],
        ..Transform::identity()
    };

    let moved = super::transform_for_gizmo_drag(
        GizmoHandle::MoveZ,
        start,
        egui::pos2(10.0, 10.0),
        egui::pos2(10.0, -40.0),
    );

    assert_eq!(moved.translation, [1.0, 2.0, 3.5]);
}

#[test]
fn gizmo_layout_uses_fitted_draw_and_selected_span() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let projection = test_projection();

    let handles = super::gizmo_layout(
        &draw,
        &projection,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Move,
    );

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
fn move_gizmo_layout_keeps_three_selectable_axes() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let projection = test_projection();

    let handles = super::gizmo_layout(
        &draw,
        &projection,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Move,
    );
    let axes = handles
        .iter()
        .map(|handle| format!("{:.1},{:.1}", handle.axis.x, handle.axis.y))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(axes.len(), 3);
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
        egui::pos2(10.0, 60.0),
    );
    let moved_z = super::transform_for_gizmo_drag(
        GizmoHandle::MoveZ,
        start,
        start_pointer,
        egui::pos2(60.0, -40.0),
    );

    assert_eq!(moved_x.translation, [1.5, 2.0, 3.0]);
    assert_eq!(moved_y.translation, [1.0, 2.5, 3.0]);
    assert_eq!(moved_z.translation, [1.0, 2.0, 3.5]);
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
    let projection = test_projection();

    let handles = super::gizmo_layout(
        &draw,
        &projection,
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
    assert_eq!(rotate_z.axis, -egui::Vec2::Y);
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
    assert_quat_close(rotated_z.rotation, Quat::from_rotation_z(0.5).to_array());
    assert_quat_close(reverse_z.rotation, Quat::from_rotation_z(-0.5).to_array());
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

    assert_eq!(grown.scale, [1.5, 2.5, 3.5]);
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
    let projection = ViewportProjection::from_view(
        &view,
        ViewportSize::new(800.0, 600.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap();
    let center = projection
        .project_world_point(draw.mesh_spans[1].world_center)
        .unwrap();

    assert!(center[0].abs() < 0.000_1);
    assert!(center[1].abs() < 0.000_1);
}

#[test]
fn fit_visible_draw_without_selection_centers_all_visible_cubes() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, None));
    let view = camera.to_viewport_view();
    let projection = ViewportProjection::from_view(
        &view,
        ViewportSize::new(800.0, 600.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap();
    let center = projection.project_world_point([0.0, 0.0, 0.0]).unwrap();

    assert!(center[0].abs() < 0.000_1);
    assert!(center[1].abs() < 0.000_1);
}

#[test]
fn draw_viewport_signature_accepts_keyboard_and_fit_guards() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("keyboard_shortcuts_allowed: bool"));
    assert!(source.contains("fit_view_requested: bool"));
    assert!(source.contains("navigation_enabled: bool"));
    assert!(source.contains("let fit_requested = fit_view_requested || keyboard_fit_requested"));
}

#[test]
fn camera_navigation_consumes_pointer_before_gizmo_and_selection() {
    assert!(super::camera_navigation_requested(
        true, true, true, false, false
    ));
    assert!(super::camera_navigation_requested(
        false, true, false, false, true
    ));
    assert!(!super::can_start_gizmo_drag(true, false, true));
    assert!(!super::can_select_viewport(true, false, true));
    assert!(super::can_start_gizmo_drag(true, false, false));
    assert!(super::can_select_viewport(true, false, false));
}

#[test]
fn active_gizmo_blocks_selection_after_camera_does_not_consume() {
    assert!(!super::can_start_gizmo_drag(true, true, false));
    assert!(!super::can_select_viewport(true, true, false));
}

#[test]
fn viewport_keyboard_fit_uses_shortcut_guard_without_hover_gate() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("keyboard_shortcuts_allowed && f_pressed"));
    assert!(!source.contains("response.hovered() && keyboard_shortcuts_allowed && f_pressed"));
}

#[test]
fn viewport_source_draws_camera_mode_overlay() {
    let source = include_str!("../viewport.rs");

    assert!(source.contains("camera.hint_text"));
}

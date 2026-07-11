// Copyright The SimpleGameEngine Contributors

use super::{
    GizmoDrag, GizmoHandle, GizmoHandleRect, GizmoMode, TransformGizmoState, ViewCamera,
    ViewMoveInput, ViewportAction, ViewportWgpuProbe, hit_test_viewport_draw,
    screen_position_for_vertex,
};
use ecs::{EntityId, Projection};
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

fn test_viewport_size() -> ViewportSize {
    ViewportSize::new(1600.0, 900.0).unwrap()
}

fn default_editor_projection() -> ViewportProjection {
    editor_projection_for_size(800.0, 600.0)
}

fn editor_projection_for_size(width: f32, height: f32) -> ViewportProjection {
    let size = ViewportSize::new(width, height).unwrap();
    ViewportProjection::from_view(
        &ViewCamera::default().to_viewport_view(size),
        size,
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
}

fn projected_screen_axis(
    projection: &ViewportProjection,
    rect: egui::Rect,
    origin: [f32; 3],
    axis: Vec3,
) -> egui::Vec2 {
    let start = projection.project_world_point(origin).unwrap();
    let end = projection
        .project_world_point((Vec3::from_array(origin) + axis).to_array())
        .unwrap();
    (screen_position_for_vertex(rect, [end[0], end[1], 0.0])
        - screen_position_for_vertex(rect, [start[0], start[1], 0.0]))
    .normalized()
}

#[test]
fn default_camera_uses_ue_fov_and_speed() {
    let camera = ViewCamera::default();

    assert_eq!(camera.horizontal_fov_degrees(), 90.0);
    assert_eq!(camera.speed_level(), 4);
    assert_eq!(camera.speed_scalar(), 1.0);
}

#[test]
fn editor_horizontal_fov_converts_to_vertical_fov() {
    let view = ViewCamera::default().to_viewport_view(test_viewport_size());
    let Projection::Perspective { fov_y_degrees } = view.projection else {
        panic!("expected perspective projection");
    };
    let expected = 2.0 * ((45.0_f32.to_radians().tan()) / (1600.0 / 900.0)).atan();

    assert!((fov_y_degrees.to_radians() - expected).abs() < 1.0e-5);
}

#[test]
fn speed_level_and_scalar_define_effective_speed() {
    let mut camera = ViewCamera::default();
    camera.set_speed_level(5);
    camera.set_speed_scalar(2.0);

    assert_eq!(camera.effective_speed(), 16.0);
    camera.adjust_speed_level(100);
    assert_eq!(camera.speed_level(), 8);
}

#[test]
fn orthographic_navigation_does_not_return_to_perspective() {
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    camera.ortho_pan(egui::vec2(20.0, -10.0));
    camera.ortho_zoom(1.0);

    assert_eq!(camera.view_mode_label(), "Top Orthographic");
}

#[test]
fn adaptive_grid_uses_decimal_steps_and_hysteresis() {
    assert_eq!(grid_step_for_spacing(3.0, 1.0), 10.0);
    assert_eq!(grid_step_for_spacing(15.0, 1.0), 1.0);
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
fn perspective_grid_survives_when_one_plane_axis_is_behind_camera() {
    let forward = Vec3::new(-1.0, 0.0, -1.0).normalize();
    let right = Vec3::NEG_Y;
    let up = forward.cross(right).normalize();
    let rotation = Quat::from_mat3(&math::Mat3::from_cols(right, up, forward));
    let view = ViewportView::new(
        EntityId::new("low_camera"),
        Transform {
            translation: [0.1, 0.0, 0.1],
            rotation: rotation.to_array(),
            scale: [1.0; 3],
        },
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );
    let size = ViewportSize::new(800.0, 600.0).unwrap();
    let projection =
        ViewportProjection::from_view(&view, size, ViewportClipPlanes::DEFAULT).unwrap();

    assert!(adaptive_grid_lines(&projection, GridPlane::XY, 1.0).is_some());
}

#[test]
fn tilted_perspective_grid_reaches_visible_viewport_edges() {
    let projection = editor_projection_for_size(384.0, 448.0);
    let frame = adaptive_grid_lines(&projection, GridPlane::XY, 1.0).unwrap();
    let line_count = frame.lines.len();
    let mut minimum = [f32::INFINITY; 2];
    let mut maximum = [f32::NEG_INFINITY; 2];
    for line in frame.lines {
        let Some(segment) = projection.project_world_segment(line.start, line.end) else {
            continue;
        };
        for point in segment {
            minimum[0] = minimum[0].min(point[0]);
            minimum[1] = minimum[1].min(point[1]);
            maximum[0] = maximum[0].max(point[0]);
            maximum[1] = maximum[1].max(point[1]);
        }
    }

    assert!(minimum[0] <= -0.95, "grid left edge: {minimum:?}");
    assert!(maximum[0] >= 0.95, "grid right edge: {maximum:?}");
    assert!(minimum[1] <= -0.95, "grid bottom edge: {minimum:?}");
    assert!(
        line_count >= 12,
        "default perspective needs a complete minor grid, got {} lines",
        line_count
    );
}

#[test]
fn perspective_grid_does_not_end_inside_visible_ground() {
    let projection = editor_projection_for_size(768.0, 914.0);
    let frame = adaptive_grid_lines(&projection, GridPlane::XY, 1.0).unwrap();
    let interior_endpoints = frame
        .lines
        .iter()
        .filter(|line| line.start[2] == 0.0 && line.end[2] == 0.0)
        .filter_map(|line| projection.project_world_segment(line.start, line.end))
        .flatten()
        .filter(|point| point[0].abs() < 0.95 && point[1].abs() < 0.95 && point[1] < 0.5)
        .count();

    assert_eq!(interior_endpoints, 0);
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
fn view_camera_clamps_pitch_and_speed_settings() {
    let mut camera = ViewCamera::default();

    camera.look(egui::vec2(0.0, 20_000.0));
    camera.set_speed_level(0);
    camera.set_speed_scalar(-10_000.0);
    assert!(camera.pitch().is_finite());
    assert!(camera.pitch() >= ViewCamera::MIN_PITCH);
    assert_eq!(camera.speed_level(), 1);
    assert_eq!(camera.speed_scalar(), 0.1);

    camera.look(egui::vec2(0.0, -20_000.0));
    camera.set_speed_level(u8::MAX);
    camera.set_speed_scalar(10_000.0);
    assert!(camera.pitch() <= ViewCamera::MAX_PITCH);
    assert_eq!(camera.speed_level(), 8);
    assert_eq!(camera.speed_scalar(), 10.0);
}

#[test]
fn view_camera_movement_changes_editor_only_view() {
    let mut camera = ViewCamera::default();
    let before = camera.to_viewport_view(test_viewport_size());

    camera.move_local(
        ViewMoveInput {
            forward: true,
            right: true,
            ..ViewMoveInput::default()
        },
        1.0,
    );
    let after = camera.to_viewport_view(test_viewport_size());
    let movement = Vec3::from_array(after.transform.translation)
        - Vec3::from_array(before.transform.translation);

    assert_ne!(before.transform.translation, after.transform.translation);
    assert!(movement.length() >= 1.0);
    assert_eq!(after.entity, EntityId::new("editor_view"));
}

fn translation_delta_after_move(input: ViewMoveInput) -> Vec3 {
    let mut camera = ViewCamera::default();
    let before = Vec3::from_array(
        camera
            .to_viewport_view(test_viewport_size())
            .transform
            .translation,
    );

    camera.move_local(input, 1.0);
    let after = Vec3::from_array(
        camera
            .to_viewport_view(test_viewport_size())
            .transform
            .translation,
    );

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

#[test]
fn fly_navigation_maps_qe_to_world_vertical_axis() {
    let up = translation_delta_after_move(ViewMoveInput {
        up: true,
        ..ViewMoveInput::default()
    });
    let down = translation_delta_after_move(ViewMoveInput {
        down: true,
        ..ViewMoveInput::default()
    });

    assert!(up.dot(Vec3::Z) > 0.0);
    assert!(down.dot(Vec3::Z) < 0.0);
}

#[test]
fn plain_wheel_moves_along_camera_forward() {
    let mut camera = ViewCamera::default();
    let before = Vec3::from_array(
        camera
            .to_viewport_view(test_viewport_size())
            .transform
            .translation,
    );
    let (forward, _, _) = camera.basis();

    camera.wheel_move(1.0);
    let after = Vec3::from_array(
        camera
            .to_viewport_view(test_viewport_size())
            .transform
            .translation,
    );

    assert!((after - before).dot(Vec3::from_array(forward)) > 0.0);
}

#[test]
fn plain_lmb_navigation_turns_and_moves_camera() {
    let mut camera = ViewCamera::default();
    let before_yaw = camera.yaw();
    let before = camera
        .to_viewport_view(test_viewport_size())
        .transform
        .translation;

    camera.lmb_navigate(egui::vec2(20.0, 10.0));

    assert_ne!(camera.yaw(), before_yaw);
    assert_ne!(
        camera
            .to_viewport_view(test_viewport_size())
            .transform
            .translation,
        before
    );
}

fn projected_origin_delta_after_look(delta: egui::Vec2) -> egui::Vec2 {
    let mut camera = ViewCamera::default();
    let before_view = camera.to_viewport_view(test_viewport_size());
    let before = ViewportProjection::from_view(
        &before_view,
        ViewportSize::new(800.0, 600.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
    .project_world_point([0.0, 0.0, 0.0])
    .unwrap();

    camera.look(delta);
    let after_view = camera.to_viewport_view(test_viewport_size());
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
        let view = camera.to_viewport_view(test_viewport_size());
        assert!(view.transform.translation.into_iter().all(f32::is_finite));
        assert!(view.transform.rotation.into_iter().all(f32::is_finite));
        let projection =
            ViewportProjection::from_view(&view, test_viewport_size(), ViewportClipPlanes::DEFAULT)
                .unwrap();
        assert!(
            projection.project_world_point([0.0, 0.0, 0.0]).is_some(),
            "{preset:?} must keep the world origin inside the clip volume"
        );
        assert!(camera.view_mode_label().contains("Orthographic"));
    }
}

#[test]
fn right_mouse_navigation_keeps_orthographic_mode() {
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    let before = camera.to_viewport_view(test_viewport_size());

    camera.ortho_pan(egui::vec2(20.0, 0.0));
    camera.ortho_zoom(1.0);
    let after = camera.to_viewport_view(test_viewport_size());

    assert_eq!(camera.view_mode_label(), "Top Orthographic");
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
    assert!(hint.contains('\n'));
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
    let view = camera.to_viewport_view(test_viewport_size());
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
    let view = camera.to_viewport_view(test_viewport_size());
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
    let before_fov = camera.horizontal_fov_degrees();

    camera.dolly(-20.0);

    assert!(camera.orbit_distance() < before_distance);
    assert_eq!(camera.horizontal_fov_degrees(), before_fov);
}

#[test]
fn orbit_pan_and_dolly_keep_pivot_contract() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();
    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let pivot = camera.orbit_pivot();
    let position = camera
        .to_viewport_view(test_viewport_size())
        .transform
        .translation;

    camera.orbit(egui::vec2(80.0, -20.0));
    let orbit_position = camera
        .to_viewport_view(test_viewport_size())
        .transform
        .translation;
    assert_eq!(camera.orbit_pivot(), pivot);
    assert_ne!(orbit_position, position);

    camera.pan(egui::vec2(12.0, -6.0));
    let panned_pivot = camera.orbit_pivot();
    let panned_position = camera
        .to_viewport_view(test_viewport_size())
        .transform
        .translation;
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
    let view = camera.to_viewport_view(test_viewport_size());

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
fn reference_grid_axis_colors_match_world_line_directions() {
    let frame = adaptive_grid_lines(&default_editor_projection(), GridPlane::XY, 1.0).unwrap();
    let red = egui::Color32::from_rgb(160, 60, 60);
    let green = egui::Color32::from_rgb(60, 150, 80);
    let red_line = frame.lines.iter().find(|line| line.color == red).unwrap();
    let green_line = frame.lines.iter().find(|line| line.color == green).unwrap();
    let red_direction = Vec3::from_array(red_line.end) - Vec3::from_array(red_line.start);
    let green_direction = Vec3::from_array(green_line.end) - Vec3::from_array(green_line.start);

    assert!(red_direction.normalize().dot(Vec3::X).abs() > 0.99);
    assert!(green_direction.normalize().dot(Vec3::Y).abs() > 0.99);
}

#[test]
fn close_camera_gizmo_keeps_axis_when_positive_endpoint_is_behind() {
    let forward = Vec3::new(-1.0, 0.0, -0.5).normalize();
    let right = Vec3::NEG_Y;
    let up = forward.cross(right).normalize();
    let rotation = Quat::from_mat3(&math::Mat3::from_cols(right, up, forward));
    let view = ViewportView::new(
        EntityId::new("close_camera"),
        Transform {
            translation: [1.0, 0.0, 0.2],
            rotation: rotation.to_array(),
            scale: [1.0; 3],
        },
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let size = ViewportSize::new(rect.width(), rect.height()).unwrap();
    let projection =
        ViewportProjection::from_view(&view, size, ViewportClipPlanes::DEFAULT).unwrap();
    let handles = super::gizmo_layout(
        &draw_with_two_mesh_spans(),
        &projection,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Move,
    );

    assert!(
        handles
            .iter()
            .any(|handle| handle.handle == GizmoHandle::MoveX)
    );
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
fn move_gizmo_axes_follow_projected_world_axes() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let projection = default_editor_projection();
    let handles = super::gizmo_layout(
        &draw,
        &projection,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Move,
    );

    for (handle, world_axis) in [
        (GizmoHandle::MoveX, Vec3::X),
        (GizmoHandle::MoveY, Vec3::Y),
        (GizmoHandle::MoveZ, Vec3::Z),
    ] {
        let actual = handles
            .iter()
            .find(|candidate| candidate.handle == handle)
            .unwrap()
            .axis;
        let expected = projected_screen_axis(&projection, rect, [0.6, 0.0, 0.0], world_axis);
        assert!(
            actual.dot(expected) > 0.99,
            "{handle:?}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
fn top_move_gizmo_omits_view_aligned_z_axis() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let size = ViewportSize::new(rect.width(), rect.height()).unwrap();
    let mut camera = ViewCamera::default();
    camera.set_preset(super::ViewPreset::Top);
    let projection = ViewportProjection::from_view(
        &camera.to_viewport_view(size),
        size,
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap();

    let handles = super::gizmo_layout(
        &draw,
        &projection,
        rect,
        Some(&EntityId::new("cube_1")),
        GizmoMode::Move,
    );

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
        !handles
            .iter()
            .any(|handle| handle.handle == GizmoHandle::MoveZ)
    );
    for (handle, world_axis) in [(GizmoHandle::MoveX, Vec3::X), (GizmoHandle::MoveY, Vec3::Y)] {
        let actual = handles
            .iter()
            .find(|candidate| candidate.handle == handle)
            .unwrap()
            .axis;
        let expected = projected_screen_axis(&projection, rect, [0.6, 0.0, 0.0], world_axis);
        assert!(actual.dot(expected) > 0.99);
    }
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
fn rotate_gizmo_layout_uses_projected_world_axes() {
    let draw = draw_with_two_mesh_spans();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let projection = default_editor_projection();

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
    assert!(
        rotate_x.axis.dot(projected_screen_axis(
            &projection,
            rect,
            [0.6, 0.0, 0.0],
            Vec3::X
        )) > 0.99
    );
    assert!(
        rotate_y.axis.dot(projected_screen_axis(
            &projection,
            rect,
            [0.6, 0.0, 0.0],
            Vec3::Y
        )) > 0.99
    );
    assert!(
        rotate_z.axis.dot(projected_screen_axis(
            &projection,
            rect,
            [0.6, 0.0, 0.0],
            Vec3::Z
        )) > 0.99
    );
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
    let view = camera.to_viewport_view(test_viewport_size());

    assert!(view.transform.translation.into_iter().all(f32::is_finite));
    assert!(view.transform.rotation.into_iter().all(f32::is_finite));
}

#[test]
fn fit_visible_draw_pans_edge_selection_toward_center() {
    let draw = draw_with_two_mesh_spans();
    let mut camera = ViewCamera::default();

    assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
    let view = camera.to_viewport_view(test_viewport_size());
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
    let view = camera.to_viewport_view(test_viewport_size());
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
    assert!(super::camera_navigation_requested(
        false, true, false, true, false
    ));
    assert!(!super::can_start_gizmo_drag(true, false, true));
    assert!(!super::can_select_viewport(true, false, true));
    assert!(super::can_start_gizmo_drag(true, false, false));
    assert!(super::can_select_viewport(true, false, false));
}

#[test]
fn active_gizmo_drag_blocks_plain_lmb_camera_navigation() {
    assert!(super::plain_lmb_navigation_requested(
        true, true, false, false, true, false
    ));
    assert!(!super::plain_lmb_navigation_requested(
        true, true, false, true, true, false
    ));
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

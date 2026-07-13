// Copyright The SimpleGameEngine Contributors
//
//! Viewport behavior tests stay adjacent to the private implementation modules.

use super::*;
use sge_math::Mat4;

fn drag(mode: GizmoMode, axis: Axis) -> GizmoDrag {
    GizmoDrag {
        entity: "50000000-0000-4000-8000-000000000001".parse().unwrap(),
        mode,
        axis,
        screen_axis: egui::Vec2::X,
        start: Transform::identity(),
        preview: Transform::identity(),
        pointer: egui::Pos2::ZERO,
    }
}

#[test]
fn each_gizmo_mode_changes_only_its_latched_axis() {
    let moved = transform_for_drag(drag(GizmoMode::Move, Axis::Y), egui::pos2(100.0, 0.0));
    assert_eq!(moved.translation, [0.0, 1.0, 0.0]);
    let scaled = transform_for_drag(drag(GizmoMode::Scale, Axis::Z), egui::pos2(100.0, 0.0));
    assert_eq!(scaled.scale, [1.0, 1.0, 2.0]);
    let rotated = transform_for_drag(drag(GizmoMode::Rotate, Axis::X), egui::pos2(100.0, 0.0));
    assert_ne!(rotated.rotation, Transform::identity().rotation);
}

#[test]
fn active_drag_preview_is_the_gizmo_transform() {
    let committed = Transform::identity();
    let mut active = Some(drag(GizmoMode::Move, Axis::X));
    update_drag_preview(&mut active, Some(egui::pos2(100.0, 0.0)));
    let active = active.unwrap();

    assert_eq!(
        gizmo_transform(committed, Some(active), active.entity),
        active.preview
    );
    assert_eq!(active.preview.translation, [1.0, 0.0, 0.0]);
    assert_eq!(
        gizmo_transform(
            committed,
            Some(active),
            "50000000-0000-4000-8000-000000000002".parse().unwrap()
        ),
        committed
    );
}

#[test]
fn ue_transform_tool_cycle_matches_qwer_and_space() {
    assert_eq!(GizmoMode::Select.next(), GizmoMode::Move);
    assert_eq!(GizmoMode::Move.next(), GizmoMode::Rotate);
    assert_eq!(GizmoMode::Rotate.next(), GizmoMode::Scale);
    assert_eq!(GizmoMode::Scale.next(), GizmoMode::Move);
    assert_eq!(
        GizmoMode::Select.status_text(EditorLanguage::English),
        "Select (Q)"
    );
    assert_eq!(
        GizmoMode::Move.status_text(EditorLanguage::English),
        "Move (W)"
    );
    assert_eq!(
        GizmoMode::Rotate.status_text(EditorLanguage::SimplifiedChinese),
        "旋转 (E)"
    );
    assert_eq!(
        GizmoMode::Scale.status_text(EditorLanguage::SimplifiedChinese),
        "缩放 (R)"
    );
}

#[test]
fn viewport_keyboard_requires_its_own_focus_and_no_text_editor() {
    assert!(viewport_keyboard_capture(true, false));
    assert!(!viewport_keyboard_capture(false, false));
    assert!(!viewport_keyboard_capture(true, true));
}

#[test]
fn ue_camera_look_maps_pointer_directions_to_view_directions() {
    let forward = |rotation: Quat| rotation * Vec3::Z;
    let initial = camera_rotation(0.0, 0.0);

    assert!(
        forward(camera_look_rotation(initial, egui::vec2(20.0, 0.0))).y > 0.0,
        "dragging right must turn the view right"
    );
    assert!(
        forward(camera_look_rotation(initial, egui::vec2(-20.0, 0.0))).y < 0.0,
        "dragging left must turn the view left"
    );
    assert!(
        forward(camera_look_rotation(initial, egui::vec2(0.0, -20.0))).z > 0.0,
        "dragging up must turn the view up"
    );
    assert!(
        forward(camera_look_rotation(initial, egui::vec2(0.0, 20.0))).z < 0.0,
        "dragging down must turn the view down"
    );
}

#[test]
fn ue_camera_look_clamps_pitch_before_the_view_flips() {
    let initial = camera_rotation(0.0, 0.0);
    let up = camera_look_rotation(initial, egui::vec2(0.0, -10_000.0)) * Vec3::Z;
    let down = camera_look_rotation(initial, egui::vec2(0.0, 10_000.0)) * Vec3::Z;

    assert!(up.z > 0.0 && up.z < 1.0);
    assert!(down.z < 0.0 && down.z > -1.0);
}

#[test]
fn ue_lmb_horizontal_navigation_uses_the_same_yaw_direction_as_look() {
    let initial = camera_rotation(0.0, 0.0);
    let right = camera_yaw_rotation(initial, 20.0) * Vec3::Z;
    let left = camera_yaw_rotation(initial, -20.0) * Vec3::Z;

    assert!(right.y > 0.0);
    assert!(left.y < 0.0);
}

#[test]
fn rmb_f_uses_local_flight_axis_instead_of_framing_selection() {
    assert!(frame_selected_requested(true, false, true));
    assert!(!frame_selected_requested(true, true, true));

    let only = |pressed| camera_fly_axes(|key| key == pressed);
    assert_eq!(only(egui::Key::R), (Vec3::Y, 0.0));
    assert_eq!(only(egui::Key::F), (-Vec3::Y, 0.0));
    assert_eq!(only(egui::Key::E), (Vec3::ZERO, 1.0));
    assert_eq!(only(egui::Key::Q), (Vec3::ZERO, -1.0));
}

#[test]
fn lmb_rmb_vertical_navigation_requires_a_viewport_drag() {
    assert!(vertical_navigation_requested(false, true, true, true, true));
    assert!(!vertical_navigation_requested(
        false, true, true, false, true
    ));
    assert!(!vertical_navigation_requested(
        false, true, true, true, false
    ));
    assert!(!vertical_navigation_requested(true, true, true, true, true));
}

#[test]
fn ue_camera_navigation_is_distance_scaled_and_frame_rate_independent() {
    let pan = camera_pan_motion(Quat::IDENTITY, 8.0, egui::vec2(20.0, -10.0));
    assert!((pan - Vec3::new(-0.4, -0.2, 0.0)).length() < 0.0001);
    assert!(dolly_distance(8.0, 10.0) > 8.0);
    let straight = camera_fly_motion(Vec3::X, 4.0, 0.25);
    let diagonal = camera_fly_motion(Vec3::new(1.0, 1.0, 0.0), 4.0, 0.25);
    assert!((straight.length() - 1.0).abs() < 0.0001);
    assert!((diagonal.length() - straight.length()).abs() < 0.0001);
}

#[test]
fn lmb_forward_motion_tracks_pointer_distance_without_frame_acceleration() {
    let rotation = Quat::IDENTITY;
    let whole = camera_lmb_forward_motion(rotation, 4.0, 20.0);
    let split = camera_lmb_forward_motion(rotation, 4.0, 8.0)
        + camera_lmb_forward_motion(rotation, 4.0, 12.0);

    assert_eq!(whole, split);
    assert_eq!(camera_lmb_forward_motion(rotation, 4.0, 0.0), Vec3::ZERO);
    assert!((whole.length() - 0.8).abs() < 0.0001);
}

#[test]
fn triangle_hit_test_accepts_interior_and_rejects_exterior() {
    let a = egui::pos2(0.0, 0.0);
    let b = egui::pos2(10.0, 0.0);
    let c = egui::pos2(0.0, 10.0);
    let point = |position, depth| ScreenPoint { position, depth };
    let depth = triangle_depth(
        egui::pos2(2.0, 2.0),
        point(a, 0.1),
        point(b, 0.4),
        point(c, 0.7),
    )
    .unwrap();
    assert!((depth - 0.28).abs() < 0.0001);
    assert!(
        triangle_depth(
            egui::pos2(9.0, 9.0),
            point(a, 0.1),
            point(b, 0.4),
            point(c, 0.7)
        )
        .is_none()
    );
}

#[test]
fn view_cube_layout_tracks_camera_rotation_and_exposes_clickable_faces() {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let identity = view_cube_faces(rect, Quat::IDENTITY);
    let rotated = view_cube_faces(rect, Quat::from_rotation_y(0.7));
    assert!(!identity.is_empty());
    assert_ne!(identity[0].polygon, rotated[0].polygon);
    let face = &rotated[0];
    let center = face
        .polygon
        .into_iter()
        .fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2())
        / 4.0;
    assert!(point_in_polygon(center.to_pos2(), face.polygon));
}

#[test]
fn initial_framing_scales_with_visible_geometry() {
    let small = frame_distance(
        Vec3::splat(-0.5),
        Vec3::splat(0.5),
        std::f32::consts::FRAC_PI_3,
    );
    let large = frame_distance(
        Vec3::splat(-5.0),
        Vec3::splat(5.0),
        std::f32::consts::FRAC_PI_3,
    );
    assert!(small >= 2.5);
    assert!(large > small * 5.0);
}

#[test]
fn scene_bounds_include_non_mesh_actor_positions() {
    let mut bounds = None;
    extend_bounds(&mut bounds, Vec3::new(-2.0, 1.0, 3.0));
    extend_bounds(&mut bounds, Vec3::new(4.0, -5.0, 2.0));

    assert_eq!(
        bounds,
        Some((Vec3::new(-2.0, -5.0, 2.0), Vec3::new(4.0, 1.0, 3.0)))
    );
}

#[test]
fn initial_camera_is_z_up_without_roll() {
    let rotation = initial_camera_rotation();
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    assert!(right.z.abs() < 0.0001);
    assert!(up.dot(Vec3::Z) > 0.5);
}

#[test]
fn world_segments_are_clipped_to_the_wgpu_frustum() {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
    let [start, end] = project_segment(
        Mat4::IDENTITY,
        Vec3::new(-2.0, 0.0, 0.5),
        Vec3::new(0.0, 0.0, 0.5),
        rect,
    )
    .expect("crossing segment must remain visible");
    assert_eq!(start.position, egui::pos2(0.0, 50.0));
    assert_eq!(end.position, egui::pos2(50.0, 50.0));
}

#[test]
fn grid_layout_covers_the_visible_ground_plane() {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 500.0));
    let matrix = Mat4::from_scale(Vec3::new(0.01, 0.02, 1.0));

    let (minimum, maximum, step) = visible_grid_layout(matrix, rect).unwrap();

    assert!(minimum.x <= -100.0 && maximum.x >= 100.0);
    assert!(minimum.y <= -50.0 && maximum.y >= 50.0);
    assert!(step > 0.0);
    assert!(line_count(minimum.x, maximum.x, step) <= 256);
    assert!(line_count(minimum.y, maximum.y, step) <= 256);
}

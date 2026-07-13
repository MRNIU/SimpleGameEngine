// Copyright The SimpleGameEngine Contributors
//
//! Editor-only world representations for scene actors without render geometry.

use eframe::egui;
use sge_math::{Mat4, Quat, Transform, Vec3};
use sge_render::{Camera, Projection};
use sge_scene::SceneEntityId;

use crate::PreviewFrame;

use super::overlays::{project_segment, projection};

const PICK_TOLERANCE: f32 = 7.0;

#[derive(Clone, Copy)]
enum ActorVisualKind {
    Camera(Camera),
    DirectionalLight,
}

#[derive(Clone, Copy)]
struct ActorVisual {
    entity: SceneEntityId,
    transform: Transform,
    kind: ActorVisualKind,
}

pub(super) fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    frame: &PreviewFrame,
    selection: Option<SceneEntityId>,
    drag_preview: Option<(SceneEntityId, Transform)>,
) {
    let Some(view_projection) = projection(frame, rect) else {
        return;
    };
    let aspect = rect.width().max(1.0) / rect.height().max(1.0);
    let painter = ui.painter_at(rect);
    for visual in actor_visuals(frame, drag_preview) {
        let selected = selection == Some(visual.entity);
        let color = if selected {
            egui::Color32::WHITE
        } else {
            visual_color(visual.kind)
        };
        let model = visual_matrix(visual.transform);
        for (start, end) in visual_segments(visual.kind, aspect) {
            let start = model.transform_point3(start);
            let end = model.transform_point3(end);
            let Some([start, end]) = project_segment(view_projection, start, end, rect) else {
                continue;
            };
            painter.line_segment(
                [start.position, end.position],
                egui::Stroke::new(if selected { 4.5 } else { 3.5 }, egui::Color32::BLACK),
            );
            painter.line_segment(
                [start.position, end.position],
                egui::Stroke::new(if selected { 2.5 } else { 1.5 }, color),
            );
        }
    }
}

pub(super) fn pick(
    frame: &PreviewFrame,
    rect: egui::Rect,
    pointer: egui::Pos2,
    drag_preview: Option<(SceneEntityId, Transform)>,
) -> Option<SceneEntityId> {
    let view_projection = projection(frame, rect)?;
    let aspect = rect.width().max(1.0) / rect.height().max(1.0);
    actor_visuals(frame, drag_preview)
        .into_iter()
        .filter_map(|visual| {
            let model = visual_matrix(visual.transform);
            visual_segments(visual.kind, aspect)
                .into_iter()
                .filter_map(|(start, end)| {
                    segment_hit_depth(
                        view_projection,
                        rect,
                        model.transform_point3(start),
                        model.transform_point3(end),
                        pointer,
                    )
                })
                .min_by(f32::total_cmp)
                .map(|depth| (depth, visual.entity))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, entity)| entity)
}

fn actor_visuals(
    frame: &PreviewFrame,
    drag_preview: Option<(SceneEntityId, Transform)>,
) -> Vec<ActorVisual> {
    let mut visuals =
        Vec::with_capacity(frame.snapshot.cameras().len() + frame.snapshot.lights().len());
    for camera in frame.snapshot.cameras() {
        if let Some(entity) = scene_entity(frame, camera.entity()) {
            visuals.push(ActorVisual {
                entity,
                transform: previewed_transform(entity, camera.transform(), drag_preview),
                kind: ActorVisualKind::Camera(camera.camera()),
            });
        }
    }
    for light in frame.snapshot.lights() {
        if let Some(entity) = scene_entity(frame, light.entity()) {
            visuals.push(ActorVisual {
                entity,
                transform: previewed_transform(entity, light.transform(), drag_preview),
                kind: ActorVisualKind::DirectionalLight,
            });
        }
    }
    visuals
}

fn scene_entity(frame: &PreviewFrame, runtime: sge_ecs::Entity) -> Option<SceneEntityId> {
    frame
        .scene_entities
        .iter()
        .find_map(|(scene, candidate)| (*candidate == runtime).then_some(*scene))
}

fn previewed_transform(
    entity: SceneEntityId,
    committed: Transform,
    drag_preview: Option<(SceneEntityId, Transform)>,
) -> Transform {
    drag_preview
        .filter(|(preview_entity, _)| *preview_entity == entity)
        .map_or(committed, |(_, transform)| transform)
}

fn visual_matrix(transform: Transform) -> Mat4 {
    Mat4::from_rotation_translation(
        Quat::from_array(transform.rotation).normalize(),
        Vec3::from_array(transform.translation),
    )
}

fn visual_color(kind: ActorVisualKind) -> egui::Color32 {
    match kind {
        ActorVisualKind::Camera(_) => egui::Color32::from_rgb(90, 205, 235),
        ActorVisualKind::DirectionalLight => egui::Color32::from_rgb(245, 205, 70),
    }
}

fn visual_segments(kind: ActorVisualKind, aspect: f32) -> Vec<(Vec3, Vec3)> {
    match kind {
        ActorVisualKind::Camera(camera) => camera_segments(camera, aspect),
        ActorVisualKind::DirectionalLight => directional_light_segments(),
    }
}

fn camera_segments(camera: Camera, aspect: f32) -> Vec<(Vec3, Vec3)> {
    let mut segments = Vec::with_capacity(32);
    let back = rectangle(0.32, 0.22, -0.25);
    let front = rectangle(0.32, 0.22, 0.25);
    push_loop(&mut segments, back);
    push_loop(&mut segments, front);
    for index in 0..4 {
        segments.push((back[index], front[index]));
    }

    let lens = rectangle(0.24, 0.16, 0.42);
    push_loop(&mut segments, lens);
    for index in 0..4 {
        segments.push((front[index], lens[index]));
    }

    let aspect = aspect.clamp(0.5, 2.5);
    let (far_x, far_y) = match camera.projection() {
        Projection::Perspective => {
            let half_y = ((camera.vertical_fov_radians() * 0.5).tan() * 0.8).clamp(0.3, 0.9);
            ((half_y * aspect).clamp(0.4, 1.4), half_y)
        }
        Projection::Orthographic => {
            let half_y = (camera.orthographic_height() * 0.05).clamp(0.3, 0.9);
            ((half_y * aspect).clamp(0.4, 1.4), half_y)
        }
    };
    let far = rectangle(far_x, far_y, 1.4);
    push_loop(&mut segments, far);
    for index in 0..4 {
        segments.push((lens[index], far[index]));
    }

    let top = Vec3::new(0.0, 0.38, -0.1);
    segments.extend([(front[2], top), (front[3], top), (front[2], front[3])]);
    segments
}

fn rectangle(half_x: f32, half_y: f32, z: f32) -> [Vec3; 4] {
    [
        Vec3::new(-half_x, -half_y, z),
        Vec3::new(half_x, -half_y, z),
        Vec3::new(half_x, half_y, z),
        Vec3::new(-half_x, half_y, z),
    ]
}

fn push_loop(segments: &mut Vec<(Vec3, Vec3)>, points: [Vec3; 4]) {
    for index in 0..4 {
        segments.push((points[index], points[(index + 1) % 4]));
    }
}

fn directional_light_segments() -> Vec<(Vec3, Vec3)> {
    let mut segments = Vec::with_capacity(48);
    const RING_SEGMENTS: usize = 12;
    const RADIUS: f32 = 0.24;
    for plane in 0..3 {
        for index in 0..RING_SEGMENTS {
            let angle = std::f32::consts::TAU * index as f32 / RING_SEGMENTS as f32;
            let next = std::f32::consts::TAU * (index + 1) as f32 / RING_SEGMENTS as f32;
            segments.push((
                ring_point(plane, angle, RADIUS),
                ring_point(plane, next, RADIUS),
            ));
        }
    }
    for direction in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        segments.push((direction * 0.32, direction * 0.5));
    }
    let tip = Vec3::new(0.0, 0.0, 1.2);
    segments.push((Vec3::new(0.0, 0.0, 0.3), tip));
    for base in [
        Vec3::new(0.16, 0.0, 0.96),
        Vec3::new(-0.16, 0.0, 0.96),
        Vec3::new(0.0, 0.16, 0.96),
        Vec3::new(0.0, -0.16, 0.96),
    ] {
        segments.push((tip, base));
    }
    segments
}

fn ring_point(plane: usize, angle: f32, radius: f32) -> Vec3 {
    let (first, second) = angle.sin_cos();
    match plane {
        0 => Vec3::new(first, second, 0.0) * radius,
        1 => Vec3::new(first, 0.0, second) * radius,
        _ => Vec3::new(0.0, first, second) * radius,
    }
}

fn segment_hit_depth(
    view_projection: Mat4,
    rect: egui::Rect,
    start: Vec3,
    end: Vec3,
    pointer: egui::Pos2,
) -> Option<f32> {
    let [start, end] = project_segment(view_projection, start, end, rect)?;
    let segment = end.position - start.position;
    let length_squared = segment.length_sq();
    let fraction = if length_squared > f32::EPSILON {
        ((pointer - start.position).dot(segment) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = start.position + segment * fraction;
    (closest.distance(pointer) <= PICK_TOLERANCE)
        .then_some(start.depth + (end.depth - start.depth) * fraction)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use eframe::egui;
    use sge_math::{Mat4, Vec3};
    use sge_render::{Camera, Projection};

    use crate::EditSession;

    use super::{
        ActorVisualKind, actor_visuals, camera_segments, directional_light_segments,
        segment_hit_depth,
    };

    #[test]
    fn scene_camera_and_light_map_to_editor_visuals_with_their_scene_entities() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game");
        let session = EditSession::open(demo_game::GAME, project).expect("demo project opens");
        let frame = session.preview_frame().expect("preview extracts");

        let visuals = actor_visuals(&frame, None);

        assert_eq!(visuals.len(), 2);
        assert!(
            visuals
                .iter()
                .any(|visual| matches!(visual.kind, ActorVisualKind::Camera(_)))
        );
        assert!(
            visuals
                .iter()
                .any(|visual| matches!(visual.kind, ActorVisualKind::DirectionalLight))
        );
        assert!(visuals.iter().all(|visual| {
            session
                .component::<sge_math::Transform>(visual.entity)
                .is_some()
        }));
    }

    #[test]
    fn camera_visual_is_a_three_dimensional_body_with_a_projection_volume() {
        let camera = Camera::new(
            true,
            Projection::Perspective,
            std::f32::consts::FRAC_PI_2,
            10.0,
            0.1,
            100.0,
        );
        let segments = camera_segments(camera, 16.0 / 9.0);

        assert!(segments.len() >= 20);
        assert!(segments.iter().any(|(start, end)| start.x != end.x));
        assert!(segments.iter().any(|(start, end)| start.y != end.y));
        assert!(segments.iter().any(|(start, end)| start.z != end.z));
        assert!(
            segments
                .iter()
                .flat_map(|(start, end)| [start.z, end.z])
                .any(|z| z > 1.0)
        );
    }

    #[test]
    fn directional_light_visual_is_a_three_dimensional_source_with_forward_arrow() {
        let segments = directional_light_segments();

        assert!(segments.len() >= 30);
        assert!(segments.iter().any(|(start, end)| start.x != end.x));
        assert!(segments.iter().any(|(start, end)| start.y != end.y));
        assert!(segments.iter().any(|(start, end)| start.z != end.z));
        assert!(
            segments
                .iter()
                .any(|(_, end)| *end == Vec3::new(0.0, 0.0, 1.2))
        );
    }

    #[test]
    fn projected_visual_segments_are_clickable_with_a_bounded_screen_tolerance() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let start = Vec3::new(-0.5, 0.0, 0.5);
        let end = Vec3::new(0.5, 0.0, 0.5);

        assert!(
            segment_hit_depth(Mat4::IDENTITY, rect, start, end, egui::pos2(50.0, 54.0)).is_some()
        );
        assert!(
            segment_hit_depth(Mat4::IDENTITY, rect, start, end, egui::pos2(50.0, 70.0)).is_none()
        );
    }
}

// Copyright The SimpleGameEngine Contributors

use eframe::egui;
use sge_math::{Quat, Vec3};

use super::{ViewPreset, ViewportAction};

const CUBE_SIZE: f32 = 72.0;
const CUBE_SCALE: f32 = 24.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrientationCubeFace {
    pub(crate) polygon: [egui::Pos2; 4],
    pub(crate) preset: ViewPreset,
    pub(crate) depth: f32,
    pub(crate) color: egui::Color32,
    label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrientationCubeLayout {
    pub(crate) faces: Vec<OrientationCubeFace>,
    pub(crate) perspective: egui::Rect,
}

struct FaceDefinition {
    preset: ViewPreset,
    normal: [f32; 3],
    corners: [[f32; 3]; 4],
    color: [u8; 3],
    label: &'static str,
}

const FACE_DEFINITIONS: [FaceDefinition; 6] = [
    FaceDefinition {
        preset: ViewPreset::Top,
        normal: [0.0, 0.0, 1.0],
        corners: [
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        color: [70, 115, 205],
        label: "T",
    },
    FaceDefinition {
        preset: ViewPreset::Bottom,
        normal: [0.0, 0.0, -1.0],
        corners: [
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
            [-1.0, -1.0, -1.0],
        ],
        color: [70, 115, 205],
        label: "B",
    },
    FaceDefinition {
        preset: ViewPreset::Front,
        normal: [-1.0, 0.0, 0.0],
        corners: [
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
        ],
        color: [165, 65, 65],
        label: "F",
    },
    FaceDefinition {
        preset: ViewPreset::Back,
        normal: [1.0, 0.0, 0.0],
        corners: [
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
        color: [165, 65, 65],
        label: "Bk",
    },
    FaceDefinition {
        preset: ViewPreset::Right,
        normal: [0.0, 1.0, 0.0],
        corners: [
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        color: [55, 145, 75],
        label: "R",
    },
    FaceDefinition {
        preset: ViewPreset::Left,
        normal: [0.0, -1.0, 0.0],
        corners: [
            [1.0, -1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
        ],
        color: [55, 145, 75],
        label: "L",
    },
];

#[must_use]
pub(crate) fn orientation_cube_layout(
    viewport: egui::Rect,
    view_rotation: Quat,
) -> OrientationCubeLayout {
    let cube_rect = egui::Rect::from_min_size(
        viewport.right_top() + egui::vec2(-CUBE_SIZE - 12.0, 12.0),
        egui::vec2(CUBE_SIZE, CUBE_SIZE),
    );
    let center = cube_rect.center();
    let inverse_rotation = view_rotation.normalize().inverse();
    let mut faces = FACE_DEFINITIONS
        .iter()
        .filter_map(|face| {
            let normal = inverse_rotation * Vec3::from_array(face.normal);
            (normal.z < -0.001).then(|| {
                let camera_corners = face
                    .corners
                    .map(|corner| inverse_rotation * Vec3::from_array(corner));
                let polygon = camera_corners.map(|corner| {
                    center + egui::vec2(corner.x * CUBE_SCALE, -corner.y * CUBE_SCALE)
                });
                let depth = camera_corners
                    .into_iter()
                    .map(|corner| corner.z)
                    .sum::<f32>()
                    / 4.0;
                OrientationCubeFace {
                    polygon,
                    preset: face.preset,
                    depth,
                    color: shaded_color(face.color, -normal.z),
                    label: face.label,
                }
            })
        })
        .collect::<Vec<_>>();
    faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    OrientationCubeLayout {
        faces,
        perspective: egui::Rect::from_center_size(
            egui::pos2(cube_rect.center().x, cube_rect.bottom() + 15.0),
            egui::vec2(28.0, 22.0),
        ),
    }
}

#[must_use]
pub(crate) fn orientation_cube_hit_test(
    layout: &OrientationCubeLayout,
    pointer: egui::Pos2,
) -> Option<ViewportAction> {
    if layout.perspective.contains(pointer) {
        return Some(ViewportAction::ReturnToPerspective);
    }
    layout.faces.iter().rev().find_map(|face| {
        point_in_convex_polygon(pointer, face.polygon)
            .then_some(ViewportAction::SetViewPreset(face.preset))
    })
}

pub(crate) fn paint_orientation_cube(painter: &egui::Painter, layout: &OrientationCubeLayout) {
    for face in &layout.faces {
        painter.add(egui::Shape::convex_polygon(
            face.polygon.to_vec(),
            face.color,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(225, 230, 235)),
        ));
        painter.text(
            polygon_center(face.polygon),
            egui::Align2::CENTER_CENTER,
            face.label,
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
    }

    painter.circle_filled(
        layout.perspective.center(),
        10.0,
        egui::Color32::from_rgb(42, 48, 54),
    );
    painter.circle_stroke(
        layout.perspective.center(),
        10.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(225, 230, 235)),
    );
    let center = layout.perspective.center();
    let points = [
        center + egui::vec2(-5.0, 1.0),
        center + egui::vec2(0.0, -4.0),
        center + egui::vec2(5.0, 1.0),
        center + egui::vec2(3.5, 1.0),
        center + egui::vec2(3.5, 5.0),
        center + egui::vec2(-3.5, 5.0),
        center + egui::vec2(-3.5, 1.0),
    ];
    painter.add(egui::Shape::line(
        points.to_vec(),
        egui::Stroke::new(1.2, egui::Color32::WHITE),
    ));
}

fn shaded_color(base: [u8; 3], facing: f32) -> egui::Color32 {
    let brightness = (0.7 + facing.clamp(0.0, 1.0) * 0.3).clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (f32::from(base[0]) * brightness) as u8,
        (f32::from(base[1]) * brightness) as u8,
        (f32::from(base[2]) * brightness) as u8,
    )
}

fn polygon_center(polygon: [egui::Pos2; 4]) -> egui::Pos2 {
    let sum = polygon
        .into_iter()
        .fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2());
    (sum / 4.0).to_pos2()
}

fn point_in_convex_polygon(pointer: egui::Pos2, polygon: [egui::Pos2; 4]) -> bool {
    let mut sign = 0.0_f32;
    for (start, end) in polygon
        .into_iter()
        .zip(polygon.into_iter().cycle().skip(1))
        .take(4)
    {
        let edge = end - start;
        let offset = pointer - start;
        let cross = edge.x * offset.y - edge.y * offset.x;
        if cross.abs() <= f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign != cross.signum() {
            return false;
        }
    }
    sign != 0.0
}

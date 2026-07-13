// Copyright The SimpleGameEngine Contributors
//
//! ViewCube face geometry, ordering and hit testing.

use eframe::egui;
use sge_math::{Quat, Vec3};

#[derive(Clone, Copy)]
pub(super) enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
}

pub(super) struct CubeFace {
    pub(super) polygon: [egui::Pos2; 4],
    pub(super) preset: ViewPreset,
    pub(super) depth: f32,
    pub(super) color: egui::Color32,
    pub(super) label: &'static str,
}

pub(super) fn preset_axes(preset: ViewPreset) -> (Vec3, Vec3) {
    match preset {
        ViewPreset::Top => (-Vec3::Z, Vec3::Y),
        ViewPreset::Bottom => (Vec3::Z, Vec3::Y),
        ViewPreset::Front => (Vec3::X, Vec3::Z),
        ViewPreset::Back => (-Vec3::X, Vec3::Z),
        ViewPreset::Right => (-Vec3::Y, Vec3::Z),
        ViewPreset::Left => (Vec3::Y, Vec3::Z),
    }
}

pub(super) fn view_cube_faces(rect: egui::Rect, rotation: Quat) -> Vec<CubeFace> {
    let center = rect.right_top() + egui::vec2(-48.0, 48.0);
    let inverse = rotation.normalize().inverse();
    let definitions = [
        (
            ViewPreset::Top,
            Vec3::Z,
            [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
            [70, 115, 205],
            "T",
        ),
        (
            ViewPreset::Bottom,
            -Vec3::Z,
            [
                [-1., 1., -1.],
                [1., 1., -1.],
                [1., -1., -1.],
                [-1., -1., -1.],
            ],
            [70, 115, 205],
            "B",
        ),
        (
            ViewPreset::Front,
            -Vec3::X,
            [
                [-1., -1., -1.],
                [-1., 1., -1.],
                [-1., 1., 1.],
                [-1., -1., 1.],
            ],
            [165, 65, 65],
            "F",
        ),
        (
            ViewPreset::Back,
            Vec3::X,
            [[1., 1., -1.], [1., -1., -1.], [1., -1., 1.], [1., 1., 1.]],
            [165, 65, 65],
            "Bk",
        ),
        (
            ViewPreset::Right,
            Vec3::Y,
            [[-1., 1., -1.], [1., 1., -1.], [1., 1., 1.], [-1., 1., 1.]],
            [55, 145, 75],
            "R",
        ),
        (
            ViewPreset::Left,
            -Vec3::Y,
            [
                [1., -1., -1.],
                [-1., -1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
            ],
            [55, 145, 75],
            "L",
        ),
    ];
    let mut faces = definitions
        .into_iter()
        .filter_map(|(preset, normal, corners, color, label)| {
            let normal = inverse * normal;
            (normal.z < -0.001).then(|| {
                let corners = corners.map(|corner| inverse * Vec3::from_array(corner));
                CubeFace {
                    polygon: corners.map(|corner| center + egui::vec2(corner.x, -corner.y) * 24.0),
                    preset,
                    depth: corners.iter().map(|corner| corner.z).sum::<f32>() / 4.0,
                    color: egui::Color32::from_rgb(color[0], color[1], color[2]),
                    label,
                }
            })
        })
        .collect::<Vec<_>>();
    faces.sort_by(|left, right| right.depth.total_cmp(&left.depth));
    faces
}

pub(super) fn point_in_polygon(pointer: egui::Pos2, polygon: [egui::Pos2; 4]) -> bool {
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

// Copyright The SimpleGameEngine Contributors
//
//! Shared viewport projection, grid and world-axis paint geometry.

use eframe::egui;
use sge_math::{Mat4, Vec3, Vec4};
use sge_render::view_projection_matrix;

use crate::PreviewFrame;

use super::gizmo::Axis;

const WORLD_AXIS_LENGTH: f32 = 1.0;

#[derive(Clone, Copy)]
pub(super) struct ScreenPoint {
    pub(super) position: egui::Pos2,
    pub(super) depth: f32,
}

pub(super) fn projection(frame: &PreviewFrame, rect: egui::Rect) -> Option<Mat4> {
    view_projection_matrix(
        frame.view,
        [rect.width().max(1.0) as u32, rect.height().max(1.0) as u32],
    )
    .ok()
    .map(|matrix| Mat4::from_cols_array(&matrix))
}

pub(super) fn project(matrix: Mat4, point: Vec3, rect: egui::Rect) -> Option<ScreenPoint> {
    let clip = matrix * Vec4::new(point.x, point.y, point.z, 1.0);
    project_clip_point(clip, rect)
}

pub(super) fn draw_grid(ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let Some((minimum, maximum, step)) = visible_grid_layout(matrix, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for index in 0..=line_count(minimum.x, maximum.x, step) {
        let value = minimum.x + index as f32 * step;
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(value, minimum.y, 0.0),
            Vec3::new(value, maximum.y, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
    }
    for index in 0..=line_count(minimum.y, maximum.y, step) {
        let value = minimum.y + index as f32 * step;
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(minimum.x, value, 0.0),
            Vec3::new(maximum.x, value, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
    }
}

pub(super) fn visible_grid_layout(matrix: Mat4, rect: egui::Rect) -> Option<(Vec3, Vec3, f32)> {
    let inverse = matrix.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let corners = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ]
    .map(|point| unproject(inverse, point))
    .into_iter()
    .collect::<Option<Vec<_>>>()?;
    let edges = [
        (0, 1),
        (1, 3),
        (3, 2),
        (2, 0),
        (4, 5),
        (5, 7),
        (7, 6),
        (6, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut intersections = Vec::new();
    for (start, end) in edges {
        let start = corners[start];
        let end = corners[end];
        if start.z.abs() <= 0.0001 {
            intersections.push(start);
        }
        if end.z.abs() <= 0.0001 {
            intersections.push(end);
        }
        if (start.z < 0.0) != (end.z < 0.0) {
            let fraction = start.z / (start.z - end.z);
            intersections.push(start.lerp(end, fraction));
        }
    }
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for point in intersections {
        minimum = minimum.min(point);
        maximum = maximum.max(point);
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        return None;
    }
    let anchor = ground_intersection(inverse, [0.0, 0.0]).unwrap_or((minimum + maximum) * 0.5);
    let mut step = projected_grid_step(matrix, rect, anchor).unwrap_or(1.0);
    while line_count(minimum.x, maximum.x, step) > 256
        || line_count(minimum.y, maximum.y, step) > 256
    {
        step *= 2.0;
    }
    minimum.x = (minimum.x / step).floor() * step;
    minimum.y = (minimum.y / step).floor() * step;
    maximum.x = (maximum.x / step).ceil() * step;
    maximum.y = (maximum.y / step).ceil() * step;
    Some((minimum, maximum, step))
}

fn unproject(inverse: Mat4, point: Vec3) -> Option<Vec3> {
    let world = inverse * point.extend(1.0);
    (world.is_finite() && world.w.abs() > f32::EPSILON).then(|| world.truncate() / world.w)
}

fn ground_intersection(inverse: Mat4, ndc: [f32; 2]) -> Option<Vec3> {
    let near = unproject(inverse, Vec3::new(ndc[0], ndc[1], 0.0))?;
    let far = unproject(inverse, Vec3::new(ndc[0], ndc[1], 1.0))?;
    if near.z.abs() <= 0.0001 {
        return Some(near);
    }
    if (near.z < 0.0) == (far.z < 0.0) {
        return None;
    }
    let fraction = near.z / (near.z - far.z);
    Some(near.lerp(far, fraction))
}

fn projected_grid_step(matrix: Mat4, rect: egui::Rect, anchor: Vec3) -> Option<f32> {
    let center = project(matrix, anchor, rect)?.position;
    let x = project(matrix, anchor + Vec3::X, rect)?.position;
    let y = project(matrix, anchor + Vec3::Y, rect)?.position;
    let spacing = center.distance(x).max(center.distance(y));
    (spacing.is_finite() && spacing > f32::EPSILON).then(|| nice_grid_step(32.0 / spacing))
}

fn nice_grid_step(target: f32) -> f32 {
    if !target.is_finite() || target <= 0.0 {
        return 1.0;
    }
    let magnitude = 10.0_f32.powf(target.log10().floor());
    let fraction = target / magnitude;
    let factor = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    factor * magnitude
}

pub(super) fn line_count(minimum: f32, maximum: f32, step: f32) -> usize {
    ((maximum - minimum) / step).round().max(0.0) as usize
}

pub(super) fn draw_world_axes(ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for (axis, end) in [(Axis::X, Vec3::X), (Axis::Y, Vec3::Y), (Axis::Z, Vec3::Z)] {
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::ZERO,
            end * WORLD_AXIS_LENGTH,
            axis.color(),
            2.5,
        );
    }
}

pub(super) fn draw_world_line(
    painter: &egui::Painter,
    rect: egui::Rect,
    matrix: Mat4,
    start: Vec3,
    end: Vec3,
    color: egui::Color32,
    width: f32,
) {
    if let Some([start, end]) = project_segment(matrix, start, end, rect) {
        painter.line_segment(
            [start.position, end.position],
            egui::Stroke::new(width, color),
        );
    }
}

pub(super) fn project_segment(
    matrix: Mat4,
    start: Vec3,
    end: Vec3,
    rect: egui::Rect,
) -> Option<[ScreenPoint; 2]> {
    let start = matrix * start.extend(1.0);
    let end = matrix * end.extend(1.0);
    let (start, end) = clip_segment(start, end)?;
    Some([
        project_clip_point(start, rect)?,
        project_clip_point(end, rect)?,
    ])
}

fn clip_segment(start: Vec4, end: Vec4) -> Option<(Vec4, Vec4)> {
    if !start.is_finite() || !end.is_finite() {
        return None;
    }
    let planes: [fn(Vec4) -> f32; 6] = [
        |point| point.x + point.w,
        |point| point.w - point.x,
        |point| point.y + point.w,
        |point| point.w - point.y,
        |point| point.z,
        |point| point.w - point.z,
    ];
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for plane in planes {
        let start_distance = plane(start);
        let end_distance = plane(end);
        if start_distance < 0.0 && end_distance < 0.0 {
            return None;
        }
        if (start_distance < 0.0) != (end_distance < 0.0) {
            let crossing = start_distance / (start_distance - end_distance);
            if start_distance < 0.0 {
                lower = lower.max(crossing);
            } else {
                upper = upper.min(crossing);
            }
        }
    }
    (lower <= upper).then(|| {
        let delta = end - start;
        (start + delta * lower, start + delta * upper)
    })
}

fn project_clip_point(clip: Vec4, rect: egui::Rect) -> Option<ScreenPoint> {
    if !clip.is_finite() || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    Some(ScreenPoint {
        position: egui::pos2(
            rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
            rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
        ),
        depth: ndc.z,
    })
}

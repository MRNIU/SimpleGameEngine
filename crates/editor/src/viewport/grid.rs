// Copyright The SimpleGameEngine Contributors

use eframe::egui;
use render::ViewportProjection;
use sge_math::Vec3;

use super::{ReferenceLine, ViewPreset};

const MIN_MINOR_SPACING: f32 = 4.0;
const MAX_MINOR_SPACING: f32 = 160.0;
const MAX_LINES_PER_AXIS: usize = 256;
const PERSPECTIVE_GRID_BASE_RADIUS: f32 = 1_000.0;
const PERSPECTIVE_GRID_HEIGHT_SCALE: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridPlane {
    XY,
    XZ,
    YZ,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GridState {
    pub(crate) minor_step: f32,
}

impl GridState {
    pub(crate) const DEFAULT: Self = Self { minor_step: 1.0 };
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridFrame {
    pub(crate) minor_step: f32,
    pub(crate) lines: Vec<ReferenceLine>,
}

#[must_use]
pub(crate) const fn grid_plane_for_preset(preset: ViewPreset) -> GridPlane {
    match preset {
        ViewPreset::Top | ViewPreset::Bottom => GridPlane::XY,
        ViewPreset::Front | ViewPreset::Back => GridPlane::YZ,
        ViewPreset::Right | ViewPreset::Left => GridPlane::XZ,
    }
}

#[must_use]
pub(crate) fn grid_step_for_spacing(spacing: f32, previous_step: f32) -> f32 {
    if !spacing.is_finite() || !previous_step.is_finite() || previous_step <= 0.0 {
        return 1.0;
    }
    if spacing < MIN_MINOR_SPACING {
        previous_step * 10.0
    } else if spacing > MAX_MINOR_SPACING {
        previous_step / 10.0
    } else {
        previous_step
    }
}

#[must_use]
pub(crate) fn adaptive_grid_lines(
    projection: &ViewportProjection,
    plane: GridPlane,
    previous_step: f32,
) -> Option<GridFrame> {
    let (u, v, normal) = plane_basis(plane);
    let intersections = [
        [-1.0, -1.0],
        [1.0, -1.0],
        [-1.0, 1.0],
        [1.0, 1.0],
        [0.0, 0.0],
    ]
    .into_iter()
    .filter_map(|ndc| projection.screen_ray(ndc))
    .filter_map(|ray| intersect_plane(ray, normal))
    .collect::<Vec<_>>();
    let anchor = intersections.last().copied()?;
    let (mut min_u, mut max_u, mut min_v, mut max_v) = plane_extent(&intersections, u, v)?;
    let spacing = match (
        projected_spacing(projection, anchor, u, previous_step),
        projected_spacing(projection, anchor, v, previous_step),
    ) {
        (Some(u), Some(v)) => u.max(v),
        (Some(spacing), None) | (None, Some(spacing)) => spacing,
        (None, None) => return None,
    };
    let mut step = grid_step_for_spacing(spacing, previous_step).clamp(0.000_1, 1_000_000.0);

    loop {
        min_u = (min_u / step).floor() * step;
        max_u = (max_u / step).ceil() * step;
        min_v = (min_v / step).floor() * step;
        max_v = (max_v / step).ceil() * step;
        let u_count = line_count(min_u, max_u, step);
        let v_count = line_count(min_v, max_v, step);
        if u_count <= MAX_LINES_PER_AXIS && v_count <= MAX_LINES_PER_AXIS {
            break;
        }
        step *= 10.0;
    }

    let mut lines = Vec::new();
    append_axis_lines(
        &mut lines,
        projection,
        u,
        v,
        (min_u, max_u, min_v, max_v),
        step,
    );
    if lines.len() < MAX_LINES_PER_AXIS * 2 {
        push_projectable(
            &mut lines,
            projection,
            ReferenceLine {
                start: Vec3::ZERO.to_array(),
                end: (normal * step * 3.0).to_array(),
                color: axis_color(normal),
                width: 2.0,
            },
        );
    }
    Some(GridFrame {
        minor_step: step,
        lines,
    })
}

#[must_use]
pub(crate) fn perspective_grid_plane(
    camera_position: [f32; 3],
    plane: GridPlane,
    minor_step: f32,
) -> Option<render::ViewportPerspectiveGrid> {
    let camera_position = Vec3::from_array(camera_position);
    let (axis_u, axis_v, normal) = plane_basis(plane);
    if !camera_position.is_finite() || !minor_step.is_finite() || minor_step <= 0.0 {
        return None;
    }
    let center = camera_position - normal * camera_position.dot(normal);
    let radius = PERSPECTIVE_GRID_BASE_RADIUS
        .max(camera_position.dot(normal).abs() * PERSPECTIVE_GRID_HEIGHT_SCALE);
    let corner = |u: f32, v: f32| render::ViewportVertex {
        position: (center + axis_u * radius * u + axis_v * radius * v).to_array(),
        color: [1.0; 4],
    };
    Some(render::ViewportPerspectiveGrid {
        vertices: vec![
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ],
        axis_u: axis_u.to_array(),
        axis_v: axis_v.to_array(),
        camera_position: camera_position.to_array(),
        minor_step,
        radius,
    })
}

fn append_axis_lines(
    lines: &mut Vec<ReferenceLine>,
    projection: &ViewportProjection,
    u: Vec3,
    v: Vec3,
    extent: (f32, f32, f32, f32),
    step: f32,
) {
    let (min_u, max_u, min_v, max_v) = extent;
    for index in 0..line_count(min_u, max_u, step) {
        let coordinate = min_u + index as f32 * step;
        push_projectable(
            lines,
            projection,
            grid_line(
                u * coordinate + v * min_v,
                u * coordinate + v * max_v,
                v,
                coordinate,
                step,
            ),
        );
    }
    for index in 0..line_count(min_v, max_v, step) {
        let coordinate = min_v + index as f32 * step;
        push_projectable(
            lines,
            projection,
            grid_line(
                v * coordinate + u * min_u,
                v * coordinate + u * max_u,
                u,
                coordinate,
                step,
            ),
        );
    }
}

fn grid_line(start: Vec3, end: Vec3, axis: Vec3, coordinate: f32, step: f32) -> ReferenceLine {
    let at_origin = coordinate.abs() <= step * 0.001;
    let major = ((coordinate / step).round() as i64).rem_euclid(10) == 0;
    ReferenceLine {
        start: start.to_array(),
        end: end.to_array(),
        color: if at_origin {
            axis_color(axis)
        } else if major {
            egui::Color32::from_rgb(86, 94, 101)
        } else {
            egui::Color32::from_rgb(57, 64, 70)
        },
        width: if at_origin { 2.0 } else { 1.0 },
    }
}

fn push_projectable(
    lines: &mut Vec<ReferenceLine>,
    projection: &ViewportProjection,
    line: ReferenceLine,
) {
    if projection
        .project_world_segment(line.start, line.end)
        .is_some()
    {
        lines.push(line);
    }
}

fn projected_spacing(
    projection: &ViewportProjection,
    anchor: Vec3,
    axis: Vec3,
    step: f32,
) -> Option<f32> {
    let start = projection.project_world_point(anchor.to_array())?;
    let end = projection.project_world_point((anchor + axis * step).to_array())?;
    let size = projection.viewport_size();
    let dx = (end[0] - start[0]) * size.width() * 0.5;
    let dy = (end[1] - start[1]) * size.height() * 0.5;
    let spacing = dx.hypot(dy);
    (spacing.is_finite() && spacing > f32::EPSILON).then_some(spacing)
}

fn plane_extent(points: &[Vec3], u: Vec3, v: Vec3) -> Option<(f32, f32, f32, f32)> {
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for point in points {
        let pu = point.dot(u);
        let pv = point.dot(v);
        min_u = min_u.min(pu);
        max_u = max_u.max(pu);
        min_v = min_v.min(pv);
        max_v = max_v.max(pv);
    }
    [min_u, max_u, min_v, max_v]
        .into_iter()
        .all(f32::is_finite)
        .then_some((min_u, max_u, min_v, max_v))
}

fn intersect_plane(ray: render::WorldRay, normal: Vec3) -> Option<Vec3> {
    let origin = Vec3::from_array(ray.origin);
    let direction = Vec3::from_array(ray.direction);
    let denominator = direction.dot(normal);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let distance = -origin.dot(normal) / denominator;
    let point = origin + direction * distance;
    (distance >= 0.0 && point.is_finite()).then_some(point)
}

fn line_count(minimum: f32, maximum: f32, step: f32) -> usize {
    (((maximum - minimum) / step).round() as usize).saturating_add(1)
}

const fn plane_basis(plane: GridPlane) -> (Vec3, Vec3, Vec3) {
    match plane {
        GridPlane::XY => (Vec3::X, Vec3::Y, Vec3::Z),
        GridPlane::XZ => (Vec3::X, Vec3::Z, Vec3::Y),
        GridPlane::YZ => (Vec3::Y, Vec3::Z, Vec3::X),
    }
}

fn axis_color(axis: Vec3) -> egui::Color32 {
    if axis == Vec3::X {
        egui::Color32::from_rgb(160, 60, 60)
    } else if axis == Vec3::Y {
        egui::Color32::from_rgb(60, 150, 80)
    } else {
        egui::Color32::from_rgb(80, 130, 240)
    }
}

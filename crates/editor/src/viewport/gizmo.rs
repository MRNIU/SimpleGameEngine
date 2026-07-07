// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::Transform;
use render::ViewportDrawCall;

use super::screen_position_for_vertex;

const GIZMO_HANDLE_LENGTH: f32 = 48.0;
const GIZMO_MOVE_HIT_SIZE: f32 = 10.0;
const GIZMO_SCALE_HIT_SIZE: f32 = 12.0;
const GIZMO_SCALE_OFFSET: egui::Vec2 = egui::vec2(14.0, -14.0);
const GIZMO_WORLD_UNITS_PER_PIXEL: f32 = 0.01;
const GIZMO_SCALE_PER_PIXEL: f32 = 0.01;
const MIN_GIZMO_SCALE: f32 = 0.01;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum GizmoMode {
    #[default]
    Move,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GizmoHandle {
    MoveX,
    MoveY,
    MoveZ,
    UniformScale,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GizmoDrag {
    pub(crate) target: EntityId,
    pub(crate) handle: GizmoHandle,
    pub(crate) start_pointer: egui::Pos2,
    pub(crate) start_transform: Transform,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TransformGizmoState {
    pub(crate) mode: GizmoMode,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
    drag: Option<GizmoDrag>,
}

impl TransformGizmoState {
    pub(crate) fn start_drag(&mut self, drag: GizmoDrag) {
        self.drag = Some(drag);
    }

    pub(crate) fn clear_drag(&mut self) {
        self.drag = None;
        self.active = None;
    }

    #[must_use]
    pub(crate) fn drag(&self) -> Option<&GizmoDrag> {
        self.drag.as_ref()
    }

    #[must_use]
    pub(crate) fn has_drag(&self) -> bool {
        self.drag.is_some()
    }

    #[must_use]
    pub(crate) fn hovered(&self) -> Option<GizmoHandle> {
        self.hovered
    }

    pub(crate) fn set_hovered(&mut self, hovered: Option<GizmoHandle>) {
        self.hovered = hovered;
    }

    #[must_use]
    pub(crate) fn active(&self) -> Option<GizmoHandle> {
        self.active
    }

    pub(crate) fn sync_active_from_drag(&mut self) {
        self.active = self.drag().map(|drag| drag.handle);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GizmoHandleRect {
    pub(crate) handle: GizmoHandle,
    pub(crate) center: egui::Pos2,
    pub(crate) axis: egui::Vec2,
    pub(crate) rect: egui::Rect,
}

impl GizmoHandleRect {
    #[must_use]
    pub(crate) fn new(
        handle: GizmoHandle,
        center: egui::Pos2,
        axis: egui::Vec2,
        size: f32,
    ) -> Self {
        Self {
            handle,
            center,
            axis: normalized_screen_axis(axis),
            rect: egui::Rect::from_center_size(center, egui::vec2(size, size)),
        }
    }
}

#[must_use]
pub(crate) fn gizmo_layout(
    draw: &ViewportDrawCall,
    rect: egui::Rect,
    selected: Option<&EntityId>,
    mode: GizmoMode,
) -> Vec<GizmoHandleRect> {
    let Some(selected) = selected else {
        return Vec::new();
    };
    let Some(span) = draw.cube_spans.iter().find(|span| &span.entity == selected) else {
        return Vec::new();
    };
    let Some(bounds) = span_screen_bounds(draw, span, rect) else {
        return Vec::new();
    };

    match mode {
        GizmoMode::Move => move_gizmo_handles(bounds.center()),
        GizmoMode::Scale => vec![GizmoHandleRect::new(
            GizmoHandle::UniformScale,
            egui::pos2(bounds.max.x, bounds.min.y) + GIZMO_SCALE_OFFSET,
            egui::Vec2::X - egui::Vec2::Y,
            GIZMO_SCALE_HIT_SIZE,
        )],
    }
}

#[must_use]
pub(crate) fn hit_test_gizmo(
    handles: &[GizmoHandleRect],
    pointer: egui::Pos2,
) -> Option<GizmoHandle> {
    handles
        .iter()
        .filter(|handle| handle.rect.contains(pointer))
        .min_by(|left, right| {
            pointer
                .distance_sq(left.center)
                .total_cmp(&pointer.distance_sq(right.center))
        })
        .map(|handle| handle.handle)
}

#[must_use]
pub(crate) fn gizmo_drag_from_press_origin(
    handles: &[GizmoHandleRect],
    press_origin: Option<egui::Pos2>,
    selected: Option<&EntityId>,
    selected_transform: Option<Transform>,
) -> Option<GizmoDrag> {
    let pointer = press_origin?;
    let target = selected?;
    let start_transform = selected_transform?;
    hit_test_gizmo(handles, pointer).map(|handle| GizmoDrag {
        target: target.clone(),
        handle,
        start_pointer: pointer,
        start_transform,
    })
}

#[must_use]
pub(crate) fn transform_for_gizmo_drag(
    handle: GizmoHandle,
    mut start: Transform,
    start_pointer: egui::Pos2,
    current_pointer: egui::Pos2,
) -> Transform {
    let delta = current_pointer - start_pointer;
    if !delta.x.is_finite() || !delta.y.is_finite() {
        return start;
    }

    match handle {
        GizmoHandle::MoveX => {
            start.translation[0] += delta.dot(egui::Vec2::X) * GIZMO_WORLD_UNITS_PER_PIXEL;
        }
        GizmoHandle::MoveY => {
            start.translation[1] += delta.dot(-egui::Vec2::Y) * GIZMO_WORLD_UNITS_PER_PIXEL;
        }
        GizmoHandle::MoveZ => {
            start.translation[2] += delta.dot(z_screen_axis()) * GIZMO_WORLD_UNITS_PER_PIXEL;
        }
        GizmoHandle::UniformScale => {
            let amount = delta.dot(z_screen_axis()) * GIZMO_SCALE_PER_PIXEL;
            let next_scale = start.scale.map(|value| value + amount);
            if next_scale.iter().any(|value| *value <= MIN_GIZMO_SCALE) {
                start.scale = [MIN_GIZMO_SCALE; 3];
            } else {
                start.scale = next_scale;
            }
        }
    }
    start
}

pub(crate) fn paint_gizmo_handles(
    painter: &egui::Painter,
    handles: &[GizmoHandleRect],
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    for handle in handles {
        let width = if active == Some(handle.handle) {
            4.0
        } else if hovered == Some(handle.handle) {
            3.0
        } else {
            2.0
        };
        match handle.handle {
            GizmoHandle::MoveX => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(width, egui::Color32::from_rgb(230, 80, 80)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(230, 80, 80));
            }
            GizmoHandle::MoveY => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(width, egui::Color32::from_rgb(80, 210, 110)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(80, 210, 110));
            }
            GizmoHandle::MoveZ => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(width, egui::Color32::from_rgb(90, 150, 240)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(90, 150, 240));
            }
            GizmoHandle::UniformScale => {
                painter.rect_filled(handle.rect, 1.0, egui::Color32::WHITE);
                painter.rect_stroke(
                    handle.rect,
                    1.0,
                    egui::Stroke::new(width, egui::Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

fn move_gizmo_handles(center: egui::Pos2) -> Vec<GizmoHandleRect> {
    vec![
        GizmoHandleRect::new(
            GizmoHandle::MoveX,
            center + egui::Vec2::X * GIZMO_HANDLE_LENGTH,
            egui::Vec2::X,
            GIZMO_MOVE_HIT_SIZE,
        ),
        GizmoHandleRect::new(
            GizmoHandle::MoveY,
            center - egui::Vec2::Y * GIZMO_HANDLE_LENGTH,
            -egui::Vec2::Y,
            GIZMO_MOVE_HIT_SIZE,
        ),
        GizmoHandleRect::new(
            GizmoHandle::MoveZ,
            center + z_screen_axis() * GIZMO_HANDLE_LENGTH,
            z_screen_axis(),
            GIZMO_MOVE_HIT_SIZE,
        ),
    ]
}

fn span_screen_bounds(
    draw: &ViewportDrawCall,
    span: &render::ViewportCubeSpan,
    rect: egui::Rect,
) -> Option<egui::Rect> {
    let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
    let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut found = false;
    for index in span.vertex_range.clone() {
        let Some(vertex) = draw.vertices.get(index) else {
            continue;
        };
        let screen = screen_position_for_vertex(rect, vertex.position);
        min.x = min.x.min(screen.x);
        min.y = min.y.min(screen.y);
        max.x = max.x.max(screen.x);
        max.y = max.y.max(screen.y);
        found = true;
    }
    found.then(|| egui::Rect::from_min_max(min, max))
}

fn z_screen_axis() -> egui::Vec2 {
    normalized_screen_axis(egui::Vec2::X - egui::Vec2::Y)
}

fn normalized_screen_axis(axis: egui::Vec2) -> egui::Vec2 {
    let length = axis.length();
    if length <= f32::EPSILON || !length.is_finite() {
        egui::Vec2::ZERO
    } else {
        axis / length
    }
}

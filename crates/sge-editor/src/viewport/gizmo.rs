// Copyright The SimpleGameEngine Contributors
//
//! Transform-gizmo state, hit testing and paint geometry.

use eframe::egui;
use sge_math::{Quat, Transform, Vec3};
use sge_scene::SceneEntityId;

use crate::{EditorLanguage, PreviewFrame, localization::EditorText};

use super::overlays::{ScreenPoint, project, projection};

const HANDLE_LENGTH: f32 = 46.0;
const HANDLE_SIZE: f32 = 14.0;
const UNITS_PER_PIXEL: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum GizmoMode {
    Select,
    #[default]
    Move,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    pub(super) const fn color(self) -> egui::Color32 {
        match self {
            Self::X => egui::Color32::from_rgb(230, 80, 80),
            Self::Y => egui::Color32::from_rgb(80, 210, 110),
            Self::Z => egui::Color32::from_rgb(90, 150, 240),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct GizmoDrag {
    pub(super) entity: SceneEntityId,
    pub(super) mode: GizmoMode,
    pub(super) axis: Axis,
    pub(super) screen_axis: egui::Vec2,
    pub(super) start: Transform,
    pub(super) preview: Transform,
    pub(super) pointer: egui::Pos2,
}

#[derive(Clone, Copy)]
pub(super) struct GizmoHandle {
    pub(super) axis: Axis,
    pub(super) screen_axis: egui::Vec2,
    pub(super) end: egui::Pos2,
    pub(super) hit: egui::Rect,
}

pub(super) fn transform_for_drag(mut drag: GizmoDrag, pointer: egui::Pos2) -> Transform {
    let amount = (pointer - drag.pointer).dot(drag.screen_axis) * UNITS_PER_PIXEL;
    drag.preview = drag.start;
    match drag.mode {
        GizmoMode::Select => {}
        GizmoMode::Move => {
            drag.preview.translation =
                (Vec3::from_array(drag.start.translation) + drag.axis.vector() * amount).to_array();
        }
        GizmoMode::Rotate => {
            drag.preview.rotation = (Quat::from_axis_angle(drag.axis.vector(), amount)
                * Quat::from_array(drag.start.rotation))
            .normalize()
            .to_array();
        }
        GizmoMode::Scale => {
            let factor = (1.0 + amount).max(0.01);
            let mut scale = drag.start.scale;
            match drag.axis {
                Axis::X => scale[0] = (scale[0] * factor).max(0.01),
                Axis::Y => scale[1] = (scale[1] * factor).max(0.01),
                Axis::Z => scale[2] = (scale[2] * factor).max(0.01),
            }
            drag.preview.scale = scale;
        }
    }
    drag.preview
}

impl GizmoMode {
    pub(super) const fn next(self) -> Self {
        match self {
            Self::Select | Self::Scale => Self::Move,
            Self::Move => Self::Rotate,
            Self::Rotate => Self::Scale,
        }
    }

    pub(super) fn status_text(self, language: EditorLanguage) -> String {
        let (text, key) = match self {
            Self::Select => (EditorText::Select, 'Q'),
            Self::Move => (EditorText::Move, 'W'),
            Self::Rotate => (EditorText::Rotate, 'E'),
            Self::Scale => (EditorText::Scale, 'R'),
        };
        format!("{} ({key})", language.text(text))
    }
}

pub(super) fn gizmo_transform(
    committed: Transform,
    drag: Option<GizmoDrag>,
    entity: SceneEntityId,
) -> Transform {
    drag.filter(|drag| drag.entity == entity)
        .map_or(committed, |drag| drag.preview)
}

pub(super) fn update_drag_preview(drag: &mut Option<GizmoDrag>, pointer: Option<egui::Pos2>) {
    if let (Some(drag), Some(pointer)) = (drag.as_mut(), pointer) {
        drag.preview = transform_for_drag(*drag, pointer);
    }
}

pub(super) fn gizmo_handles(
    frame: &PreviewFrame,
    rect: egui::Rect,
    transform: Transform,
) -> Vec<GizmoHandle> {
    let Some(matrix) = projection(frame, rect) else {
        return Vec::new();
    };
    let origin = Vec3::from_array(transform.translation);
    let Some(center) = project(matrix, origin, rect) else {
        return Vec::new();
    };
    Axis::ALL
        .into_iter()
        .filter_map(|axis| {
            let end = project(matrix, origin + axis.vector(), rect)?;
            let delta = end.position - center.position;
            let screen_axis = if delta.length_sq() > 0.0001 {
                delta.normalized()
            } else {
                return None;
            };
            let end = center.position + screen_axis * HANDLE_LENGTH;
            Some(GizmoHandle {
                axis,
                screen_axis,
                end,
                hit: egui::Rect::from_center_size(end, egui::vec2(HANDLE_SIZE, HANDLE_SIZE)),
            })
        })
        .collect()
}

pub(super) fn paint_gizmo(
    ui: &egui::Ui,
    transform: Transform,
    frame: &PreviewFrame,
    rect: egui::Rect,
    handles: &[GizmoHandle],
    mode: GizmoMode,
) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let Some(center) = project(matrix, Vec3::from_array(transform.translation), rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for handle in handles {
        painter.line_segment(
            [center.position, handle.end],
            egui::Stroke::new(3.0, handle.axis.color()),
        );
        match mode {
            GizmoMode::Select => {}
            GizmoMode::Move => {
                painter.rect_filled(handle.hit, 1.0, handle.axis.color());
            }
            GizmoMode::Rotate => {
                painter.circle_stroke(handle.end, 6.0, egui::Stroke::new(3.0, handle.axis.color()));
            }
            GizmoMode::Scale => {
                painter.rect_stroke(
                    handle.hit,
                    1.0,
                    egui::Stroke::new(3.0, handle.axis.color()),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

pub(super) fn pick_mesh(
    frame: &PreviewFrame,
    rect: egui::Rect,
    pointer: egui::Pos2,
) -> Option<SceneEntityId> {
    let matrix = projection(frame, rect)?;
    frame
        .snapshot
        .meshes()
        .iter()
        .filter_map(|instance| {
            let mesh = frame.assets.mesh(instance.mesh()).ok()?;
            let model = instance.transform().matrix();
            let mut closest = None::<f32>;
            for triangle in mesh.indices().chunks_exact(3) {
                let [a, b, c] = triangle else { continue };
                let points = [*a, *b, *c].map(|index| {
                    let vertex = mesh.vertices().get(index as usize)?;
                    project(
                        matrix,
                        model.transform_point3(Vec3::from_array(*vertex.position())),
                        rect,
                    )
                });
                let [Some(a), Some(b), Some(c)] = points else {
                    continue;
                };
                if let Some(depth) = triangle_depth(pointer, a, b, c) {
                    closest = Some(closest.map_or(depth, |current| current.min(depth)));
                }
            }
            let scene = frame
                .scene_entities
                .iter()
                .find(|(_, runtime)| *runtime == instance.entity())?
                .0;
            closest.map(|depth| (depth, scene))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, scene)| scene)
}

pub(super) fn triangle_depth(
    point: egui::Pos2,
    a: ScreenPoint,
    b: ScreenPoint,
    c: ScreenPoint,
) -> Option<f32> {
    let edge = |from: egui::Pos2, to: egui::Pos2, point: egui::Pos2| {
        (point.x - from.x) * (to.y - from.y) - (point.y - from.y) * (to.x - from.x)
    };
    let area = edge(a.position, b.position, c.position);
    if area.abs() <= f32::EPSILON {
        return None;
    }
    let wa = edge(b.position, c.position, point) / area;
    let wb = edge(c.position, a.position, point) / area;
    let wc = 1.0 - wa - wb;
    (wa >= 0.0 && wb >= 0.0 && wc >= 0.0).then_some(wa * a.depth + wb * b.depth + wc * c.depth)
}

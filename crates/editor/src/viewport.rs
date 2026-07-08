// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::Transform;
use render::{ViewportDrawCall, fit_viewport_draw_to_size};

mod camera;
mod gizmo;
mod wgpu_bridge;

pub(crate) use camera::{ViewCamera, ViewMoveInput, ViewPreset};
#[cfg(test)]
pub(crate) use gizmo::GizmoHandleRect;
pub(crate) use gizmo::{
    GizmoDrag, GizmoHandle, GizmoMode, TransformGizmoState, gizmo_drag_from_press_origin,
    gizmo_layout, hit_test_gizmo, paint_gizmo_handles, transform_for_gizmo_drag,
};
use wgpu_bridge::paint_wgpu_viewport;
pub(crate) use wgpu_bridge::{ViewportWgpuProbe, install_viewport_renderer};

const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);

pub(crate) struct ViewportUiOptions<'a> {
    pub(crate) keyboard_shortcuts_allowed: bool,
    pub(crate) fit_view_requested: bool,
    pub(crate) wgpu_probe: Option<&'a ViewportWgpuProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReferenceLine {
    pub(crate) start: [f32; 3],
    pub(crate) end: [f32; 3],
    pub(crate) color: egui::Color32,
    pub(crate) width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OrientationCubeLayout {
    pub(crate) top: egui::Rect,
    pub(crate) bottom: egui::Rect,
    pub(crate) front: egui::Rect,
    pub(crate) back: egui::Rect,
    pub(crate) right: egui::Rect,
    pub(crate) left: egui::Rect,
    pub(crate) perspective: egui::Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ViewportAction {
    None,
    Select(EntityId),
    ClearSelection,
    PreviewTransform {
        target: EntityId,
        transform: Transform,
    },
    CommitTransform {
        target: EntityId,
        before: Transform,
        after: Transform,
    },
    RestoreTransform {
        target: EntityId,
        transform: Transform,
    },
    SetViewPreset(ViewPreset),
    ReturnToPerspective,
    Status(String),
}

pub(crate) fn draw_viewport(
    ui: &mut egui::Ui,
    draw: Option<&ViewportDrawCall>,
    selected: Option<&EntityId>,
    selected_transform: Option<Transform>,
    camera: &mut ViewCamera,
    gizmo: &mut TransformGizmoState,
    options: ViewportUiOptions<'_>,
) -> ViewportAction {
    let ViewportUiOptions {
        keyboard_shortcuts_allowed,
        fit_view_requested,
        wgpu_probe,
    } = options;
    ui.heading("Viewport");
    let (rect, response) = ui.allocate_exact_size(
        viewport_canvas_size(ui.available_size_before_wrap()),
        egui::Sense::click_and_drag(),
    );
    let mut action = ViewportAction::None;
    let right_down = ui.input(|input| input.pointer.secondary_down());
    let pointer_delta = ui.input(|input| input.pointer.delta());
    if response.dragged_by(egui::PointerButton::Secondary) && right_down {
        camera.look(pointer_delta);
    }
    let scroll_y = ui.input(|input| input.smooth_scroll_delta.y);
    if response.hovered() && scroll_y != 0.0 {
        camera.adjust_speed(scroll_y);
    }
    if right_down && response.hovered() {
        ui.ctx().request_repaint();
        camera.move_local(
            ViewMoveInput {
                forward: ui.input(|input| input.key_down(egui::Key::W)),
                backward: ui.input(|input| input.key_down(egui::Key::S)),
                left: ui.input(|input| input.key_down(egui::Key::A)),
                right: ui.input(|input| input.key_down(egui::Key::D)),
            },
            ui.input(|input| input.stable_dt),
        );
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 24, 29));
    let fitted_draw =
        draw.map(|draw| fit_viewport_draw_to_size(draw, [rect.width(), rect.height()]));
    let f_pressed = ui.input(|input| input.key_pressed(egui::Key::F));
    let keyboard_fit_requested = keyboard_shortcuts_allowed && f_pressed;
    let fit_requested = fit_view_requested || keyboard_fit_requested;
    if fit_requested {
        match draw {
            Some(draw) if camera.fit_draw(draw, selected) => {
                ui.ctx().request_repaint();
            }
            Some(_) | None => action = ViewportAction::Status("No visible mesh to fit".to_owned()),
        }
    }
    let handles = fitted_draw.as_ref().map_or_else(Vec::new, |draw| {
        gizmo_layout(draw, rect, selected, gizmo.mode)
    });
    gizmo.set_hovered(
        response
            .hover_pos()
            .and_then(|pointer| hit_test_gizmo(&handles, pointer)),
    );
    gizmo.sync_active_from_drag();
    let primary_down = ui.input(|input| input.pointer.primary_down());
    let primary_pressed = ui.input(|input| input.pointer.primary_pressed());
    let press_origin = ui.input(|input| input.pointer.press_origin());
    let esc_pressed = ui.input(|input| input.key_pressed(egui::Key::Escape));
    let mut pointer_consumed_by_gizmo = false;
    let orientation_layout = orientation_cube_layout(rect);

    if esc_pressed && let Some(drag) = gizmo.drag().cloned() {
        gizmo.clear_drag();
        return ViewportAction::RestoreTransform {
            target: drag.target,
            transform: drag.start_transform,
        };
    }

    if primary_pressed
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(overlay_action) = orientation_cube_hit_test(&orientation_layout, pointer)
    {
        return overlay_action;
    }

    if !primary_down && let Some(drag) = gizmo.drag().cloned() {
        pointer_consumed_by_gizmo = true;
        gizmo.clear_drag();
        if let Some(pointer) = response.interact_pointer_pos() {
            return ViewportAction::CommitTransform {
                target: drag.target,
                before: drag.start_transform,
                after: transform_for_gizmo_drag(
                    drag.handle,
                    drag.start_transform,
                    drag.start_pointer,
                    pointer,
                ),
            };
        }
    }

    if let Some(drag) = gizmo.drag().cloned() {
        pointer_consumed_by_gizmo = true;
        match selected {
            Some(selected) if selected == &drag.target => {
                if let Some(pointer) = response.interact_pointer_pos() {
                    action = ViewportAction::PreviewTransform {
                        target: drag.target,
                        transform: transform_for_gizmo_drag(
                            drag.handle,
                            drag.start_transform,
                            drag.start_pointer,
                            pointer,
                        ),
                    };
                }
            }
            _ => {
                gizmo.clear_drag();
            }
        }
    }

    if primary_pressed
        && gizmo.drag().is_none()
        && let Some(drag) =
            gizmo_drag_from_press_origin(&handles, press_origin, selected, selected_transform)
    {
        pointer_consumed_by_gizmo = true;
        gizmo.start_drag(drag);
    }

    if response.clicked_by(egui::PointerButton::Primary)
        && !pointer_consumed_by_gizmo
        && let (Some(draw), Some(pointer)) = (fitted_draw.as_ref(), response.interact_pointer_pos())
    {
        action = hit_test_viewport_draw(draw, rect, pointer);
    }

    if let Some((draw, probe)) = fitted_draw.as_ref().zip(wgpu_probe) {
        paint_wgpu_viewport(&painter, rect, draw, probe);
    } else if let Some(draw) = fitted_draw.as_ref() {
        paint_fallback_viewport(rect, &painter, draw);
    }
    let view = camera.to_viewport_view();
    if let Some(projection) = render::ViewportProjection::from_view(&view) {
        paint_reference_lines(&painter, rect, &projection);
    }
    painter.text(
        rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        camera.hint_text(draw, selected),
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(205, 214, 224),
    );
    paint_orientation_cube(&painter, &orientation_layout);
    paint_gizmo_handles(&painter, &handles, gizmo.hovered(), gizmo.active());
    action
}

pub(crate) fn reference_lines() -> Vec<ReferenceLine> {
    let mut lines = Vec::new();
    for i in -10..=10 {
        let value = i as f32;
        lines.push(ReferenceLine {
            start: [-10.0, value, 0.0],
            end: [10.0, value, 0.0],
            color: egui::Color32::from_rgb(65, 72, 78),
            width: 1.0,
        });
        lines.push(ReferenceLine {
            start: [value, -10.0, 0.0],
            end: [value, 10.0, 0.0],
            color: egui::Color32::from_rgb(65, 72, 78),
            width: 1.0,
        });
    }
    lines.push(ReferenceLine {
        start: [-10.0, 0.0, 0.0],
        end: [10.0, 0.0, 0.0],
        color: egui::Color32::from_rgb(160, 60, 60),
        width: 2.0,
    });
    lines.push(ReferenceLine {
        start: [0.0, -10.0, 0.0],
        end: [0.0, 10.0, 0.0],
        color: egui::Color32::from_rgb(60, 150, 80),
        width: 2.0,
    });
    lines.push(ReferenceLine {
        start: [0.0, 0.0, 0.0],
        end: [0.0, 0.0, 3.0],
        color: egui::Color32::from_rgb(80, 130, 240),
        width: 2.0,
    });
    lines
}

pub(crate) fn orientation_cube_layout(rect: egui::Rect) -> OrientationCubeLayout {
    let size = egui::vec2(30.0, 22.0);
    let origin = rect.right_top() + egui::vec2(-118.0, 12.0);
    OrientationCubeLayout {
        top: egui::Rect::from_min_size(origin + egui::vec2(36.0, 0.0), size),
        bottom: egui::Rect::from_min_size(origin + egui::vec2(36.0, 52.0), size),
        front: egui::Rect::from_min_size(origin + egui::vec2(36.0, 26.0), size),
        back: egui::Rect::from_min_size(origin + egui::vec2(72.0, 26.0), size),
        right: egui::Rect::from_min_size(origin + egui::vec2(72.0, 0.0), size),
        left: egui::Rect::from_min_size(origin, size),
        perspective: egui::Rect::from_min_size(
            origin + egui::vec2(0.0, 82.0),
            egui::vec2(102.0, 22.0),
        ),
    }
}

pub(crate) fn orientation_cube_hit_test(
    layout: &OrientationCubeLayout,
    pointer: egui::Pos2,
) -> Option<ViewportAction> {
    [
        (layout.top, ViewportAction::SetViewPreset(ViewPreset::Top)),
        (
            layout.bottom,
            ViewportAction::SetViewPreset(ViewPreset::Bottom),
        ),
        (
            layout.front,
            ViewportAction::SetViewPreset(ViewPreset::Front),
        ),
        (layout.back, ViewportAction::SetViewPreset(ViewPreset::Back)),
        (
            layout.right,
            ViewportAction::SetViewPreset(ViewPreset::Right),
        ),
        (layout.left, ViewportAction::SetViewPreset(ViewPreset::Left)),
        (layout.perspective, ViewportAction::ReturnToPerspective),
    ]
    .into_iter()
    .find_map(|(rect, action)| rect.contains(pointer).then_some(action))
}

fn viewport_canvas_size(available: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        if available.x > 0.0 {
            available.x
        } else {
            VIEWPORT_MIN_SIZE.x
        },
        if available.y > 0.0 {
            available.y
        } else {
            VIEWPORT_MIN_SIZE.y
        },
    )
}

fn paint_fallback_viewport(rect: egui::Rect, painter: &egui::Painter, draw: &ViewportDrawCall) {
    let min_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::INFINITY, f32::min);
    let max_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let to_screen = |x: f32, y: f32| rect.center() + egui::vec2(x * 86.0, -y * 86.0);
    let cube = egui::Rect::from_two_pos(to_screen(min_x, min_y), to_screen(max_x, max_y));
    painter.rect_filled(cube, 2.0, egui::Color32::from_rgb(77, 163, 255));
    painter.rect_stroke(
        cube,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );
}

fn paint_reference_lines(
    painter: &egui::Painter,
    rect: egui::Rect,
    projection: &render::ViewportProjection,
) {
    for line in reference_lines() {
        let Some(start) = projection.project_world_point(line.start) else {
            continue;
        };
        let Some(end) = projection.project_world_point(line.end) else {
            continue;
        };
        painter.line_segment(
            [
                screen_position_for_vertex(rect, [start[0], start[1], 0.0]),
                screen_position_for_vertex(rect, [end[0], end[1], 0.0]),
            ],
            egui::Stroke::new(line.width, line.color),
        );
    }
}

fn paint_orientation_cube(painter: &egui::Painter, layout: &OrientationCubeLayout) {
    for (rect, label, color) in [
        (layout.top, "Top", egui::Color32::from_rgb(80, 130, 240)),
        (layout.bottom, "Bot", egui::Color32::from_rgb(80, 130, 240)),
        (layout.front, "Front", egui::Color32::from_rgb(160, 60, 60)),
        (layout.back, "Back", egui::Color32::from_rgb(160, 60, 60)),
        (layout.right, "Right", egui::Color32::from_rgb(60, 150, 80)),
        (layout.left, "Left", egui::Color32::from_rgb(60, 150, 80)),
        (
            layout.perspective,
            "Perspective",
            egui::Color32::from_rgb(42, 48, 54),
        ),
    ] {
        painter.rect_filled(rect, 2.0, color);
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
}

#[must_use]
pub(crate) fn screen_position_for_vertex(rect: egui::Rect, position: [f32; 3]) -> egui::Pos2 {
    egui::pos2(
        rect.left() + (position[0] + 1.0) * 0.5 * rect.width(),
        rect.top() + (1.0 - (position[1] + 1.0) * 0.5) * rect.height(),
    )
}

#[must_use]
pub(crate) fn hit_test_viewport_draw(
    draw: &ViewportDrawCall,
    rect: egui::Rect,
    pointer: egui::Pos2,
) -> ViewportAction {
    if !rect.is_positive() {
        return ViewportAction::None;
    }
    let mut best: Option<(f32, EntityId)> = None;
    for span in &draw.mesh_spans {
        let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
        let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for index in span.vertex_range.clone() {
            let Some(vertex) = draw.vertices.get(index) else {
                continue;
            };
            let screen = screen_position_for_vertex(rect, vertex.position);
            min.x = min.x.min(screen.x);
            min.y = min.y.min(screen.y);
            max.x = max.x.max(screen.x);
            max.y = max.y.max(screen.y);
        }
        let bounds = egui::Rect::from_min_max(min, max);
        if bounds.contains(pointer) {
            let distance = pointer.distance_sq(bounds.center());
            if best
                .as_ref()
                .is_none_or(|(best_distance, _)| distance < *best_distance)
            {
                best = Some((distance, span.entity.clone()));
            }
        }
    }
    best.map_or(ViewportAction::ClearSelection, |(_, entity)| {
        ViewportAction::Select(entity)
    })
}

#[cfg(test)]
mod tests;

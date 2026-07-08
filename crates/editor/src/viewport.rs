// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::Transform;
use render::{ViewportDrawCall, fit_viewport_draw_to_size};

mod camera;
mod gizmo;
mod wgpu_bridge;

pub(crate) use camera::{ViewCamera, ViewMoveInput};
#[cfg(test)]
pub(crate) use gizmo::GizmoHandleRect;
pub(crate) use gizmo::{
    GizmoDrag, GizmoHandle, GizmoMode, TransformGizmoState, gizmo_drag_from_press_origin,
    gizmo_layout, hit_test_gizmo, paint_gizmo_handles, transform_for_gizmo_drag,
};
use wgpu_bridge::paint_wgpu_viewport;
pub(crate) use wgpu_bridge::{ViewportWgpuProbe, install_viewport_renderer};

const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);
pub(crate) const EDITOR_CAMERA_LABEL: &str = "Editor Camera";
pub(crate) const PILOT_CAMERA_LABEL: &str = "Pilot Camera";

pub(crate) struct ViewportUiOptions<'a> {
    pub(crate) keyboard_shortcuts_allowed: bool,
    pub(crate) fit_view_requested: bool,
    pub(crate) view_mode_label: &'a str,
    pub(crate) wgpu_probe: Option<&'a ViewportWgpuProbe>,
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
        view_mode_label,
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

    if esc_pressed && let Some(drag) = gizmo.drag().cloned() {
        gizmo.clear_drag();
        return ViewportAction::RestoreTransform {
            target: drag.target,
            transform: drag.start_transform,
        };
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
    painter.text(
        rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        view_mode_label,
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(205, 214, 224),
    );
    paint_gizmo_handles(&painter, &handles, gizmo.hovered(), gizmo.active());
    action
}

fn viewport_canvas_size(available: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        available.x.max(VIEWPORT_MIN_SIZE.x),
        available.y.max(VIEWPORT_MIN_SIZE.y),
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

// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::{Transform, Vec3};
use render::{
    ViewportClipPlanes, ViewportDrawCall, ViewportProjection, ViewportSize, ViewportView,
};

mod camera;
mod gizmo;
mod grid;
mod wgpu_bridge;

pub(crate) use camera::{ViewCamera, ViewMoveInput, ViewPreset};
#[cfg(test)]
pub(crate) use gizmo::GizmoHandleRect;
pub(crate) use gizmo::{
    GizmoDrag, GizmoHandle, GizmoMode, TransformGizmoState, gizmo_drag_from_press_origin,
    gizmo_layout, hit_test_gizmo, paint_gizmo_handles, transform_for_gizmo_drag,
    transform_for_gizmo_drag_along_axis,
};
pub(crate) use grid::{GridPlane, adaptive_grid_lines};
use wgpu_bridge::paint_wgpu_viewport;
pub(crate) use wgpu_bridge::{ViewportWgpuProbe, install_viewport_renderer};

const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);

pub(crate) struct ViewportUiOptions<'a> {
    pub(crate) keyboard_shortcuts_allowed: bool,
    pub(crate) fit_view_requested: bool,
    pub(crate) navigation_enabled: bool,
    pub(crate) view_override: Option<&'a ViewportView>,
    pub(crate) hint_text_override: Option<&'a str>,
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

#[must_use]
pub(crate) const fn camera_navigation_requested(
    alt_down: bool,
    viewport_hovered: bool,
    primary_down: bool,
    middle_down: bool,
    right_down: bool,
) -> bool {
    let any_navigation_button = primary_down || middle_down || right_down;
    let camera_button_allowed = alt_down || middle_down || right_down;
    viewport_hovered && any_navigation_button && camera_button_allowed
}

#[must_use]
pub(crate) const fn can_start_gizmo_drag(
    primary_pressed: bool,
    drag_active: bool,
    pointer_consumed_by_camera: bool,
) -> bool {
    primary_pressed && !drag_active && !pointer_consumed_by_camera
}

#[must_use]
pub(crate) const fn can_select_viewport(
    primary_clicked: bool,
    pointer_consumed_by_gizmo: bool,
    pointer_consumed_by_camera: bool,
) -> bool {
    primary_clicked && !pointer_consumed_by_gizmo && !pointer_consumed_by_camera
}

#[must_use]
pub(crate) const fn plain_lmb_navigation_requested(
    viewport_hovered: bool,
    primary_down: bool,
    other_navigation_input: bool,
    gizmo_drag_active: bool,
    primary_dragged: bool,
    orthographic: bool,
) -> bool {
    viewport_hovered
        && primary_down
        && !other_navigation_input
        && !gizmo_drag_active
        && primary_dragged
        && !orthographic
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
        navigation_enabled,
        view_override,
        hint_text_override,
        wgpu_probe,
    } = options;
    ui.heading("Viewport");
    let (rect, response) = ui.allocate_exact_size(
        viewport_canvas_size(ui.available_size_before_wrap()),
        egui::Sense::click_and_drag(),
    );
    let mut action = ViewportAction::None;
    let pointer_delta = ui.input(|input| input.pointer.delta());
    let scroll_y = ui.input(|input| input.smooth_scroll_delta.y);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 24, 29));
    let viewport_size = ViewportSize::new(rect.width(), rect.height());
    let view = view_override
        .cloned()
        .or_else(|| viewport_size.map(|size| camera.to_viewport_view(size)));
    let projection = view.as_ref().zip(viewport_size).and_then(|(view, size)| {
        ViewportProjection::from_view(view, size, ViewportClipPlanes::DEFAULT)
    });
    let f_pressed = ui.input(|input| input.key_pressed(egui::Key::F));
    let keyboard_fit_requested = keyboard_shortcuts_allowed && f_pressed;
    let fit_requested = fit_view_requested || keyboard_fit_requested;
    if fit_requested {
        if navigation_enabled {
            camera.frame_visible(draw, selected);
            ui.ctx().request_repaint();
        } else {
            action =
                ViewportAction::Status("Disable Pilot Camera to navigate editor view".to_owned());
        }
    }
    let handles = draw
        .zip(projection.as_ref())
        .map_or_else(Vec::new, |(draw, projection)| {
            gizmo_layout(draw, projection, rect, selected, gizmo.mode)
        });
    gizmo.set_hovered(
        response
            .hover_pos()
            .and_then(|pointer| hit_test_gizmo(&handles, pointer)),
    );
    gizmo.sync_active_from_drag();
    let primary_down = ui.input(|input| input.pointer.primary_down());
    let primary_pressed = ui.input(|input| input.pointer.primary_pressed());
    let middle_down = ui.input(|input| input.pointer.middle_down());
    let right_down = ui.input(|input| input.pointer.secondary_down());
    let alt_down = ui.input(|input| input.modifiers.alt);
    let press_origin = ui.input(|input| input.pointer.press_origin());
    let esc_pressed = ui.input(|input| input.key_pressed(egui::Key::Escape));
    let mut pointer_consumed_by_gizmo = false;
    let mut pointer_consumed_by_camera = false;
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
                after: transform_for_gizmo_drag_along_axis(
                    drag.handle,
                    gizmo_axis(&handles, drag.handle),
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
                        transform: transform_for_gizmo_drag_along_axis(
                            drag.handle,
                            gizmo_axis(&handles, drag.handle),
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

    let viewport_hovered = response.hovered();
    let camera_navigation = camera_navigation_requested(
        alt_down,
        viewport_hovered,
        primary_down,
        middle_down,
        right_down,
    );
    if camera_navigation {
        pointer_consumed_by_camera = true;
        if navigation_enabled {
            camera.begin_navigation();
            ui.ctx().request_repaint();
            if camera.is_orthographic() {
                if primary_down && right_down {
                    camera.ortho_zoom(-pointer_delta.y * 0.02);
                } else if right_down || middle_down {
                    camera.ortho_pan(pointer_delta);
                }
            } else if alt_down && primary_down {
                if response.dragged_by(egui::PointerButton::Primary) {
                    camera.orbit(pointer_delta);
                }
            } else if alt_down && middle_down {
                if response.dragged_by(egui::PointerButton::Middle) {
                    camera.pan(pointer_delta);
                }
            } else if alt_down && right_down {
                if response.dragged_by(egui::PointerButton::Secondary) {
                    camera.dolly(pointer_delta.y);
                }
            } else if middle_down || (primary_down && right_down) {
                camera.pan(pointer_delta);
            } else if right_down {
                if response.dragged_by(egui::PointerButton::Secondary) {
                    camera.look(pointer_delta);
                }
                if scroll_y != 0.0 {
                    camera.adjust_speed_level(if scroll_y > 0.0 { 1 } else { -1 });
                }
                camera.move_local(
                    ViewMoveInput {
                        forward: ui.input(|input| input.key_down(egui::Key::W)),
                        backward: ui.input(|input| input.key_down(egui::Key::S)),
                        left: ui.input(|input| input.key_down(egui::Key::A)),
                        right: ui.input(|input| input.key_down(egui::Key::D)),
                        up: ui.input(|input| input.key_down(egui::Key::E)),
                        down: ui.input(|input| input.key_down(egui::Key::Q)),
                    },
                    ui.input(|input| input.stable_dt),
                );
            }
        } else {
            action =
                ViewportAction::Status("Disable Pilot Camera to navigate editor view".to_owned());
        }
    }

    if viewport_hovered && scroll_y != 0.0 && !right_down {
        pointer_consumed_by_camera = true;
        if navigation_enabled {
            if camera.is_orthographic() {
                camera.ortho_zoom(scroll_y.signum());
            } else {
                camera.wheel_move(scroll_y);
            }
            ui.ctx().request_repaint();
        }
    }

    if plain_lmb_navigation_requested(
        viewport_hovered,
        primary_down,
        alt_down || middle_down || right_down,
        gizmo.drag().is_some(),
        response.dragged_by(egui::PointerButton::Primary),
        camera.is_orthographic(),
    ) && gizmo.hovered().is_none()
    {
        pointer_consumed_by_camera = true;
        if navigation_enabled {
            camera.lmb_navigate(pointer_delta);
            ui.ctx().request_repaint();
        }
    }

    if can_start_gizmo_drag(
        primary_pressed,
        gizmo.drag().is_some(),
        pointer_consumed_by_camera,
    ) && let Some(drag) =
        gizmo_drag_from_press_origin(&handles, press_origin, selected, selected_transform)
    {
        pointer_consumed_by_gizmo = true;
        gizmo.start_drag(drag);
    }

    if can_select_viewport(
        response.clicked_by(egui::PointerButton::Primary),
        pointer_consumed_by_gizmo,
        pointer_consumed_by_camera,
    ) && let (Some(draw), Some(projection), Some(pointer)) =
        (draw, projection.as_ref(), response.interact_pointer_pos())
    {
        action = hit_test_viewport_draw(draw, projection, rect, pointer);
    }

    let grid_frame = projection.as_ref().and_then(|projection| {
        grid::adaptive_grid_lines(projection, camera.grid_plane(), camera.grid_minor_step())
    });
    if let Some(frame) = grid_frame.as_ref() {
        camera.set_grid_minor_step(frame.minor_step);
    }
    let grid_lines = grid_frame
        .as_ref()
        .map_or(&[] as &[ReferenceLine], |frame| frame.lines.as_slice());
    if let (Some(projection), Some(probe)) = (projection.as_ref(), wgpu_probe) {
        paint_wgpu_viewport(&painter, rect, draw, grid_lines, projection, probe);
    } else if let Some(projection) = projection.as_ref() {
        paint_reference_lines(&painter, rect, projection, grid_lines);
        if let Some(draw) = draw {
            paint_fallback_viewport(rect, &painter, draw, projection);
        }
    }
    let hint_text = hint_text_override.map_or_else(
        || camera.hint_text(draw, selected),
        std::borrow::ToOwned::to_owned,
    );
    painter.text(
        rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        hint_text,
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(205, 214, 224),
    );
    paint_orientation_cube(&painter, &orientation_layout);
    paint_gizmo_handles(&painter, &handles, gizmo.hovered(), gizmo.active());
    action
}

fn gizmo_axis(handles: &[gizmo::GizmoHandleRect], target: GizmoHandle) -> egui::Vec2 {
    handles
        .iter()
        .find(|handle| handle.handle == target)
        .map_or_else(|| gizmo::default_screen_axis(target), |handle| handle.axis)
}

#[must_use]
pub(crate) fn pilot_camera_hint_text(
    view: &ViewportView,
    draw: Option<&ViewportDrawCall>,
    selected: Option<&EntityId>,
) -> String {
    let projection = match view.projection {
        ecs::Projection::Perspective { fov_y_degrees } => {
            format!("Perspective FOV {fov_y_degrees:.1}")
        }
        ecs::Projection::Orthographic { vertical_size } => {
            format!("Orthographic Size {vertical_size:.2}")
        }
    };
    let center = draw
        .and_then(|draw| visible_mesh_center(draw, selected))
        .unwrap_or(Vec3::ZERO);
    let distance = center.distance(Vec3::from_array(view.transform.translation));
    let mesh_count = draw.map_or(0, |draw| draw.mesh_spans.len());
    format!("Pilot Camera\n{projection}  Distance {distance:.2}  Meshes {mesh_count}")
}

fn visible_mesh_center(draw: &ViewportDrawCall, selected: Option<&EntityId>) -> Option<Vec3> {
    if let Some(center) = selected
        .and_then(|id| draw.mesh_spans.iter().find(|span| &span.entity == id))
        .map(|span| Vec3::from_array(span.world_center))
        .filter(|center| center.is_finite())
    {
        return Some(center);
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for span in &draw.mesh_spans {
        let span_min = Vec3::from_array(span.world_bounds_min);
        let span_max = Vec3::from_array(span.world_bounds_max);
        if !span_min.is_finite() || !span_max.is_finite() {
            continue;
        }
        min = min.min(span_min);
        max = max.max(span_max);
        found = true;
    }
    let center = (min + max) * 0.5;
    (found && center.is_finite()).then_some(center)
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

fn paint_fallback_viewport(
    rect: egui::Rect,
    painter: &egui::Painter,
    draw: &ViewportDrawCall,
    projection: &ViewportProjection,
) {
    let mut projected = draw
        .vertices
        .iter()
        .filter_map(|vertex| projection.project_world_point(vertex.position));
    let Some(first) = projected.next() else {
        return;
    };
    let (mut min, mut max) = (first, first);
    for point in projected {
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
    }
    let cube = egui::Rect::from_two_pos(
        screen_position_for_vertex(rect, [min[0], min[1], 0.0]),
        screen_position_for_vertex(rect, [max[0], max[1], 0.0]),
    );
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
    lines: &[ReferenceLine],
) {
    for line in lines {
        let Some([start, end]) = projection.project_world_segment(line.start, line.end) else {
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
    projection: &ViewportProjection,
    rect: egui::Rect,
    pointer: egui::Pos2,
) -> ViewportAction {
    if !rect.is_positive() {
        return ViewportAction::None;
    }
    let pointer_ndc = [
        (pointer.x - rect.left()) / rect.width() * 2.0 - 1.0,
        1.0 - (pointer.y - rect.top()) / rect.height() * 2.0,
    ];
    let Some(ray) = projection.screen_ray(pointer_ndc) else {
        return ViewportAction::None;
    };
    let mut best: Option<(f32, EntityId)> = None;
    for span in &draw.mesh_spans {
        let Some(indices) = draw.indices.get(span.index_range.clone()) else {
            continue;
        };
        for triangle in indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (
                draw.vertices.get(usize::from(triangle[0])),
                draw.vertices.get(usize::from(triangle[1])),
                draw.vertices.get(usize::from(triangle[2])),
            ) else {
                continue;
            };
            if let Some(distance) = ray_triangle_distance(ray, [a.position, b.position, c.position])
                && best
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

fn ray_triangle_distance(ray: render::WorldRay, triangle: [[f32; 3]; 3]) -> Option<f32> {
    let origin = Vec3::from_array(ray.origin);
    let direction = Vec3::from_array(ray.direction);
    let a = Vec3::from_array(triangle[0]);
    let edge_ab = Vec3::from_array(triangle[1]) - a;
    let edge_ac = Vec3::from_array(triangle[2]) - a;
    let p = direction.cross(edge_ac);
    let determinant = edge_ab.dot(p);
    if determinant.abs() <= f32::EPSILON {
        return None;
    }
    let inverse = determinant.recip();
    let offset = origin - a;
    let u = offset.dot(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = offset.cross(edge_ab);
    let v = direction.dot(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge_ac.dot(q) * inverse;
    (distance.is_finite() && distance > 0.0).then_some(distance)
}

#[cfg(test)]
mod tests;

// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use ecs::EntityId;
use eframe::{egui, egui_wgpu, wgpu};
use math::{Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportRenderer, ViewportView, fit_viewport_draw_to_size};

const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);
const EDITOR_VIEW_ENTITY: &str = "editor_view";
const LOOK_SENSITIVITY: f32 = 0.01;
const MOVE_SCALE: f32 = 4.0;
const SPEED_SCROLL_SCALE: f32 = 0.05;
const FIT_SCREEN_TO_WORLD_SCALE: f32 = 1.0 / 0.12;
const GIZMO_HANDLE_LENGTH: f32 = 48.0;
const GIZMO_MOVE_HIT_SIZE: f32 = 10.0;
const GIZMO_SCALE_HIT_SIZE: f32 = 12.0;
const GIZMO_SCALE_OFFSET: egui::Vec2 = egui::vec2(14.0, -14.0);
const GIZMO_WORLD_UNITS_PER_PIXEL: f32 = 0.01;
const GIZMO_SCALE_PER_PIXEL: f32 = 0.01;
const MIN_GIZMO_SCALE: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    speed: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ViewMoveInput {
    pub(crate) forward: bool,
    pub(crate) backward: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
}

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
    drag: Option<GizmoDrag>,
}

impl TransformGizmoState {
    pub(crate) fn start_drag(&mut self, drag: GizmoDrag) {
        self.drag = Some(drag);
    }

    pub(crate) fn clear_drag(&mut self) {
        self.drag = None;
    }

    #[must_use]
    pub(crate) fn drag(&self) -> Option<&GizmoDrag> {
        self.drag.as_ref()
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

impl ViewCamera {
    pub(crate) const MIN_SPEED: f32 = 0.05;
    pub(crate) const MAX_SPEED: f32 = 20.0;
    pub(crate) const MIN_PITCH: f32 = -1.45;
    pub(crate) const MAX_PITCH: f32 = 1.45;

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn pitch(self) -> f32 {
        self.pitch
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn speed(self) -> f32 {
        self.speed
    }

    pub(crate) fn look(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.yaw -= delta.x * LOOK_SENSITIVITY;
        self.pitch =
            (self.pitch - delta.y * LOOK_SENSITIVITY).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
    }

    pub(crate) fn adjust_speed(&mut self, scroll_y: f32) {
        if !scroll_y.is_finite() {
            return;
        }
        self.speed =
            (self.speed + scroll_y * SPEED_SCROLL_SCALE).clamp(Self::MIN_SPEED, Self::MAX_SPEED);
    }

    pub(crate) fn move_local(&mut self, input: ViewMoveInput, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let rotation = self.rotation();
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        let mut direction = Vec3::ZERO;
        if input.forward {
            direction += forward;
        }
        if input.backward {
            direction -= forward;
        }
        if input.right {
            direction += right;
        }
        if input.left {
            direction -= right;
        }
        if direction.length_squared() <= f32::EPSILON {
            return;
        }
        let moved =
            Vec3::from_array(self.position) + direction.normalize() * self.speed * dt * MOVE_SCALE;
        self.position = moved.to_array();
    }

    #[must_use]
    pub(crate) fn to_viewport_view(self) -> ViewportView {
        ViewportView::new(
            EntityId::new(EDITOR_VIEW_ENTITY),
            Transform {
                translation: self.position,
                rotation: self.rotation().to_array(),
                scale: [1.0, 1.0, 1.0],
            },
        )
    }

    pub(crate) fn fit_draw(
        &mut self,
        draw: &ViewportDrawCall,
        selected: Option<&EntityId>,
    ) -> bool {
        let selected_span = selected
            .and_then(|id| draw.cube_spans.iter().find(|span| &span.entity == id))
            .or_else(|| draw.cube_spans.first().filter(|_| selected.is_some()));
        let mut center = Vec3::ZERO;
        let mut count = 0usize;
        let mut accumulate_span = |span: &render::ViewportCubeSpan| {
            for index in span.vertex_range.clone() {
                let Some(vertex) = draw.vertices.get(index) else {
                    continue;
                };
                center += Vec3::from_array(vertex.position);
                count += 1;
            }
        };

        if let Some(span) = selected_span {
            accumulate_span(span);
        } else {
            for span in &draw.cube_spans {
                accumulate_span(span);
            }
        }
        if count == 0 {
            return false;
        }
        let center = center / count as f32;
        if !center.is_finite() {
            return false;
        }
        let moved = Vec3::from_array(self.position)
            + self.rotation()
                * Vec3::new(
                    center.x * FIT_SCREEN_TO_WORLD_SCALE,
                    center.y * FIT_SCREEN_TO_WORLD_SCALE,
                    0.0,
                );
        self.position = moved.to_array();
        true
    }

    fn rotation(self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }
}

impl Default for ViewCamera {
    fn default() -> Self {
        Self {
            position: [0.0, 2.0, 5.0],
            yaw: 0.0,
            pitch: 0.0,
            speed: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ViewportWgpuReport {
    pub(crate) prepare_count: usize,
    pub(crate) paint_count: usize,
    pub(crate) completed: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ViewportWgpuProbe {
    inner: Arc<ViewportWgpuProbeInner>,
}

#[derive(Debug, Default)]
struct ViewportWgpuProbeInner {
    prepare_count: AtomicUsize,
    paint_count: AtomicUsize,
}

impl ViewportWgpuProbe {
    fn mark_prepared(&self) {
        self.inner.prepare_count.fetch_add(1, Ordering::Relaxed);
    }

    fn mark_painted(&self) {
        self.inner.paint_count.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub(crate) fn report(&self) -> ViewportWgpuReport {
        let prepare_count = self.inner.prepare_count.load(Ordering::Relaxed);
        let paint_count = self.inner.paint_count.load(Ordering::Relaxed);
        ViewportWgpuReport {
            prepare_count,
            paint_count,
            completed: prepare_count > 0 && paint_count > 0,
        }
    }
}

struct ViewportGpuResources {
    renderer: ViewportRenderer,
}

impl ViewportGpuResources {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: ViewportRenderer::new(device, color_format),
        }
    }
}

#[derive(Clone)]
struct ViewportWgpuCallback {
    draw: ViewportDrawCall,
    probe: ViewportWgpuProbe,
}

impl egui_wgpu::CallbackTrait for ViewportWgpuCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = callback_resources.get_mut::<ViewportGpuResources>() {
            resources.renderer.prepare(device, Some(&self.draw));
            self.probe.mark_prepared();
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(resources) = callback_resources.get::<ViewportGpuResources>() {
            resources.renderer.paint(render_pass);
            self.probe.mark_painted();
        }
    }
}

pub(crate) fn install_viewport_renderer(creation_context: &eframe::CreationContext<'_>) -> bool {
    let Some(render_state) = creation_context.wgpu_render_state.as_ref() else {
        return false;
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(ViewportGpuResources::new(
            &render_state.device,
            render_state.target_format,
        ));
    true
}

pub(crate) fn draw_viewport(
    ui: &mut egui::Ui,
    draw: Option<&ViewportDrawCall>,
    selected: Option<&EntityId>,
    selected_transform: Option<Transform>,
    camera: &mut ViewCamera,
    gizmo: &mut TransformGizmoState,
    wgpu_probe: Option<&ViewportWgpuProbe>,
) -> ViewportAction {
    ui.heading("Viewport");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut gizmo.mode, GizmoMode::Move, "Move");
        ui.selectable_value(&mut gizmo.mode, GizmoMode::Scale, "Scale");
    });
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
        response.request_focus();
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
    if ui.input(|input| input.key_pressed(egui::Key::F)) {
        match draw {
            Some(draw) if camera.fit_draw(draw, selected) => {
                ui.ctx().request_repaint();
            }
            Some(_) | None => action = ViewportAction::Status("No visible cube to fit".to_owned()),
        }
    }
    let handles = fitted_draw.as_ref().map_or_else(Vec::new, |draw| {
        gizmo_layout(draw, rect, selected, gizmo.mode)
    });
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
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportWgpuCallback {
                draw: draw.clone(),
                probe: probe.clone(),
            },
        ));
    } else if let Some(draw) = fitted_draw.as_ref() {
        paint_fallback_viewport(rect, &painter, draw);
    }
    paint_gizmo_handles(&painter, &handles);
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

fn paint_gizmo_handles(painter: &egui::Painter, handles: &[GizmoHandleRect]) {
    for handle in handles {
        match handle.handle {
            GizmoHandle::MoveX => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 80, 80)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(230, 80, 80));
            }
            GizmoHandle::MoveY => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 210, 110)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(80, 210, 110));
            }
            GizmoHandle::MoveZ => {
                painter.line_segment(
                    [
                        handle.center - handle.axis * GIZMO_HANDLE_LENGTH,
                        handle.center,
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 150, 240)),
                );
                painter.rect_filled(handle.rect, 1.0, egui::Color32::from_rgb(90, 150, 240));
            }
            GizmoHandle::UniformScale => {
                painter.rect_filled(handle.rect, 1.0, egui::Color32::WHITE);
                painter.rect_stroke(
                    handle.rect,
                    1.0,
                    egui::Stroke::new(1.0, egui::Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            }
        }
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
    for span in &draw.cube_spans {
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
mod tests {
    use super::{
        GizmoDrag, GizmoHandle, GizmoHandleRect, GizmoMode, TransformGizmoState, ViewCamera,
        ViewMoveInput, ViewportAction, ViewportWgpuProbe, hit_test_viewport_draw,
        screen_position_for_vertex,
    };
    use ecs::EntityId;
    use math::{Transform, Vec3};
    use render::{ViewportCubeSpan, ViewportDrawCall, ViewportVertex};

    fn draw_with_two_cube_spans() -> ViewportDrawCall {
        ViewportDrawCall {
            label: "primitive:cube".to_owned(),
            camera_entity: EntityId::new("editor_view"),
            vertex_count: 8,
            index_count: 12,
            vertices: vec![
                ViewportVertex {
                    position: [-0.8, -0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [-0.4, -0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [-0.4, 0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [-0.8, 0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [0.4, -0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [0.8, -0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [0.8, 0.2, 0.0],
                    color: [1.0; 4],
                },
                ViewportVertex {
                    position: [0.4, 0.2, 0.0],
                    color: [1.0; 4],
                },
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            cube_spans: vec![
                ViewportCubeSpan {
                    entity: EntityId::new("cube"),
                    vertex_range: 0..4,
                    index_range: 0..6,
                },
                ViewportCubeSpan {
                    entity: EntityId::new("cube_1"),
                    vertex_range: 4..8,
                    index_range: 6..12,
                },
            ],
        }
    }

    #[test]
    fn viewport_wgpu_probe_requires_prepare_and_paint() {
        let probe = ViewportWgpuProbe::default();

        assert!(!probe.report().completed);

        probe.mark_prepared();
        assert!(!probe.report().completed);

        probe.mark_painted();
        let report = probe.report();

        assert!(report.completed);
        assert_eq!(report.prepare_count, 1);
        assert_eq!(report.paint_count, 1);
    }

    #[test]
    fn viewport_canvas_keeps_nonzero_paint_area() {
        assert_eq!(
            super::viewport_canvas_size(egui::vec2(0.0, 0.0)),
            egui::vec2(240.0, 180.0)
        );
        assert_eq!(
            super::viewport_canvas_size(egui::vec2(320.0, 240.0)),
            egui::vec2(320.0, 240.0)
        );
    }

    #[test]
    fn view_camera_clamps_pitch_and_speed() {
        let mut camera = ViewCamera::default();

        camera.adjust_speed(10.0);
        assert!(camera.speed() >= 1.5);

        camera.look(egui::vec2(0.0, 20_000.0));
        camera.adjust_speed(-10_000.0);
        assert!(camera.pitch().is_finite());
        assert!(camera.pitch() >= ViewCamera::MIN_PITCH);
        assert_eq!(camera.speed(), ViewCamera::MIN_SPEED);

        camera.look(egui::vec2(0.0, -20_000.0));
        camera.adjust_speed(10_000.0);
        assert!(camera.pitch() <= ViewCamera::MAX_PITCH);
        assert_eq!(camera.speed(), ViewCamera::MAX_SPEED);
    }

    #[test]
    fn view_camera_movement_changes_editor_only_view() {
        let mut camera = ViewCamera::default();
        let before = camera.to_viewport_view();

        camera.move_local(
            ViewMoveInput {
                forward: true,
                right: true,
                ..ViewMoveInput::default()
            },
            1.0,
        );
        let after = camera.to_viewport_view();
        let movement = Vec3::from_array(after.transform.translation)
            - Vec3::from_array(before.transform.translation);

        assert_ne!(before.transform.translation, after.transform.translation);
        assert!(movement.length() >= 1.0);
        assert_eq!(after.entity, EntityId::new("editor_view"));
    }

    #[test]
    fn hit_test_uses_entity_span_metadata() {
        let draw = draw_with_two_cube_spans();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
        let hit = screen_position_for_vertex(rect, draw.vertices[5].position);

        let action = hit_test_viewport_draw(&draw, rect, hit);

        assert_eq!(action, ViewportAction::Select(EntityId::new("cube_1")));
    }

    #[test]
    fn hit_test_empty_space_clears_selection() {
        let draw = draw_with_two_cube_spans();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));

        let action = hit_test_viewport_draw(&draw, rect, egui::pos2(100.0, 100.0));

        assert_eq!(action, ViewportAction::ClearSelection);
    }

    #[test]
    fn gizmo_layout_uses_fitted_draw_and_selected_span() {
        let draw = draw_with_two_cube_spans();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));

        let handles =
            super::gizmo_layout(&draw, rect, Some(&EntityId::new("cube_1")), GizmoMode::Move);

        assert_eq!(handles.len(), 3);
        assert!(
            handles
                .iter()
                .any(|handle| handle.handle == GizmoHandle::MoveX)
        );
        assert!(
            handles
                .iter()
                .any(|handle| handle.handle == GizmoHandle::MoveY)
        );
        assert!(
            handles
                .iter()
                .any(|handle| handle.handle == GizmoHandle::MoveZ)
        );
        assert!(
            handles
                .iter()
                .all(|handle| rect.expand(64.0).contains(handle.center))
        );
    }

    #[test]
    fn gizmo_hit_test_prefers_nearest_handle() {
        let handles = vec![
            GizmoHandleRect::new(
                GizmoHandle::MoveX,
                egui::pos2(100.0, 100.0),
                egui::Vec2::X,
                20.0,
            ),
            GizmoHandleRect::new(
                GizmoHandle::MoveY,
                egui::pos2(104.0, 100.0),
                -egui::Vec2::Y,
                20.0,
            ),
        ];

        let hit = super::hit_test_gizmo(&handles, egui::pos2(103.0, 100.0));

        assert_eq!(hit, Some(GizmoHandle::MoveY));
    }

    #[test]
    fn move_gizmo_drag_changes_only_selected_axis() {
        let start = Transform {
            translation: [1.0, 2.0, 3.0],
            ..Transform::identity()
        };
        let start_pointer = egui::pos2(10.0, 10.0);

        let moved_x = super::transform_for_gizmo_drag(
            GizmoHandle::MoveX,
            start,
            start_pointer,
            egui::pos2(60.0, 10.0),
        );
        let moved_y = super::transform_for_gizmo_drag(
            GizmoHandle::MoveY,
            start,
            start_pointer,
            egui::pos2(10.0, -40.0),
        );
        let moved_z = super::transform_for_gizmo_drag(
            GizmoHandle::MoveZ,
            start,
            start_pointer,
            egui::pos2(60.0, -40.0),
        );

        assert_eq!(moved_x.translation, [1.5, 2.0, 3.0]);
        assert_eq!(moved_y.translation, [1.0, 2.5, 3.0]);
        assert_eq!(moved_z.translation, [1.0, 2.0, 3.707_106_8]);
    }

    #[test]
    fn uniform_scale_drag_changes_all_scale_axes_and_clamps_minimum() {
        let start = Transform {
            scale: [1.0, 2.0, 3.0],
            ..Transform::identity()
        };
        let start_pointer = egui::pos2(10.0, 10.0);

        let grown = super::transform_for_gizmo_drag(
            GizmoHandle::UniformScale,
            start,
            start_pointer,
            egui::pos2(60.0, -40.0),
        );
        let clamped = super::transform_for_gizmo_drag(
            GizmoHandle::UniformScale,
            start,
            start_pointer,
            egui::pos2(-200.0, 220.0),
        );

        assert_eq!(grown.scale, [1.707_106_8, 2.707_106_8, 3.707_106_8]);
        assert_eq!(clamped.scale, [0.01, 0.01, 0.01]);
    }

    #[test]
    fn gizmo_state_stores_and_clears_drag_target() {
        let mut state = TransformGizmoState::default();
        let drag = GizmoDrag {
            target: EntityId::new("cube"),
            handle: GizmoHandle::MoveX,
            start_pointer: egui::pos2(10.0, 10.0),
            start_transform: Transform::identity(),
        };

        state.start_drag(drag.clone());
        assert_eq!(state.drag(), Some(&drag));

        state.clear_drag();
        assert_eq!(state.drag(), None);
    }

    #[test]
    fn gizmo_drag_starts_from_press_origin_before_drag_threshold() {
        let target = EntityId::new("cube");
        let start_transform = Transform::from_translation([1.0, 2.0, 3.0]);
        let handles = vec![GizmoHandleRect::new(
            GizmoHandle::MoveX,
            egui::pos2(100.0, 100.0),
            egui::Vec2::X,
            10.0,
        )];

        let drag = super::gizmo_drag_from_press_origin(
            &handles,
            Some(egui::pos2(100.0, 100.0)),
            Some(&target),
            Some(start_transform),
        );

        assert_eq!(
            drag,
            Some(GizmoDrag {
                target,
                handle: GizmoHandle::MoveX,
                start_pointer: egui::pos2(100.0, 100.0),
                start_transform,
            })
        );
    }

    #[test]
    fn viewport_transform_actions_distinguish_preview_commit_and_restore() {
        let target = EntityId::new("cube");
        let before = Transform::identity();
        let after = Transform::from_translation([1.0, 2.0, 3.0]);

        assert_eq!(
            ViewportAction::PreviewTransform {
                target: target.clone(),
                transform: after,
            },
            ViewportAction::PreviewTransform {
                target: EntityId::new("cube"),
                transform: Transform::from_translation([1.0, 2.0, 3.0]),
            }
        );
        assert_eq!(
            ViewportAction::CommitTransform {
                target: target.clone(),
                before,
                after,
            },
            ViewportAction::CommitTransform {
                target: EntityId::new("cube"),
                before: Transform::identity(),
                after: Transform::from_translation([1.0, 2.0, 3.0]),
            }
        );
        assert_eq!(
            ViewportAction::RestoreTransform {
                target,
                transform: before,
            },
            ViewportAction::RestoreTransform {
                target: EntityId::new("cube"),
                transform: Transform::identity(),
            }
        );
    }

    #[test]
    fn fit_visible_draw_keeps_camera_finite() {
        let draw = draw_with_two_cube_spans();
        let mut camera = ViewCamera::default();

        assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube"))));
        let view = camera.to_viewport_view();

        assert!(view.transform.translation.into_iter().all(f32::is_finite));
        assert!(view.transform.rotation.into_iter().all(f32::is_finite));
    }

    #[test]
    fn fit_visible_draw_pans_edge_selection_toward_center() {
        let draw = draw_with_two_cube_spans();
        let mut camera = ViewCamera::default();

        assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube_1"))));
        let view = camera.to_viewport_view();

        assert!((view.transform.translation[0] - 5.0).abs() < 0.1);
    }

    #[test]
    fn fit_visible_draw_without_selection_centers_all_visible_cubes() {
        let draw = draw_with_two_cube_spans();
        let mut camera = ViewCamera::default();

        assert!(camera.fit_draw(&draw, None));
        let view = camera.to_viewport_view();

        assert!(view.transform.translation[0].abs() < 0.1);
    }
}

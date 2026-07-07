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
const MOVE_SCALE: f32 = 0.2;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ViewportAction {
    None,
    Select(EntityId),
    ClearSelection,
    Status(String),
}

impl ViewCamera {
    pub(crate) const MIN_SPEED: f32 = 0.05;
    pub(crate) const MAX_SPEED: f32 = 20.0;
    pub(crate) const MIN_PITCH: f32 = -1.45;
    pub(crate) const MAX_PITCH: f32 = 1.45;

    #[must_use]
    pub(crate) const fn pitch(self) -> f32 {
        self.pitch
    }

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
        self.speed = (self.speed + scroll_y * 0.01).clamp(Self::MIN_SPEED, Self::MAX_SPEED);
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
        let chosen_span = selected
            .and_then(|id| draw.cube_spans.iter().find(|span| &span.entity == id))
            .or_else(|| draw.cube_spans.first());
        let Some(span) = chosen_span else {
            return false;
        };
        let center = span
            .vertex_range
            .clone()
            .filter_map(|index| draw.vertices.get(index))
            .fold(Vec3::ZERO, |acc, vertex| {
                acc + Vec3::from_array(vertex.position)
            })
            / span.vertex_range.len().max(1) as f32;
        if !center.is_finite() {
            return false;
        }
        self.position = [center.x, center.y, 5.0];
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
    wgpu_probe: Option<&ViewportWgpuProbe>,
) {
    ui.heading("Viewport");
    let (rect, _response) = ui.allocate_exact_size(
        viewport_canvas_size(ui.available_size_before_wrap()),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 24, 29));
    if let Some((draw, probe)) = draw.zip(wgpu_probe) {
        let draw = fit_viewport_draw_to_size(draw, [rect.width(), rect.height()]);
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportWgpuCallback {
                draw,
                probe: probe.clone(),
            },
        ));
    } else if let Some(draw) = draw {
        paint_fallback_viewport(rect, &painter, draw);
    }
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
        ViewCamera, ViewMoveInput, ViewportAction, ViewportWgpuProbe, hit_test_viewport_draw,
        screen_position_for_vertex,
    };
    use ecs::EntityId;
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

        assert_ne!(before.transform.translation, after.transform.translation);
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
    fn fit_visible_draw_keeps_camera_finite() {
        let draw = draw_with_two_cube_spans();
        let mut camera = ViewCamera::default();

        assert!(camera.fit_draw(&draw, Some(&EntityId::new("cube"))));
        let view = camera.to_viewport_view();

        assert!(view.transform.translation.into_iter().all(f32::is_finite));
        assert!(view.transform.rotation.into_iter().all(f32::is_finite));
    }
}

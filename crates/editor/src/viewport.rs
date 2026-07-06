// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use eframe::{egui, egui_wgpu, wgpu};
use render::{ViewportDrawCall, ViewportRenderer, fit_viewport_draw_to_size};

const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);

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

#[cfg(test)]
mod tests {
    use super::ViewportWgpuProbe;

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
}

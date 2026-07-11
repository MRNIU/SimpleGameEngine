// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use eframe::{egui, egui_wgpu, wgpu};
use render::{ViewportDrawCall, ViewportRenderFrame, ViewportRenderer, ViewportVertex};

use super::ReferenceLine;

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
    pub(crate) fn mark_prepared(&self) {
        self.inner.prepare_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn mark_painted(&self) {
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
    draw: Option<ViewportDrawCall>,
    grid_vertices: Vec<ViewportVertex>,
    view_projection: [f32; 16],
    logical_size: [f32; 2],
    probe: ViewportWgpuProbe,
}

impl egui_wgpu::CallbackTrait for ViewportWgpuCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = callback_resources.get_mut::<ViewportGpuResources>() {
            let pixels_per_point = screen_descriptor.pixels_per_point;
            let target_size = [
                (self.logical_size[0] * pixels_per_point).round().max(1.0) as u32,
                (self.logical_size[1] * pixels_per_point).round().max(1.0) as u32,
            ];
            resources.renderer.prepare(
                device,
                queue,
                egui_encoder,
                ViewportRenderFrame {
                    draw: self.draw.as_ref(),
                    grid_vertices: &self.grid_vertices,
                    view_projection: self.view_projection,
                    target_size,
                },
            );
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

pub(crate) fn paint_wgpu_viewport(
    painter: &egui::Painter,
    rect: egui::Rect,
    draw: Option<&ViewportDrawCall>,
    grid_lines: &[ReferenceLine],
    projection: &render::ViewportProjection,
    probe: &ViewportWgpuProbe,
) {
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        ViewportWgpuCallback {
            draw: draw.cloned(),
            grid_vertices: grid_vertices(grid_lines),
            view_projection: projection.view_projection_array(),
            logical_size: [rect.width(), rect.height()],
            probe: probe.clone(),
        },
    ));
}

fn grid_vertices(lines: &[ReferenceLine]) -> Vec<ViewportVertex> {
    lines
        .iter()
        .flat_map(|line| {
            let color = line
                .color
                .to_array()
                .map(|channel| f32::from(channel) / 255.0);
            [
                ViewportVertex {
                    position: line.start,
                    color,
                },
                ViewportVertex {
                    position: line.end,
                    color,
                },
            ]
        })
        .collect()
}

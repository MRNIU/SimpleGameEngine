// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use eframe::{egui, egui_wgpu, wgpu};
use render::{ViewportDrawCall, ViewportRenderer};

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

pub(crate) fn paint_wgpu_viewport(
    painter: &egui::Painter,
    rect: egui::Rect,
    draw: &ViewportDrawCall,
    probe: &ViewportWgpuProbe,
) {
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        ViewportWgpuCallback {
            draw: draw.clone(),
            probe: probe.clone(),
        },
    ));
}

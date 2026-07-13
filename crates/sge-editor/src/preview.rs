// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use eframe::{egui, egui_wgpu, wgpu};
use sge_asset::RuntimeAssetStore;
use sge_render::{
    BackendFrame, BackendRenderContext, BackendRenderer, FramePerformanceMonitor,
    FramePhaseDurations, RenderBackend,
};

use crate::PreviewFrame;

struct PreviewGpuResources {
    renderer: BackendRenderer,
    assets: Option<Arc<RuntimeAssetStore>>,
}

impl PreviewGpuResources {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: BackendRenderer::new(device, format, RenderBackend::Wgpu),
            assets: None,
        }
    }

    fn select_assets(&mut self, assets: &Arc<RuntimeAssetStore>) {
        if store_replaced(self.assets.as_ref(), assets) {
            self.renderer.clear_asset_cache();
            self.assets = Some(Arc::clone(assets));
        }
    }
}

#[derive(Clone)]
struct PreviewCallback {
    frame: PreviewFrame,
    backend: RenderBackend,
    logical_size: [f32; 2],
    probe: PreviewProbe,
}

impl egui_wgpu::CallbackTrait for PreviewCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<PreviewGpuResources>() else {
            self.probe.fail("preview GPU resources are missing");
            return Vec::new();
        };
        let size = preview_target_size(self.backend, self.logical_size, screen.pixels_per_point);
        gpu.renderer.set_backend(self.backend);
        gpu.select_assets(&self.frame.assets);
        let started = Instant::now();
        let result = gpu
            .renderer
            .render_offscreen(
                BackendRenderContext {
                    device,
                    queue,
                    encoder,
                },
                size,
                BackendFrame {
                    snapshot: &self.frame.snapshot,
                    view: self.frame.view,
                    assets: self.frame.assets.as_ref(),
                },
            )
            .map_err(|error| error.to_string());
        let duration = started.elapsed();
        match result {
            Ok(()) => self.probe.mark_prepared(self.backend, duration),
            Err(error) => self.probe.fail(error),
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(gpu) = resources.get::<PreviewGpuResources>() else {
            self.probe.fail("preview GPU resources are missing");
            return;
        };
        let started = Instant::now();
        match gpu.renderer.composite(pass) {
            Ok(()) => self.probe.mark_painted(self.backend, started.elapsed()),
            Err(error) => self.probe.fail(error.to_string()),
        }
    }
}

pub(crate) fn install_renderer(
    creation_context: &eframe::CreationContext<'_>,
) -> Result<(), &'static str> {
    let render_state = creation_context
        .wgpu_render_state
        .as_ref()
        .ok_or("eframe WGPU render state is unavailable")?;
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(PreviewGpuResources::new(
            &render_state.device,
            render_state.target_format,
        ));
    Ok(())
}

pub(crate) fn paint(
    ui: &mut egui::Ui,
    frame: &PreviewFrame,
    probe: &PreviewProbe,
    backend: RenderBackend,
    paint_background: impl FnOnce(&egui::Ui, egui::Rect),
) -> egui::Response {
    let available = ui.available_size_before_wrap();
    let size = egui::vec2(available.x.max(240.0), available.y.max(180.0));
    let (rect, response) = ui.allocate_exact_size(size, viewport_sense());
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_rgb(13, 15, 18));
    paint_background(ui, rect);
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        PreviewCallback {
            frame: frame.clone(),
            backend,
            logical_size: [rect.width(), rect.height()],
            probe: probe.clone(),
        },
    ));
    response
}

fn viewport_sense() -> egui::Sense {
    egui::Sense::click_and_drag()
}

fn preview_target_size(
    backend: RenderBackend,
    logical_size: [f32; 2],
    pixels_per_point: f32,
) -> [u32; 2] {
    let scale = match backend {
        RenderBackend::Wgpu => pixels_per_point,
        RenderBackend::Cpu => 1.0,
    };
    logical_size.map(|points| (points * scale).round().max(1.0) as u32)
}

fn store_replaced(current: Option<&Arc<RuntimeAssetStore>>, next: &Arc<RuntimeAssetStore>) -> bool {
    current.is_none_or(|current| !Arc::ptr_eq(current, next))
}

#[derive(Debug, Clone, Default)]
pub struct PreviewProbe {
    inner: Arc<PreviewProbeInner>,
}

#[derive(Debug, Default)]
struct PreviewProbeInner {
    prepared: AtomicUsize,
    painted: AtomicUsize,
    wgpu_prepared: AtomicUsize,
    cpu_prepared: AtomicUsize,
    performance: Mutex<PreviewPerformanceState>,
    error: Mutex<Option<String>>,
}

#[derive(Debug, Default)]
struct PreviewPerformanceState {
    backend: Option<RenderBackend>,
    pending_prepare: Option<Duration>,
    monitor: FramePerformanceMonitor,
}

impl PreviewProbe {
    fn mark_prepared(&self, backend: RenderBackend, duration: Duration) {
        self.inner.prepared.fetch_add(1, Ordering::Relaxed);
        match backend {
            RenderBackend::Wgpu => &self.inner.wgpu_prepared,
            RenderBackend::Cpu => &self.inner.cpu_prepared,
        }
        .fetch_add(1, Ordering::Relaxed);
        if let Ok(mut performance) = self.inner.performance.lock() {
            if performance.backend != Some(backend) {
                performance.monitor.reset();
                performance.backend = Some(backend);
            }
            performance.pending_prepare = Some(duration);
        }
    }

    fn mark_painted(&self, backend: RenderBackend, duration: Duration) {
        self.inner.painted.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut performance) = self.inner.performance.lock() {
            if performance.backend != Some(backend) {
                performance.monitor.reset();
                performance.backend = Some(backend);
            }
            let render = performance
                .pending_prepare
                .take()
                .unwrap_or(Duration::ZERO)
                .saturating_add(duration);
            performance
                .monitor
                .record_completed(FramePhaseDurations::render(render));
        }
    }

    fn fail(&self, error: impl Into<String>) {
        if let Ok(mut slot) = self.inner.error.lock()
            && slot.is_none()
        {
            *slot = Some(error.into());
        }
    }

    #[must_use]
    pub fn frames_per_second(&self) -> Option<u32> {
        self.inner
            .performance
            .lock()
            .ok()
            .and_then(|performance| performance.monitor.frames_per_second())
    }

    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.inner.error.lock().map_or_else(
            |_| Some("preview probe lock poisoned".to_owned()),
            |slot| slot.clone(),
        )
    }

    #[must_use]
    pub fn report(&self) -> PreviewProbeReport {
        let performance = self
            .inner
            .performance
            .lock()
            .map(|performance| performance.monitor.summary());
        let mut error = self.inner.error.lock().map_or_else(
            |_| Some("preview probe lock poisoned".to_owned()),
            |slot| slot.clone(),
        );
        if performance.is_err() && error.is_none() {
            error = Some("preview performance lock poisoned".to_owned());
        }
        let performance = performance.unwrap_or_default();
        PreviewProbeReport {
            prepare_count: self.inner.prepared.load(Ordering::Relaxed),
            paint_count: self.inner.painted.load(Ordering::Relaxed),
            wgpu_prepare_count: self.inner.wgpu_prepared.load(Ordering::Relaxed),
            cpu_prepare_count: self.inner.cpu_prepared.load(Ordering::Relaxed),
            frames_per_second: performance.frames_per_second(),
            performance,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use eframe::egui;
    use sge_asset::RuntimeAssetStore;

    use super::{preview_target_size, store_replaced, viewport_sense};

    #[test]
    fn wgpu_preview_uses_physical_pixels() {
        assert_eq!(
            preview_target_size(sge_render::RenderBackend::Wgpu, [640.0, 360.0], 2.0),
            [1280, 720]
        );
    }

    #[test]
    fn cpu_preview_uses_logical_pixels_to_keep_retina_interactive() {
        assert_eq!(
            preview_target_size(sge_render::RenderBackend::Cpu, [640.0, 360.0], 2.0),
            [640, 360]
        );
    }

    #[test]
    fn viewport_accepts_clicks_drags_and_keyboard_focus() {
        let sense = viewport_sense();
        assert!(sense.senses_click());
        assert!(sense.senses_drag());
        assert!(sense.is_focusable());
    }

    #[test]
    fn software_events_reach_the_viewport_response() {
        let context = egui::Context::default();
        let position = egui::pos2(40.0, 40.0);
        let _ = viewport_response(&context, Vec::new());
        let pressed = viewport_response(
            &context,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(pressed.hovered());
        pressed.request_focus();

        let dragged = viewport_response(
            &context,
            vec![egui::Event::PointerMoved(egui::pos2(70.0, 40.0))],
        );
        assert!(dragged.has_focus());
        assert!(dragged.dragged_by(egui::PointerButton::Secondary));
    }

    fn viewport_response(context: &egui::Context, events: Vec<egui::Event>) -> egui::Response {
        let mut response = None;
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(160.0, 120.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                response = Some(
                    ui.allocate_exact_size(egui::vec2(160.0, 120.0), viewport_sense())
                        .1,
                );
            },
        );
        response.expect("viewport response")
    }

    #[test]
    fn arc_identity_distinguishes_store_replacement_from_frame_reuse() {
        let first = Arc::new(RuntimeAssetStore::from_meshes([]).expect("empty store is valid"));
        let reused = Arc::clone(&first);
        let replacement =
            Arc::new(RuntimeAssetStore::from_meshes([]).expect("empty store is valid"));

        assert!(!store_replaced(Some(&first), &reused));
        assert!(store_replaced(Some(&first), &replacement));
        assert!(store_replaced(None, &first));
    }

    #[test]
    fn preview_performance_resets_when_the_backend_changes() {
        let probe = super::PreviewProbe::default();
        probe.mark_prepared(
            sge_render::RenderBackend::Wgpu,
            std::time::Duration::from_millis(2),
        );
        probe.mark_painted(
            sge_render::RenderBackend::Wgpu,
            std::time::Duration::from_millis(1),
        );
        probe.mark_prepared(
            sge_render::RenderBackend::Wgpu,
            std::time::Duration::from_millis(2),
        );
        probe.mark_painted(
            sge_render::RenderBackend::Wgpu,
            std::time::Duration::from_millis(1),
        );
        assert_eq!(probe.report().performance.sample_count(), 1);

        probe.mark_prepared(
            sge_render::RenderBackend::Cpu,
            std::time::Duration::from_millis(4),
        );
        assert_eq!(probe.report().performance.sample_count(), 0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewProbeReport {
    pub prepare_count: usize,
    pub paint_count: usize,
    pub wgpu_prepare_count: usize,
    pub cpu_prepare_count: usize,
    pub frames_per_second: Option<u32>,
    pub performance: sge_render::FramePerformanceSummary,
    pub error: Option<String>,
}

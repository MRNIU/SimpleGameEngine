// Copyright The SimpleGameEngine Contributors

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use eframe::{egui, egui_wgpu, wgpu};
use sge_asset::RuntimeAssetStore;
use sge_render::WgpuRenderer;

use crate::PreviewFrame;

struct PreviewGpuResources {
    renderer: WgpuRenderer,
    assets: Option<Arc<RuntimeAssetStore>>,
}

impl PreviewGpuResources {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: WgpuRenderer::new(device, format),
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
        let size = [
            logical_dimension(self.logical_size[0], screen.pixels_per_point),
            logical_dimension(self.logical_size[1], screen.pixels_per_point),
        ];
        gpu.select_assets(&self.frame.assets);
        let result = gpu
            .renderer
            .prepare_assets(
                device,
                queue,
                &self.frame.snapshot,
                self.frame.assets.as_ref(),
            )
            .map_err(|error| error.to_string())
            .and_then(|()| {
                gpu.renderer
                    .render_offscreen(device, encoder, size, &self.frame.snapshot, self.frame.view)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => self.probe.mark_prepared(),
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
        match gpu.renderer.composite(pass) {
            Ok(()) => self.probe.mark_painted(),
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
            logical_size: [rect.width(), rect.height()],
            probe: probe.clone(),
        },
    ));
    response
}

fn viewport_sense() -> egui::Sense {
    egui::Sense::click_and_drag()
}

fn logical_dimension(points: f32, pixels_per_point: f32) -> u32 {
    (points * pixels_per_point).round().max(1.0) as u32
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
    error: Mutex<Option<String>>,
}

impl PreviewProbe {
    fn mark_prepared(&self) {
        self.inner.prepared.fetch_add(1, Ordering::Relaxed);
    }

    fn mark_painted(&self) {
        self.inner.painted.fetch_add(1, Ordering::Relaxed);
    }

    fn fail(&self, error: impl Into<String>) {
        if let Ok(mut slot) = self.inner.error.lock()
            && slot.is_none()
        {
            *slot = Some(error.into());
        }
    }

    #[must_use]
    pub fn report(&self) -> PreviewProbeReport {
        PreviewProbeReport {
            prepare_count: self.inner.prepared.load(Ordering::Relaxed),
            paint_count: self.inner.painted.load(Ordering::Relaxed),
            error: self.inner.error.lock().map_or_else(
                |_| Some("preview probe lock poisoned".to_owned()),
                |slot| slot.clone(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use eframe::egui;
    use sge_asset::RuntimeAssetStore;

    use super::{store_replaced, viewport_sense};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewProbeReport {
    pub prepare_count: usize,
    pub paint_count: usize,
    pub error: Option<String>,
}

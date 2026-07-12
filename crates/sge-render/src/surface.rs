// Copyright The SimpleGameEngine Contributors

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use sge_asset::RuntimeAssetStore;

use crate::{
    GpuAssetError, RenderFrameError, RenderSnapshot, RenderTargetError, RenderView, WgpuRenderer,
};

pub struct SurfaceRenderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + 'static,
{
    renderer: WgpuRenderer,
    queue: wgpu::Queue,
    device: wgpu::Device,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    _instance: wgpu::Instance,
    target: Arc<W>,
    config: wgpu::SurfaceConfiguration,
    drawable: bool,
}

impl<W> SurfaceRenderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + Send + Sync + 'static,
{
    pub fn new(target: Arc<W>, size: [u32; 2]) -> Result<Self, SurfaceRenderError> {
        validate_surface_size(size)?;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(Arc::clone(&target))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        validate_device_size(&device, size)?;
        let config = surface
            .get_default_config(&adapter, size[0], size[1])
            .ok_or(SurfaceRenderError::UnsupportedSurface)?;
        surface.configure(&device, &config);
        let renderer = WgpuRenderer::new(&device, config.format);
        Ok(Self {
            renderer,
            queue,
            device,
            surface,
            _adapter: adapter,
            _instance: instance,
            target,
            config,
            drawable: true,
        })
    }

    pub fn resize(&mut self, size: [u32; 2]) -> Result<(), SurfaceRenderError> {
        if size.contains(&0) {
            self.drawable = false;
            return Ok(());
        }
        validate_surface_size(size)?;
        validate_device_size(&self.device, size)?;
        self.config.width = size[0];
        self.config.height = size[1];
        self.surface.configure(&self.device, &self.config);
        self.drawable = true;
        Ok(())
    }

    pub fn render(
        &mut self,
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
    ) -> Result<SurfaceRenderOutcome, SurfaceRenderError> {
        if !self.drawable {
            return Ok(SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::ZeroSize));
        }
        self.renderer
            .prepare_assets(&self.device, &self.queue, snapshot, assets)?;
        let (frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Ok(SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Timeout));
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Occluded));
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Outdated));
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err(SurfaceRenderError::Lost),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(SurfaceRenderError::Validation);
            }
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("sge_surface_frame"),
            });
        self.renderer.render_to_target(
            &self.device,
            &mut encoder,
            &frame.texture.create_view(&Default::default()),
            [self.config.width, self.config.height],
            snapshot,
            view,
        )?;
        self.queue.submit([encoder.finish()]);
        frame.present();
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.config);
        }
        Ok(SurfaceRenderOutcome::Presented)
    }

    #[must_use]
    pub fn target(&self) -> &Arc<W> {
        &self.target
    }
}

fn validate_surface_size(size: [u32; 2]) -> Result<(), RenderTargetError> {
    if size.contains(&0) {
        Err(RenderTargetError::ZeroSize)
    } else {
        Ok(())
    }
}

fn validate_device_size(device: &wgpu::Device, size: [u32; 2]) -> Result<(), RenderTargetError> {
    let max = device.limits().max_texture_dimension_2d;
    if size[0] > max || size[1] > max {
        Err(RenderTargetError::TooLarge {
            width: size[0],
            height: size[1],
            max,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRenderOutcome {
    Presented,
    Skipped(SkippedSurfaceFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedSurfaceFrame {
    ZeroSize,
    Timeout,
    Occluded,
    Outdated,
}

#[derive(Debug, thiserror::Error)]
pub enum SurfaceRenderError {
    #[error(transparent)]
    Target(#[from] RenderTargetError),
    #[error("cannot create WGPU surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible WGPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("cannot create WGPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("surface has no supported configuration")]
    UnsupportedSurface,
    #[error(transparent)]
    Assets(#[from] GpuAssetError),
    #[error(transparent)]
    Frame(#[from] RenderFrameError),
    #[error("WGPU surface was lost")]
    Lost,
    #[error("WGPU surface acquisition failed validation")]
    Validation,
}

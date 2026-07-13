// Copyright The SimpleGameEngine Contributors

use std::sync::{Arc, mpsc};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use sge_asset::RuntimeAssetStore;

use crate::{
    BackendFrame, BackendRenderContext, BackendRenderError, BackendRenderer, RenderBackend,
    RenderSnapshot, RenderTargetError, RenderView,
};

pub struct SurfaceRenderer<W>
where
    W: HasDisplayHandle + HasWindowHandle + 'static,
{
    renderer: BackendRenderer,
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
        Self::new_with_backend(target, size, RenderBackend::Wgpu)
    }

    pub fn new_with_backend(
        target: Arc<W>,
        size: [u32; 2],
        backend: RenderBackend,
    ) -> Result<Self, SurfaceRenderError> {
        Self::create(target, size, false, backend)
    }

    pub fn new_with_readback(target: Arc<W>, size: [u32; 2]) -> Result<Self, SurfaceRenderError> {
        Self::new_with_readback_and_backend(target, size, RenderBackend::Wgpu)
    }

    pub fn new_with_readback_and_backend(
        target: Arc<W>,
        size: [u32; 2],
        backend: RenderBackend,
    ) -> Result<Self, SurfaceRenderError> {
        Self::create(target, size, true, backend)
    }

    fn create(
        target: Arc<W>,
        size: [u32; 2],
        readback: bool,
        backend: RenderBackend,
    ) -> Result<Self, SurfaceRenderError> {
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
        let mut config = surface
            .get_default_config(&adapter, size[0], size[1])
            .ok_or(SurfaceRenderError::UnsupportedSurface)?;
        if readback {
            let capabilities = surface.get_capabilities(&adapter);
            if !capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                return Err(SurfaceRenderError::ReadbackUnsupported);
            }
            validate_readback_format(config.format)?;
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&device, &config);
        let renderer = BackendRenderer::new(&device, config.format, backend);
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
        self.render_frame(snapshot, view, assets, false)
            .map(|(outcome, _)| outcome)
    }

    pub fn render_with_readback(
        &mut self,
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
    ) -> Result<(SurfaceRenderOutcome, Option<SurfaceReadback>), SurfaceRenderError> {
        self.render_frame(snapshot, view, assets, true)
    }

    fn render_frame(
        &mut self,
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
        readback: bool,
    ) -> Result<(SurfaceRenderOutcome, Option<SurfaceReadback>), SurfaceRenderError> {
        if !self.drawable {
            return Ok((
                SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::ZeroSize),
                None,
            ));
        }
        let (frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Ok((
                    SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Timeout),
                    None,
                ));
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok((
                    SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Occluded),
                    None,
                ));
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok((
                    SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Outdated),
                    None,
                ));
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
            BackendRenderContext {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
            },
            &frame.texture.create_view(&Default::default()),
            [self.config.width, self.config.height],
            BackendFrame {
                snapshot,
                view,
                assets,
                settings: crate::RenderSettings::default(),
            },
        )?;
        let readback_buffer = readback.then(|| {
            let padded_bytes_per_row = padded_bytes_per_row(self.config.width);
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sge_surface_readback"),
                size: u64::from(padded_bytes_per_row) * u64::from(self.config.height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
            );
            (buffer, padded_bytes_per_row)
        });
        self.queue.submit([encoder.finish()]);
        frame.present();
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.config);
        }
        let image = readback_buffer
            .map(|(buffer, padded)| self.readback(buffer, padded))
            .transpose()?;
        Ok((SurfaceRenderOutcome::Presented, image))
    }

    #[must_use]
    pub fn target(&self) -> &Arc<W> {
        &self.target
    }

    fn readback(
        &self,
        buffer: wgpu::Buffer,
        padded_bytes_per_row: u32,
    ) -> Result<SurfaceReadback, SurfaceRenderError> {
        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(SurfaceRenderError::ReadbackPoll)?;
        receiver
            .recv()
            .map_err(|_| SurfaceRenderError::ReadbackChannel)?
            .map_err(SurfaceRenderError::ReadbackMap)?;
        let mapped = slice.get_mapped_range();
        let rgba = unpack_rgba(
            &mapped,
            [self.config.width, self.config.height],
            padded_bytes_per_row,
            self.config.format,
        )?;
        drop(mapped);
        buffer.unmap();
        Ok(SurfaceReadback {
            size: [self.config.width, self.config.height],
            rgba,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceReadback {
    size: [u32; 2],
    rgba: Vec<u8>,
}

impl SurfaceReadback {
    #[must_use]
    pub const fn size(&self) -> [u32; 2] {
        self.size
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

fn padded_bytes_per_row(width: u32) -> u32 {
    let bytes = width * 4;
    bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

fn validate_readback_format(format: wgpu::TextureFormat) -> Result<(), SurfaceRenderError> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => Ok(()),
        _ => Err(SurfaceRenderError::ReadbackFormat(format)),
    }
}

fn unpack_rgba(
    mapped: &[u8],
    size: [u32; 2],
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
) -> Result<Vec<u8>, SurfaceRenderError> {
    validate_readback_format(format)?;
    let row_bytes = size[0] as usize * 4;
    let padded = padded_bytes_per_row as usize;
    let mut rgba = Vec::with_capacity(row_bytes * size[1] as usize);
    for row in mapped.chunks_exact(padded).take(size[1] as usize) {
        rgba.extend_from_slice(&row[..row_bytes]);
    }
    if matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    Ok(rgba)
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
    #[error("surface does not support texture readback")]
    ReadbackUnsupported,
    #[error("surface texture format {0:?} does not support RGBA readback")]
    ReadbackFormat(wgpu::TextureFormat),
    #[error("cannot poll GPU surface readback: {0}")]
    ReadbackPoll(wgpu::PollError),
    #[error("cannot map GPU surface readback: {0}")]
    ReadbackMap(wgpu::BufferAsyncError),
    #[error("GPU surface readback callback was dropped")]
    ReadbackChannel,
    #[error(transparent)]
    Backend(#[from] BackendRenderError),
    #[error("WGPU surface was lost")]
    Lost,
    #[error("WGPU surface acquisition failed validation")]
    Validation,
}

#[cfg(test)]
mod tests {
    use super::{padded_bytes_per_row, unpack_rgba};

    #[test]
    fn surface_readback_strips_row_padding_and_normalizes_bgra()
    -> Result<(), Box<dyn std::error::Error>> {
        let padded = padded_bytes_per_row(2);
        assert_eq!(padded, 256);
        let mut mapped = vec![0_u8; padded as usize * 2];
        mapped[..8].copy_from_slice(&[3, 2, 1, 255, 6, 5, 4, 255]);
        mapped[padded as usize..padded as usize + 8]
            .copy_from_slice(&[9, 8, 7, 255, 12, 11, 10, 255]);

        assert_eq!(
            unpack_rgba(&mapped, [2, 2], padded, wgpu::TextureFormat::Bgra8UnormSrgb,)?,
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
        Ok(())
    }
}

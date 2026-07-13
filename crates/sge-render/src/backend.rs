// Copyright The SimpleGameEngine Contributors

use std::{fmt, str::FromStr};

use sge_asset::RuntimeAssetStore;

use crate::{
    CpuRenderError, CpuRenderer, GpuAssetError, RenderFrameError, RenderSnapshot,
    RenderTargetError, RenderView, WgpuRenderer,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderBackend {
    #[default]
    Wgpu,
    Cpu,
}

impl RenderBackend {
    pub const ALL: [Self; 2] = [Self::Wgpu, Self::Cpu];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu",
            Self::Cpu => "cpu",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Wgpu => "WGPU",
            Self::Cpu => "CPU",
        }
    }
}

impl fmt::Display for RenderBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RenderBackend {
    type Err = RenderBackendParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wgpu" => Ok(Self::Wgpu),
            "cpu" => Ok(Self::Cpu),
            _ => Err(RenderBackendParseError(value.to_owned())),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown render backend {0:?}; expected wgpu or cpu")]
pub struct RenderBackendParseError(String);

pub struct BackendRenderer {
    backend: RenderBackend,
    wgpu: WgpuRenderer,
    cpu: CpuRenderer,
}

pub struct BackendRenderContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
}

#[derive(Clone, Copy)]
pub struct BackendFrame<'a> {
    pub snapshot: &'a RenderSnapshot,
    pub view: RenderView,
    pub assets: &'a RuntimeAssetStore,
}

impl BackendRenderer {
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        backend: RenderBackend,
    ) -> Self {
        Self {
            backend,
            wgpu: WgpuRenderer::new(device, target_format),
            cpu: CpuRenderer::new(),
        }
    }

    #[must_use]
    pub const fn backend(&self) -> RenderBackend {
        self.backend
    }

    pub const fn set_backend(&mut self, backend: RenderBackend) {
        self.backend = backend;
    }

    pub fn render_to_target(
        &mut self,
        context: BackendRenderContext<'_>,
        target_view: &wgpu::TextureView,
        target_size: [u32; 2],
        frame: BackendFrame<'_>,
    ) -> Result<(), BackendRenderError> {
        match self.backend {
            RenderBackend::Wgpu => {
                self.wgpu.prepare_assets(
                    context.device,
                    context.queue,
                    frame.snapshot,
                    frame.assets,
                )?;
                self.wgpu.render_to_target(
                    context.device,
                    context.encoder,
                    target_view,
                    target_size,
                    frame.snapshot,
                    frame.view,
                )?;
            }
            RenderBackend::Cpu => {
                let cpu_frame =
                    self.cpu
                        .render(target_size, frame.snapshot, frame.view, frame.assets)?;
                self.wgpu.upload_offscreen_rgba(
                    context.device,
                    context.queue,
                    cpu_frame.size(),
                    cpu_frame.rgba(),
                )?;
                let mut pass = context
                    .encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sge_render_cpu_surface_composite"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: target_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 13.0 / 255.0,
                                    g: 15.0 / 255.0,
                                    b: 18.0 / 255.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });
                self.wgpu.composite(&mut pass)?;
            }
        }
        Ok(())
    }

    pub fn render_offscreen(
        &mut self,
        context: BackendRenderContext<'_>,
        target_size: [u32; 2],
        frame: BackendFrame<'_>,
    ) -> Result<(), BackendRenderError> {
        match self.backend {
            RenderBackend::Wgpu => {
                self.wgpu.prepare_assets(
                    context.device,
                    context.queue,
                    frame.snapshot,
                    frame.assets,
                )?;
                self.wgpu.render_offscreen(
                    context.device,
                    context.encoder,
                    target_size,
                    frame.snapshot,
                    frame.view,
                )?;
            }
            RenderBackend::Cpu => {
                let cpu_frame = self.cpu.render_offscreen(
                    target_size,
                    frame.snapshot,
                    frame.view,
                    frame.assets,
                )?;
                self.wgpu.upload_offscreen_rgba(
                    context.device,
                    context.queue,
                    cpu_frame.size(),
                    cpu_frame.rgba(),
                )?;
            }
        }
        Ok(())
    }

    pub fn composite(&self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), RenderTargetError> {
        self.wgpu.composite(pass)
    }

    pub fn clear_asset_cache(&mut self) {
        self.wgpu.clear_asset_cache();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendRenderError {
    #[error(transparent)]
    Assets(#[from] GpuAssetError),
    #[error(transparent)]
    Gpu(#[from] RenderFrameError),
    #[error(transparent)]
    Cpu(#[from] CpuRenderError),
    #[error(transparent)]
    Target(#[from] RenderTargetError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_names_are_stable_and_strict() {
        assert_eq!(RenderBackend::Wgpu.to_string(), "wgpu");
        assert_eq!("cpu".parse(), Ok(RenderBackend::Cpu));
        assert!("gpu".parse::<RenderBackend>().is_err());
    }
}

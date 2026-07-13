// Copyright The SimpleGameEngine Contributors

//! Typed render components and runtime rendering products.

mod backend;
mod components;
mod cpu;
mod extract;
mod frame_rate;
mod gpu;
mod plugin;
mod projection;
mod snapshot;
mod surface;

pub use backend::{
    BackendFrame, BackendRenderContext, BackendRenderError, BackendRenderer, RenderBackend,
    RenderBackendParseError,
};
pub use components::{Camera, Light, Material, MeshRenderer, Projection};
pub use cpu::{CpuFrame, CpuRenderError, CpuRenderer};
pub use extract::{RenderComponentKind, RenderExtractionError, RenderItemKind, extract};
pub use frame_rate::FrameRateCounter;
pub use gpu::{
    FrameNotPreparedError, GpuAssetError, GpuBufferKind, RenderFrameError, RenderTargetError,
    ViewProjectionError, WgpuRenderer,
};
pub use plugin::RenderPlugin;
pub use projection::view_projection_matrix;
pub use snapshot::{
    RenderCamera, RenderLight, RenderMeshInstance, RenderSnapshot, RenderView, RenderViewError,
};
pub use surface::{
    SkippedSurfaceFrame, SurfaceReadback, SurfaceRenderError, SurfaceRenderOutcome, SurfaceRenderer,
};

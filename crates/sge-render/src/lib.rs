// Copyright The SimpleGameEngine Contributors

//! Typed render components and runtime rendering products.

mod components;
mod extract;
mod gpu;
mod plugin;
mod snapshot;

pub use components::{Camera, Light, Material, MeshRenderer, Projection};
pub use extract::{RenderComponentKind, RenderExtractionError, RenderItemKind, extract};
pub use gpu::{
    FrameNotPreparedError, GpuAssetError, GpuBufferKind, RenderFrameError, RenderTargetError,
    ViewProjectionError, WgpuRenderer, view_projection_matrix,
};
pub use plugin::RenderPlugin;
pub use snapshot::{
    RenderCamera, RenderLight, RenderMeshInstance, RenderSnapshot, RenderView, RenderViewError,
};

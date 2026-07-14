// Copyright The SimpleGameEngine Contributors

use sge_asset::AssetId;

use crate::RenderViewError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBufferKind {
    Vertex,
    WireframeVertex,
    Index,
}

#[derive(Debug, thiserror::Error)]
pub enum GpuAssetError {
    #[error("GPU mesh asset is missing: {asset}")]
    MissingAsset { asset: AssetId },
    #[error("GPU texture asset is missing: {asset}")]
    MissingTexture { asset: AssetId },
    #[error("GPU mesh index count exceeds u32 for asset {asset}")]
    IndexCountOverflow { asset: AssetId },
    #[error(
        "GPU {buffer:?} buffer for asset {asset} requires {size} bytes, exceeding device limit {max}"
    )]
    BufferTooLarge {
        asset: AssetId,
        buffer: GpuBufferKind,
        size: u64,
        max: u64,
    },
    #[error("GPU texture {asset:?} extent {width}x{height} exceeds device 2D texture limit {max}")]
    TextureTooLarge {
        asset: Option<AssetId>,
        width: u32,
        height: u32,
        max: u32,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RenderTargetError {
    #[error("render target size must be non-zero")]
    ZeroSize,
    #[error("render target {width}x{height} exceeds device 2D texture limit {max}")]
    TooLarge { width: u32, height: u32, max: u32 },
    #[error("offscreen render target has not been prepared")]
    OffscreenUnavailable,
    #[error("CPU frame upload requires RGBA/BGRA 8-bit target format, found {0:?}")]
    CpuUploadFormat(wgpu::TextureFormat),
    #[error("CPU frame RGBA length mismatch: expected {expected}, found {found}")]
    InvalidRgbaLength { expected: usize, found: usize },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ViewProjectionError {
    #[error(transparent)]
    Target(#[from] RenderTargetError),
    #[error(transparent)]
    View(#[from] RenderViewError),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameNotPreparedError {
    #[error("GPU mesh asset is not prepared: {asset}")]
    Asset { asset: AssetId },
    #[error("GPU texture asset is not prepared: {asset}")]
    Texture { asset: AssetId },
    #[error("render instance count exceeds u32 for asset {asset}")]
    InstanceCountOverflow { asset: AssetId },
    #[error("render instance buffer requires {size} bytes, exceeding device limit {max}")]
    InstanceBufferTooLarge { size: u64, max: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum RenderFrameError {
    #[error(transparent)]
    Target(#[from] RenderTargetError),
    #[error(transparent)]
    Projection(#[from] ViewProjectionError),
    #[error(transparent)]
    NotPrepared(#[from] FrameNotPreparedError),
}

// Copyright The SimpleGameEngine Contributors

use sge_asset::AssetId;

use super::{
    errors::{GpuAssetError, GpuBufferKind},
    renderer::checked_buffer_size,
};

#[test]
fn oversized_gpu_buffer_is_a_typed_error() {
    let asset = AssetId::new_v4();
    assert!(matches!(
        checked_buffer_size(asset, GpuBufferKind::Vertex, 9, 8),
        Err(GpuAssetError::BufferTooLarge {
            asset: found,
            buffer: GpuBufferKind::Vertex,
            size: 9,
            max: 8,
        }) if found == asset
    ));
}

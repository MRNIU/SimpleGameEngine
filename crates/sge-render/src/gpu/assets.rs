// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{AssetId, RuntimeAssetStore};
use wgpu::util::DeviceExt;

use crate::RenderSnapshot;

use super::errors::{GpuAssetError, GpuBufferKind};

pub(super) struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub wireframe_vertex_buffer: wgpu::Buffer,
    pub wireframe_vertex_count: u32,
}

pub(super) fn prepare_assets(
    meshes: &mut BTreeMap<AssetId, GpuMesh>,
    device: &wgpu::Device,
    snapshot: &RenderSnapshot,
    store: &RuntimeAssetStore,
) -> Result<(), GpuAssetError> {
    for instance in snapshot.meshes() {
        let asset = *instance.mesh().id();
        if meshes.contains_key(&asset) {
            continue;
        }
        let mesh = store
            .mesh(instance.mesh())
            .map_err(|_| GpuAssetError::MissingAsset { asset })?;
        let index_count = u32::try_from(mesh.indices().len())
            .map_err(|_| GpuAssetError::IndexCountOverflow { asset })?;
        let vertex_bytes = mesh
            .vertices()
            .iter()
            .flat_map(|vertex| {
                vertex
                    .position()
                    .iter()
                    .copied()
                    .chain(vertex.normal().copied().unwrap_or([0.0, 0.0, 1.0]))
            })
            .flat_map(f32::to_ne_bytes)
            .collect::<Vec<_>>();
        let index_bytes = mesh
            .indices()
            .iter()
            .copied()
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>();
        let wireframe_vertex_bytes = mesh
            .indices()
            .chunks_exact(3)
            .flat_map(|indices| {
                indices
                    .iter()
                    .zip([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
                    .flat_map(|(index, barycentric)| {
                        mesh.vertices()[*index as usize]
                            .position()
                            .iter()
                            .copied()
                            .chain(barycentric)
                    })
            })
            .flat_map(f32::to_ne_bytes)
            .collect::<Vec<_>>();
        for (buffer, size) in [
            (GpuBufferKind::Vertex, vertex_bytes.len()),
            (GpuBufferKind::Index, index_bytes.len()),
            (GpuBufferKind::WireframeVertex, wireframe_vertex_bytes.len()),
        ] {
            checked_buffer_size(asset, buffer, size, device.limits().max_buffer_size)?;
        }
        meshes.insert(
            asset,
            GpuMesh {
                vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sge_render_mesh_vertices"),
                    contents: &vertex_bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sge_render_mesh_indices_u32"),
                    contents: &index_bytes,
                    usage: wgpu::BufferUsages::INDEX,
                }),
                index_count,
                wireframe_vertex_buffer: device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("sge_render_wireframe_vertices"),
                        contents: &wireframe_vertex_bytes,
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ),
                wireframe_vertex_count: index_count,
            },
        );
    }
    Ok(())
}

pub(super) fn checked_buffer_size(
    asset: AssetId,
    buffer: GpuBufferKind,
    size: usize,
    max: u64,
) -> Result<u64, GpuAssetError> {
    let size = u64::try_from(size).unwrap_or(u64::MAX);
    if size > max {
        return Err(GpuAssetError::BufferTooLarge {
            asset,
            buffer,
            size,
            max,
        });
    }
    Ok(size)
}

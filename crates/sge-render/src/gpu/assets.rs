// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{AssetId, RuntimeAssetStore, TextureAsset};
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

pub(super) struct GpuTexture {
    pub _texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub(super) struct GpuUploadContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub texture_layout: &'a wgpu::BindGroupLayout,
    pub texture_sampler: &'a wgpu::Sampler,
}

pub(super) fn prepare_assets(
    meshes: &mut BTreeMap<AssetId, GpuMesh>,
    textures: &mut BTreeMap<AssetId, GpuTexture>,
    upload: GpuUploadContext<'_>,
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
                    .chain(vertex.texcoord().copied().unwrap_or([0.0; 2]))
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
            checked_buffer_size(asset, buffer, size, upload.device.limits().max_buffer_size)?;
        }
        meshes.insert(
            asset,
            GpuMesh {
                vertex_buffer: upload.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("sge_render_mesh_vertices"),
                        contents: &vertex_bytes,
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ),
                index_buffer: upload
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sge_render_mesh_indices_u32"),
                        contents: &index_bytes,
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                index_count,
                wireframe_vertex_buffer: upload.device.create_buffer_init(
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
    for instance in snapshot.meshes() {
        let Some(reference) = instance.material().texture() else {
            continue;
        };
        let asset = *reference.id();
        if textures.contains_key(&asset) {
            continue;
        }
        let texture = store
            .texture(reference)
            .map_err(|_| GpuAssetError::MissingTexture { asset })?;
        textures.insert(
            asset,
            create_texture(
                upload.device,
                upload.queue,
                upload.texture_layout,
                upload.texture_sampler,
                texture,
                Some(asset),
            )?,
        );
    }
    Ok(())
}

pub(super) fn create_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    source: &TextureAsset,
    asset: Option<AssetId>,
) -> Result<GpuTexture, GpuAssetError> {
    let [width, height] = source.size();
    let max = device.limits().max_texture_dimension_2d;
    if width > max || height > max {
        return Err(GpuAssetError::TextureTooLarge {
            asset,
            width,
            height,
            max,
        });
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_render_color_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        source.rgba8_srgb(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sge_render_color_texture_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Ok(GpuTexture {
        _texture: texture,
        bind_group,
    })
}

pub(super) fn create_fallback_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> GpuTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_render_untextured_binding"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sge_render_untextured_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    GpuTexture {
        _texture: texture,
        bind_group,
    }
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

// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{AssetId, RuntimeAssetStore};
use sge_math::Mat3;
use wgpu::util::DeviceExt;

use crate::{RenderSnapshot, RenderView};

use super::{
    errors::{
        FrameNotPreparedError, GpuAssetError, GpuBufferKind, RenderFrameError, RenderTargetError,
    },
    frame::{
        create_depth_target, extent, normalized_model_matrix, uniform_bytes, validate_target_size,
    },
    pipeline::{create_composite_pipeline, create_mesh_pipeline},
};

const SURFACE_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 13.0 / 255.0,
    g: 15.0 / 255.0,
    b: 18.0 / 255.0,
    a: 1.0,
};
const OFFSCREEN_CLEAR_COLOR: wgpu::Color = wgpu::Color::TRANSPARENT;

pub struct WgpuRenderer {
    target_format: wgpu::TextureFormat,
    mesh_pipeline: wgpu::RenderPipeline,
    frame_layout: wgpu::BindGroupLayout,
    composite_pipeline: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    meshes: BTreeMap<AssetId, GpuMesh>,
    offscreen: Option<OffscreenTarget>,
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct OffscreenTarget {
    size: [u32; 2],
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    composite_bind_group: wgpu::BindGroup,
}

struct DrawBatch {
    asset: AssetId,
    instances: std::ops::Range<u32>,
}

struct FrameTarget<'a> {
    view: &'a wgpu::TextureView,
    size: [u32; 2],
    clear_color: wgpu::Color,
}

impl WgpuRenderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sge_render_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sge_render_frame_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sge_render_composite_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let mesh_pipeline = create_mesh_pipeline(device, &shader, &frame_layout, target_format);
        let composite_pipeline =
            create_composite_pipeline(device, &shader, &composite_layout, target_format);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sge_render_composite_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            target_format,
            mesh_pipeline,
            frame_layout,
            composite_pipeline,
            composite_layout,
            sampler,
            meshes: BTreeMap::new(),
            offscreen: None,
        }
    }

    pub fn prepare_assets(
        &mut self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        snapshot: &RenderSnapshot,
        store: &RuntimeAssetStore,
    ) -> Result<(), GpuAssetError> {
        for instance in snapshot.meshes() {
            let asset = *instance.mesh().id();
            if self.meshes.contains_key(&asset) {
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
            checked_buffer_size(
                asset,
                GpuBufferKind::Vertex,
                vertex_bytes.len(),
                device.limits().max_buffer_size,
            )?;
            checked_buffer_size(
                asset,
                GpuBufferKind::Index,
                index_bytes.len(),
                device.limits().max_buffer_size,
            )?;
            self.meshes.insert(
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
                },
            );
        }
        Ok(())
    }

    pub fn render_to_target(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        target_size: [u32; 2],
        snapshot: &RenderSnapshot,
        view: RenderView,
    ) -> Result<(), RenderFrameError> {
        self.render_to_target_with_clear(
            device,
            encoder,
            FrameTarget {
                view: target_view,
                size: target_size,
                clear_color: SURFACE_CLEAR_COLOR,
            },
            snapshot,
            view,
        )
    }

    fn render_to_target_with_clear(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: FrameTarget<'_>,
        snapshot: &RenderSnapshot,
        view: RenderView,
    ) -> Result<(), RenderFrameError> {
        validate_target_size(device, target.size)?;
        let (mut instance_bytes, batches) = self.prepare_instances(snapshot)?;
        if instance_bytes.is_empty() {
            instance_bytes.resize(128, 0);
        }
        let instance_size = u64::try_from(instance_bytes.len()).unwrap_or(u64::MAX);
        let max_buffer_size = device.limits().max_buffer_size;
        if instance_size > max_buffer_size {
            return Err(FrameNotPreparedError::InstanceBufferTooLarge {
                size: instance_size,
                max: max_buffer_size,
            }
            .into());
        }
        let uniform_bytes = uniform_bytes(snapshot, view, target.size)?;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sge_render_frame_uniform"),
            contents: &uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sge_render_frame_bind_group"),
            layout: &self.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sge_render_instances"),
            contents: &instance_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let depth = create_depth_target(device, target.size);
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sge_render_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(target.clear_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.mesh_pipeline);
        pass.set_bind_group(0, &frame_bind_group, &[]);
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        for batch in batches {
            let mesh = &self.meshes[&batch.asset];
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, batch.instances);
        }
        Ok(())
    }

    pub fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_size: [u32; 2],
        snapshot: &RenderSnapshot,
        view: RenderView,
    ) -> Result<(), RenderFrameError> {
        if self
            .offscreen
            .as_ref()
            .is_none_or(|target| target.size != target_size)
        {
            self.offscreen = Some(self.create_offscreen_target(device, target_size)?);
        }
        let target = self
            .offscreen
            .take()
            .ok_or(RenderTargetError::OffscreenUnavailable)?;
        let result = self.render_to_target_with_clear(
            device,
            encoder,
            FrameTarget {
                view: &target.color_view,
                size: target_size,
                clear_color: OFFSCREEN_CLEAR_COLOR,
            },
            snapshot,
            view,
        );
        self.offscreen = Some(target);
        result
    }

    pub fn composite(&self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), RenderTargetError> {
        let target = self
            .offscreen
            .as_ref()
            .ok_or(RenderTargetError::OffscreenUnavailable)?;
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &target.composite_bind_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }

    #[must_use]
    pub fn cached_mesh_count(&self) -> usize {
        self.meshes.len()
    }

    pub fn clear_asset_cache(&mut self) {
        self.meshes.clear();
    }

    fn prepare_instances(
        &self,
        snapshot: &RenderSnapshot,
    ) -> Result<(Vec<u8>, Vec<DrawBatch>), FrameNotPreparedError> {
        let mut grouped = BTreeMap::<AssetId, Vec<_>>::new();
        for instance in snapshot.meshes() {
            let asset = *instance.mesh().id();
            if !self.meshes.contains_key(&asset) {
                return Err(FrameNotPreparedError::Asset { asset });
            }
            grouped.entry(asset).or_default().push(*instance);
        }
        let mut bytes = Vec::new();
        let mut batches = Vec::new();
        let mut first = 0_u32;
        for (asset, instances) in grouped {
            let count = u32::try_from(instances.len())
                .map_err(|_| FrameNotPreparedError::InstanceCountOverflow { asset })?;
            for instance in instances {
                let model = normalized_model_matrix(instance.transform());
                let normal = Mat3::from_mat4(model).inverse().transpose().to_cols_array();
                let normal_padded = [
                    normal[0], normal[1], normal[2], 0.0, normal[3], normal[4], normal[5], 0.0,
                    normal[6], normal[7], normal[8], 0.0,
                ];
                bytes.extend(
                    model
                        .to_cols_array()
                        .into_iter()
                        .chain(instance.material().base_color())
                        .chain(normal_padded)
                        .flat_map(f32::to_ne_bytes),
                );
            }
            let end = first
                .checked_add(count)
                .ok_or(FrameNotPreparedError::InstanceCountOverflow { asset })?;
            batches.push(DrawBatch {
                asset,
                instances: first..end,
            });
            first = end;
        }
        Ok((bytes, batches))
    }

    fn create_offscreen_target(
        &self,
        device: &wgpu::Device,
        size: [u32; 2],
    ) -> Result<OffscreenTarget, RenderTargetError> {
        validate_target_size(device, size)?;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sge_render_offscreen_color"),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sge_render_composite_bind_group"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Ok(OffscreenTarget {
            size,
            _color: color,
            color_view,
            composite_bind_group,
        })
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

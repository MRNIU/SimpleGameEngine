// Copyright The SimpleGameEngine Contributors
//
//! 渲染数据抽取与 wgpu viewport 边界。

use std::{borrow::Cow, collections::BTreeMap, ops::Range};

use ecs::{Camera, EntityId, Light, Projection, World};
use math::{Quat, Transform, Vec3};
use wgpu::util::DeviceExt;

mod viewport_projection;

pub use viewport_projection::{
    DEFAULT_FAR_PLANE, DEFAULT_NEAR_PLANE, ViewportClipPlanes,
    ViewportProjectionMatrix as ViewportProjection, ViewportSize, WorldRay,
};

pub const VIEWPORT_SHADER: &str = include_str!("viewport.wgsl");
const VIEWPORT_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 12,
        shader_location: 1,
    },
];
const CUBE_COLOR: [f32; 4] = [0.3, 0.64, 1.0, 1.0];
const SELECTED_CUBE_COLOR: [f32; 4] = [1.0, 0.78, 0.25, 1.0];
const VIEWPORT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VIEWPORT_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderScene {
    pub meshes: Vec<MeshDraw>,
    pub lights: Vec<LightDraw>,
    pub active_camera: Option<CameraView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshDraw {
    pub entity: EntityId,
    pub transform: Transform,
    pub mesh_asset: String,
    pub material_asset: String,
    pub base_color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightDraw {
    pub entity: EntityId,
    pub light: Light,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraView {
    pub entity: EntityId,
    pub transform: Transform,
    pub camera: Camera,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportPipelineInfo {
    pub label: &'static str,
    pub color_format: wgpu::TextureFormat,
    pub primitive_topology: wgpu::PrimitiveTopology,
    pub shader_source: &'static str,
    pub depth_format: Option<wgpu::TextureFormat>,
    pub grid_topology: wgpu::PrimitiveTopology,
    pub grid_depth_write: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportDrawCall {
    pub label: String,
    pub camera_entity: EntityId,
    pub vertex_count: usize,
    pub index_count: usize,
    pub vertices: Vec<ViewportVertex>,
    pub indices: Vec<u16>,
    pub mesh_spans: Vec<ViewportMeshSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportView {
    pub entity: EntityId,
    pub transform: Transform,
    pub projection: Projection,
}

impl ViewportView {
    #[must_use]
    pub const fn new(entity: EntityId, transform: Transform, projection: Projection) -> Self {
        Self {
            entity,
            transform,
            projection,
        }
    }

    #[must_use]
    pub fn from_camera(camera: &CameraView) -> Self {
        Self {
            entity: camera.entity.clone(),
            transform: camera.transform,
            projection: camera.camera.projection.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportMeshSpan {
    pub entity: EntityId,
    pub vertex_range: Range<usize>,
    pub index_range: Range<usize>,
    pub world_bounds_min: [f32; 3],
    pub world_bounds_max: [f32; 3],
    pub world_center: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
struct ViewportProjectionContext {
    light_multiplier: [f32; 3],
}

pub struct ViewportRenderer {
    grid_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Option<ViewportTargets>,
    vertex_buffer: wgpu::Buffer,
    grid_vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    grid_vertex_count: u32,
}

pub struct ViewportRenderFrame<'a> {
    pub draw: Option<&'a ViewportDrawCall>,
    pub grid_vertices: &'a [ViewportVertex],
    pub view_projection: [f32; 16],
    pub target_size: [u32; 2],
}

struct ViewportTargets {
    size: [u32; 2],
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    composite_bind_group: wgpu::BindGroup,
}

impl ViewportRenderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sge_viewport_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(VIEWPORT_SHADER)),
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sge_viewport_camera_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sge_viewport_composite_layout"),
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
        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sge_viewport_mesh_pipeline_layout"),
            bind_group_layouts: &[Some(&camera_bind_group_layout)],
            immediate_size: 0,
        });
        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sge_viewport_composite_pipeline_layout"),
                bind_group_layouts: &[Some(&composite_bind_group_layout)],
                immediate_size: 0,
            });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sge_viewport_camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sge_viewport_camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sge_viewport_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vertex_layout = viewport_vertex_buffer_layout();
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sge_viewport_mesh_pipeline"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mesh"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: VIEWPORT_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: VIEWPORT_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sge_viewport_grid_pipeline"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mesh"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[viewport_vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: VIEWPORT_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: VIEWPORT_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sge_viewport_composite_pipeline"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_composite"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_composite"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            grid_pipeline,
            mesh_pipeline,
            composite_pipeline,
            camera_buffer,
            camera_bind_group,
            composite_bind_group_layout,
            sampler,
            targets: None,
            vertex_buffer: empty_buffer(
                device,
                wgpu::BufferUsages::VERTEX,
                "sge_viewport_vertices",
            ),
            grid_vertex_buffer: empty_buffer(
                device,
                wgpu::BufferUsages::VERTEX,
                "sge_viewport_grid_vertices",
            ),
            index_buffer: empty_buffer(device, wgpu::BufferUsages::INDEX, "sge_viewport_indices"),
            index_count: 0,
            grid_vertex_count: 0,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: ViewportRenderFrame<'_>,
    ) {
        let ViewportRenderFrame {
            draw,
            grid_vertices,
            view_projection,
            target_size,
        } = frame;
        if target_size.contains(&0) {
            self.index_count = 0;
            return;
        }

        if let Some(draw) = draw {
            self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sge_viewport_vertices"),
                contents: &viewport_vertex_bytes(&draw.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sge_viewport_indices"),
                contents: &viewport_index_bytes(&draw.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.index_count = draw.index_count as u32;
        } else {
            self.index_count = 0;
        }
        if grid_vertices.is_empty() {
            self.grid_vertex_count = 0;
        } else {
            self.grid_vertex_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sge_viewport_grid_vertices"),
                    contents: &viewport_vertex_bytes(grid_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            self.grid_vertex_count = grid_vertices.len() as u32;
        }
        queue.write_buffer(
            &self.camera_buffer,
            0,
            &view_projection
                .into_iter()
                .flat_map(f32::to_ne_bytes)
                .collect::<Vec<_>>(),
        );
        if self
            .targets
            .as_ref()
            .is_none_or(|targets| targets.size != target_size)
        {
            self.targets = Some(create_viewport_targets(
                device,
                target_size,
                &self.composite_bind_group_layout,
                &self.sampler,
            ));
        }
        let Some(targets) = self.targets.as_ref() else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sge_viewport_offscreen_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 18.0 / 255.0,
                        g: 24.0 / 255.0,
                        b: 29.0 / 255.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth_view,
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
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_pipeline(&self.grid_pipeline);
        pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
        pass.draw(0..self.grid_vertex_count, 0..1);
        pass.set_pipeline(&self.mesh_pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let Some(targets) = self.targets.as_ref() else {
            return;
        };
        render_pass.set_pipeline(&self.composite_pipeline);
        render_pass.set_bind_group(0, &targets.composite_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn create_viewport_targets(
    device: &wgpu::Device,
    size: [u32; 2],
    composite_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> ViewportTargets {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_viewport_color"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: VIEWPORT_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_viewport_depth"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: VIEWPORT_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sge_viewport_composite_bind_group"),
        layout: composite_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&color_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    ViewportTargets {
        size,
        _color: color,
        color_view,
        _depth: depth,
        depth_view,
        composite_bind_group,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererConfig {
    pub clear_color: wgpu::Color,
    pub backends: wgpu::Backends,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            clear_color: wgpu::Color {
                r: 0.05,
                g: 0.06,
                b: 0.07,
                a: 1.0,
            },
            backends: wgpu::Backends::PRIMARY,
        }
    }
}

#[must_use]
pub fn extract_render_scene(world: &World) -> RenderScene {
    let meshes = world
        .entities()
        .filter_map(|record| {
            record.mesh.as_ref().map(|mesh| MeshDraw {
                entity: record.id.clone(),
                transform: record.transform,
                mesh_asset: mesh.asset.clone(),
                material_asset: mesh.material.clone(),
                base_color: record
                    .material_override
                    .as_ref()
                    .map_or(CUBE_COLOR, |material| material.base_color),
            })
        })
        .collect();

    let lights = world
        .entities()
        .filter_map(|record| {
            record.light.as_ref().map(|light| LightDraw {
                entity: record.id.clone(),
                light: light.clone(),
            })
        })
        .collect();

    let active_camera = world.entities().find_map(|record| {
        record.camera.as_ref().map(|camera| CameraView {
            entity: record.id.clone(),
            transform: record.transform,
            camera: camera.clone(),
        })
    });

    RenderScene {
        meshes,
        lights,
        active_camera,
    }
}

#[must_use]
pub const fn viewport_pipeline_info(color_format: wgpu::TextureFormat) -> ViewportPipelineInfo {
    ViewportPipelineInfo {
        label: "sge_viewport_pipeline",
        color_format,
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        shader_source: VIEWPORT_SHADER,
        depth_format: Some(VIEWPORT_DEPTH_FORMAT),
        grid_topology: wgpu::PrimitiveTopology::LineList,
        grid_depth_write: true,
    }
}

#[must_use]
pub const fn viewport_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 28,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VIEWPORT_VERTEX_ATTRIBUTES,
    }
}

#[must_use]
pub fn viewport_draw_call(scene: &RenderScene) -> Option<ViewportDrawCall> {
    viewport_draw_call_with_selection(scene, None)
}

#[must_use]
pub fn viewport_draw_call_with_selection(
    scene: &RenderScene,
    selected_entity: Option<&EntityId>,
) -> Option<ViewportDrawCall> {
    let camera = scene.active_camera.as_ref()?;
    let view = ViewportView::from_camera(camera);
    viewport_draw_call_with_view(scene, selected_entity, &view)
}

#[must_use]
pub fn viewport_draw_call_with_view(
    scene: &RenderScene,
    selected_entity: Option<&EntityId>,
    view: &ViewportView,
) -> Option<ViewportDrawCall> {
    viewport_draw_call_with_view_and_meshes(scene, selected_entity, view, &BTreeMap::new())
}

#[must_use]
pub fn viewport_draw_call_with_view_and_meshes(
    scene: &RenderScene,
    selected_entity: Option<&EntityId>,
    view: &ViewportView,
    imported_meshes: &BTreeMap<asset::AssetUuid, asset::ImportedMesh>,
) -> Option<ViewportDrawCall> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut mesh_spans = Vec::new();
    let size = 0.28;
    let light_multiplier = light_multiplier(scene);
    let projection_context = ViewportProjectionContext { light_multiplier };
    let mut has_imported_mesh = false;
    let mut has_non_cube_primitive = false;

    for mesh in &scene.meshes {
        let vertex_start = vertices.len();
        let index_start = indices.len();
        let color = lit_material_color(mesh.base_color, light_multiplier);
        let color = if selected_entity.is_some_and(|selected| selected == &mesh.entity) {
            selected_tint(color)
        } else {
            color
        };

        let (world_bounds_min, world_bounds_max, world_center) = match mesh.mesh_asset.as_str() {
            "primitive:cube" => push_cube_mesh(&mut vertices, &mut indices, mesh, color, size)?,
            "primitive:sphere" => {
                has_non_cube_primitive = true;
                push_sphere_mesh(&mut vertices, &mut indices, mesh, color, size)?
            }
            "primitive:cone" => {
                has_non_cube_primitive = true;
                push_cone_mesh(&mut vertices, &mut indices, mesh, color, size)?
            }
            "primitive:cylinder" => {
                has_non_cube_primitive = true;
                push_cylinder_mesh(&mut vertices, &mut indices, mesh, color, size)?
            }
            asset if asset.starts_with("primitive:") => continue,
            _ => continue,
        };

        mesh_spans.push(ViewportMeshSpan {
            entity: mesh.entity.clone(),
            vertex_range: vertex_start..vertices.len(),
            index_range: index_start..indices.len(),
            world_bounds_min,
            world_bounds_max,
            world_center,
        });
    }

    for mesh in &scene.meshes {
        let Ok(uuid) = asset::AssetUuid::parse_asset_ref(&mesh.mesh_asset) else {
            continue;
        };
        let Some(imported_mesh) = imported_meshes.get(&uuid) else {
            continue;
        };
        has_imported_mesh = true;
        push_imported_mesh(
            &mut vertices,
            &mut indices,
            &mut mesh_spans,
            mesh,
            imported_mesh,
            selected_entity,
            projection_context,
        )?;
    }

    if vertices.is_empty() {
        return None;
    }

    Some(ViewportDrawCall {
        label: if has_imported_mesh {
            "viewport:mesh".to_owned()
        } else if has_non_cube_primitive {
            "viewport:primitive".to_owned()
        } else {
            "primitive:cube".to_owned()
        },
        camera_entity: view.entity.clone(),
        vertex_count: vertices.len(),
        index_count: indices.len(),
        vertices,
        indices,
        mesh_spans,
    })
}

fn push_cube_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh: &MeshDraw,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let world_points = transformed_cube_world_points(&mesh.transform, size);

    push_cube_face(
        vertices,
        indices,
        &world_points,
        [0, 1, 2, 3],
        shade_color(color, 0.38),
    )?;
    push_cube_face(
        vertices,
        indices,
        &world_points,
        [0, 4, 7, 3],
        shade_color(color, 0.46),
    )?;
    push_cube_face(
        vertices,
        indices,
        &world_points,
        [0, 1, 5, 4],
        shade_color(color, 0.54),
    )?;
    push_cube_face(
        vertices,
        indices,
        &world_points,
        [4, 5, 6, 7],
        shade_color(color, 1.0),
    )?;
    push_cube_face(
        vertices,
        indices,
        &world_points,
        [1, 5, 6, 2],
        shade_color(color, 0.82),
    )?;
    push_cube_face(
        vertices,
        indices,
        &world_points,
        [3, 2, 6, 7],
        shade_color(color, 0.68),
    )?;
    bounds_from_world_points(&world_points)
}

fn push_sphere_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh: &MeshDraw,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let vertex_start = vertices.len();
    let radius = size;
    let world_points = push_primitive_vertices(
        vertices,
        mesh,
        color,
        &[
            Vec3::new(0.0, radius, 0.0),
            Vec3::new(radius, 0.0, 0.0),
            Vec3::new(0.0, 0.0, radius),
            Vec3::new(-radius, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -radius),
            Vec3::new(0.0, -radius, 0.0),
        ],
    )?;
    push_primitive_indices(
        indices,
        vertex_start,
        &[
            0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1, 5, 2, 1, 5, 3, 2, 5, 4, 3, 5, 1, 4,
        ],
    )?;
    bounds_from_world_points(&world_points)
}

fn push_cone_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh: &MeshDraw,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let vertex_start = vertices.len();
    let radius = size;
    let height = size;
    let world_points = push_primitive_vertices(
        vertices,
        mesh,
        color,
        &[
            Vec3::new(0.0, height, 0.0),
            Vec3::new(radius, -height, 0.0),
            Vec3::new(0.707_106_77 * radius, -height, 0.707_106_77 * radius),
            Vec3::new(0.0, -height, radius),
            Vec3::new(-0.707_106_77 * radius, -height, 0.707_106_77 * radius),
            Vec3::new(-radius, -height, 0.0),
            Vec3::new(-0.707_106_77 * radius, -height, -0.707_106_77 * radius),
            Vec3::new(0.0, -height, -radius),
            Vec3::new(0.707_106_77 * radius, -height, -0.707_106_77 * radius),
            Vec3::new(0.0, -height, 0.0),
        ],
    )?;
    push_primitive_indices(
        indices,
        vertex_start,
        &[
            0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5, 0, 5, 6, 0, 6, 7, 0, 7, 8, 0, 8, 1, 9, 2, 1, 9, 3,
            2, 9, 4, 3, 9, 5, 4, 9, 6, 5, 9, 7, 6, 9, 8, 7, 9, 1, 8,
        ],
    )?;
    bounds_from_world_points(&world_points)
}

fn push_cylinder_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh: &MeshDraw,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let vertex_start = vertices.len();
    let radius = size;
    let height = size;
    let diagonal = 0.707_106_77 * radius;
    let world_points = push_primitive_vertices(
        vertices,
        mesh,
        color,
        &[
            Vec3::new(0.0, height, 0.0),
            Vec3::new(0.0, -height, 0.0),
            Vec3::new(radius, height, 0.0),
            Vec3::new(diagonal, height, diagonal),
            Vec3::new(0.0, height, radius),
            Vec3::new(-diagonal, height, diagonal),
            Vec3::new(-radius, height, 0.0),
            Vec3::new(-diagonal, height, -diagonal),
            Vec3::new(0.0, height, -radius),
            Vec3::new(diagonal, height, -diagonal),
            Vec3::new(radius, -height, 0.0),
            Vec3::new(diagonal, -height, diagonal),
            Vec3::new(0.0, -height, radius),
            Vec3::new(-diagonal, -height, diagonal),
            Vec3::new(-radius, -height, 0.0),
            Vec3::new(-diagonal, -height, -diagonal),
            Vec3::new(0.0, -height, -radius),
            Vec3::new(diagonal, -height, -diagonal),
        ],
    )?;
    for segment in 0_u16..8 {
        let next = (segment + 1) % 8;
        let top = 2 + segment;
        let top_next = 2 + next;
        let bottom = 10 + segment;
        let bottom_next = 10 + next;
        push_primitive_indices(
            indices,
            vertex_start,
            &[top, bottom, bottom_next, top, bottom_next, top_next],
        )?;
        push_primitive_indices(indices, vertex_start, &[0, top, top_next])?;
        push_primitive_indices(indices, vertex_start, &[1, bottom_next, bottom])?;
    }
    bounds_from_world_points(&world_points)
}

fn push_primitive_vertices(
    vertices: &mut Vec<ViewportVertex>,
    mesh: &MeshDraw,
    color: [f32; 4],
    locals: &[Vec3],
) -> Option<Vec<Vec3>> {
    let transform_rotation = Quat::from_array(normalized_quaternion(mesh.transform.rotation));
    let transform_translation = Vec3::from_array(mesh.transform.translation);
    let transform_scale = Vec3::from_array(mesh.transform.scale);
    let mut world_points = Vec::with_capacity(locals.len());

    for local in locals {
        let world = transform_rotation * (*local * transform_scale) + transform_translation;
        vertices.push(ViewportVertex {
            position: world.to_array(),
            color,
        });
        world_points.push(world);
    }
    Some(world_points)
}

fn push_primitive_indices(
    indices: &mut Vec<u16>,
    vertex_start: usize,
    local_indices: &[u16],
) -> Option<()> {
    for index in local_indices {
        let index = vertex_start.checked_add(usize::from(*index))?;
        indices.push(u16::try_from(index).ok()?);
    }
    Some(())
}

fn push_imported_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh_spans: &mut Vec<ViewportMeshSpan>,
    mesh: &MeshDraw,
    imported_mesh: &asset::ImportedMesh,
    selected_entity: Option<&EntityId>,
    projection_context: ViewportProjectionContext,
) -> Option<()> {
    let vertex_start = vertices.len();
    let index_start = indices.len();
    let transform_rotation = Quat::from_array(normalized_quaternion(mesh.transform.rotation));
    let transform_translation = Vec3::from_array(mesh.transform.translation);
    let transform_scale = Vec3::from_array(mesh.transform.scale);
    let mut world_points = Vec::with_capacity(imported_mesh.vertices.len());
    let color = lit_material_color(mesh.base_color, projection_context.light_multiplier);
    let color = if selected_entity.is_some_and(|selected| selected == &mesh.entity) {
        selected_tint(color)
    } else {
        color
    };

    for vertex in &imported_mesh.vertices {
        let local = Vec3::from_array(vertex.position) * transform_scale;
        let world = transform_rotation * local + transform_translation;
        vertices.push(ViewportVertex {
            position: world.to_array(),
            color,
        });
        world_points.push(world);
    }

    for index in &imported_mesh.indices {
        let index = vertex_start.checked_add(usize::from(*index))?;
        indices.push(u16::try_from(index).ok()?);
    }

    let (world_bounds_min, world_bounds_max, world_center) =
        bounds_from_world_points(&world_points)?;
    mesh_spans.push(ViewportMeshSpan {
        entity: mesh.entity.clone(),
        vertex_range: vertex_start..vertices.len(),
        index_range: index_start..indices.len(),
        world_bounds_min,
        world_bounds_max,
        world_center,
    });
    Some(())
}

fn push_cube_face(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    world_points: &[Vec3; 8],
    face: [usize; 4],
    color: [f32; 4],
) -> Option<()> {
    let base = u16::try_from(vertices.len()).ok()?;
    let i1 = base.checked_add(1)?;
    let i2 = base.checked_add(2)?;
    let i3 = base.checked_add(3)?;

    for corner in face {
        vertices.push(ViewportVertex {
            position: world_points[corner].to_array(),
            color,
        });
    }
    indices.extend([base, i1, i2, base, i2, i3]);
    Some(())
}

fn shade_color(color: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (color[0] * factor).min(1.0),
        (color[1] * factor).min(1.0),
        (color[2] * factor).min(1.0),
        color[3],
    ]
}

fn light_multiplier(scene: &RenderScene) -> [f32; 3] {
    const AMBIENT: f32 = 0.15;
    const PREVIEW_GAIN: f32 = 1.5;

    scene.lights.first().map_or([1.0, 1.0, 1.0], |light| {
        [
            (AMBIENT + light.light.color[0] * light.light.intensity * PREVIEW_GAIN).clamp(0.0, 2.0),
            (AMBIENT + light.light.color[1] * light.light.intensity * PREVIEW_GAIN).clamp(0.0, 2.0),
            (AMBIENT + light.light.color[2] * light.light.intensity * PREVIEW_GAIN).clamp(0.0, 2.0),
        ]
    })
}

fn lit_material_color(base_color: [f32; 4], multiplier: [f32; 3]) -> [f32; 4] {
    [
        (base_color[0] * multiplier[0]).clamp(0.0, 1.0),
        (base_color[1] * multiplier[1]).clamp(0.0, 1.0),
        (base_color[2] * multiplier[2]).clamp(0.0, 1.0),
        base_color[3].clamp(0.0, 1.0),
    ]
}

fn selected_tint(color: [f32; 4]) -> [f32; 4] {
    const TINT: f32 = 0.35;
    [
        color[0] * (1.0 - TINT) + SELECTED_CUBE_COLOR[0] * TINT,
        color[1] * (1.0 - TINT) + SELECTED_CUBE_COLOR[1] * TINT,
        color[2] * (1.0 - TINT) + SELECTED_CUBE_COLOR[2] * TINT,
        color[3],
    ]
}

fn transformed_cube_world_points(transform: &Transform, size: f32) -> [Vec3; 8] {
    let rotation = Quat::from_array(normalized_quaternion(transform.rotation));
    let scale = Vec3::from_array(transform.scale) * size;
    let translation = Vec3::from_array(transform.translation);

    [
        transform_local_point(rotation, translation, [-scale.x, -scale.y, -scale.z]),
        transform_local_point(rotation, translation, [scale.x, -scale.y, -scale.z]),
        transform_local_point(rotation, translation, [scale.x, scale.y, -scale.z]),
        transform_local_point(rotation, translation, [-scale.x, scale.y, -scale.z]),
        transform_local_point(rotation, translation, [-scale.x, -scale.y, scale.z]),
        transform_local_point(rotation, translation, [scale.x, -scale.y, scale.z]),
        transform_local_point(rotation, translation, [scale.x, scale.y, scale.z]),
        transform_local_point(rotation, translation, [-scale.x, scale.y, scale.z]),
    ]
}

fn bounds_from_world_points(points: &[Vec3]) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let first = *points.first()?;
    let mut min = first;
    let mut max = first;
    for point in points {
        if !point.is_finite() {
            return None;
        }
        min = min.min(*point);
        max = max.max(*point);
    }
    let center = (min + max) * 0.5;
    Some((min.to_array(), max.to_array(), center.to_array()))
}

fn normalized_quaternion(rotation: [f32; 4]) -> [f32; 4] {
    let len = rotation
        .into_iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if !len.is_finite() || len == 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    [
        rotation[0] / len,
        rotation[1] / len,
        rotation[2] / len,
        rotation[3] / len,
    ]
}

fn transform_local_point(mesh_rotation: Quat, translation: Vec3, point: [f32; 3]) -> Vec3 {
    mesh_rotation * Vec3::from_array(point) + translation
}

#[must_use]
pub fn viewport_vertex_bytes(vertices: &[ViewportVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * 28);
    for vertex in vertices {
        for value in vertex.position.into_iter().chain(vertex.color) {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

#[must_use]
pub fn viewport_index_bytes(indices: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(indices.len() * 2);
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
    bytes
}

fn empty_buffer(
    device: &wgpu::Device,
    usage: wgpu::BufferUsages,
    label: &'static str,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 4,
        usage,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests;

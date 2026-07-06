// Copyright The SimpleGameEngine Contributors
//
//! 渲染数据抽取与 wgpu viewport 边界。

use std::borrow::Cow;

use ecs::{Camera, EntityId, World};
use math::Transform;
use wgpu::util::DeviceExt;

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

#[derive(Debug, Clone, PartialEq)]
pub struct RenderScene {
    pub meshes: Vec<MeshDraw>,
    pub active_camera: Option<CameraView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshDraw {
    pub entity: EntityId,
    pub transform: Transform,
    pub mesh_asset: String,
    pub material_asset: String,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportDrawCall {
    pub label: String,
    pub camera_entity: EntityId,
    pub vertex_count: usize,
    pub index_count: usize,
    pub vertices: Vec<ViewportVertex>,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl ViewportRenderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sge_viewport_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(VIEWPORT_SHADER)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sge_viewport_pipeline_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let vertex_layout = viewport_vertex_buffer_layout();
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sge_viewport_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(vertex_layout)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            vertex_buffer: empty_buffer(
                device,
                wgpu::BufferUsages::VERTEX,
                "sge_viewport_vertices",
            ),
            index_buffer: empty_buffer(device, wgpu::BufferUsages::INDEX, "sge_viewport_indices"),
            index_count: 0,
        }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, draw: Option<&ViewportDrawCall>) {
        let Some(draw) = draw else {
            self.index_count = 0;
            return;
        };

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
    }

    pub fn paint<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        if self.index_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
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
    let camera = scene.active_camera.as_ref()?;
    let mesh = scene
        .meshes
        .iter()
        .find(|mesh| mesh.mesh_asset == "primitive:cube")?;

    let x = mesh.transform.translation[0] * 0.12;
    let y = mesh.transform.translation[1] * 0.12;
    let size = 0.28;

    Some(ViewportDrawCall {
        label: mesh.mesh_asset.clone(),
        camera_entity: camera.entity.clone(),
        vertex_count: 4,
        index_count: 6,
        vertices: vec![
            ViewportVertex {
                position: [x - size, y - size, 0.0],
                color: [0.3, 0.64, 1.0, 1.0],
            },
            ViewportVertex {
                position: [x + size, y - size, 0.0],
                color: [0.3, 0.64, 1.0, 1.0],
            },
            ViewportVertex {
                position: [x + size, y + size, 0.0],
                color: [0.3, 0.64, 1.0, 1.0],
            },
            ViewportVertex {
                position: [x - size, y + size, 0.0],
                color: [0.3, 0.64, 1.0, 1.0],
            },
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    })
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
mod tests {
    use super::{
        extract_render_scene, viewport_draw_call, viewport_pipeline_info,
        viewport_vertex_buffer_layout, viewport_vertex_bytes,
    };
    use ecs::{Camera, EntityId, MeshRef, Projection, World};
    use math::Transform;

    #[test]
    fn extracts_mesh_draws_from_ecs() {
        let mut world = World::new();
        world.spawn(EntityId::new("cube"), "Cube", Transform::identity());
        world
            .insert_mesh(
                "cube",
                MeshRef::new("primitive:cube", "primitive:default_material"),
            )
            .unwrap();

        let render_scene = extract_render_scene(&world);

        assert_eq!(render_scene.meshes.len(), 1);
        assert_eq!(render_scene.meshes[0].mesh_asset, "primitive:cube");
    }

    #[test]
    fn viewport_pipeline_uses_wgpu_triangle_pipeline() {
        let info = viewport_pipeline_info(wgpu::TextureFormat::Bgra8UnormSrgb);

        assert_eq!(info.label, "sge_viewport_pipeline");
        assert_eq!(info.color_format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!(
            info.primitive_topology,
            wgpu::PrimitiveTopology::TriangleList
        );
        assert!(info.shader_source.contains("@vertex"));
        assert!(info.shader_source.contains("@fragment"));
    }

    #[test]
    fn viewport_draw_call_uses_camera_and_cube_mesh() {
        let mut world = World::new();
        world.spawn(
            EntityId::new("camera"),
            "Camera",
            Transform::from_translation([0.0, 2.0, 5.0]),
        );
        world
            .insert_camera(
                "camera",
                Camera::new(Projection::Perspective {
                    fov_y_degrees: 60.0,
                }),
            )
            .unwrap();
        world.spawn(EntityId::new("cube"), "Cube", Transform::identity());
        world
            .insert_mesh(
                "cube",
                MeshRef::new("primitive:cube", "primitive:default_material"),
            )
            .unwrap();

        let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

        assert_eq!(draw.label, "primitive:cube");
        assert_eq!(draw.vertex_count, 4);
        assert_eq!(draw.index_count, 6);
        assert_eq!(draw.camera_entity, EntityId::new("camera"));
    }

    #[test]
    fn viewport_vertex_layout_matches_shader_locations() {
        let layout = viewport_vertex_buffer_layout();

        assert_eq!(layout.array_stride, 28);
        assert_eq!(layout.attributes[0].shader_location, 0);
        assert_eq!(layout.attributes[0].format, wgpu::VertexFormat::Float32x3);
        assert_eq!(layout.attributes[1].shader_location, 1);
        assert_eq!(layout.attributes[1].format, wgpu::VertexFormat::Float32x4);
    }

    #[test]
    fn viewport_vertex_bytes_match_vertex_count() {
        let mut world = World::new();
        world.spawn(EntityId::new("camera"), "Camera", Transform::identity());
        world
            .insert_camera(
                "camera",
                Camera::new(Projection::Perspective {
                    fov_y_degrees: 60.0,
                }),
            )
            .unwrap();
        world.spawn(EntityId::new("cube"), "Cube", Transform::identity());
        world
            .insert_mesh(
                "cube",
                MeshRef::new("primitive:cube", "primitive:default_material"),
            )
            .unwrap();
        let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

        assert_eq!(
            viewport_vertex_bytes(&draw.vertices).len(),
            draw.vertex_count * 28
        );
    }
}

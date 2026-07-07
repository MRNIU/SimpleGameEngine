// Copyright The SimpleGameEngine Contributors
//
//! 渲染数据抽取与 wgpu viewport 边界。

use std::{borrow::Cow, collections::BTreeMap, ops::Range};

use ecs::{Camera, EntityId, Light, Projection, World};
use math::{Quat, Transform, Vec3};
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
const CUBE_COLOR: [f32; 4] = [0.3, 0.64, 1.0, 1.0];
const SELECTED_CUBE_COLOR: [f32; 4] = [1.0, 0.78, 0.25, 1.0];
const VIEWPORT_WORLD_SCALE: f32 = 0.12;
const VIEWPORT_DEPTH_SKEW: [f32; 2] = [0.35, 0.2];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportMeshSpan {
    pub entity: EntityId,
    pub vertex_range: Range<usize>,
    pub index_range: Range<usize>,
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
                buffers: &[vertex_layout],
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

    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
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
    let camera_rotation = Quat::from_array(normalized_quaternion(view.transform.rotation));
    let view_rotation = camera_rotation.inverse();
    let camera_translation = Vec3::from_array(view.transform.translation);
    let projection_scale = projection_scale(&view.projection);
    let light_multiplier = light_multiplier(scene);
    let mut has_imported_mesh = false;

    for mesh in &scene.meshes {
        if mesh.mesh_asset != "primitive:cube" {
            continue;
        }
        let vertex_start = vertices.len();
        let index_start = indices.len();
        let mesh_translation = Vec3::from_array(mesh.transform.translation);
        let center = view_rotation
            * (mesh_translation - camera_translation)
            * VIEWPORT_WORLD_SCALE
            * projection_scale;
        let corners =
            transformed_cube_corners(&mesh.transform, view_rotation, size * projection_scale);
        let color = lit_material_color(mesh.base_color, light_multiplier);
        let color = if selected_entity.is_some_and(|selected| selected == &mesh.entity) {
            selected_tint(color)
        } else {
            color
        };

        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [0, 1, 2, 3],
            shade_color(color, 0.38),
        )?;
        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [0, 4, 7, 3],
            shade_color(color, 0.46),
        )?;
        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [0, 1, 5, 4],
            shade_color(color, 0.54),
        )?;
        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [4, 5, 6, 7],
            shade_color(color, 1.0),
        )?;
        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [1, 5, 6, 2],
            shade_color(color, 0.82),
        )?;
        push_cube_face(
            &mut vertices,
            &mut indices,
            center,
            &corners,
            [3, 2, 6, 7],
            shade_color(color, 0.68),
        )?;

        mesh_spans.push(ViewportMeshSpan {
            entity: mesh.entity.clone(),
            vertex_range: vertex_start..vertices.len(),
            index_range: index_start..indices.len(),
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
            view_rotation,
            camera_translation,
            projection_scale,
            light_multiplier,
        )?;
    }

    if vertices.is_empty() {
        return None;
    }

    Some(ViewportDrawCall {
        label: if has_imported_mesh {
            "viewport:mesh".to_owned()
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

fn push_imported_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh_spans: &mut Vec<ViewportMeshSpan>,
    mesh: &MeshDraw,
    imported_mesh: &asset::ImportedMesh,
    selected_entity: Option<&EntityId>,
    view_rotation: Quat,
    camera_translation: Vec3,
    projection_scale: f32,
    light_multiplier: [f32; 3],
) -> Option<()> {
    let vertex_start = vertices.len();
    let index_start = indices.len();
    let transform_rotation = Quat::from_array(normalized_quaternion(mesh.transform.rotation));
    let transform_translation = Vec3::from_array(mesh.transform.translation);
    let transform_scale = Vec3::from_array(mesh.transform.scale);
    let color = lit_material_color(mesh.base_color, light_multiplier);
    let color = if selected_entity.is_some_and(|selected| selected == &mesh.entity) {
        selected_tint(color)
    } else {
        color
    };

    for vertex in &imported_mesh.vertices {
        let local = Vec3::from_array(vertex.position) * transform_scale;
        let world = transform_rotation * local + transform_translation;
        let view_position =
            view_rotation * (world - camera_translation) * VIEWPORT_WORLD_SCALE * projection_scale;
        let projected = project_point(view_position);
        vertices.push(ViewportVertex {
            position: [projected[0], projected[1], 0.0],
            color,
        });
    }

    for index in &imported_mesh.indices {
        let index = vertex_start.checked_add(usize::from(*index))?;
        indices.push(u16::try_from(index).ok()?);
    }

    mesh_spans.push(ViewportMeshSpan {
        entity: mesh.entity.clone(),
        vertex_range: vertex_start..vertices.len(),
        index_range: index_start..indices.len(),
    });
    Some(())
}

fn push_cube_face(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    center: Vec3,
    corners: &[Vec3; 8],
    face: [usize; 4],
    color: [f32; 4],
) -> Option<()> {
    let base = u16::try_from(vertices.len()).ok()?;
    let i1 = base.checked_add(1)?;
    let i2 = base.checked_add(2)?;
    let i3 = base.checked_add(3)?;

    vertices.extend(face.map(|corner| {
        let projected = project_point(center + corners[corner]);
        ViewportVertex {
            position: [projected[0], projected[1], 0.0],
            color,
        }
    }));
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

fn projection_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Perspective { fov_y_degrees } => {
            let clamped = fov_y_degrees.clamp(1.0, 179.0).to_radians();
            (60.0_f32.to_radians() * 0.5).tan() / (clamped * 0.5).tan()
        }
        Projection::Orthographic { vertical_size } => (5.0 / vertical_size.max(0.01)).min(100.0),
    }
}

fn transformed_cube_corners(transform: &Transform, view_rotation: Quat, size: f32) -> [Vec3; 8] {
    let rotation = Quat::from_array(normalized_quaternion(transform.rotation));
    let scale = Vec3::from_array(transform.scale) * size;

    [
        transform_corner(view_rotation, rotation, [-scale.x, -scale.y, -scale.z]),
        transform_corner(view_rotation, rotation, [scale.x, -scale.y, -scale.z]),
        transform_corner(view_rotation, rotation, [scale.x, scale.y, -scale.z]),
        transform_corner(view_rotation, rotation, [-scale.x, scale.y, -scale.z]),
        transform_corner(view_rotation, rotation, [-scale.x, -scale.y, scale.z]),
        transform_corner(view_rotation, rotation, [scale.x, -scale.y, scale.z]),
        transform_corner(view_rotation, rotation, [scale.x, scale.y, scale.z]),
        transform_corner(view_rotation, rotation, [-scale.x, scale.y, scale.z]),
    ]
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

fn transform_corner(view_rotation: Quat, mesh_rotation: Quat, point: [f32; 3]) -> Vec3 {
    view_rotation * (mesh_rotation * Vec3::from_array(point))
}

fn project_point(point: Vec3) -> [f32; 2] {
    [
        point.x + point.z * VIEWPORT_DEPTH_SKEW[0],
        point.y + point.z * VIEWPORT_DEPTH_SKEW[1],
    ]
}

#[must_use]
pub fn fit_viewport_draw_to_size(
    draw: &ViewportDrawCall,
    viewport_size: [f32; 2],
) -> ViewportDrawCall {
    let [width, height] = viewport_size;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return draw.clone();
    }

    let mut fitted = draw.clone();
    if width < height {
        let scale = width / height;
        for vertex in &mut fitted.vertices {
            vertex.position[1] *= scale;
        }
    } else if width > height {
        let scale = height / width;
        for vertex in &mut fitted.vertices {
            vertex.position[0] *= scale;
        }
    }
    fitted
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

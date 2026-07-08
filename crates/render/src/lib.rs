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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportProjection {
    view_rotation: Quat,
    camera_translation: Vec3,
    projection_scale: f32,
    projection: ProjectionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionKind {
    Perspective,
    Orthographic,
}

impl ViewportProjection {
    #[must_use]
    pub fn from_view(view: &ViewportView) -> Option<Self> {
        let view_rotation =
            Quat::from_array(normalized_quaternion(view.transform.rotation)).inverse();
        let camera_translation = Vec3::from_array(view.transform.translation);
        let projection_scale = projection_scale(&view.projection);
        if !camera_translation.is_finite() || !projection_scale.is_finite() {
            return None;
        }
        Some(Self {
            view_rotation,
            camera_translation,
            projection_scale,
            projection: match view.projection {
                Projection::Perspective { .. } => ProjectionKind::Perspective,
                Projection::Orthographic { .. } => ProjectionKind::Orthographic,
            },
        })
    }

    #[must_use]
    pub fn project_world_point(&self, world: [f32; 3]) -> Option<[f32; 2]> {
        let world = Vec3::from_array(world);
        if !world.is_finite() {
            return None;
        }
        let view_position = self.view_rotation
            * (world - self.camera_translation)
            * VIEWPORT_WORLD_SCALE
            * self.projection_scale;
        let projected = match self.projection {
            ProjectionKind::Perspective => project_perspective_point(view_position),
            ProjectionKind::Orthographic => [view_position.x, view_position.y],
        };
        projected.into_iter().all(f32::is_finite).then_some(projected)
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewportProjectionContext {
    projection: ViewportProjection,
    light_multiplier: [f32; 3],
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
    let projection = ViewportProjection::from_view(view)?;
    let light_multiplier = light_multiplier(scene);
    let projection_context = ViewportProjectionContext {
        projection,
        light_multiplier,
    };
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
            "primitive:cube" => {
                push_cube_mesh(
                    &mut vertices,
                    &mut indices,
                    mesh,
                    projection_context,
                    color,
                    size,
                )?
            }
            "primitive:sphere" => {
                has_non_cube_primitive = true;
                push_sphere_mesh(
                    &mut vertices,
                    &mut indices,
                    mesh,
                    projection_context,
                    color,
                    size,
                )?
            }
            "primitive:cone" => {
                has_non_cube_primitive = true;
                push_cone_mesh(
                    &mut vertices,
                    &mut indices,
                    mesh,
                    projection_context,
                    color,
                    size,
                )?
            }
            "primitive:cylinder" => {
                has_non_cube_primitive = true;
                push_cylinder_mesh(
                    &mut vertices,
                    &mut indices,
                    mesh,
                    projection_context,
                    color,
                    size,
                )?
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
    projection_context: ViewportProjectionContext,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let world_points = transformed_cube_world_points(&mesh.transform, size);
    let display_points = transformed_cube_world_points(&mesh.transform, size / VIEWPORT_WORLD_SCALE);

    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [0, 1, 2, 3],
        shade_color(color, 0.38),
    )?;
    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [0, 4, 7, 3],
        shade_color(color, 0.46),
    )?;
    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [0, 1, 5, 4],
        shade_color(color, 0.54),
    )?;
    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [4, 5, 6, 7],
        shade_color(color, 1.0),
    )?;
    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [1, 5, 6, 2],
        shade_color(color, 0.82),
    )?;
    push_cube_face(
        vertices,
        indices,
        projection_context.projection,
        &display_points,
        [3, 2, 6, 7],
        shade_color(color, 0.68),
    )?;
    bounds_from_world_points(&world_points)
}

fn push_sphere_mesh(
    vertices: &mut Vec<ViewportVertex>,
    indices: &mut Vec<u16>,
    mesh: &MeshDraw,
    projection_context: ViewportProjectionContext,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let vertex_start = vertices.len();
    let radius = size;
    let world_points = push_primitive_vertices(
        vertices,
        mesh,
        projection_context,
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
    projection_context: ViewportProjectionContext,
    color: [f32; 4],
    size: f32,
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let vertex_start = vertices.len();
    let radius = size;
    let height = size;
    let world_points = push_primitive_vertices(
        vertices,
        mesh,
        projection_context,
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
    projection_context: ViewportProjectionContext,
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
        projection_context,
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
    projection_context: ViewportProjectionContext,
    color: [f32; 4],
    locals: &[Vec3],
) -> Option<Vec<Vec3>> {
    let transform_rotation = Quat::from_array(normalized_quaternion(mesh.transform.rotation));
    let transform_translation = Vec3::from_array(mesh.transform.translation);
    let transform_scale = Vec3::from_array(mesh.transform.scale);
    let mut world_points = Vec::with_capacity(locals.len());

    for local in locals {
        let world = transform_rotation * (*local * transform_scale) + transform_translation;
        let display_world =
            transform_rotation * (*local * transform_scale / VIEWPORT_WORLD_SCALE)
                + transform_translation;
        let projected = projection_context
            .projection
            .project_world_point(display_world.to_array())?;
        vertices.push(ViewportVertex {
            position: [projected[0], projected[1], 0.0],
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
        let projected = projection_context
            .projection
            .project_world_point(world.to_array())?;
        vertices.push(ViewportVertex {
            position: [projected[0], projected[1], 0.0],
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
    projection: ViewportProjection,
    world_points: &[Vec3; 8],
    face: [usize; 4],
    color: [f32; 4],
) -> Option<()> {
    let base = u16::try_from(vertices.len()).ok()?;
    let i1 = base.checked_add(1)?;
    let i2 = base.checked_add(2)?;
    let i3 = base.checked_add(3)?;

    for corner in face {
        let projected = projection.project_world_point(world_points[corner].to_array())?;
        vertices.push(ViewportVertex {
            position: [projected[0], projected[1], 0.0],
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

fn projection_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Perspective { fov_y_degrees } => {
            let clamped = fov_y_degrees.clamp(1.0, 179.0).to_radians();
            (60.0_f32.to_radians() * 0.5).tan() / (clamped * 0.5).tan()
        }
        Projection::Orthographic { vertical_size } => (5.0 / vertical_size.max(0.01)).min(100.0),
    }
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

fn project_perspective_point(point: Vec3) -> [f32; 2] {
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

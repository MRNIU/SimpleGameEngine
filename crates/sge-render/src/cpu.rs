// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetId, RuntimeAssetStore};
use sge_math::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

use crate::{
    RenderSnapshot, RenderTargetError, RenderView, ViewProjectionError, view_projection_matrix,
};

const SURFACE_CLEAR: [f32; 4] = [13.0 / 255.0, 15.0 / 255.0, 18.0 / 255.0, 1.0];
const OFFSCREEN_CLEAR: [f32; 4] = [0.0; 4];
const EDGE_EPSILON: f32 = 1.0e-6;

#[derive(Debug, Default)]
pub struct CpuRenderer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuFrame {
    size: [u32; 2],
    rgba: Vec<u8>,
}

impl CpuFrame {
    #[must_use]
    pub const fn size(&self) -> [u32; 2] {
        self.size
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

impl CpuRenderer {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn render(
        &mut self,
        target_size: [u32; 2],
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
    ) -> Result<CpuFrame, CpuRenderError> {
        self.render_with_clear(target_size, snapshot, view, assets, SURFACE_CLEAR)
    }

    pub(crate) fn render_offscreen(
        &mut self,
        target_size: [u32; 2],
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
    ) -> Result<CpuFrame, CpuRenderError> {
        self.render_with_clear(target_size, snapshot, view, assets, OFFSCREEN_CLEAR)
    }

    fn render_with_clear(
        &mut self,
        target_size: [u32; 2],
        snapshot: &RenderSnapshot,
        view: RenderView,
        assets: &RuntimeAssetStore,
        clear: [f32; 4],
    ) -> Result<CpuFrame, CpuRenderError> {
        let pixel_count = pixel_count(target_size)?;
        let view_projection = Mat4::from_cols_array(&view_projection_matrix(view, target_size)?);
        let light = FrameLight::from_snapshot(snapshot);
        let mut colors = vec![clear; pixel_count];
        let mut depths = vec![1.0_f32; pixel_count];
        for instance in snapshot.meshes() {
            let asset = *instance.mesh().id();
            let mesh = assets
                .mesh(instance.mesh())
                .map_err(|_| CpuRenderError::MissingAsset { asset })?;
            let transform = instance.transform();
            let model = Mat4::from_scale_rotation_translation(
                Vec3::from_array(transform.scale),
                Quat::from_array(transform.rotation).normalize(),
                Vec3::from_array(transform.translation),
            );
            let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
            let vertices = mesh
                .vertices()
                .iter()
                .map(|vertex| ClipVertex {
                    position: view_projection
                        * model
                        * Vec4::from((Vec3::from_array(*vertex.position()), 1.0)),
                    normal: (normal_matrix
                        * Vec3::from_array(vertex.normal().copied().unwrap_or([0.0, 0.0, 1.0])))
                    .normalize_or_zero(),
                })
                .collect::<Vec<_>>();
            for indices in mesh.indices().chunks_exact(3) {
                let triangle = [
                    vertices[indices[0] as usize],
                    vertices[indices[1] as usize],
                    vertices[indices[2] as usize],
                ];
                for clipped in clip_triangle(triangle) {
                    rasterize_triangle(
                        clipped,
                        target_size,
                        instance.material().base_color(),
                        light,
                        &mut colors,
                        &mut depths,
                    );
                }
            }
        }
        let rgba = colors.into_iter().flat_map(linear_rgba_to_srgb8).collect();
        Ok(CpuFrame {
            size: target_size,
            rgba,
        })
    }
}

fn pixel_count(size: [u32; 2]) -> Result<usize, CpuRenderError> {
    if size.contains(&0) {
        return Err(RenderTargetError::ZeroSize.into());
    }
    let pixels = u64::from(size[0]) * u64::from(size[1]);
    let pixels = usize::try_from(pixels).map_err(|_| CpuRenderError::TargetTooLarge {
        width: size[0],
        height: size[1],
    })?;
    if pixels
        .checked_mul(std::mem::size_of::<[f32; 4]>() + std::mem::size_of::<f32>())
        .is_none_or(|bytes| bytes > isize::MAX as usize)
    {
        return Err(CpuRenderError::TargetTooLarge {
            width: size[0],
            height: size[1],
        });
    }
    Ok(pixels)
}

#[derive(Debug, Clone, Copy)]
struct FrameLight {
    direction: Vec3,
    color: [f32; 4],
    intensity: Option<f32>,
}

impl FrameLight {
    fn from_snapshot(snapshot: &RenderSnapshot) -> Self {
        snapshot.lights().first().map_or(
            Self {
                direction: Vec3::ZERO,
                color: [1.0; 4],
                intensity: None,
            },
            |light| Self {
                direction: Quat::from_array(light.transform().rotation).normalize() * Vec3::Z,
                color: light.light().color(),
                intensity: Some(light.light().intensity()),
            },
        )
    }

    fn shade(self, normal: Vec3, material: [f32; 4]) -> [f32; 4] {
        let Some(intensity) = self.intensity else {
            return material;
        };
        let lambert = normal.dot(-self.direction).max(0.0);
        let strength = 0.15 + lambert * intensity;
        [
            material[0] * self.color[0] * strength,
            material[1] * self.color[1] * strength,
            material[2] * self.color[2] * strength,
            material[3],
        ]
    }
}

#[derive(Debug, Clone, Copy)]
struct ClipVertex {
    position: Vec4,
    normal: Vec3,
}

impl ClipVertex {
    fn interpolate(self, other: Self, amount: f32) -> Self {
        Self {
            position: self.position.lerp(other.position, amount),
            normal: self.normal.lerp(other.normal, amount),
        }
    }
}

fn clip_triangle(triangle: [ClipVertex; 3]) -> Vec<[ClipVertex; 3]> {
    let planes: [fn(Vec4) -> f32; 6] = [
        |position| position.x + position.w,
        |position| position.w - position.x,
        |position| position.y + position.w,
        |position| position.w - position.y,
        |position| position.z,
        |position| position.w - position.z,
    ];
    let mut polygon = triangle.to_vec();
    for distance in planes {
        polygon = clip_polygon(&polygon, distance);
        if polygon.len() < 3 {
            return Vec::new();
        }
    }
    (1..polygon.len() - 1)
        .map(|index| [polygon[0], polygon[index], polygon[index + 1]])
        .collect()
}

fn clip_polygon(vertices: &[ClipVertex], distance: fn(Vec4) -> f32) -> Vec<ClipVertex> {
    let mut output = Vec::new();
    let Some(mut previous) = vertices.last().copied() else {
        return output;
    };
    let mut previous_distance = distance(previous.position);
    for current in vertices.iter().copied() {
        let current_distance = distance(current.position);
        let previous_inside = previous_distance >= 0.0;
        let current_inside = current_distance >= 0.0;
        if previous_inside != current_inside {
            let amount = previous_distance / (previous_distance - current_distance);
            output.push(previous.interpolate(current, amount));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_distance = current_distance;
    }
    output
}

#[derive(Debug, Clone, Copy)]
struct ScreenVertex {
    position: Vec2,
    depth: f32,
    inverse_w: f32,
    normal_over_w: Vec3,
}

fn rasterize_triangle(
    triangle: [ClipVertex; 3],
    size: [u32; 2],
    material: [f32; 4],
    light: FrameLight,
    colors: &mut [[f32; 4]],
    depths: &mut [f32],
) {
    let screen = triangle.map(|vertex| {
        let inverse_w = vertex.position.w.recip();
        let ndc = vertex.position.truncate() * inverse_w;
        ScreenVertex {
            position: Vec2::new(
                (ndc.x * 0.5 + 0.5) * size[0] as f32,
                (0.5 - ndc.y * 0.5) * size[1] as f32,
            ),
            depth: ndc.z,
            inverse_w,
            normal_over_w: vertex.normal * inverse_w,
        }
    });
    let area = edge(screen[0].position, screen[1].position, screen[2].position);
    if area >= -EDGE_EPSILON {
        return;
    }
    let minimum = screen
        .iter()
        .fold(Vec2::splat(f32::INFINITY), |value, vertex| {
            value.min(vertex.position)
        });
    let maximum = screen
        .iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), |value, vertex| {
            value.max(vertex.position)
        });
    let min_x = minimum.x.floor().max(0.0) as u32;
    let min_y = minimum.y.floor().max(0.0) as u32;
    let max_x = maximum.x.ceil().min(size[0] as f32) as u32;
    let max_y = maximum.y.ceil().min(size[1] as f32) as u32;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let weights = [
                edge(screen[1].position, screen[2].position, point) / area,
                edge(screen[2].position, screen[0].position, point) / area,
                edge(screen[0].position, screen[1].position, point) / area,
            ];
            if weights.iter().any(|weight| *weight < -EDGE_EPSILON) {
                continue;
            }
            let depth = weights[0] * screen[0].depth
                + weights[1] * screen[1].depth
                + weights[2] * screen[2].depth;
            let index = y as usize * size[0] as usize + x as usize;
            if !(0.0..=1.0).contains(&depth) || depth > depths[index] {
                continue;
            }
            let inverse_w = weights[0] * screen[0].inverse_w
                + weights[1] * screen[1].inverse_w
                + weights[2] * screen[2].inverse_w;
            if inverse_w.abs() <= f32::EPSILON {
                continue;
            }
            let normal = ((weights[0] * screen[0].normal_over_w
                + weights[1] * screen[1].normal_over_w
                + weights[2] * screen[2].normal_over_w)
                / inverse_w)
                .normalize_or_zero();
            colors[index] = alpha_blend(light.shade(normal, material), colors[index]);
            depths[index] = depth;
        }
    }
}

fn edge(start: Vec2, end: Vec2, point: Vec2) -> f32 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

fn alpha_blend(source: [f32; 4], destination: [f32; 4]) -> [f32; 4] {
    let inverse_alpha = 1.0 - source[3];
    [
        source[0] * source[3] + destination[0] * inverse_alpha,
        source[1] * source[3] + destination[1] * inverse_alpha,
        source[2] * source[3] + destination[2] * inverse_alpha,
        source[3] + destination[3] * inverse_alpha,
    ]
}

fn linear_rgba_to_srgb8(color: [f32; 4]) -> [u8; 4] {
    [
        linear_to_srgb8(color[0]),
        linear_to_srgb8(color[1]),
        linear_to_srgb8(color[2]),
        normalized_to_u8(color[3]),
    ]
}

fn linear_to_srgb8(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    normalized_to_u8(srgb)
}

fn normalized_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[derive(Debug, thiserror::Error)]
pub enum CpuRenderError {
    #[error(transparent)]
    Target(#[from] RenderTargetError),
    #[error(transparent)]
    Projection(#[from] ViewProjectionError),
    #[error("CPU render target {width}x{height} exceeds addressable memory")]
    TargetTooLarge { width: u32, height: u32 },
    #[error("CPU mesh asset is missing: {asset}")]
    MissingAsset { asset: AssetId },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(position: [f32; 4]) -> ClipVertex {
        ClipVertex {
            position: Vec4::from_array(position),
            normal: Vec3::Z,
        }
    }

    #[test]
    fn clipping_keeps_partial_triangles_and_rejects_outside_triangles() {
        let partial = [
            vertex([-0.5, -0.5, -0.5, 1.0]),
            vertex([0.5, -0.5, 0.5, 1.0]),
            vertex([0.0, 0.5, 0.5, 1.0]),
        ];
        assert_eq!(clip_triangle(partial).len(), 2);
        let outside = [
            vertex([-0.5, -0.5, -1.0, 1.0]),
            vertex([0.5, -0.5, -1.0, 1.0]),
            vertex([0.0, 0.5, -1.0, 1.0]),
        ];
        assert!(clip_triangle(outside).is_empty());
    }

    #[test]
    fn back_faces_do_not_touch_color_or_depth() {
        let triangle = [
            vertex([-0.5, -0.5, 0.5, 1.0]),
            vertex([0.0, 0.5, 0.5, 1.0]),
            vertex([0.5, -0.5, 0.5, 1.0]),
        ];
        let mut colors = vec![OFFSCREEN_CLEAR; 16];
        let mut depths = vec![1.0; 16];
        rasterize_triangle(
            triangle,
            [4, 4],
            [1.0; 4],
            FrameLight {
                direction: Vec3::ZERO,
                color: [1.0; 4],
                intensity: None,
            },
            &mut colors,
            &mut depths,
        );
        assert!(colors.iter().all(|color| *color == OFFSCREEN_CLEAR));
        assert!(depths.iter().all(|depth| *depth == 1.0));
    }
}

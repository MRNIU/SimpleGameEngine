// Copyright The SimpleGameEngine Contributors

use sge_asset::TextureAsset;
use sge_math::{Vec2, Vec3};

use super::{
    clip::ClipVertex,
    shade::{FrameLight, alpha_blend, sample_texture_repeat_bilinear},
};

const EDGE_EPSILON: f32 = 1.0e-6;
pub(super) const RASTER_TILE_ROWS: usize = 32;

#[derive(Debug, Clone, Copy)]
pub(super) struct RasterTile {
    pub width: u32,
    pub first_row: u32,
    pub last_row: u32,
}

#[derive(Debug, Clone, Copy)]
struct ScreenVertex {
    position: Vec2,
    depth: f32,
    inverse_w: f32,
    normal_over_w: Vec3,
    texcoord_over_w: Vec2,
    barycentric: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RasterTriangle<'asset> {
    screen: [ScreenVertex; 3],
    area: f32,
    bounds: [u32; 4],
    material: [f32; 4],
    texture: Option<&'asset TextureAsset>,
}

pub(super) fn prepare_triangle(
    triangle: [ClipVertex; 3],
    size: [u32; 2],
    material: [f32; 4],
    texture: Option<&TextureAsset>,
    cull_back_faces: bool,
) -> Option<RasterTriangle<'_>> {
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
            texcoord_over_w: vertex.texcoord * inverse_w,
            barycentric: vertex.barycentric,
        }
    });
    let area = edge(screen[0].position, screen[1].position, screen[2].position);
    if area.abs() <= EDGE_EPSILON || (cull_back_faces && area > 0.0) {
        return None;
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
    (min_x < max_x && min_y < max_y).then_some(RasterTriangle {
        screen,
        area,
        bounds: [min_x, min_y, max_x, max_y],
        material,
        texture,
    })
}

pub(super) fn rasterize_triangle_tile(
    triangle: RasterTriangle<'_>,
    tile: RasterTile,
    light: FrameLight,
    lit: bool,
    colors: &mut [[f32; 4]],
    depths: &mut [f32],
) {
    let [min_x, min_y, max_x, max_y] = triangle.bounds;
    let min_y = min_y.max(tile.first_row);
    let max_y = max_y.min(tile.last_row);
    if min_y >= max_y {
        return;
    }
    for y in min_y..max_y {
        for x in min_x..max_x {
            let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let weights = [
                edge(
                    triangle.screen[1].position,
                    triangle.screen[2].position,
                    point,
                ) / triangle.area,
                edge(
                    triangle.screen[2].position,
                    triangle.screen[0].position,
                    point,
                ) / triangle.area,
                edge(
                    triangle.screen[0].position,
                    triangle.screen[1].position,
                    point,
                ) / triangle.area,
            ];
            if weights.iter().any(|weight| *weight < -EDGE_EPSILON) {
                continue;
            }
            let depth = weights[0] * triangle.screen[0].depth
                + weights[1] * triangle.screen[1].depth
                + weights[2] * triangle.screen[2].depth;
            let index = (y - tile.first_row) as usize * tile.width as usize + x as usize;
            if !(0.0..=1.0).contains(&depth) || depth > depths[index] {
                continue;
            }
            let inverse_w = weights[0] * triangle.screen[0].inverse_w
                + weights[1] * triangle.screen[1].inverse_w
                + weights[2] * triangle.screen[2].inverse_w;
            if inverse_w.abs() <= f32::EPSILON {
                continue;
            }
            let normal = ((weights[0] * triangle.screen[0].normal_over_w
                + weights[1] * triangle.screen[1].normal_over_w
                + weights[2] * triangle.screen[2].normal_over_w)
                / inverse_w)
                .normalize_or_zero();
            let texcoord = (weights[0] * triangle.screen[0].texcoord_over_w
                + weights[1] * triangle.screen[1].texcoord_over_w
                + weights[2] * triangle.screen[2].texcoord_over_w)
                / inverse_w;
            let material = triangle.texture.map_or(triangle.material, |texture| {
                let sampled = sample_texture_repeat_bilinear(texture, texcoord);
                std::array::from_fn(|index| sampled[index] * triangle.material[index])
            });
            let source = if lit {
                light.shade(normal, material)
            } else {
                material
            };
            colors[index] = alpha_blend(source, colors[index]);
            depths[index] = depth;
        }
    }
}

pub(super) fn rasterize_triangle_wire_tile(
    triangle: RasterTriangle<'_>,
    tile: RasterTile,
    width_pixels: f32,
    color: [f32; 4],
    colors: &mut [[f32; 4]],
    depths: &[f32],
    depth_test: bool,
) {
    visit_triangle_pixels(triangle, tile, |index, point, depth| {
        if depth_test && depth > depths[index] + 1.0e-5 {
            return;
        }
        let barycentric = interpolate_barycentric(triangle, barycentric_weights(triangle, point));
        let barycentric_x =
            interpolate_barycentric(triangle, barycentric_weights(triangle, point + Vec2::X));
        let barycentric_y =
            interpolate_barycentric(triangle, barycentric_weights(triangle, point + Vec2::Y));
        let gradient_x = barycentric_x - barycentric;
        let gradient_y = barycentric_y - barycentric;
        let edge_distance = (0..3)
            .map(|index| {
                barycentric[index].abs()
                    / gradient_x[index].hypot(gradient_y[index]).max(f32::EPSILON)
            })
            .fold(f32::INFINITY, f32::min);
        if edge_distance <= width_pixels {
            colors[index] = alpha_blend(color, colors[index]);
        }
    });
}

fn visit_triangle_pixels(
    triangle: RasterTriangle<'_>,
    tile: RasterTile,
    mut visit: impl FnMut(usize, Vec2, f32),
) {
    let [min_x, min_y, max_x, max_y] = triangle.bounds;
    let min_y = min_y.max(tile.first_row);
    let max_y = max_y.min(tile.last_row);
    if min_y >= max_y {
        return;
    }
    for y in min_y..max_y {
        for x in min_x..max_x {
            let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let weights = barycentric_weights(triangle, point);
            if weights.iter().any(|weight| *weight < -EDGE_EPSILON) {
                continue;
            }
            let depth = interpolate_depth(triangle, weights);
            if !(0.0..=1.0).contains(&depth) {
                continue;
            }
            let index = (y - tile.first_row) as usize * tile.width as usize + x as usize;
            visit(index, point, depth);
        }
    }
}

fn barycentric_weights(triangle: RasterTriangle<'_>, point: Vec2) -> [f32; 3] {
    [
        edge(
            triangle.screen[1].position,
            triangle.screen[2].position,
            point,
        ) / triangle.area,
        edge(
            triangle.screen[2].position,
            triangle.screen[0].position,
            point,
        ) / triangle.area,
        edge(
            triangle.screen[0].position,
            triangle.screen[1].position,
            point,
        ) / triangle.area,
    ]
}

fn interpolate_depth(triangle: RasterTriangle<'_>, weights: [f32; 3]) -> f32 {
    weights[0] * triangle.screen[0].depth
        + weights[1] * triangle.screen[1].depth
        + weights[2] * triangle.screen[2].depth
}

fn interpolate_barycentric(triangle: RasterTriangle<'_>, weights: [f32; 3]) -> Vec3 {
    weights[0] * triangle.screen[0].barycentric
        + weights[1] * triangle.screen[1].barycentric
        + weights[2] * triangle.screen[2].barycentric
}

fn edge(start: Vec2, end: Vec2, point: Vec2) -> f32 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

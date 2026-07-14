// Copyright The SimpleGameEngine Contributors

use sge_asset::TextureAsset;
use sge_math::{Vec2, Vec3, Vec4};

use super::{
    clip::{ClipVertex, clip_triangle},
    raster::prepare_triangle,
    shade::sample_texture_repeat_bilinear,
};

fn vertex(position: [f32; 4]) -> ClipVertex {
    ClipVertex {
        position: Vec4::from_array(position),
        normal: Vec3::Z,
        texcoord: Vec2::ZERO,
        barycentric: Vec3::ZERO,
    }
}

#[test]
fn texture_sampling_repeats_and_filters_in_linear_space() -> Result<(), Box<dyn std::error::Error>>
{
    let texture = TextureAsset::new(
        2,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )?;
    let red = sample_texture_repeat_bilinear(&texture, Vec2::new(0.25, 0.25));
    let repeated = sample_texture_repeat_bilinear(&texture, Vec2::new(1.25, -0.75));
    let center = sample_texture_repeat_bilinear(&texture, Vec2::new(0.5, 0.5));

    assert_eq!(red, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(repeated, red);
    for channel in &center[..3] {
        assert!((*channel - 0.5).abs() <= 1.0e-6);
    }
    assert_eq!(center[3], 1.0);
    Ok(())
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
    assert!(prepare_triangle(triangle, [4, 4], [1.0; 4], None, true).is_none());
    assert!(prepare_triangle(triangle, [4, 4], [1.0; 4], None, false).is_some());
}

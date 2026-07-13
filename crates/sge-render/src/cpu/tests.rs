// Copyright The SimpleGameEngine Contributors

use sge_math::{Vec3, Vec4};

use super::{
    clip::{ClipVertex, clip_triangle},
    raster::prepare_triangle,
};

fn vertex(position: [f32; 4]) -> ClipVertex {
    ClipVertex {
        position: Vec4::from_array(position),
        normal: Vec3::Z,
        barycentric: Vec3::ZERO,
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
    assert!(prepare_triangle(triangle, [4, 4], [1.0; 4], true).is_none());
    assert!(prepare_triangle(triangle, [4, 4], [1.0; 4], false).is_some());
}

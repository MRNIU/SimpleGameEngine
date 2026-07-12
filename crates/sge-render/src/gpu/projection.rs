// Copyright The SimpleGameEngine Contributors

use sge_math::{Mat4, Quat, Vec3};

use crate::{Projection, RenderTargetError, RenderView, RenderViewError, ViewProjectionError};

pub fn view_projection_matrix(
    view: RenderView,
    target_size: [u32; 2],
) -> Result<[f32; 16], ViewProjectionError> {
    if target_size.contains(&0) {
        return Err(RenderTargetError::ZeroSize.into());
    }
    let transform = view.transform();
    let position = Vec3::from_array(transform.translation);
    let rotation = Quat::from_array(transform.rotation).normalize();
    let view_matrix = Mat4::from_quat(rotation.inverse()) * Mat4::from_translation(-position);
    let camera = view.camera();
    if camera.near() <= 0.0
        || camera.far() <= camera.near()
        || match camera.projection() {
            Projection::Perspective => {
                camera.vertical_fov_radians() <= 0.0
                    || camera.vertical_fov_radians() >= std::f32::consts::PI
            }
            Projection::Orthographic => camera.orthographic_height() <= 0.0,
        }
    {
        return Err(RenderViewError::InvalidProjection {
            entity: view.entity(),
        }
        .into());
    }
    let aspect = target_size[0] as f32 / target_size[1] as f32;
    let projection = match camera.projection() {
        Projection::Perspective => perspective(
            camera.vertical_fov_radians(),
            aspect,
            camera.near(),
            camera.far(),
        ),
        Projection::Orthographic => orthographic(
            camera.orthographic_height(),
            aspect,
            camera.near(),
            camera.far(),
        ),
    };
    let matrix = projection * view_matrix;
    if matrix.is_finite() {
        Ok(matrix.to_cols_array())
    } else {
        Err(RenderViewError::InvalidProjection {
            entity: view.entity(),
        }
        .into())
    }
}

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let focal_y = 1.0 / (fov_y * 0.5).tan();
    let focal_x = focal_y / aspect;
    let depth = far / (far - near);
    Mat4::from_cols_array(&[
        focal_x,
        0.0,
        0.0,
        0.0,
        0.0,
        focal_y,
        0.0,
        0.0,
        0.0,
        0.0,
        depth,
        1.0,
        0.0,
        0.0,
        -near * depth,
        0.0,
    ])
}

fn orthographic(height: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let half_height = height * 0.5;
    let half_width = half_height * aspect;
    let depth = 1.0 / (far - near);
    Mat4::from_cols_array(&[
        1.0 / half_width,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / half_height,
        0.0,
        0.0,
        0.0,
        0.0,
        depth,
        0.0,
        0.0,
        0.0,
        -near * depth,
        1.0,
    ])
}

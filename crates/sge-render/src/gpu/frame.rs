// Copyright The SimpleGameEngine Contributors

use sge_math::{Mat4, Quat, Transform, Vec3};

use crate::{RenderSnapshot, RenderView};

use super::errors::{RenderTargetError, ViewProjectionError};
use crate::view_projection_matrix;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(super) fn uniform_bytes(
    snapshot: &RenderSnapshot,
    view: RenderView,
    target_size: [u32; 2],
) -> Result<Vec<u8>, ViewProjectionError> {
    let matrix = view_projection_matrix(view, target_size)?;
    let (direction_intensity, color) =
        snapshot
            .lights()
            .first()
            .map_or(([0.0, 0.0, 0.0, -1.0], [1.0; 4]), |light| {
                let direction = Quat::from_array(light.transform().rotation).normalize() * Vec3::Z;
                let intensity = light.light().intensity();
                (
                    [direction.x, direction.y, direction.z, intensity],
                    light.light().color(),
                )
            });
    Ok(matrix
        .into_iter()
        .chain(direction_intensity)
        .chain(color)
        .flat_map(f32::to_ne_bytes)
        .collect())
}

pub(super) fn create_depth_target(device: &wgpu::Device, size: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_render_depth"),
        size: extent(size),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

pub(super) fn normalized_model_matrix(transform: Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation).normalize(),
        Vec3::from_array(transform.translation),
    )
}

pub(super) fn validate_target_size(
    device: &wgpu::Device,
    size: [u32; 2],
) -> Result<(), RenderTargetError> {
    if size.contains(&0) {
        return Err(RenderTargetError::ZeroSize);
    }
    let max = device.limits().max_texture_dimension_2d;
    if size[0] > max || size[1] > max {
        return Err(RenderTargetError::TooLarge {
            width: size[0],
            height: size[1],
            max,
        });
    }
    Ok(())
}

pub(super) const fn extent(size: [u32; 2]) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    }
}

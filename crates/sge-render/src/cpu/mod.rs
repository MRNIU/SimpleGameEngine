// Copyright The SimpleGameEngine Contributors

mod clip;
mod raster;
mod shade;

#[cfg(test)]
mod tests;

use rayon::prelude::*;
use sge_asset::{AssetId, RuntimeAssetStore};
use sge_math::{Mat3, Mat4, Quat, Vec3, Vec4};

use self::{
    clip::{ClipVertex, clip_triangle},
    raster::{RASTER_TILE_ROWS, prepare_triangle, rasterize_triangle_tile},
    shade::{FrameLight, linear_rgba_to_srgb8},
};
use crate::{
    RenderSnapshot, RenderTargetError, RenderView, ViewProjectionError, view_projection_matrix,
};

const SURFACE_CLEAR: [f32; 4] = [13.0 / 255.0, 15.0 / 255.0, 18.0 / 255.0, 1.0];
const OFFSCREEN_CLEAR: [f32; 4] = [0.0; 4];

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
        let mut triangles = Vec::new();
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
                    if let Some(triangle) =
                        prepare_triangle(clipped, target_size, instance.material().base_color())
                    {
                        triangles.push(triangle);
                    }
                }
            }
        }
        let mut colors = vec![clear; pixel_count];
        let mut depths = vec![1.0_f32; pixel_count];
        let width = target_size[0] as usize;
        let tile_pixels = width * RASTER_TILE_ROWS;
        colors
            .par_chunks_mut(tile_pixels)
            .zip(depths.par_chunks_mut(tile_pixels))
            .enumerate()
            .for_each(|(tile_index, (tile_colors, tile_depths))| {
                let first_row = (tile_index * RASTER_TILE_ROWS) as u32;
                let last_row = (first_row + RASTER_TILE_ROWS as u32).min(target_size[1]);
                for triangle in &triangles {
                    rasterize_triangle_tile(
                        *triangle,
                        target_size[0],
                        first_row,
                        last_row,
                        light,
                        tile_colors,
                        tile_depths,
                    );
                }
            });
        let mut rgba = vec![0_u8; pixel_count * 4];
        rgba.par_chunks_exact_mut(4)
            .zip(colors.par_iter())
            .for_each(|(target, color)| target.copy_from_slice(&linear_rgba_to_srgb8(*color)));
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

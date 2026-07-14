// Copyright The SimpleGameEngine Contributors

use sge_asset::TextureAsset;
use sge_math::{Quat, Vec2, Vec3};

use crate::RenderSnapshot;

#[derive(Debug, Clone, Copy)]
pub(super) struct FrameLight {
    direction: Vec3,
    color: [f32; 4],
    intensity: Option<f32>,
}

impl FrameLight {
    pub(super) fn from_snapshot(snapshot: &RenderSnapshot) -> Self {
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

    pub(super) fn shade(self, normal: Vec3, material: [f32; 4]) -> [f32; 4] {
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

pub(super) fn alpha_blend(source: [f32; 4], destination: [f32; 4]) -> [f32; 4] {
    let inverse_alpha = 1.0 - source[3];
    [
        source[0] * source[3] + destination[0] * inverse_alpha,
        source[1] * source[3] + destination[1] * inverse_alpha,
        source[2] * source[3] + destination[2] * inverse_alpha,
        source[3] + destination[3] * inverse_alpha,
    ]
}

pub(super) fn linear_rgba_to_srgb8(color: [f32; 4]) -> [u8; 4] {
    [
        linear_to_srgb8(color[0]),
        linear_to_srgb8(color[1]),
        linear_to_srgb8(color[2]),
        normalized_to_u8(color[3]),
    ]
}

pub(super) fn sample_texture_repeat_bilinear(texture: &TextureAsset, uv: Vec2) -> [f32; 4] {
    let [width, height] = texture.size();
    let x = uv.x.rem_euclid(1.0) * width as f32 - 0.5;
    let y = uv.y.rem_euclid(1.0) * height as f32 - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x.floor();
    let ty = y - y.floor();
    let sample = |x: i64, y: i64| {
        let x = x.rem_euclid(i64::from(width)) as u32;
        let y = y.rem_euclid(i64::from(height)) as u32;
        let index = (y as usize * width as usize + x as usize) * 4;
        let rgba = &texture.rgba8_srgb()[index..index + 4];
        [
            srgb8_to_linear(rgba[0]),
            srgb8_to_linear(rgba[1]),
            srgb8_to_linear(rgba[2]),
            f32::from(rgba[3]) / 255.0,
        ]
    };
    let top = lerp(sample(x0, y0), sample(x0 + 1, y0), tx);
    let bottom = lerp(sample(x0, y0 + 1), sample(x0 + 1, y0 + 1), tx);
    lerp(top, bottom, ty)
}

fn lerp(left: [f32; 4], right: [f32; 4], amount: f32) -> [f32; 4] {
    std::array::from_fn(|index| left[index] + (right[index] - left[index]) * amount)
}

fn srgb8_to_linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
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

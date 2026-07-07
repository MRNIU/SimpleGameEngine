// Copyright The SimpleGameEngine Contributors

use ecs::{Camera, Light, MaterialOverride, Projection};
use math::Transform;

use super::EditorError;

pub(super) fn canonical_transform(mut transform: Transform) -> Result<Transform, EditorError> {
    if !transform
        .translation
        .into_iter()
        .chain(transform.rotation)
        .chain(transform.scale)
        .all(f32::is_finite)
        || transform.scale.contains(&0.0)
    {
        return Err(EditorError::InvalidTransformValue);
    }

    let rotation_len = transform
        .rotation
        .into_iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if rotation_len == 0.0 {
        return Err(EditorError::InvalidTransformValue);
    }
    for value in &mut transform.rotation {
        *value /= rotation_len;
    }
    Ok(transform)
}

pub(super) fn validated_transform(transform: Transform) -> Result<Transform, EditorError> {
    canonical_transform(transform)
}

pub(super) fn validated_material_override(
    material: Option<MaterialOverride>,
) -> Result<Option<MaterialOverride>, EditorError> {
    material
        .map(|mut material| {
            if !material.base_color.into_iter().all(f32::is_finite) {
                return Err(EditorError::InvalidSceneContentValue);
            }
            for channel in &mut material.base_color {
                *channel = channel.clamp(0.0, 1.0);
            }
            Ok(material)
        })
        .transpose()
}

pub(super) fn validated_light(mut light: Light) -> Result<Light, EditorError> {
    if !light.color.into_iter().all(f32::is_finite)
        || !light.intensity.is_finite()
        || light.intensity < 0.0
    {
        return Err(EditorError::InvalidSceneContentValue);
    }
    for channel in &mut light.color {
        *channel = channel.clamp(0.0, 1.0);
    }
    Ok(light)
}

pub(super) fn validated_camera(camera: Camera) -> Result<Camera, EditorError> {
    match &camera.projection {
        Projection::Perspective { fov_y_degrees }
            if fov_y_degrees.is_finite() && *fov_y_degrees > 0.0 => {}
        Projection::Orthographic { vertical_size }
            if vertical_size.is_finite() && *vertical_size > 0.0 => {}
        _ => return Err(EditorError::InvalidSceneContentValue),
    }
    Ok(camera)
}

// Copyright The SimpleGameEngine Contributors

use sge_app::{EngineApp, Plugin, RegistrationError};
use sge_math::{Quat, Transform, Vec3};
use sge_reflect::{
    FieldKey, FieldKind, FieldMetadata, FieldRegistration, ReflectError, TypeDescriptor, TypeKey,
    ValidationErrors, ValidationIssue, Value,
};

use crate::{Camera, Light, Material, MeshRenderer, Projection};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut EngineApp) -> Result<(), RegistrationError> {
        app.register_reflected_component::<Transform>(transform_descriptor())?;
        app.register_reflected_component::<Camera>(camera_descriptor())?;
        app.register_reflected_component::<MeshRenderer>(mesh_renderer_descriptor())?;
        app.register_reflected_component::<Material>(material_descriptor())?;
        app.register_reflected_component::<Light>(light_descriptor())?;
        Ok(())
    }
}

fn transform_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Transform>(key("sge.transform"), 1, "Transform", Transform::identity)
        .field(FieldRegistration::new(
            field("translation"),
            FieldMetadata::new("Translation", FieldKind::Vec3),
            |value: &Transform| Value::Vec3(Vec3::from_array(value.translation)),
            |value: &mut Transform, field| {
                value.translation = vec3(field, "translation")?.to_array();
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("rotation"),
            FieldMetadata::new("Rotation", FieldKind::Quat),
            |value: &Transform| Value::Quat(Quat::from_array(value.rotation)),
            |value: &mut Transform, field| {
                value.rotation = quat(field, "rotation")?.to_array();
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("scale"),
            FieldMetadata::new("Scale", FieldKind::Vec3),
            |value: &Transform| Value::Vec3(Vec3::from_array(value.scale)),
            |value: &mut Transform, field| {
                value.scale = vec3(field, "scale")?.to_array();
                Ok(())
            },
        ))
        .validator(validate_transform)
        .scene_saveable()
        .build()
        .expect("static Transform descriptor must be valid")
}

fn camera_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Camera>(key("sge.camera"), 1, "Camera", Camera::default)
        .field(FieldRegistration::new(
            field("active"),
            FieldMetadata::new("Active", FieldKind::Bool),
            |value: &Camera| Value::Bool(value.active()),
            |value: &mut Camera, field| {
                let Value::Bool(active) = field else {
                    return Err(kind("active", "Bool", field));
                };
                value.active = *active;
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("projection"),
            FieldMetadata::new(
                "Projection",
                FieldKind::Enum {
                    options: vec!["Perspective".to_owned(), "Orthographic".to_owned()],
                },
            ),
            |value: &Camera| Value::Enum(projection_name(value.projection()).to_owned()),
            |value: &mut Camera, field| {
                let Value::Enum(projection) = field else {
                    return Err(kind("projection", "Enum", field));
                };
                value.projection = match projection.as_str() {
                    "Perspective" => Projection::Perspective,
                    "Orthographic" => Projection::Orthographic,
                    _ => {
                        return Err(ReflectError::value_kind(
                            "projection",
                            "Perspective or Orthographic",
                            field.kind(),
                        ));
                    }
                };
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("vertical_fov_radians"),
            FieldMetadata::new("Vertical FOV", FieldKind::F32),
            |value: &Camera| Value::F32(value.vertical_fov_radians()),
            |value: &mut Camera, field| {
                value.vertical_fov_radians = number(field, "vertical_fov_radians")?;
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("orthographic_height"),
            FieldMetadata::new("Orthographic Height", FieldKind::F32),
            |value: &Camera| Value::F32(value.orthographic_height()),
            |value: &mut Camera, field| {
                value.orthographic_height = number(field, "orthographic_height")?;
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("near"),
            FieldMetadata::new("Near", FieldKind::F32),
            |value: &Camera| Value::F32(value.near()),
            |value: &mut Camera, field| {
                value.near = number(field, "near")?;
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("far"),
            FieldMetadata::new("Far", FieldKind::F32),
            |value: &Camera| Value::F32(value.far()),
            |value: &mut Camera, field| {
                value.far = number(field, "far")?;
                Ok(())
            },
        ))
        .validator(validate_camera)
        .scene_saveable()
        .build()
        .expect("static Camera descriptor must be valid")
}

fn mesh_renderer_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<MeshRenderer>(
        key("sge.mesh_renderer"),
        1,
        "Mesh Renderer",
        MeshRenderer::default,
    )
    .field(
        FieldRegistration::reference(
            field("mesh"),
            "Mesh",
            |value: &MeshRenderer| &value.mesh,
            |value: &mut MeshRenderer, mesh| value.mesh = mesh,
        )
        .expect("MeshAsset reference semantic must be valid"),
    )
    .scene_saveable()
    .build()
    .expect("static MeshRenderer descriptor must be valid")
}

fn material_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Material>(key("sge.material"), 2, "Material", Material::default)
        .field(FieldRegistration::new(
            field("base_color"),
            FieldMetadata::new("Base Color", FieldKind::Color),
            |value: &Material| Value::Color(value.base_color()),
            |value: &mut Material, field| {
                value.base_color = color(field, "base_color")?;
                Ok(())
            },
        ))
        .field(
            FieldRegistration::reference(
                field("texture"),
                "Texture",
                |value: &Material| &value.texture,
                |value: &mut Material, texture| value.texture = texture,
            )
            .expect("optional TextureAsset reference semantic must be valid"),
        )
        .validator(validate_material)
        .scene_saveable()
        .build()
        .expect("static Material descriptor must be valid")
}

fn light_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Light>(key("sge.light"), 1, "Light", Light::default)
        .field(FieldRegistration::new(
            field("color"),
            FieldMetadata::new("Color", FieldKind::Color),
            |value: &Light| Value::Color(value.color()),
            |value: &mut Light, field| {
                value.color = color(field, "color")?;
                Ok(())
            },
        ))
        .field(FieldRegistration::new(
            field("intensity"),
            FieldMetadata::new("Intensity", FieldKind::F32),
            |value: &Light| Value::F32(value.intensity()),
            |value: &mut Light, field| {
                value.intensity = number(field, "intensity")?;
                Ok(())
            },
        ))
        .validator(validate_light)
        .scene_saveable()
        .build()
        .expect("static Light descriptor must be valid")
}

pub(crate) fn validate_transform(value: &Transform) -> Result<(), ValidationErrors> {
    if !value
        .translation
        .into_iter()
        .chain(value.rotation)
        .chain(value.scale)
        .all(f32::is_finite)
    {
        return invalid("Transform components must be finite");
    }
    if Quat::from_array(value.rotation).length_squared() == 0.0 {
        return invalid("Transform rotation quaternion must be non-zero");
    }
    if value.scale.contains(&0.0) {
        return invalid("Transform scale components must be non-zero");
    }
    Ok(())
}

pub(crate) fn validate_camera(value: &Camera) -> Result<(), ValidationErrors> {
    let fields = [
        value.vertical_fov_radians(),
        value.orthographic_height(),
        value.near(),
        value.far(),
    ];
    if !fields.into_iter().all(f32::is_finite) {
        return invalid("Camera numeric fields must be finite");
    }
    if value.vertical_fov_radians() <= 0.0
        || value.vertical_fov_radians() >= std::f32::consts::PI
        || value.orthographic_height() <= 0.0
    {
        return invalid("Camera FOV must be between zero and pi and orthographic height positive");
    }
    if value.near() <= 0.0 || value.far() <= value.near() {
        return invalid("Camera clip planes must satisfy 0 < near < far");
    }
    Ok(())
}

pub(crate) fn validate_material(value: &Material) -> Result<(), ValidationErrors> {
    validate_color("base_color", value.base_color())
}

pub(crate) fn validate_light(value: &Light) -> Result<(), ValidationErrors> {
    validate_color("color", value.color())?;
    if !value.intensity().is_finite() || value.intensity() < 0.0 {
        return invalid("Light intensity must be finite and non-negative");
    }
    Ok(())
}

fn validate_color(name: &str, color: [f32; 4]) -> Result<(), ValidationErrors> {
    if color
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
    {
        Ok(())
    } else {
        invalid(format!("{name} channels must be finite and within [0, 1]"))
    }
}

fn invalid(message: impl Into<String>) -> Result<(), ValidationErrors> {
    Err(ValidationErrors::one(ValidationIssue::component(message)))
}

fn vec3(value: &Value, name: &str) -> Result<Vec3, ReflectError> {
    match value {
        Value::Vec3(value) => Ok(*value),
        other => Err(kind(name, "Vec3", other)),
    }
}

fn quat(value: &Value, name: &str) -> Result<Quat, ReflectError> {
    match value {
        Value::Quat(value) => Ok(*value),
        other => Err(kind(name, "Quat", other)),
    }
}

fn number(value: &Value, name: &str) -> Result<f32, ReflectError> {
    match value {
        Value::F32(value) => Ok(*value),
        other => Err(kind(name, "F32", other)),
    }
}

fn color(value: &Value, name: &str) -> Result<[f32; 4], ReflectError> {
    match value {
        Value::Color(value) => Ok(*value),
        other => Err(kind(name, "Color", other)),
    }
}

fn kind(field: &str, expected: &'static str, value: &Value) -> ReflectError {
    ReflectError::value_kind(field, expected, value.kind())
}

fn projection_name(projection: Projection) -> &'static str {
    match projection {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    }
}

fn key(value: &'static str) -> TypeKey {
    TypeKey::new(value).expect("built-in render type key must be valid")
}

fn field(value: &'static str) -> FieldKey {
    FieldKey::new(value).expect("built-in render field key must be valid")
}

// Copyright The SimpleGameEngine Contributors

use sge_app::{EngineApp, RegistrationError};
use sge_asset::{AssetId, AssetRef, MeshAsset, TextureAsset};
use sge_math::Transform;
use sge_reflect::{FieldKind, ReferenceSemantic, ReflectError};
use sge_render::{Camera, Light, Material, MeshRenderer, Projection, RenderPlugin};

fn ready_app() -> Result<EngineApp, RegistrationError> {
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    Ok(app)
}

#[test]
fn plugin_registers_exact_scene_saveable_schema() -> Result<(), Box<dyn std::error::Error>> {
    let app = ready_app()?;
    let expected = [
        ("sge.transform", 1, vec!["rotation", "scale", "translation"]),
        (
            "sge.camera",
            1,
            vec![
                "active",
                "far",
                "near",
                "orthographic_height",
                "projection",
                "vertical_fov_radians",
            ],
        ),
        ("sge.mesh_renderer", 1, vec!["mesh"]),
        ("sge.material", 2, vec!["base_color", "texture"]),
        ("sge.light", 1, vec!["color", "intensity"]),
    ];

    for (type_key, version, fields) in expected {
        let descriptor = app
            .type_registry()
            .descriptor(type_key)
            .ok_or("missing render descriptor")?;
        assert_eq!(descriptor.schema_version(), version);
        assert!(descriptor.scene_saveable());
        assert_eq!(
            descriptor
                .fields()
                .map(|(field, _)| field.as_str())
                .collect::<Vec<_>>(),
            fields
        );
    }
    assert!(matches!(
        app.type_registry()
            .descriptor("sge.mesh_renderer")
            .and_then(|descriptor| descriptor.field("mesh"))
            .map(|metadata| metadata.kind()),
        Some(FieldKind::Reference(ReferenceSemantic::Asset { asset_type }))
            if asset_type.as_str() == "sge.mesh"
    ));
    assert!(matches!(
        app.type_registry()
            .descriptor("sge.material")
            .and_then(|descriptor| descriptor.field("texture"))
            .map(|metadata| metadata.kind()),
        Some(FieldKind::Reference(ReferenceSemantic::OptionalAsset { asset_type }))
            if asset_type.as_str() == "sge.texture"
    ));
    Ok(())
}

#[test]
fn component_codecs_roundtrip_private_values() -> Result<(), Box<dyn std::error::Error>> {
    let app = ready_app()?;
    let registry = app.type_registry();
    let camera = Camera::new(true, Projection::Perspective, 1.2, 8.0, 0.1, 500.0);
    let mesh = MeshRenderer::new(AssetRef::<MeshAsset>::new(AssetId::new_v4()));
    let texture = AssetRef::<TextureAsset>::new(AssetId::new_v4());
    let material = Material::with_texture([0.2, 0.4, 0.6, 1.0], texture);
    let light = Light::new([1.0, 0.9, 0.8, 1.0], 2.5);

    let decoded = registry.decode(&registry.encode(&camera)?)?;
    assert_eq!(
        *decoded.downcast::<Camera>().map_err(|_| "camera type")?,
        camera
    );
    let decoded = registry.decode(&registry.encode(&mesh)?)?;
    assert_eq!(
        *decoded
            .downcast::<MeshRenderer>()
            .map_err(|_| "mesh type")?,
        mesh
    );
    let decoded = registry.decode(&registry.encode(&material)?)?;
    assert_eq!(
        *decoded
            .downcast::<Material>()
            .map_err(|_| "material type")?,
        material
    );
    assert_eq!(material.texture(), Some(texture));
    assert_eq!(Material::new([1.0; 4]).texture(), None);
    let decoded = registry.decode(&registry.encode(&light)?)?;
    assert_eq!(
        *decoded.downcast::<Light>().map_err(|_| "light type")?,
        light
    );
    Ok(())
}

#[test]
fn validators_reject_invalid_component_values() -> Result<(), Box<dyn std::error::Error>> {
    let app = ready_app()?;
    let registry = app.type_registry();
    let invalid = [
        registry.validate(&Transform {
            translation: [f32::NAN, 0.0, 0.0],
            ..Transform::identity()
        }),
        registry.validate(&Transform {
            rotation: [0.0; 4],
            ..Transform::identity()
        }),
        registry.validate(&Transform {
            scale: [1.0, 0.0, 1.0],
            ..Transform::identity()
        }),
        registry.validate(&Camera::new(
            true,
            Projection::Perspective,
            0.0,
            8.0,
            0.1,
            500.0,
        )),
        registry.validate(&Camera::new(
            true,
            Projection::Perspective,
            std::f32::consts::PI,
            8.0,
            0.1,
            500.0,
        )),
        registry.validate(&Camera::new(
            true,
            Projection::Orthographic,
            1.0,
            -1.0,
            0.1,
            500.0,
        )),
        registry.validate(&Camera::new(
            true,
            Projection::Perspective,
            1.0,
            8.0,
            1.0,
            1.0,
        )),
        registry.validate(&Material::new([1.1, 0.0, 0.0, 1.0])),
        registry.validate(&Material::new([f32::NAN, 0.0, 0.0, 1.0])),
        registry.validate(&Light::new([1.1, 1.0, 1.0, 1.0], 1.0)),
        registry.validate(&Light::new([1.0, 1.0, 1.0, 1.0], -0.1)),
        registry.validate(&Light::new([1.0, 1.0, 1.0, 1.0], f32::INFINITY)),
    ];

    assert!(
        invalid
            .into_iter()
            .all(|result| matches!(result, Err(ReflectError::Validation(_))))
    );
    Ok(())
}

#[test]
fn duplicate_plugin_registration_is_typed() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;

    assert!(matches!(
        app.add_plugin(RenderPlugin),
        Err(RegistrationError::Ecs(_))
    ));
    Ok(())
}

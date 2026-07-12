// Copyright The SimpleGameEngine Contributors

use sge_app::EngineApp;
use sge_asset::{AssetId, AssetRef, MeshAsset, MeshVertex, RuntimeAssetStore};
use sge_math::Transform;
use sge_render::{
    Camera, Light, Material, MeshRenderer, Projection, RenderComponentKind, RenderExtractionError,
    RenderPlugin, RenderView, RenderViewError, extract,
};

fn ready_app() -> Result<EngineApp, Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    Ok(app)
}

fn mesh() -> Result<MeshAsset, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([0.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([1.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?)
}

fn store(asset: AssetId) -> Result<RuntimeAssetStore, Box<dyn std::error::Error>> {
    Ok(RuntimeAssetStore::from_meshes([(asset, mesh()?)])?)
}

#[test]
fn empty_snapshot_is_owned_and_not_an_error() -> Result<(), Box<dyn std::error::Error>> {
    fn assert_owned<T: 'static>(_: &T) {}

    let app = ready_app()?;
    let snapshot = extract(app.world(), &RuntimeAssetStore::from_meshes([])?)?;

    assert_owned(&snapshot);
    assert!(snapshot.cameras().is_empty());
    assert!(snapshot.meshes().is_empty());
    assert!(snapshot.lights().is_empty());
    assert!(matches!(
        RenderView::from_active_camera(&snapshot),
        Err(RenderViewError::MissingActiveCamera)
    ));
    Ok(())
}

#[test]
fn extractor_builds_typed_sorted_snapshot_and_view() -> Result<(), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let store = store(asset)?;
    let mut app = ready_app()?;
    let (mesh_entity, camera_entity, light_entity) = {
        let mut world = app.world_initializer()?;
        let mesh_entity = world.spawn();
        world.insert(mesh_entity, Transform::from_translation([1.0, 0.0, 0.0]))?;
        world.insert(mesh_entity, MeshRenderer::new(AssetRef::new(asset)))?;
        world.insert(mesh_entity, Material::new([0.2, 0.4, 0.6, 1.0]))?;
        let camera_entity = world.spawn();
        world.insert(camera_entity, Transform::from_translation([0.0, -4.0, 2.0]))?;
        world.insert(
            camera_entity,
            Camera::new(true, Projection::Perspective, 1.0, 10.0, 0.1, 100.0),
        )?;
        let light_entity = world.spawn();
        world.insert(light_entity, Transform::identity())?;
        world.insert(light_entity, Light::new([1.0; 4], 2.0))?;
        (mesh_entity, camera_entity, light_entity)
    };

    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;

    assert_eq!(snapshot.meshes()[0].entity(), mesh_entity);
    assert_eq!(snapshot.meshes()[0].mesh().id(), &asset);
    assert_eq!(snapshot.cameras()[0].entity(), camera_entity);
    assert_eq!(snapshot.lights()[0].entity(), light_entity);
    assert_eq!(view.entity(), camera_entity);
    Ok(())
}

#[test]
fn extractor_rejects_missing_companion_components() -> Result<(), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let store = store(asset)?;

    let mut mesh_app = ready_app()?;
    let mesh_entity = {
        let mut world = mesh_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, MeshRenderer::new(AssetRef::new(asset)))?;
        entity
    };
    assert!(matches!(
        extract(mesh_app.world(), &store),
        Err(RenderExtractionError::MissingTransform {
            entity,
            component: RenderComponentKind::MeshRenderer,
        }) if entity == mesh_entity
    ));

    let mut material_app = ready_app()?;
    let material_entity = {
        let mut world = material_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(entity, MeshRenderer::new(AssetRef::new(asset)))?;
        entity
    };
    assert!(matches!(
        extract(material_app.world(), &store),
        Err(RenderExtractionError::MissingMaterial { entity }) if entity == material_entity
    ));

    for component in [RenderComponentKind::Camera, RenderComponentKind::Light] {
        let mut app = ready_app()?;
        let entity = {
            let mut world = app.world_initializer()?;
            let entity = world.spawn();
            if component == RenderComponentKind::Camera {
                world.insert(entity, Camera::default())?;
            } else {
                world.insert(entity, Light::default())?;
            }
            entity
        };
        assert!(matches!(
            extract(app.world(), &store),
            Err(RenderExtractionError::MissingTransform {
                entity: found,
                component: found_component,
            }) if found == entity && found_component == component
        ));
    }
    Ok(())
}

#[test]
fn extractor_rejects_missing_asset_and_invalid_runtime_values()
-> Result<(), Box<dyn std::error::Error>> {
    let assigned = AssetId::new_v4();
    let missing = AssetId::new_v4();
    let store = store(assigned)?;
    let mut missing_app = ready_app()?;
    let entity = {
        let mut world = missing_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(entity, MeshRenderer::new(AssetRef::new(missing)))?;
        world.insert(entity, Material::default())?;
        entity
    };
    assert!(matches!(
        extract(missing_app.world(), &store),
        Err(RenderExtractionError::MissingMeshAsset { entity: found, asset })
            if found == entity && asset == missing
    ));

    let mut invalid_app = ready_app()?;
    let invalid_entity = {
        let mut world = invalid_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(entity, MeshRenderer::new(AssetRef::new(assigned)))?;
        world.insert(entity, Material::new([f32::NAN, 1.0, 1.0, 1.0]))?;
        entity
    };
    assert!(matches!(
        extract(invalid_app.world(), &store),
        Err(RenderExtractionError::InvalidComponent {
            entity,
            component: RenderComponentKind::Material,
            ..
        }) if entity == invalid_entity
    ));
    Ok(())
}

#[test]
fn extractor_reuses_transform_camera_and_light_validators() -> Result<(), Box<dyn std::error::Error>>
{
    let store = RuntimeAssetStore::from_meshes([])?;

    let mut transform_app = ready_app()?;
    let transform_entity = {
        let mut world = transform_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                translation: [f32::NAN, 0.0, 0.0],
                ..Transform::identity()
            },
        )?;
        world.insert(entity, Camera::default())?;
        entity
    };
    assert!(matches!(
        extract(transform_app.world(), &store),
        Err(RenderExtractionError::InvalidComponent {
            entity,
            component: RenderComponentKind::Transform,
            ..
        }) if entity == transform_entity
    ));

    let mut camera_app = ready_app()?;
    let camera_entity = {
        let mut world = camera_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(
            entity,
            Camera::new(true, Projection::Perspective, 1.0, 10.0, 1.0, 1.0),
        )?;
        entity
    };
    assert!(matches!(
        extract(camera_app.world(), &store),
        Err(RenderExtractionError::InvalidComponent {
            entity,
            component: RenderComponentKind::Camera,
            ..
        }) if entity == camera_entity
    ));

    let mut light_app = ready_app()?;
    let light_entity = {
        let mut world = light_app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(entity, Light::new([1.0; 4], f32::NAN))?;
        entity
    };
    assert!(matches!(
        extract(light_app.world(), &store),
        Err(RenderExtractionError::InvalidComponent {
            entity,
            component: RenderComponentKind::Light,
            ..
        }) if entity == light_entity
    ));
    Ok(())
}

#[test]
fn light_and_active_camera_cardinality_are_owned_by_distinct_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let store = RuntimeAssetStore::from_meshes([])?;
    let mut app = ready_app()?;
    let (first_camera, second_camera) = {
        let mut world = app.world_initializer()?;
        let first = world.spawn();
        world.insert(first, Transform::identity())?;
        world.insert(
            first,
            Camera::new(true, Projection::Perspective, 1.0, 10.0, 0.1, 100.0),
        )?;
        let second = world.spawn();
        world.insert(second, Transform::identity())?;
        world.insert(
            second,
            Camera::new(true, Projection::Perspective, 1.0, 10.0, 0.1, 100.0),
        )?;
        (first, second)
    };
    let snapshot = extract(app.world(), &store)?;
    assert_eq!(snapshot.cameras().len(), 2);
    assert_eq!(snapshot.cameras()[0].entity(), first_camera);
    assert_eq!(snapshot.cameras()[1].entity(), second_camera);
    assert!(matches!(
        RenderView::from_active_camera(&snapshot),
        Err(RenderViewError::MultipleActiveCameras { .. })
    ));

    let mut lights = ready_app()?;
    let (first, second) = {
        let mut world = lights.world_initializer()?;
        let first = world.spawn();
        world.insert(first, Transform::identity())?;
        world.insert(first, Light::default())?;
        let second = world.spawn();
        world.insert(second, Transform::identity())?;
        world.insert(second, Light::default())?;
        (first, second)
    };
    assert!(matches!(
        extract(lights.world(), &store),
        Err(RenderExtractionError::MultipleLights { first: found_first, second: found_second })
            if found_first == first && found_second == second
    ));
    Ok(())
}

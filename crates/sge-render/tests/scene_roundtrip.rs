// Copyright The SimpleGameEngine Contributors

use sge_app::EngineApp;
use sge_asset::{AssetId, AssetRef, MeshAsset, MeshVertex, RuntimeAssetStore};
use sge_math::Transform;
use sge_render::{Camera, Light, Material, MeshRenderer, Projection, RenderPlugin};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, SceneEntityId, instantiate, parent_descriptor,
    prepare, scene_entity_id_descriptor,
};

#[test]
fn reflected_render_components_survive_scene_reopen_and_instantiate()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.register_reflected_component::<SceneEntityId>(scene_entity_id_descriptor()?)?;
    app.register_reflected_component::<Parent>(parent_descriptor()?)?;
    app.add_plugin(RenderPlugin)?;
    app.finish()?;

    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, triangle_mesh()?)])?;
    let source_id = SceneEntityId::new_v4();
    let transform = Transform::from_translation([1.0, 2.0, 3.0]);
    let camera = Camera::new(true, Projection::Perspective, 1.0, 12.0, 0.1, 1000.0);
    let mesh = MeshRenderer::new(AssetRef::new(asset));
    let material = Material::new([0.3, 0.5, 0.7, 1.0]);
    let light = Light::new([1.0, 1.0, 1.0, 1.0], 1.5);
    let registry = app.type_registry();
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(
        source_id,
        None,
        vec![
            registry.encode(&transform)?,
            registry.encode(&camera)?,
            registry.encode(&mesh)?,
            registry.encode(&material)?,
            registry.encode(&light)?,
        ],
    )?])?;
    let reopened = AuthoringScene::from_ron(&scene.to_ron()?)?;
    let prepared = prepare(&reopened, registry, &store)?;
    let instance = instantiate(prepared, app.world_initializer()?)?;
    let entity = instance.entity(&source_id).ok_or("missing scene entity")?;

    assert_eq!(app.world().get::<Transform>(entity), Some(&transform));
    assert_eq!(app.world().get::<Camera>(entity), Some(&camera));
    assert_eq!(app.world().get::<MeshRenderer>(entity), Some(&mesh));
    assert_eq!(app.world().get::<Material>(entity), Some(&material));
    assert_eq!(app.world().get::<Light>(entity), Some(&light));
    Ok(())
}

fn triangle_mesh() -> Result<MeshAsset, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([0.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([1.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?)
}

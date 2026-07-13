// Copyright The SimpleGameEngine Contributors

use sge_app::EngineApp;
use sge_asset::{AssetId, AssetRef, MeshAsset, MeshVertex, RuntimeAssetStore};
use sge_math::Transform;
use sge_render::{
    Camera, CpuRenderer, Light, Material, MeshRenderer, Projection, RenderPlugin, RenderView,
    extract,
};

#[test]
fn cpu_renderer_is_send_and_renders_current_lambert_light() -> Result<(), Box<dyn std::error::Error>>
{
    fn assert_send<T: Send>() {}
    assert_send::<CpuRenderer>();

    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, triangle(false)?)])?;
    let mut app = render_app()?;
    {
        let mut world = app.world_initializer()?;
        spawn_camera(&mut world)?;
        spawn_mesh(&mut world, asset, [0.0, 0.0, 2.0], [1.0, 0.05, 0.05, 1.0])?;
        let light = world.spawn();
        world.insert(
            light,
            Transform {
                rotation: [1.0, 0.0, 0.0, 0.0],
                ..Transform::identity()
            },
        )?;
        world.insert(light, Light::new([1.0; 4], 1.0))?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    let frame = CpuRenderer::new().render([64, 64], &snapshot, view, &store)?;
    let corner = &frame.rgba()[..4];
    assert!(frame.rgba().chunks_exact(4).any(|pixel| {
        pixel != corner
            && pixel[0] > 220
            && pixel[0] > pixel[1].saturating_add(100)
            && pixel[0] > pixel[2].saturating_add(100)
    }));
    Ok(())
}

#[test]
fn cpu_parallel_tiles_match_single_worker_output() -> Result<(), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, triangle(false)?)])?;
    let mut app = render_app()?;
    {
        let mut world = app.world_initializer()?;
        spawn_camera(&mut world)?;
        spawn_mesh(&mut world, asset, [0.0, 0.0, 2.0], [0.9, 0.2, 0.1, 1.0])?;
        spawn_mesh(&mut world, asset, [0.2, 0.1, 3.0], [0.1, 0.3, 0.9, 0.8])?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    let single_worker = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let parallel = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
    let single =
        single_worker.install(|| CpuRenderer::new().render([97, 79], &snapshot, view, &store))?;
    let parallel =
        parallel.install(|| CpuRenderer::new().render([97, 79], &snapshot, view, &store))?;

    assert_eq!(parallel, single);
    Ok(())
}

#[test]
fn cpu_depth_test_keeps_near_triangle_when_far_triangle_is_drawn_later()
-> Result<(), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, triangle(false)?)])?;
    let mut app = render_app()?;
    {
        let mut world = app.world_initializer()?;
        spawn_camera(&mut world)?;
        spawn_mesh(&mut world, asset, [0.0, 0.0, 2.0], [0.05, 0.05, 1.0, 1.0])?;
        spawn_mesh(&mut world, asset, [0.0, 0.0, 3.0], [1.0, 0.05, 0.05, 1.0])?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    let frame = CpuRenderer::new().render([64, 64], &snapshot, view, &store)?;
    let center = pixel(&frame, 32, 32);
    assert!(center[2] > center[0].saturating_add(100), "{center:?}");
    Ok(())
}

#[test]
fn cpu_backface_culling_leaves_only_the_clear_color() -> Result<(), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, triangle(true)?)])?;
    let mut app = render_app()?;
    {
        let mut world = app.world_initializer()?;
        spawn_camera(&mut world)?;
        spawn_mesh(&mut world, asset, [0.0, 0.0, 2.0], [1.0, 0.0, 0.0, 1.0])?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    let frame = CpuRenderer::new().render([32, 32], &snapshot, view, &store)?;
    let corner = frame.rgba()[..4].to_vec();
    assert!(frame.rgba().chunks_exact(4).all(|pixel| pixel == corner));
    Ok(())
}

fn render_app() -> Result<EngineApp, Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    Ok(app)
}

fn spawn_camera(
    world: &mut sge_ecs::WorldInitializer<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let camera = world.spawn();
    world.insert(camera, Transform::identity())?;
    world.insert(
        camera,
        Camera::new(
            true,
            Projection::Perspective,
            std::f32::consts::FRAC_PI_2,
            10.0,
            0.1,
            100.0,
        ),
    )?;
    Ok(())
}

fn spawn_mesh(
    world: &mut sge_ecs::WorldInitializer<'_>,
    asset: AssetId,
    translation: [f32; 3],
    color: [f32; 4],
) -> Result<(), Box<dyn std::error::Error>> {
    let mesh = world.spawn();
    world.insert(mesh, Transform::from_translation(translation))?;
    world.insert(mesh, MeshRenderer::new(AssetRef::new(asset)))?;
    world.insert(mesh, Material::new(color))?;
    Ok(())
}

fn triangle(reversed: bool) -> Result<MeshAsset, Box<dyn std::error::Error>> {
    let indices = if reversed {
        vec![0, 2, 1]
    } else {
        vec![0, 1, 2]
    };
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([-0.8, -0.8, 0.0], None, None)?,
            MeshVertex::new([0.8, -0.8, 0.0], None, None)?,
            MeshVertex::new([0.0, 0.8, 0.0], None, None)?,
        ],
        indices,
    )?)
}

fn pixel(frame: &sge_render::CpuFrame, x: u32, y: u32) -> &[u8] {
    let offset = (y as usize * frame.size()[0] as usize + x as usize) * 4;
    &frame.rgba()[offset..offset + 4]
}

// Copyright The SimpleGameEngine Contributors

use sge_app::EngineApp;
use sge_asset::{AssetId, AssetRef, MeshAsset, MeshVertex, RuntimeAssetStore};
use sge_math::Transform;
use sge_render::{
    Camera, FrameNotPreparedError, Light, Material, MeshRenderer, Projection, RenderFrameError,
    RenderPlugin, RenderSnapshot, RenderTargetError, RenderView, ViewProjectionError, WgpuRenderer,
    extract, view_projection_matrix,
};

#[test]
fn projection_uses_wgpu_depth_and_rejects_zero_target() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    let entity = {
        let mut world = app.world_initializer()?;
        let entity = world.spawn();
        world.insert(entity, Transform::identity())?;
        world.insert(
            entity,
            Camera::new(
                true,
                Projection::Perspective,
                std::f32::consts::FRAC_PI_2,
                10.0,
                0.1,
                100.0,
            ),
        )?;
        entity
    };
    let snapshot = extract(app.world(), &RuntimeAssetStore::from_meshes([])?)?;
    let view = RenderView::from_active_camera(&snapshot)?;

    let matrix = view_projection_matrix(view, [800, 600])?;
    assert!(matrix.into_iter().all(f32::is_finite));
    assert_eq!(view.entity(), Some(entity));
    assert!(matches!(
        view_projection_matrix(view, [0, 600]),
        Err(ViewProjectionError::Target(RenderTargetError::ZeroSize))
    ));
    Ok(())
}

#[test]
fn wgpu_renderer_public_contract_is_constructible() {
    fn assert_send<T: Send>() {}
    assert_send::<WgpuRenderer>();
}

#[test]
fn mesh_cache_is_retained_clearable_and_frame_checked() -> Result<(), Box<dyn std::error::Error>> {
    let (device, queue) = gpu()?;
    let (snapshot, view, store, _) = render_fixture()?;
    let mut renderer = WgpuRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert_eq!(renderer.cached_mesh_count(), 0);

    renderer.prepare_assets(&device, &queue, &snapshot, &store)?;
    assert_eq!(renderer.cached_mesh_count(), 2);
    renderer.prepare_assets(&device, &queue, &snapshot, &store)?;
    assert_eq!(renderer.cached_mesh_count(), 2);
    renderer.clear_asset_cache();
    assert_eq!(renderer.cached_mesh_count(), 0);

    let target = target_texture(&device, [64, 64]);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(matches!(
        renderer.render_to_target(
            &device,
            &mut encoder,
            &target.create_view(&Default::default()),
            [64, 64],
            &snapshot,
            view,
        ),
        Err(RenderFrameError::NotPrepared(
            FrameNotPreparedError::Asset { .. }
        ))
    ));
    Ok(())
}

#[test]
fn real_offscreen_adapter_renders_non_clear_pixels() -> Result<(), Box<dyn std::error::Error>> {
    let (device, queue) = gpu()?;
    let (snapshot, view, store, _) = render_fixture()?;
    let mut renderer = WgpuRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    renderer.prepare_assets(&device, &queue, &snapshot, &store)?;
    let size = [64, 64];
    let target = target_texture(&device, size);
    let readback = readback_buffer(&device, size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render_to_target(
        &device,
        &mut encoder,
        &target.create_view(&Default::default()),
        size,
        &snapshot,
        view,
    )?;
    copy_to_readback(&mut encoder, &target, &readback);
    queue.submit([encoder.finish()]);
    assert_red_pixel(&mapped_bytes(&device, &readback)?);
    Ok(())
}

#[test]
fn offscreen_composite_uses_the_same_rendered_frame() -> Result<(), Box<dyn std::error::Error>> {
    let (device, queue) = gpu()?;
    let (snapshot, view, store, _) = render_fixture()?;
    let mut renderer = WgpuRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    renderer.prepare_assets(&device, &queue, &snapshot, &store)?;
    let size = [64, 64];
    let target = target_texture(&device, size);
    let readback = readback_buffer(&device, size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render_offscreen(&device, &mut encoder, size, &snapshot, view)?;
    {
        let target_view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sge_render_composite_test"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        renderer.composite(&mut pass)?;
    }
    copy_to_readback(&mut encoder, &target, &readback);
    queue.submit([encoder.finish()]);
    let bytes = mapped_bytes(&device, &readback)?;
    assert_eq!(&bytes[..4], &[0, 255, 0, 255]);
    assert_red_pixel(&bytes);
    Ok(())
}

#[test]
fn uint32_index_path_renders_an_index_above_u16_range() -> Result<(), Box<dyn std::error::Error>> {
    let (device, queue) = gpu()?;
    let asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([(asset, large_index_mesh()?)])?;
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    {
        let mut world = app.world_initializer()?;
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
        let mesh = world.spawn();
        world.insert(mesh, Transform::from_translation([0.0, 0.0, 2.0]))?;
        world.insert(mesh, MeshRenderer::new(AssetRef::new(asset)))?;
        world.insert(mesh, Material::new([1.0, 0.1, 0.1, 1.0]))?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    let mut renderer = WgpuRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    renderer.prepare_assets(&device, &queue, &snapshot, &store)?;
    let size = [64, 64];
    let target = target_texture(&device, size);
    let readback = readback_buffer(&device, size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render_to_target(
        &device,
        &mut encoder,
        &target.create_view(&Default::default()),
        size,
        &snapshot,
        view,
    )?;
    copy_to_readback(&mut encoder, &target, &readback);
    queue.submit([encoder.finish()]);
    assert_red_pixel(&mapped_bytes(&device, &readback)?);
    Ok(())
}

fn gpu() -> Result<(wgpu::Device, wgpu::Queue), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))?;
    Ok(pollster::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor::default()),
    )?)
}

fn readback_buffer(device: &wgpu::Device, size: [u32; 2]) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sge_render_test_readback"),
        size: u64::from(256 * size[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn copy_to_readback(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
    readback: &wgpu::Buffer,
) {
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
        },
        target.size(),
    );
}

fn mapped_bytes(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    readback.map_async(wgpu::MapMode::Read, .., move |result| {
        sender.send(result).expect("readback receiver must remain");
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver.recv()??;
    Ok(readback.get_mapped_range(..).to_vec())
}

fn assert_red_pixel(bytes: &[u8]) {
    let corner = &bytes[..4];
    assert!(bytes.chunks_exact(4).any(|pixel| {
        pixel != corner
            && pixel[0] > pixel[1].saturating_add(50)
            && pixel[0] > pixel[2].saturating_add(50)
    }));
}

fn render_fixture()
-> Result<(RenderSnapshot, RenderView, RuntimeAssetStore, AssetId), Box<dyn std::error::Error>> {
    let asset = AssetId::new_v4();
    let second_asset = AssetId::new_v4();
    let store = RuntimeAssetStore::from_meshes([
        (asset, triangle_mesh()?),
        (second_asset, triangle_mesh()?),
    ])?;
    let mut app = EngineApp::new();
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    {
        let mut world = app.world_initializer()?;
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
        for (translation, mesh_asset) in [
            ([-0.2, 0.0, 2.0], asset),
            ([0.2, 0.0, 2.0], asset),
            ([0.0, 0.2, 2.0], second_asset),
        ] {
            let mesh = world.spawn();
            world.insert(
                mesh,
                Transform {
                    translation,
                    rotation: [0.0, 0.0, 0.0, 2.0],
                    scale: [1.5, 0.75, 1.0],
                },
            )?;
            world.insert(mesh, MeshRenderer::new(AssetRef::new(mesh_asset)))?;
            world.insert(mesh, Material::new([1.0, 0.1, 0.1, 1.0]))?;
        }
        let light = world.spawn();
        world.insert(
            light,
            Transform {
                rotation: [2.0, 0.0, 0.0, 0.0],
                ..Transform::identity()
            },
        )?;
        world.insert(light, Light::new([1.0; 4], 1.0))?;
    }
    let snapshot = extract(app.world(), &store)?;
    let view = RenderView::from_active_camera(&snapshot)?;
    Ok((snapshot, view, store, asset))
}

fn triangle_mesh() -> Result<MeshAsset, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([-0.8, -0.8, 0.0], None, None)?,
            MeshVertex::new([0.8, -0.8, 0.0], None, None)?,
            MeshVertex::new([0.0, 0.8, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?)
}

fn large_index_mesh() -> Result<MeshAsset, Box<dyn std::error::Error>> {
    let vertices = (0..=u32::from(u16::MAX) + 1)
        .map(|index| {
            let position = match index {
                0 => [-0.8, -0.8, 0.0],
                1 => [0.0, 0.8, 0.0],
                index if index == u32::from(u16::MAX) + 1 => [0.8, -0.8, 0.0],
                _ => [0.0; 3],
            };
            MeshVertex::new(position, None, None)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MeshAsset::new(
        vertices,
        vec![0, u32::from(u16::MAX) + 1, 1],
    )?)
}

fn target_texture(device: &wgpu::Device, size: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sge_render_test_target"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

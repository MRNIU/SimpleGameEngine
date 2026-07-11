use std::collections::{BTreeMap, BTreeSet};

use super::{
    ViewportClipPlanes, ViewportProjection, ViewportSize, ViewportView, extract_render_scene,
    viewport_draw_call, viewport_draw_call_with_selection, viewport_draw_call_with_view,
    viewport_draw_call_with_view_and_meshes, viewport_pipeline_info, viewport_vertex_buffer_layout,
    viewport_vertex_bytes,
};
use asset::{AssetUuid, ImportedMesh, ImportedVertex};
use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, MeshRef, Projection, World};
use math::Transform;

fn world_with_camera() -> World {
    let mut world = World::new();
    world.spawn(EntityId::new("camera"), "Camera", Transform::identity());
    world
        .insert_camera(
            "camera",
            Camera::new(Projection::Perspective {
                fov_y_degrees: 60.0,
            }),
        )
        .unwrap();
    world
}

fn world_with_camera_transform(transform: Transform) -> World {
    let mut world = World::new();
    world.spawn(EntityId::new("camera"), "Camera", transform);
    world
        .insert_camera(
            "camera",
            Camera::new(Projection::Perspective {
                fov_y_degrees: 60.0,
            }),
        )
        .unwrap();
    world
}

fn matrix_projection() -> ViewportProjection {
    let view = ViewportView::new(
        EntityId::new("matrix_camera"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 90.0,
        },
    );
    ViewportProjection::from_view(
        &view,
        ViewportSize::new(1600.0, 900.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap()
}

#[test]
fn viewport_projection_matrix_preserves_world_line_collinearity() {
    let projection = matrix_projection();
    assert!(
        projection
            .view_projection_array()
            .into_iter()
            .all(f32::is_finite)
    );
    let a = projection.project_world_point([-2.0, 0.0, 4.0]).unwrap();
    let b = projection.project_world_point([0.0, 0.0, 8.0]).unwrap();
    let c = projection.project_world_point([2.0, 0.0, 12.0]).unwrap();
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);

    assert!(cross.abs() < 1.0e-5, "projected line bent: {cross}");
}

#[test]
fn viewport_projection_matrix_rejects_points_outside_depth_range() {
    let projection = matrix_projection();

    assert!(projection.project_world_point([0.0, 0.0, -1.0]).is_none());
    assert!(projection.project_world_point([0.0, 0.0, 0.05]).is_none());
    assert!(
        projection
            .project_world_point([0.0, 0.0, 20_000.0])
            .is_none()
    );
}

#[test]
fn viewport_projection_matrix_center_ray_matches_camera_forward() {
    let ray = matrix_projection().screen_ray([0.0, 0.0]).unwrap();

    assert!((ray.direction[0]).abs() < 1.0e-5);
    assert!((ray.direction[1]).abs() < 1.0e-5);
    assert!((ray.direction[2] - 1.0).abs() < 1.0e-5);
}

#[test]
fn viewport_projection_matrix_rejects_invalid_size_and_clip_planes() {
    assert!(ViewportSize::new(0.0, 900.0).is_none());
    assert!(ViewportSize::new(f32::NAN, 900.0).is_none());
    assert!(ViewportClipPlanes::new(0.0, 100.0).is_none());
    assert!(ViewportClipPlanes::new(10.0, 1.0).is_none());
}

#[test]
fn viewport_projection_matrix_clips_segment_to_frustum() {
    let projection = matrix_projection();
    let clipped = projection
        .project_world_segment([-4.0, 0.0, 0.05], [4.0, 0.0, 8.0])
        .unwrap();

    assert!(clipped.into_iter().flatten().all(f32::is_finite));
    assert!(clipped[0][0] >= -1.0 && clipped[0][0] <= 1.0);
    assert!(clipped[1][0] >= -1.0 && clipped[1][0] <= 1.0);
}

fn add_cube(world: &mut World, id: &str, translation: [f32; 3]) {
    world.spawn(
        EntityId::new(id),
        "Cube",
        Transform::from_translation(translation),
    );
    world
        .insert_mesh(
            id,
            MeshRef::new("primitive:cube", "primitive:default_material"),
        )
        .unwrap();
}

fn add_cube_with_transform(world: &mut World, id: &str, transform: Transform) {
    world.spawn(EntityId::new(id), "Cube", transform);
    world
        .insert_mesh(
            id,
            MeshRef::new("primitive:cube", "primitive:default_material"),
        )
        .unwrap();
}

fn add_mesh(world: &mut World, id: &str, name: &str, asset: &str, transform: Transform) {
    world.spawn(EntityId::new(id), name, transform);
    world
        .insert_mesh(id, MeshRef::new(asset, "primitive:default_material"))
        .unwrap();
}

fn rounded_positions(draw: &super::ViewportDrawCall, span_index: usize) -> BTreeSet<String> {
    draw.mesh_spans[span_index]
        .vertex_range
        .clone()
        .map(|index| {
            let position = draw.vertices[index].position;
            format!("{:.3},{:.3},{:.3}", position[0], position[1], position[2])
        })
        .collect()
}

fn span_index_for(draw: &super::ViewportDrawCall, entity: &str) -> usize {
    draw.mesh_spans
        .iter()
        .position(|span| span.entity == EntityId::new(entity))
        .unwrap()
}

#[test]
fn viewport_draw_call_keeps_world_positions() {
    let mut world = world_with_camera();
    add_cube(&mut world, "world_cube", [0.0, 0.0, 4.0]);
    let scene = extract_render_scene(&world);
    let view = ViewportView::new(
        EntityId::new("editor_view"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 90.0,
        },
    );
    let draw = viewport_draw_call_with_view(&scene, None, &view).unwrap();
    let span = &draw.mesh_spans[0];

    assert!(span.vertex_range.clone().all(|index| {
        let position = draw.vertices[index].position;
        position[2] > 3.0 && position[2] < 5.0
    }));
}

#[test]
fn viewport_shader_uses_view_projection_and_composite_passes() {
    let info = viewport_pipeline_info(wgpu::TextureFormat::Rgba8UnormSrgb);

    assert!(info.shader_source.contains("view_projection"));
    assert!(info.shader_source.contains("vs_mesh"));
    assert!(info.shader_source.contains("vs_composite"));
}

#[test]
fn viewport_pipeline_declares_depth_testing() {
    let info = viewport_pipeline_info(wgpu::TextureFormat::Rgba8UnormSrgb);

    assert_eq!(info.depth_format, Some(wgpu::TextureFormat::Depth32Float));
    assert_eq!(info.grid_topology, wgpu::PrimitiveTopology::LineList);
    assert!(info.grid_depth_write);
}

fn add_light(world: &mut World, id: &str, color: [f32; 3], intensity: f32) {
    world.spawn(EntityId::new(id), "Light", Transform::identity());
    world
        .insert_light(
            id,
            Light {
                kind: LightKind::Directional,
                color,
                intensity,
            },
        )
        .unwrap();
}

fn imported_triangle_mesh() -> ImportedMesh {
    ImportedMesh {
        vertices: vec![
            ImportedVertex {
                position: [0.0, 0.0, 0.0],
                normal: None,
                uv: None,
            },
            ImportedVertex {
                position: [1.0, 0.0, 0.0],
                normal: None,
                uv: None,
            },
            ImportedVertex {
                position: [0.0, 1.0, 0.0],
                normal: None,
                uv: None,
            },
        ],
        indices: vec![0, 1, 2],
    }
}

#[test]
fn extracts_mesh_draws_from_ecs() {
    let mut world = World::new();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);

    let render_scene = extract_render_scene(&world);

    assert_eq!(render_scene.meshes.len(), 1);
    assert_eq!(render_scene.meshes[0].mesh_asset, "primitive:cube");
}

#[test]
fn viewport_pipeline_uses_wgpu_triangle_pipeline() {
    let info = viewport_pipeline_info(wgpu::TextureFormat::Bgra8UnormSrgb);

    assert_eq!(info.label, "sge_viewport_pipeline");
    assert_eq!(info.color_format, wgpu::TextureFormat::Bgra8UnormSrgb);
    assert_eq!(
        info.primitive_topology,
        wgpu::PrimitiveTopology::TriangleList
    );
    assert!(info.shader_source.contains("@vertex"));
    assert!(info.shader_source.contains("@fragment"));
}

#[test]
fn viewport_draw_call_uses_camera_and_cube_mesh() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.label, "primitive:cube");
    assert_eq!(draw.vertex_count, 24);
    assert_eq!(draw.index_count, 36);
    assert_eq!(draw.camera_entity, EntityId::new("camera"));
}

#[test]
fn viewport_draw_call_includes_all_cube_meshes() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    add_cube(&mut world, "cube_1", [2.0, 0.0, 0.0]);

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.vertex_count, 48);
    assert_eq!(draw.index_count, 72);
}

#[test]
fn viewport_draw_call_marks_selected_cube_with_distinct_color() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    add_cube(&mut world, "cube_1", [2.0, 0.0, 0.0]);
    let scene = extract_render_scene(&world);

    let normal = viewport_draw_call(&scene).unwrap();
    let selected =
        viewport_draw_call_with_selection(&scene, Some(&EntityId::new("cube_1"))).unwrap();

    assert_eq!(selected.vertices[0].color, normal.vertices[0].color);
    assert_ne!(selected.vertices[24].color, normal.vertices[24].color);
}

#[test]
fn viewport_draw_call_uses_material_override_color() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    world.entity_mut("cube").unwrap().material_override = Some(MaterialOverride {
        base_color: [0.9, 0.1, 0.2, 0.7],
    });

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.vertices[0].color[3], 0.7);
    assert_ne!(
        draw.vertices[0].color,
        [0.3 * 0.38, 0.64 * 0.38, 1.0 * 0.38, 1.0]
    );
}

#[test]
fn viewport_draw_call_applies_first_light_only() {
    let mut first = world_with_camera();
    add_cube(&mut first, "cube", [0.0, 0.0, 0.0]);
    add_light(&mut first, "a_light", [0.1, 1.0, 0.1], 1.0);
    add_light(&mut first, "z_light", [1.0, 0.1, 0.1], 2.0);
    let mut changed_second = first.clone();
    changed_second
        .entity_mut("z_light")
        .unwrap()
        .light
        .as_mut()
        .unwrap()
        .intensity = 0.0;

    let first_draw = viewport_draw_call(&extract_render_scene(&first)).unwrap();
    let changed_second_draw = viewport_draw_call(&extract_render_scene(&changed_second)).unwrap();

    assert_eq!(first_draw.vertices, changed_second_draw.vertices);
}

#[test]
fn viewport_draw_call_makes_first_light_color_visibly_affect_cube_color() {
    let mut red_light = world_with_camera();
    add_cube(&mut red_light, "cube", [0.0, 0.0, 0.0]);
    red_light.entity_mut("cube").unwrap().material_override = Some(MaterialOverride {
        base_color: [0.6, 0.6, 0.6, 1.0],
    });
    add_light(&mut red_light, "directional_light", [1.0, 0.0, 0.0], 1.0);

    let mut blue_light = red_light.clone();
    blue_light
        .entity_mut("directional_light")
        .unwrap()
        .light
        .as_mut()
        .unwrap()
        .color = [0.0, 0.0, 1.0];

    let red_draw = viewport_draw_call(&extract_render_scene(&red_light)).unwrap();
    let blue_draw = viewport_draw_call(&extract_render_scene(&blue_light)).unwrap();
    let max_channel_delta = red_draw.vertices[0]
        .color
        .into_iter()
        .zip(blue_draw.vertices[0].color)
        .map(|(red, blue)| (red - blue).abs())
        .fold(0.0, f32::max);

    assert!(max_channel_delta > 0.3);
}

#[test]
fn viewport_view_projection_changes_projected_positions() {
    let wide = ViewportView::new(
        EntityId::new("wide"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 30.0,
        },
    );
    let narrow = ViewportView::new(
        EntityId::new("narrow"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 90.0,
        },
    );

    let size = ViewportSize::new(1600.0, 900.0).unwrap();
    let wide = ViewportProjection::from_view(&wide, size, ViewportClipPlanes::DEFAULT).unwrap();
    let narrow = ViewportProjection::from_view(&narrow, size, ViewportClipPlanes::DEFAULT).unwrap();

    assert_ne!(
        wide.project_world_point([1.0, 0.0, 4.0]),
        narrow.project_world_point([1.0, 0.0, 4.0])
    );
}

#[test]
fn perspective_projection_scales_with_camera_distance() {
    let far = ViewportView::new(
        EntityId::new("far"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );
    let near = ViewportView::new(
        EntityId::new("near"),
        Transform::from_translation([0.0, 0.0, 2.0]),
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );

    let size = ViewportSize::new(1600.0, 900.0).unwrap();
    let projected_width = |view: &ViewportView| {
        let projection =
            ViewportProjection::from_view(view, size, ViewportClipPlanes::DEFAULT).unwrap();
        let left = projection.project_world_point([-0.28, 0.0, 4.0]).unwrap();
        let right = projection.project_world_point([0.28, 0.0, 4.0]).unwrap();
        right[0] - left[0]
    };
    let far_width = projected_width(&far);
    let near_width = projected_width(&near);

    assert!(
        near_width > far_width,
        "near camera should enlarge projected span: near={near_width}, far={far_width}"
    );
}

#[test]
fn orthographic_projection_does_not_skew_screen_xy_by_depth() {
    let view = ViewportView::new(
        EntityId::new("top"),
        Transform::identity(),
        Projection::Orthographic { vertical_size: 5.0 },
    );
    let projection = ViewportProjection::from_view(
        &view,
        ViewportSize::new(1600.0, 900.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap();

    let near = projection.project_world_point([1.0, 2.0, 1.0]).unwrap();
    let far = projection.project_world_point([1.0, 2.0, 10.0]).unwrap();

    assert_eq!(near, far);
}

#[test]
fn viewport_projection_projects_draw_call_world_center() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [2.0, 0.0, 4.0]);
    let scene = extract_render_scene(&world);
    let view = ViewportView::new(
        EntityId::new("editor_view"),
        Transform::identity(),
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );

    let draw = viewport_draw_call_with_view(&scene, Some(&EntityId::new("cube")), &view).unwrap();
    let projection = ViewportProjection::from_view(
        &view,
        ViewportSize::new(1600.0, 900.0).unwrap(),
        ViewportClipPlanes::DEFAULT,
    )
    .unwrap();
    let projected = projection
        .project_world_point(draw.mesh_spans[0].world_center)
        .unwrap();

    assert!(projected.into_iter().all(f32::is_finite));
    assert_eq!(draw.mesh_spans[0].world_center, [2.0, 0.0, 4.0]);
}

#[test]
fn viewport_mesh_span_records_world_metrics_for_primitives() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [1.0, 2.0, 3.0]);

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();
    let span = &draw.mesh_spans[0];

    assert_eq!(span.entity, EntityId::new("cube"));
    assert!(span.world_bounds_min[0] < 1.0);
    assert!(span.world_bounds_max[0] > 1.0);
    assert_eq!(span.world_center, [1.0, 2.0, 3.0]);
}

#[test]
fn viewport_mesh_span_records_world_metrics_for_imported_meshes() {
    let uuid = AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let asset_ref = format!("asset:{uuid}");
    let mut meshes = BTreeMap::new();
    meshes.insert(uuid, imported_triangle_mesh());
    let mut world = world_with_camera();
    add_mesh(
        &mut world,
        "imported",
        "Imported",
        &asset_ref,
        Transform::from_translation([3.0, 4.0, 5.0]),
    );

    let draw = viewport_draw_call_with_view_and_meshes(
        &extract_render_scene(&world),
        Some(&EntityId::new("imported")),
        &ViewportView::new(
            EntityId::new("editor_view"),
            Transform::identity(),
            Projection::Perspective {
                fov_y_degrees: 60.0,
            },
        ),
        &meshes,
    )
    .unwrap();
    let span = &draw.mesh_spans[0];

    assert_eq!(span.entity, EntityId::new("imported"));
    assert_eq!(span.world_bounds_min, [3.0, 4.0, 5.0]);
    assert_eq!(span.world_bounds_max, [4.0, 5.0, 5.0]);
    assert_eq!(span.world_center, [3.5, 4.5, 5.0]);
}

#[test]
fn viewport_draw_call_applies_cube_scale_and_z_rotation() {
    let mut world = world_with_camera();
    add_cube_with_transform(
        &mut world,
        "cube",
        Transform {
            rotation: [
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            scale: [2.0, 1.0, 1.0],
            ..Transform::identity()
        },
    );

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();
    let min_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::INFINITY, f32::min);
    let max_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::NEG_INFINITY, f32::max);

    assert!((max_x - min_x) < (max_y - min_y));
}

#[test]
fn viewport_draw_call_projects_x_axis_rotation() {
    let mut identity_rotation = world_with_camera();
    add_cube_with_transform(
        &mut identity_rotation,
        "cube",
        Transform {
            scale: [1.0, 2.0, 1.0],
            ..Transform::identity()
        },
    );
    let mut x_rotation = world_with_camera();
    add_cube_with_transform(
        &mut x_rotation,
        "cube",
        Transform {
            rotation: [0.5, 0.0, 0.0, 0.866_025_4],
            scale: [1.0, 2.0, 1.0],
            ..Transform::identity()
        },
    );

    let identity_draw = viewport_draw_call(&extract_render_scene(&identity_rotation)).unwrap();
    let x_rotation_draw = viewport_draw_call(&extract_render_scene(&x_rotation)).unwrap();

    assert_ne!(identity_draw.vertices, x_rotation_draw.vertices);
}

#[test]
fn viewport_draw_call_applies_cube_z_scale() {
    let mut flat = world_with_camera();
    add_cube_with_transform(
        &mut flat,
        "cube",
        Transform {
            scale: [1.0, 1.0, 0.5],
            ..Transform::identity()
        },
    );
    let mut deep = world_with_camera();
    add_cube_with_transform(
        &mut deep,
        "cube",
        Transform {
            scale: [1.0, 1.0, 3.0],
            ..Transform::identity()
        },
    );

    let flat_draw = viewport_draw_call(&extract_render_scene(&flat)).unwrap();
    let deep_draw = viewport_draw_call(&extract_render_scene(&deep)).unwrap();

    assert_ne!(flat_draw.vertices, deep_draw.vertices);
}

#[test]
fn viewport_draw_call_applies_camera_translation_and_rotation() {
    let mut default_camera = world_with_camera_transform(Transform::identity());
    add_cube(&mut default_camera, "cube", [1.0, 0.0, 0.0]);
    let mut moved_camera = world_with_camera_transform(Transform {
        translation: [1.0, 0.0, 0.0],
        rotation: [
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        ],
        ..Transform::identity()
    });
    add_cube(&mut moved_camera, "cube", [1.0, 0.0, 0.0]);

    let default_view = ViewportView::from_camera(
        extract_render_scene(&default_camera)
            .active_camera
            .as_ref()
            .unwrap(),
    );
    let moved_view = ViewportView::from_camera(
        extract_render_scene(&moved_camera)
            .active_camera
            .as_ref()
            .unwrap(),
    );
    let size = ViewportSize::new(1600.0, 900.0).unwrap();
    let default_projection =
        ViewportProjection::from_view(&default_view, size, ViewportClipPlanes::DEFAULT).unwrap();
    let moved_projection =
        ViewportProjection::from_view(&moved_view, size, ViewportClipPlanes::DEFAULT).unwrap();

    assert_ne!(
        default_projection.project_world_point([1.0, 0.0, 4.0]),
        moved_projection.project_world_point([1.0, 0.0, 4.0])
    );
}

#[test]
fn viewport_draw_call_with_view_uses_explicit_view() {
    let mut world = world_with_camera_transform(Transform::identity());
    add_cube(&mut world, "cube", [1.0, 0.0, 4.0]);
    let scene = extract_render_scene(&world);
    let scene_camera_draw = viewport_draw_call(&scene).unwrap();
    let editor_view = ViewportView::new(
        EntityId::new("editor_view"),
        Transform {
            translation: [1.0, 0.0, 0.0],
            rotation: [
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            ..Transform::identity()
        },
        Projection::Perspective {
            fov_y_degrees: 60.0,
        },
    );

    let editor_draw = viewport_draw_call_with_view(&scene, None, &editor_view).unwrap();
    let scene_view = ViewportView::from_camera(scene.active_camera.as_ref().unwrap());
    let size = ViewportSize::new(1600.0, 900.0).unwrap();
    let scene_projection =
        ViewportProjection::from_view(&scene_view, size, ViewportClipPlanes::DEFAULT).unwrap();
    let editor_projection =
        ViewportProjection::from_view(&editor_view, size, ViewportClipPlanes::DEFAULT).unwrap();

    assert_eq!(editor_draw.camera_entity, EntityId::new("editor_view"));
    assert_eq!(scene_camera_draw.vertices, editor_draw.vertices);
    assert_ne!(
        scene_projection.project_world_point([1.0, 0.0, 4.0]),
        editor_projection.project_world_point([1.0, 0.0, 4.0])
    );
}

#[test]
fn viewport_draw_call_records_entity_spans_for_each_cube() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    add_cube(&mut world, "cube_1", [2.0, 0.0, 0.0]);

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.mesh_spans.len(), 2);
    assert_eq!(draw.mesh_spans[0].entity, EntityId::new("cube"));
    assert_eq!(draw.mesh_spans[0].vertex_range, 0..24);
    assert_eq!(draw.mesh_spans[0].index_range, 0..36);
    assert_eq!(draw.mesh_spans[1].entity, EntityId::new("cube_1"));
    assert_eq!(draw.mesh_spans[1].vertex_range, 24..48);
    assert_eq!(draw.mesh_spans[1].index_range, 36..72);
}

#[test]
fn viewport_draw_call_renders_distinct_primitive_geometry_and_spans() {
    let mut world = world_with_camera();
    add_mesh(
        &mut world,
        "cube",
        "Cube",
        "primitive:cube",
        Transform::identity(),
    );
    add_mesh(
        &mut world,
        "sphere",
        "Sphere",
        "primitive:sphere",
        Transform::from_translation([2.0, 0.0, 0.0]),
    );
    add_mesh(
        &mut world,
        "cone",
        "Cone",
        "primitive:cone",
        Transform::from_translation([4.0, 0.0, 0.0]),
    );
    add_mesh(
        &mut world,
        "cylinder",
        "Cylinder",
        "primitive:cylinder",
        Transform::from_translation([6.0, 0.0, 0.0]),
    );

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.mesh_spans.len(), 4);
    let cube = span_index_for(&draw, "cube");
    let sphere = span_index_for(&draw, "sphere");
    let cone = span_index_for(&draw, "cone");
    let cylinder = span_index_for(&draw, "cylinder");
    assert_eq!(draw.mesh_spans[cube].vertex_range.len(), 24);
    assert_eq!(draw.mesh_spans[cube].index_range.len(), 36);
    assert_eq!(draw.mesh_spans[sphere].vertex_range.len(), 6);
    assert_eq!(draw.mesh_spans[sphere].index_range.len(), 24);
    assert_eq!(draw.mesh_spans[cone].vertex_range.len(), 10);
    assert_eq!(draw.mesh_spans[cone].index_range.len(), 48);
    assert_eq!(draw.mesh_spans[cylinder].vertex_range.len(), 18);
    assert_eq!(draw.mesh_spans[cylinder].index_range.len(), 96);
    assert_ne!(
        rounded_positions(&draw, cube),
        rounded_positions(&draw, sphere)
    );
    assert_ne!(
        rounded_positions(&draw, sphere),
        rounded_positions(&draw, cone)
    );
    assert_ne!(
        rounded_positions(&draw, cone),
        rounded_positions(&draw, cylinder)
    );
}

#[test]
fn viewport_draw_call_material_and_selection_apply_to_non_cube_primitives() {
    let mut world = world_with_camera();
    add_mesh(
        &mut world,
        "sphere",
        "Sphere",
        "primitive:sphere",
        Transform::identity(),
    );
    add_mesh(
        &mut world,
        "cone",
        "Cone",
        "primitive:cone",
        Transform::from_translation([2.0, 0.0, 0.0]),
    );
    world.entity_mut("sphere").unwrap().material_override = Some(MaterialOverride {
        base_color: [0.2, 0.8, 0.3, 0.6],
    });
    let scene = extract_render_scene(&world);

    let normal = viewport_draw_call(&scene).unwrap();
    let selected = viewport_draw_call_with_selection(&scene, Some(&EntityId::new("cone"))).unwrap();

    let sphere_start = normal.mesh_spans[span_index_for(&normal, "sphere")]
        .vertex_range
        .start;
    assert_eq!(normal.vertices[sphere_start].color[3], 0.6);
    let cone_start = normal.mesh_spans[span_index_for(&normal, "cone")]
        .vertex_range
        .start;
    assert_ne!(
        selected.vertices[cone_start].color,
        normal.vertices[cone_start].color
    );
}

#[test]
fn viewport_draw_call_applies_non_cube_rotation_and_scale() {
    let mut identity = world_with_camera();
    add_mesh(
        &mut identity,
        "sphere",
        "Sphere",
        "primitive:sphere",
        Transform::identity(),
    );
    add_mesh(
        &mut identity,
        "cone",
        "Cone",
        "primitive:cone",
        Transform::from_translation([2.0, 0.0, 0.0]),
    );
    let mut transformed = world_with_camera();
    add_mesh(
        &mut transformed,
        "sphere",
        "Sphere",
        "primitive:sphere",
        Transform {
            rotation: [
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            scale: [1.0, 2.0, 0.5],
            ..Transform::identity()
        },
    );
    add_mesh(
        &mut transformed,
        "cone",
        "Cone",
        "primitive:cone",
        Transform {
            translation: [2.0, 0.0, 0.0],
            rotation: [0.5, 0.0, 0.0, 0.866_025_4],
            scale: [0.5, 1.5, 2.0],
        },
    );

    let identity_draw = viewport_draw_call(&extract_render_scene(&identity)).unwrap();
    let transformed_draw = viewport_draw_call(&extract_render_scene(&transformed)).unwrap();

    assert_ne!(
        rounded_positions(&identity_draw, 0),
        rounded_positions(&transformed_draw, 0)
    );
    assert_ne!(
        rounded_positions(&identity_draw, 1),
        rounded_positions(&transformed_draw, 1)
    );
}

#[test]
fn viewport_draw_call_skips_unknown_primitive_without_blocking_known_meshes() {
    let mut world = world_with_camera();
    add_mesh(
        &mut world,
        "unknown",
        "Unknown",
        "primitive:capsule",
        Transform::identity(),
    );
    add_mesh(
        &mut world,
        "sphere",
        "Sphere",
        "primitive:sphere",
        Transform::identity(),
    );

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(draw.mesh_spans.len(), 1);
    assert_eq!(draw.mesh_spans[0].entity, EntityId::new("sphere"));
}

#[test]
fn viewport_draw_call_renders_imported_mesh_and_span() {
    let uuid = AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let mut world = world_with_camera();
    world.spawn(EntityId::new("imported"), "Imported", Transform::identity());
    world
        .insert_mesh(
            "imported",
            MeshRef::new(uuid.to_asset_ref(), "primitive:default_material"),
        )
        .unwrap();
    let mut meshes = BTreeMap::new();
    meshes.insert(uuid, imported_triangle_mesh());

    let draw = viewport_draw_call_with_view_and_meshes(
        &extract_render_scene(&world),
        Some(&EntityId::new("imported")),
        &ViewportView::new(
            EntityId::new("view"),
            Transform::identity(),
            Projection::Perspective {
                fov_y_degrees: 60.0,
            },
        ),
        &meshes,
    )
    .unwrap();

    assert_eq!(draw.vertex_count, 3);
    assert_eq!(draw.index_count, 3);
    assert_eq!(draw.mesh_spans.len(), 1);
    assert_eq!(draw.mesh_spans[0].entity, EntityId::new("imported"));
    assert_eq!(draw.mesh_spans[0].vertex_range, 0..3);
    assert_eq!(draw.mesh_spans[0].index_range, 0..3);
}

#[test]
fn viewport_draw_call_skips_missing_imported_mesh_but_keeps_cubes() {
    let uuid = AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    world.spawn(EntityId::new("missing"), "Missing", Transform::identity());
    world
        .insert_mesh(
            "missing",
            MeshRef::new(uuid.to_asset_ref(), "primitive:default_material"),
        )
        .unwrap();

    let draw = viewport_draw_call_with_view_and_meshes(
        &extract_render_scene(&world),
        None,
        &ViewportView::new(
            EntityId::new("view"),
            Transform::identity(),
            Projection::Perspective {
                fov_y_degrees: 60.0,
            },
        ),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(draw.index_count, 36);
    assert_eq!(draw.mesh_spans.len(), 1);
    assert_eq!(draw.mesh_spans[0].entity, EntityId::new("cube"));
}

#[test]
fn viewport_draw_call_shades_cube_faces_distinctly() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);

    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();
    let distinct_colors = draw
        .vertices
        .iter()
        .map(|vertex| vertex.color.map(|channel| (channel * 255.0).round() as u16))
        .collect::<std::collections::BTreeSet<_>>();

    assert!(distinct_colors.len() >= 6);
}

#[test]
fn viewport_vertex_layout_matches_shader_locations() {
    let layout = viewport_vertex_buffer_layout();

    assert_eq!(layout.array_stride, 28);
    assert_eq!(layout.attributes[0].shader_location, 0);
    assert_eq!(layout.attributes[0].format, wgpu::VertexFormat::Float32x3);
    assert_eq!(layout.attributes[1].shader_location, 1);
    assert_eq!(layout.attributes[1].format, wgpu::VertexFormat::Float32x4);
}

#[test]
fn viewport_vertex_bytes_match_vertex_count() {
    let mut world = world_with_camera();
    add_cube(&mut world, "cube", [0.0, 0.0, 0.0]);
    let draw = viewport_draw_call(&extract_render_scene(&world)).unwrap();

    assert_eq!(
        viewport_vertex_bytes(&draw.vertices).len(),
        draw.vertex_count * 28
    );
}

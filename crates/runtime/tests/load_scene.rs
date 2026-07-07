// Copyright The SimpleGameEngine Contributors
//
//! runtime 示例场景加载测试。

use std::path::Path;

#[test]
fn runtime_loads_editor_smoke_scene() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/editor_smoke.scene.ron");
    let render_scene = runtime::load_scene_from_path(&path).unwrap();

    assert_eq!(render_scene.meshes.len(), 1);
    assert!(render_scene.active_camera.is_some());
    assert_eq!(
        runtime::load_viewport_draw_from_path(&path)
            .unwrap()
            .index_count,
        36
    );
}

#[test]
fn runtime_loads_imported_asset_from_explicit_project_root() {
    let root = temp_runtime_project("runtime_imported_asset");
    let uuid = asset::AssetUuid::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    write_triangle_obj(&root.join("assets/imported/triangle.obj"));
    let mut manifest = asset::AssetManifest::default();
    manifest.upsert(asset::AssetRecord {
        uuid: uuid.clone(),
        name: "triangle".to_owned(),
        kind: asset::AssetKind::Mesh,
        path: "assets/imported/triangle.obj".into(),
        importer: asset::AssetImporter::Obj,
        source_name: "triangle.obj".to_owned(),
    });
    manifest.save_to_project_root(&root).unwrap();
    let scene_path = root.join("../external_scene.scene.ron");
    std::fs::write(&scene_path, imported_scene(&uuid)).unwrap();

    let draw = runtime::load_viewport_draw_from_path_with_project_root(&scene_path, &root).unwrap();

    assert_eq!(draw.mesh_spans.len(), 1);
    assert_eq!(draw.index_count, 3);
}

fn temp_runtime_project(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/runtime_tests")
        .join(format!("{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/imported")).unwrap();
    root
}

fn write_triangle_obj(path: &std::path::Path) {
    std::fs::write(path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
}

fn imported_scene(uuid: &asset::AssetUuid) -> String {
    format!(
        r#"(
    entities: [
        (
            id: "camera",
            name: "Camera",
            transform: (
                translation: (0.0, 2.0, 5.0),
                rotation: (0.0, 0.0, 0.0, 1.0),
                scale: (1.0, 1.0, 1.0),
            ),
            parent: None,
            camera: Some((projection: Perspective(fov_y_degrees: 60.0))),
            mesh: None,
            material_override: None,
            light: None,
        ),
        (
            id: "imported",
            name: "Imported",
            transform: (
                translation: (0.0, 0.0, 0.0),
                rotation: (0.0, 0.0, 0.0, 1.0),
                scale: (1.0, 1.0, 1.0),
            ),
            parent: None,
            camera: None,
            mesh: Some((asset: "{}", material: "primitive:default_material")),
            material_override: None,
            light: None,
        ),
    ],
)"#,
        uuid.to_asset_ref()
    )
}

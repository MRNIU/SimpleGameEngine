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
        6
    );
}

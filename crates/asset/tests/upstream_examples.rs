// Copyright The SimpleGameEngine Contributors

use std::path::Path;

#[test]
fn upstream_obj_samples_load() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/obj");
    for relative_path in [
        "african_head.obj",
        "box.obj",
        "cornell_box.obj",
        "cube.obj",
        "cube2.obj",
        "cube3.obj",
        "helmet.obj",
        "utah-teapot/utah-teapot.obj",
        "utah-teapot-texture/teapot.obj",
    ] {
        let mesh = asset::load_obj_mesh(&root.join(relative_path)).unwrap_or_else(|error| {
            panic!("{relative_path} should load: {error}");
        });
        assert!(!mesh.vertices.is_empty(), "{relative_path} has vertices");
        assert!(!mesh.indices.is_empty(), "{relative_path} has indices");
    }
}

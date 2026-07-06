// Copyright The SimpleGameEngine Contributors
//
//! runtime smoke 命令入口。

use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let scene_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/examples/editor_smoke.scene.ron"));
    let render_scene = runtime::load_scene_from_path(&scene_path)?;
    let viewport_draw = runtime::load_viewport_draw_from_path(&scene_path)?;
    println!(
        "loaded {} mesh(es), camera: {}, viewport indices: {}",
        render_scene.meshes.len(),
        render_scene.active_camera.is_some(),
        viewport_draw.index_count
    );
    Ok(())
}

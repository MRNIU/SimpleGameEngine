// Copyright The SimpleGameEngine Contributors
//
//! runtime 场景加载边界。

use std::{fs, path::Path};

use anyhow::Context;
use render::{RenderScene, ViewportDrawCall};

pub fn load_scene_from_path(path: &Path) -> anyhow::Result<RenderScene> {
    let input =
        fs::read_to_string(path).with_context(|| format!("read scene {}", path.display()))?;
    let world =
        scene::load_scene(&input).with_context(|| format!("parse scene {}", path.display()))?;
    Ok(render::extract_render_scene(&world))
}

pub fn load_viewport_draw_from_path(path: &Path) -> anyhow::Result<ViewportDrawCall> {
    let render_scene = load_scene_from_path(path)?;
    render::viewport_draw_call(&render_scene)
        .with_context(|| format!("build viewport draw call {}", path.display()))
}

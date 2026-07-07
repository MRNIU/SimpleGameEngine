// Copyright The SimpleGameEngine Contributors
//
//! runtime 场景加载边界。

use std::{collections::BTreeMap, fs, path::Path};

use anyhow::Context;
use render::{RenderScene, ViewportDrawCall, ViewportView};

pub fn load_scene_from_path(path: &Path) -> anyhow::Result<RenderScene> {
    let input =
        fs::read_to_string(path).with_context(|| format!("read scene {}", path.display()))?;
    let world =
        scene::load_scene(&input).with_context(|| format!("parse scene {}", path.display()))?;
    Ok(render::extract_render_scene(&world))
}

pub fn load_viewport_draw_from_path(path: &Path) -> anyhow::Result<ViewportDrawCall> {
    let project_root = std::env::current_dir().context("resolve current project root")?;
    load_viewport_draw_from_path_with_project_root(path, &project_root)
}

pub fn load_viewport_draw_from_path_with_project_root(
    scene_path: &Path,
    project_root: &Path,
) -> anyhow::Result<ViewportDrawCall> {
    let input = fs::read_to_string(scene_path)
        .with_context(|| format!("read scene {}", scene_path.display()))?;
    let world = scene::load_scene(&input)
        .with_context(|| format!("parse scene {}", scene_path.display()))?;
    let render_scene = render::extract_render_scene(&world);
    let manifest =
        asset::AssetManifest::load_from_project_root(project_root).with_context(|| {
            format!(
                "load manifest {}",
                asset::manifest_path(project_root).display()
            )
        })?;
    let imported_meshes = load_imported_meshes(project_root, &manifest)?;
    let view = render_scene
        .active_camera
        .as_ref()
        .map(ViewportView::from_camera)
        .ok_or_else(|| anyhow::anyhow!("scene has no active camera"))?;
    render::viewport_draw_call_with_view_and_meshes(&render_scene, None, &view, &imported_meshes)
        .with_context(|| format!("build viewport draw call {}", scene_path.display()))
}

fn load_imported_meshes(
    project_root: &Path,
    manifest: &asset::AssetManifest,
) -> anyhow::Result<BTreeMap<asset::AssetUuid, asset::ImportedMesh>> {
    let mut meshes = BTreeMap::new();
    for record in &manifest.assets {
        if record.kind != asset::AssetKind::Mesh || record.importer != asset::AssetImporter::Obj {
            continue;
        }
        let path = project_root.join(&record.path);
        let mesh = asset::load_obj_mesh(&path)
            .with_context(|| format!("load imported mesh {}", path.display()))?;
        meshes.insert(record.uuid.clone(), mesh);
    }
    Ok(meshes)
}

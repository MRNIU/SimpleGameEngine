// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf};

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use sge_asset::{
    AssetId, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset, RuntimeAssetStore, RuntimeContentRoot,
    TEXTURE_ASSET_TYPE_KEY, TextureAsset,
};
use sge_asset_pipeline::{CookOutputRoot, full_cook};
use sge_ecs::World;
use sge_project::{
    AuthoringAssetManifest, ObjImportSettings, ProjectDescriptor, ProjectPath, ProjectRoot,
    SourceAssetRecord, SourceImporter,
};
use sge_reflect::{FieldKey, FieldRegistration, TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{AuthoringEntity, AuthoringScene, Parent, SceneEntityId};

const GAME_ID: &str = "texture.cook.test";

#[derive(Clone)]
struct TexturedConsumer {
    mesh: AssetRef<MeshAsset>,
    texture: AssetRef<TextureAsset>,
}

#[test]
fn texture_cook_is_stable_invalidates_on_png_change_and_remains_source_free()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fixture_root();
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("project/Content"))?;
    fs::create_dir_all(root.join("project/Scenes"))?;
    fs::create_dir_all(root.join("output"))?;
    let project = ProjectRoot::open(root.join("project"))?;
    let mesh: AssetId = "10000000-0000-4000-8000-000000000001".parse()?;
    let texture: AssetId = "10000000-0000-4000-8000-000000000002".parse()?;
    fs::write(
        root.join("project/Content/model.obj"),
        b"v -1 -1 0\nv 1 -1 0\nv 0 1 0\nvt 0 1\nvt 1 1\nvt 0.5 0\nf 1/1 2/2 3/3\n",
    )?;
    fs::write(
        root.join("project/Content/color.png"),
        png([255, 0, 0, 255])?,
    )?;
    ProjectDescriptor::new(
        GAME_ID,
        "texture-game",
        "texture-player",
        "texture-build",
        ProjectPath::new("Scenes/main.scene.ron")?,
    )?
    .save(&project)?;
    AuthoringAssetManifest::new(vec![
        SourceAssetRecord::new(
            mesh,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            ProjectPath::new("Content/model.obj")?,
            SourceImporter::Obj(ObjImportSettings::new(false)),
        )?,
        SourceAssetRecord::new(
            texture,
            TypeKey::new(TEXTURE_ASSET_TYPE_KEY)?,
            ProjectPath::new("Content/color.png")?,
            SourceImporter::Png,
        )?,
    ])?
    .save(&project)?;
    let registry = registry()?;
    let scene = AuthoringScene::new(vec![AuthoringEntity::new(
        "20000000-0000-4000-8000-000000000001".parse::<SceneEntityId>()?,
        None,
        vec![registry.encode(&TexturedConsumer {
            mesh: AssetRef::new(mesh),
            texture: AssetRef::new(texture),
        })?],
    )?])?;
    project.write_atomic(
        &ProjectPath::new("Scenes/main.scene.ron")?,
        scene.to_ron()?.as_bytes(),
    )?;
    let world = world()?;
    let output = CookOutputRoot::open(root.join("output"))?;

    let first = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    let stable = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert_eq!(first.generation(), stable.generation());
    assert_eq!(first.published_assets(), &[mesh, texture]);
    let first_generation = first.generation().clone();

    fs::write(
        root.join("project/Content/color.png"),
        png([0, 255, 0, 255])?,
    )?;
    let changed = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert_ne!(&first_generation, changed.generation());
    let content = RuntimeContentRoot::open(root.join("output"))?;
    let generation = content.load_current(GAME_ID)?;
    let store = RuntimeAssetStore::load(&generation)?;
    assert_eq!(
        store.texture(AssetRef::new(texture))?.rgba8_srgb(),
        &[0, 255, 0, 255]
    );

    fs::remove_dir_all(root.join("project"))?;
    let copied = root.join("copied");
    copy_tree(&root.join("output"), &copied)?;
    let generation = RuntimeContentRoot::open(&copied)?.load_current(GAME_ID)?;
    let store = RuntimeAssetStore::load(&generation)?;
    assert!(store.mesh(AssetRef::new(mesh)).is_ok());
    assert!(store.texture(AssetRef::new(texture)).is_ok());
    fs::remove_dir_all(root)?;
    Ok(())
}

fn registry() -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let descriptor = TypeDescriptor::builder::<TexturedConsumer>(
        TypeKey::new("test.textured_consumer")?,
        1,
        "Textured Consumer",
        || TexturedConsumer {
            mesh: AssetRef::new(AssetId::new_v4()),
            texture: AssetRef::new(AssetId::new_v4()),
        },
    )
    .field(FieldRegistration::reference(
        FieldKey::new("mesh")?,
        "Mesh",
        |value: &TexturedConsumer| &value.mesh,
        |value: &mut TexturedConsumer, mesh| value.mesh = mesh,
    )?)
    .field(FieldRegistration::reference(
        FieldKey::new("texture")?,
        "Texture",
        |value: &TexturedConsumer| &value.texture,
        |value: &mut TexturedConsumer, texture| value.texture = texture,
    )?)
    .scene_saveable()
    .build()?;
    let mut registry = TypeRegistry::new();
    registry.register(descriptor)?;
    registry.freeze()?;
    Ok(registry)
}

fn world() -> Result<World, Box<dyn std::error::Error>> {
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.register_component::<TexturedConsumer>()?;
    world.finish_registration();
    Ok(world)
}

fn png(pixel: [u8; 4]) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(&pixel, 1, 1, ExtendedColorType::Rgba8)?;
    Ok(bytes)
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/sge_texture_cook")
        .join(std::process::id().to_string())
}

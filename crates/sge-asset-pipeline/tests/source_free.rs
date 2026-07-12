// Copyright The SimpleGameEngine Contributors

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use sge_app::GameDescriptor;
use sge_asset::{
    AssetRef, MeshAsset, RuntimeAssetCatalog, RuntimeAssetStore, RuntimeCatalogError,
    RuntimeContentError, RuntimeContentRoot,
};
use sge_asset_pipeline::{CookOutputRoot, full_cook};
use sge_scene::{RuntimeScene, instantiate, prepare_runtime};

use support::{
    FullCookFixture, GAME_ID, MeshConsumer, app_creations, create_ready_app, reset_app_creations,
};

#[test]
fn copied_runtime_is_source_free_and_builds_a_second_ready_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    reset_app_creations();
    let game = GameDescriptor::new(GAME_ID, create_ready_app);
    let other_game = GameDescriptor::new("other.game", create_ready_app);
    let fixture = FullCookFixture::new("source-free")?;
    let source_root = fs::canonicalize(fixture.root_path())?;
    let copy = RuntimeCopy::new()?;

    {
        let project = fixture.project()?;
        let first_app = game.create_app()?;
        full_cook(
            &project,
            game.game_id(),
            first_app.type_registry(),
            first_app.world(),
            &CookOutputRoot::open(fixture.output_path())?,
        )?;
    }
    copy_current_runtime(fixture.output_path(), copy.root())?;
    let cache_paths = exact_cache_paths(fixture.root_path())?;
    assert_eq!(cache_paths.len(), 2);
    assert_declared_tree_is_strict(copy.root())?;
    assert_no_authoring_leaks(copy.root(), &source_root, &cache_paths)?;
    fixture.delete_source_project()?;

    let catalog = load_catalog(copy.root())?;
    let generation_path = copy
        .root()
        .join("generations")
        .join(catalog.generation().as_str());
    let removed_generation = copy.base().join("removed-generation");
    fs::rename(&generation_path, &removed_generation)?;
    let content = RuntimeContentRoot::open(copy.root())?;
    assert!(matches!(
        content.load_current(other_game.game_id()),
        Err(RuntimeContentError::GameMismatch { .. })
    ));
    assert!(matches!(
        content.load_current(game.game_id()),
        Err(RuntimeContentError::MissingPath { .. })
    ));
    fs::rename(&removed_generation, &generation_path)?;

    let product_path = generation_path.join(runtime_path(catalog.assets()[0].product().as_str()));
    let product_bytes = fs::read(&product_path)?;
    fs::write(&product_path, b"corrupt runtime product")?;
    assert!(matches!(
        content.load_current(other_game.game_id()),
        Err(RuntimeContentError::GameMismatch { .. })
    ));
    assert!(matches!(
        content.load_current(game.game_id()),
        Err(RuntimeContentError::Catalog {
            source: RuntimeCatalogError::GenerationMismatch { .. }
        })
    ));
    fs::write(&product_path, product_bytes)?;

    let generation = content.load_current(game.game_id())?;
    let store = RuntimeAssetStore::load(&generation)?;
    let runtime = RuntimeScene::from_ron(std::str::from_utf8(generation.entry_scene_bytes())?)?;
    let mut second_app = game.create_app()?;
    let prepared = prepare_runtime(&runtime, second_app.type_registry(), &store)?;
    let instance = instantiate(prepared, second_app.world_initializer()?)?;
    let runtime_entity = instance
        .iter()
        .map(|(_, entity)| entity)
        .find(|entity| second_app.world().get::<MeshConsumer>(*entity).is_some())
        .ok_or("runtime MeshConsumer entity missing")?;
    let consumer = second_app
        .world()
        .get::<MeshConsumer>(runtime_entity)
        .ok_or("runtime MeshConsumer missing")?;
    assert_eq!(*consumer.mesh.id(), fixture.used);
    let mesh = store.mesh(AssetRef::<MeshAsset>::new(*consumer.mesh.id()))?;
    assert_eq!(mesh.indices().len(), 3);
    assert_eq!(app_creations(), 2);
    Ok(())
}

fn copy_current_runtime(
    source: &Path,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog(source)?;
    fs::copy(
        source.join("runtime_catalog.ron"),
        destination.join("runtime_catalog.ron"),
    )?;
    let source_generation = source
        .join("generations")
        .join(catalog.generation().as_str());
    let destination_generation = destination
        .join("generations")
        .join(catalog.generation().as_str());
    copy_declared(
        &source_generation,
        &destination_generation,
        catalog.entry_scene().as_str(),
    )?;
    for asset in catalog.assets() {
        copy_declared(
            &source_generation,
            &destination_generation,
            asset.product().as_str(),
        )?;
    }
    Ok(())
}

fn copy_declared(source: &Path, destination: &Path, relative: &str) -> Result<(), std::io::Error> {
    let relative = runtime_path(relative);
    let target = destination.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source.join(relative), target)?;
    Ok(())
}

fn assert_declared_tree_is_strict(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog(root)?;
    let root_roles = fs::read_dir(root)?
        .map(|entry| Ok(entry?.file_name()))
        .collect::<Result<BTreeSet<_>, std::io::Error>>()?;
    assert_eq!(
        root_roles,
        BTreeSet::from(["generations".into(), "runtime_catalog.ron".into()])
    );
    let generations = root.join("generations");
    let generation_roles = fs::read_dir(&generations)?
        .map(|entry| Ok(entry?.file_name()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    assert_eq!(generation_roles, [catalog.generation().as_str()]);

    let generation = generations.join(catalog.generation().as_str());
    let mut expected = BTreeSet::from([runtime_path(catalog.entry_scene().as_str())]);
    expected.extend(
        catalog
            .assets()
            .iter()
            .map(|asset| runtime_path(asset.product().as_str())),
    );
    let files = collect_files(&generation)?;
    assert_eq!(files.keys().cloned().collect::<BTreeSet<_>>(), expected);
    RuntimeScene::from_ron(std::str::from_utf8(
        &files[&runtime_path(catalog.entry_scene().as_str())],
    )?)?;
    for asset in catalog.assets() {
        MeshAsset::from_ron(std::str::from_utf8(
            &files[&runtime_path(asset.product().as_str())],
        )?)?;
    }
    Ok(())
}

fn assert_no_authoring_leaks(
    root: &Path,
    source_root: &Path,
    cache_paths: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut exact_needles = vec![
        source_root.to_string_lossy().into_owned(),
        "Content/used.obj".to_owned(),
        "Content/unused.obj".to_owned(),
        "used.obj".to_owned(),
        "unused.obj".to_owned(),
        "Cache/Imported".to_owned(),
    ];
    exact_needles.extend(cache_paths.iter().cloned());
    for bytes in collect_files(root)?.values() {
        for needle in &exact_needles {
            assert!(
                !contains_bytes(bytes, needle.as_bytes()),
                "source-free runtime leaked exact authoring value {needle:?}"
            );
        }
    }
    Ok(())
}

fn exact_cache_paths(project_root: &Path) -> Result<Vec<String>, std::io::Error> {
    let cache_root = project_root.join("Cache");
    Ok(collect_files(&cache_root)?
        .into_keys()
        .map(|path| {
            PathBuf::from("Cache")
                .join(path)
                .to_string_lossy()
                .into_owned()
        })
        .collect())
}

fn load_catalog(root: &Path) -> Result<RuntimeAssetCatalog, Box<dyn std::error::Error>> {
    Ok(RuntimeAssetCatalog::from_ron(std::str::from_utf8(
        &fs::read(root.join("runtime_catalog.ron"))?,
    )?)?)
}

fn collect_files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, std::io::Error> {
    let mut files = BTreeMap::new();
    collect_files_at(root, Path::new(""), &mut files)?;
    Ok(files)
}

fn collect_files_at(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let child = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect_files_at(root, &child, files)?;
        } else {
            files.insert(child.clone(), fs::read(root.join(child))?);
        }
    }
    Ok(())
}

fn runtime_path(value: &str) -> PathBuf {
    value.split('/').collect()
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

struct RuntimeCopy {
    base: PathBuf,
    root: PathBuf,
}

impl RuntimeCopy {
    fn new() -> Result<Self, std::io::Error> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_asset_pipeline_source_free")
            .join(std::process::id().to_string());
        let _ = fs::remove_dir_all(&base);
        let root = base.join("runtime");
        fs::create_dir_all(root.join("generations"))?;
        Ok(Self { base, root })
    }

    fn base(&self) -> &Path {
        &self.base
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for RuntimeCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

// Copyright The SimpleGameEngine Contributors

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sge_asset::{
    AssetId, MESH_ASSET_TYPE_KEY, MeshAsset, MeshVertex, RuntimeAssetCatalog, RuntimeAssetRecord,
    RuntimeAssetStore, RuntimeContentError, RuntimeContentRoot, RuntimeProductPath,
};
use sge_reflect::TypeKey;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Result<Self, std::io::Error> {
        let path = std::env::temp_dir().join(format!(
            "sge-runtime-content-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    root: TempRoot,
    asset: AssetId,
    generation_dir: PathBuf,
    product_path: PathBuf,
}

fn asset_id() -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok("10000000-0000-4000-8000-000000000001".parse()?)
}

fn mesh_bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([0.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([1.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?
    .to_ron()?
    .into_bytes())
}

fn catalog_with_mesh(
    entry_bytes: &[u8],
    product_bytes: &[u8],
) -> Result<RuntimeAssetCatalog, Box<dyn std::error::Error>> {
    let asset = asset_id()?;
    Ok(RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        vec![RuntimeAssetRecord::new(
            asset,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            RuntimeProductPath::new(format!("Content/{asset}.mesh.ron"))?,
            Vec::new(),
        )?],
        entry_bytes,
        &BTreeMap::from([(asset, product_bytes.to_vec())]),
    )?)
}

fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let root = TempRoot::new()?;
    let asset = asset_id()?;
    let entry_bytes = b"runtime scene";
    let product_bytes = mesh_bytes()?;
    let catalog = catalog_with_mesh(entry_bytes, &product_bytes)?;
    fs::write(root.path().join("runtime_catalog.ron"), catalog.to_ron()?)?;
    let generation_dir = root
        .path()
        .join("generations")
        .join(catalog.generation().as_str());
    let scene_dir = generation_dir.join("Scenes");
    let content_dir = generation_dir.join("Content");
    fs::create_dir_all(&scene_dir)?;
    fs::create_dir_all(&content_dir)?;
    fs::write(scene_dir.join("entry.runtime-scene.ron"), entry_bytes)?;
    let product_path = content_dir.join(format!("{asset}.mesh.ron"));
    fs::write(&product_path, product_bytes)?;
    Ok(Fixture {
        root,
        asset,
        generation_dir,
        product_path,
    })
}

#[test]
fn runtime_content_loads_verified_generation_and_store() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    let root = RuntimeContentRoot::open(fixture.root.path())?;
    let generation = root.load_current("demo.game")?;
    let store = RuntimeAssetStore::load(&generation)?;

    assert_eq!(generation.catalog().game_id().as_str(), "demo.game");
    assert_eq!(generation.entry_scene_bytes(), b"runtime scene");
    assert_eq!(
        store
            .mesh(sge_asset::AssetRef::new(fixture.asset))?
            .indices(),
        &[0, 1, 2]
    );
    Ok(())
}

#[test]
fn wrong_game_fails_before_missing_generation_access() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new()?;
    let catalog = catalog_with_mesh(b"runtime scene", &mesh_bytes()?)?;
    fs::write(root.path().join("runtime_catalog.ron"), catalog.to_ron()?)?;
    let root = RuntimeContentRoot::open(root.path())?;

    assert!(matches!(
        root.load_current("other.game"),
        Err(RuntimeContentError::GameMismatch { .. })
    ));
    Ok(())
}

#[test]
fn runtime_content_rejects_missing_extra_and_digest_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let missing = fixture()?;
    fs::remove_file(&missing.product_path)?;
    assert!(matches!(
        RuntimeContentRoot::open(missing.root.path())?.load_current("demo.game"),
        Err(RuntimeContentError::MissingPath { .. })
    ));

    let extra = fixture()?;
    fs::write(extra.generation_dir.join("extra.bin"), b"extra")?;
    assert!(matches!(
        RuntimeContentRoot::open(extra.root.path())?.load_current("demo.game"),
        Err(RuntimeContentError::UnexpectedPath { .. })
    ));

    let root_extra = fixture()?;
    fs::create_dir(root_extra.root.path().join("Cache"))?;
    fs::write(root_extra.root.path().join("source.obj"), b"source")?;
    assert!(matches!(
        RuntimeContentRoot::open(root_extra.root.path())?.load_current("demo.game"),
        Err(RuntimeContentError::UnexpectedPath { .. })
    ));

    let changed = fixture()?;
    fs::write(&changed.product_path, b"changed")?;
    assert!(matches!(
        RuntimeContentRoot::open(changed.root.path())?.load_current("demo.game"),
        Err(RuntimeContentError::Catalog { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn runtime_content_rejects_symlink_product() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let fixture = fixture()?;
    fs::remove_file(&fixture.product_path)?;
    symlink(
        fixture.root.path().join("runtime_catalog.ron"),
        &fixture.product_path,
    )?;

    assert!(matches!(
        RuntimeContentRoot::open(fixture.root.path())?.load_current("demo.game"),
        Err(RuntimeContentError::Symlink { .. })
    ));
    Ok(())
}

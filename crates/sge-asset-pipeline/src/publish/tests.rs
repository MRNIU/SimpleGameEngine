// Copyright The SimpleGameEngine Contributors

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_asset::{
    AssetId, MESH_ASSET_TYPE_KEY, RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeAssetStoreError,
    RuntimeContentRoot, RuntimeProductPath,
};
use sge_ecs::World;
use sge_reflect::{TypeKey, TypeRegistry};
use sge_scene::{Parent, SceneEntityId};

use super::{publish_with_commit, verify_unpublished_tree};
use crate::output::{CookOutputRoot, CookPublishError};

const RUNTIME_SCENE: &[u8] =
    b"(\n    format_version: 1,\n    scene_role: Runtime,\n    entities: [],\n)";
const OLD_RUNTIME_SCENE: &[u8] = b"(format_version:1,scene_role:Runtime,entities:[])";

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn unpublished_tree_rejects_missing_declared_role() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("tree-missing")?;
    let expected = expected_tree();
    write_file(
        fixture.path(),
        "Scenes/entry.runtime-scene.ron",
        RUNTIME_SCENE,
    )?;

    assert!(matches!(
        verify_unpublished_tree(fixture.path(), &expected),
        Err(CookPublishError::MissingPath { .. })
    ));
    Ok(())
}

#[test]
fn unpublished_tree_rejects_extra_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("tree-extra")?;
    let expected = expected_tree();
    write_expected(fixture.path(), &expected)?;
    write_file(fixture.path(), "Content/extra.mesh.ron", b"extra")?;

    assert!(matches!(
        verify_unpublished_tree(fixture.path(), &expected),
        Err(CookPublishError::UnexpectedPath { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn unpublished_tree_rejects_symlink() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("tree-symlink")?;
    let expected = expected_tree();
    write_expected(fixture.path(), &expected)?;
    symlink(
        fixture.path().join("Scenes/entry.runtime-scene.ron"),
        fixture.path().join("Content/linked.mesh.ron"),
    )?;

    assert!(matches!(
        verify_unpublished_tree(fixture.path(), &expected),
        Err(CookPublishError::InvalidPathRole { .. })
    ));
    Ok(())
}

#[test]
fn unpublished_tree_rejects_byte_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("tree-byte-mismatch")?;
    let expected = expected_tree();
    write_expected(fixture.path(), &expected)?;
    write_file(
        fixture.path(),
        "Content/10000000-0000-4000-8000-000000000001.mesh.ron",
        b"changed",
    )?;

    assert!(matches!(
        verify_unpublished_tree(fixture.path(), &expected),
        Err(CookPublishError::ProductRead { .. })
    ));
    Ok(())
}

#[test]
fn existing_generation_rejects_missing_role_without_commit()
-> Result<(), Box<dyn std::error::Error>> {
    assert_existing_generation_failure(
        "existing-missing",
        |path| {
            fs::create_dir(path)?;
            Ok(())
        },
        |error| matches!(error, CookPublishError::MissingPath { .. }),
    )
}

#[test]
fn existing_generation_rejects_extra_role_without_commit() -> Result<(), Box<dyn std::error::Error>>
{
    assert_existing_generation_failure(
        "existing-extra",
        |path| {
            write_file(path, "Scenes/entry.runtime-scene.ron", RUNTIME_SCENE)?;
            write_file(path, "extra", b"extra")
        },
        |error| matches!(error, CookPublishError::UnexpectedPath { .. }),
    )
}

#[cfg(unix)]
#[test]
fn existing_generation_rejects_symlink_without_commit() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    assert_existing_generation_failure(
        "existing-symlink",
        |path| {
            let target = path.with_extension("target");
            fs::create_dir(&target)?;
            symlink(target, path)
        },
        |error| matches!(error, CookPublishError::InvalidPathRole { .. }),
    )
}

#[test]
fn existing_generation_rejects_byte_mismatch_without_commit()
-> Result<(), Box<dyn std::error::Error>> {
    assert_existing_generation_failure(
        "existing-bytes",
        |path| write_file(path, "Scenes/entry.runtime-scene.ron", b"changed"),
        |error| matches!(error, CookPublishError::ProductRead { .. }),
    )
}

#[test]
fn digest_valid_corrupt_mesh_fails_store_barrier_before_commit()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("corrupt-mesh")?;
    let output = CookOutputRoot::open(fixture.path())?;
    let (registry, world) = runtime_types()?;
    let asset = asset_id()?;
    let product_bytes = BTreeMap::from([(asset, b"not ron".to_vec())]);
    let catalog = RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        vec![RuntimeAssetRecord::new(
            asset,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            RuntimeProductPath::new(format!("Content/{asset}.mesh.ron"))?,
            Vec::new(),
        )?],
        RUNTIME_SCENE,
        &product_bytes,
    )?;
    let commit_calls = AtomicUsize::new(0);

    let error = publish_with_commit(
        &output,
        &catalog,
        RUNTIME_SCENE,
        &product_bytes,
        &registry,
        &world,
        |_, _| {
            commit_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
    )
    .expect_err("digest-valid corrupt mesh reached catalog commit");

    assert!(matches!(
        error,
        CookPublishError::Store(RuntimeAssetStoreError::MeshDecode { .. })
    ));
    assert_eq!(commit_calls.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn commit_error_may_leave_complete_old_catalog() -> Result<(), Box<dyn std::error::Error>> {
    assert_commit_error_visibility(false)
}

#[test]
fn commit_error_may_leave_complete_new_catalog() -> Result<(), Box<dyn std::error::Error>> {
    assert_commit_error_visibility(true)
}

#[test]
fn successful_commit_is_called_once_and_is_the_last_fallible_step()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("commit-success")?;
    let output = CookOutputRoot::open(fixture.path())?;
    let (registry, world) = runtime_types()?;
    let products = BTreeMap::new();
    let catalog = catalog(&products)?;
    let calls = AtomicUsize::new(0);

    publish_with_commit(
        &output,
        &catalog,
        RUNTIME_SCENE,
        &products,
        &registry,
        &world,
        |path, bytes| {
            calls.fetch_add(1, Ordering::Relaxed);
            fs::write(path.join("runtime_catalog.ron"), bytes).map_err(|source| {
                CookPublishError::CatalogCommit {
                    path: path.join("runtime_catalog.ron"),
                    source,
                }
            })
        },
    )?;

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        fs::read(fixture.path().join("runtime_catalog.ron"))?,
        catalog.to_ron()?.into_bytes()
    );
    Ok(())
}

fn assert_commit_error_visibility(write_new: bool) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(if write_new {
        "commit-error-new"
    } else {
        "commit-error-old"
    })?;
    let catalog_path = fixture.path().join("runtime_catalog.ron");
    let old_products = BTreeMap::new();
    let old_catalog = catalog_for(OLD_RUNTIME_SCENE, &old_products)?;
    seed_generation(
        fixture.path(),
        &old_catalog,
        OLD_RUNTIME_SCENE,
        &old_products,
    )?;
    let old_catalog_bytes = old_catalog.to_ron()?.into_bytes();
    fs::write(&catalog_path, &old_catalog_bytes)?;
    let output = CookOutputRoot::open(fixture.path())?;
    let (registry, world) = runtime_types()?;
    let products = BTreeMap::new();
    let catalog = catalog(&products)?;
    let new_catalog = catalog.to_ron()?.into_bytes();

    let error = publish_with_commit(
        &output,
        &catalog,
        RUNTIME_SCENE,
        &products,
        &registry,
        &world,
        |root, bytes| {
            let path = root.join("runtime_catalog.ron");
            if write_new {
                fs::write(&path, bytes).map_err(|source| CookPublishError::CatalogCommit {
                    path: path.clone(),
                    source,
                })?;
            }
            Err(CookPublishError::CatalogCommit {
                path,
                source: io::Error::other("injected final commit failure"),
            })
        },
    )
    .expect_err("injected commit failure was reported as success");

    assert!(matches!(error, CookPublishError::CatalogCommit { .. }));
    let visible = fs::read(catalog_path)?;
    assert!(visible == old_catalog_bytes || visible == new_catalog);
    assert_eq!(
        visible,
        if write_new {
            new_catalog.clone()
        } else {
            old_catalog_bytes
        }
    );
    let loaded = RuntimeContentRoot::open(fixture.path())?.load_current("demo.game")?;
    assert_eq!(
        loaded.catalog().generation(),
        if write_new {
            catalog.generation()
        } else {
            old_catalog.generation()
        }
    );
    Ok(())
}

fn assert_existing_generation_failure(
    name: &str,
    setup: impl FnOnce(&Path) -> Result<(), io::Error>,
    expected: impl FnOnce(&CookPublishError) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(name)?;
    let output = CookOutputRoot::open(fixture.path())?;
    let (registry, world) = runtime_types()?;
    let products = BTreeMap::new();
    let catalog = catalog(&products)?;
    let generations = fixture.path().join("generations");
    fs::create_dir(&generations)?;
    let existing = generations.join(catalog.generation().as_str());
    setup(&existing)?;
    let commit_calls = AtomicUsize::new(0);

    let error = publish_with_commit(
        &output,
        &catalog,
        RUNTIME_SCENE,
        &products,
        &registry,
        &world,
        |_, _| {
            commit_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
    )
    .expect_err("invalid existing generation reached commit");

    assert!(expected(&error), "unexpected error: {error:?}");
    assert_eq!(commit_calls.load(Ordering::Relaxed), 0);
    Ok(())
}

fn expected_tree() -> BTreeMap<PathBuf, Vec<u8>> {
    BTreeMap::from([
        (
            PathBuf::from("Content/10000000-0000-4000-8000-000000000001.mesh.ron"),
            b"mesh bytes".to_vec(),
        ),
        (
            PathBuf::from("Scenes/entry.runtime-scene.ron"),
            RUNTIME_SCENE.to_vec(),
        ),
    ])
}

fn write_expected(root: &Path, expected: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), io::Error> {
    for (path, bytes) in expected {
        write_file(root, path, bytes)?;
    }
    Ok(())
}

fn write_file(root: &Path, relative: impl AsRef<Path>, bytes: &[u8]) -> Result<(), io::Error> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

fn asset_id() -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok(AssetId::from_str("10000000-0000-4000-8000-000000000001")?)
}

fn catalog(
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> Result<RuntimeAssetCatalog, Box<dyn std::error::Error>> {
    catalog_for(RUNTIME_SCENE, product_bytes)
}

fn catalog_for(
    scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> Result<RuntimeAssetCatalog, Box<dyn std::error::Error>> {
    Ok(RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        Vec::new(),
        scene_bytes,
        product_bytes,
    )?)
}

fn seed_generation(
    root: &Path,
    catalog: &RuntimeAssetCatalog,
    scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> Result<(), io::Error> {
    let generation = root.join("generations").join(catalog.generation().as_str());
    write_file(&generation, catalog.entry_scene().as_str(), scene_bytes)?;
    for record in catalog.assets() {
        if let Some(bytes) = product_bytes.get(record.id()) {
            write_file(&generation, record.product().as_str(), bytes)?;
        }
    }
    Ok(())
}

fn runtime_types() -> Result<(TypeRegistry, World), Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.freeze()?;
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    world.finish_registration();
    Ok((registry, world))
}

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, io::Error> {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_asset_pipeline_publish")
            .join(format!("{name}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

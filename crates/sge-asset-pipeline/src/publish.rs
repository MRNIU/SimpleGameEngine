// Copyright The SimpleGameEngine Contributors

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sge_asset::{
    AssetId, RuntimeAssetCatalog, RuntimeAssetStore, RuntimeGeneration, RuntimeProductPath,
};
use sge_ecs::World;
use sge_reflect::TypeRegistry;
use sge_scene::{RuntimeScene, preflight_instantiation, prepare_runtime};

use crate::output::{CookOutputRoot, CookPublishError};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn publish(
    output: &CookOutputRoot,
    catalog: &RuntimeAssetCatalog,
    entry_scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
    registry: &TypeRegistry,
    world: &World,
) -> Result<(), CookPublishError> {
    let generations = output.path().join("generations");
    let generations_created = ensure_directory(&generations)?;
    let _generation_directory = CreatedDirectory::new(generations.clone(), generations_created);
    let temp_path = generations.join(format!(
        ".unpublished-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&temp_path).map_err(|source| CookPublishError::TempCreate {
        path: temp_path.clone(),
        source,
    })?;
    let mut temp = TempGeneration::new(temp_path);

    let expected = expected_files(catalog, entry_scene_bytes, product_bytes);
    write_files(temp.path(), &expected)?;
    let readback = read_exact_tree(temp.path(), &expected)?;
    let entry_relative = runtime_path(catalog.entry_scene());
    let readback_scene =
        readback
            .get(&entry_relative)
            .cloned()
            .ok_or_else(|| CookPublishError::MissingPath {
                path: entry_relative.clone(),
            })?;
    let readback_products = catalog
        .assets()
        .iter()
        .map(|record| {
            let path = runtime_path(record.product());
            let bytes = readback
                .get(&path)
                .cloned()
                .ok_or(CookPublishError::MissingPath { path })?;
            Ok((*record.id(), bytes))
        })
        .collect::<Result<BTreeMap<_, _>, CookPublishError>>()?;

    let generation =
        RuntimeGeneration::verify_owned(catalog.clone(), readback_scene, readback_products)
            .map_err(CookPublishError::GenerationVerify)?;
    let store = RuntimeAssetStore::load(&generation).map_err(CookPublishError::Store)?;
    let scene_text =
        std::str::from_utf8(generation.entry_scene_bytes()).map_err(CookPublishError::SceneText)?;
    let scene = RuntimeScene::from_ron(scene_text).map_err(CookPublishError::SceneDecode)?;
    let prepared = prepare_runtime(&scene, registry, &store)
        .map_err(|source| CookPublishError::ScenePrepare(Box::new(source)))?;
    preflight_instantiation(&prepared, world).map_err(CookPublishError::ScenePreflight)?;

    let final_generation = generations.join(catalog.generation().as_str());
    if fs::symlink_metadata(&final_generation).is_ok() {
        return Err(CookPublishError::ExistingGeneration {
            path: final_generation,
        });
    }
    fs::rename(temp.path(), &final_generation).map_err(|source| {
        CookPublishError::GenerationRename {
            from: temp.path().to_path_buf(),
            to: final_generation,
            source,
        }
    })?;
    temp.disarm();

    let catalog_bytes = canonical_catalog(catalog, entry_scene_bytes, product_bytes)?;
    commit_catalog(output.path(), &catalog_bytes)?;
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<bool, CookPublishError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(CookPublishError::GenerationDirectory {
                path: path.to_path_buf(),
                source: std::io::Error::other("generation path is not a regular directory"),
            })
        }
        Ok(_) => Ok(false),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)
            .map_err(|source| CookPublishError::GenerationDirectory {
                path: path.to_path_buf(),
                source,
            })
            .map(|()| true),
        Err(source) => Err(CookPublishError::GenerationDirectory {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn expected_files(
    catalog: &RuntimeAssetCatalog,
    entry_scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::from([(
        runtime_path(catalog.entry_scene()),
        entry_scene_bytes.to_vec(),
    )]);
    for record in catalog.assets() {
        if let Some(bytes) = product_bytes.get(record.id()) {
            files.insert(runtime_path(record.product()), bytes.clone());
        }
    }
    files
}

fn write_files(root: &Path, files: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), CookPublishError> {
    for (relative, bytes) in files {
        let path = root.join(relative);
        let parent = path
            .parent()
            .ok_or_else(|| CookPublishError::ProductWrite {
                path: path.clone(),
                source: std::io::Error::other("runtime product has no parent"),
            })?;
        fs::create_dir_all(parent).map_err(|source| CookPublishError::ProductWrite {
            path: parent.to_path_buf(),
            source,
        })?;
        fs::write(&path, bytes)
            .map_err(|source| CookPublishError::ProductWrite { path, source })?;
    }
    Ok(())
}

fn read_exact_tree(
    root: &Path,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, CookPublishError> {
    let mut actual_files = BTreeSet::new();
    let mut actual_directories = BTreeSet::new();
    scan_tree(
        root,
        Path::new(""),
        &mut actual_files,
        &mut actual_directories,
    )?;
    let expected_files = expected.keys().cloned().collect::<BTreeSet<_>>();
    let expected_directories = expected_files
        .iter()
        .flat_map(|path| path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    if let Some(path) = expected_files.difference(&actual_files).next() {
        return Err(CookPublishError::MissingPath { path: path.clone() });
    }
    if let Some(path) = actual_files.difference(&expected_files).next() {
        return Err(CookPublishError::UnexpectedPath { path: path.clone() });
    }
    if let Some(path) = expected_directories.difference(&actual_directories).next() {
        return Err(CookPublishError::MissingPath { path: path.clone() });
    }
    if let Some(path) = actual_directories.difference(&expected_directories).next() {
        return Err(CookPublishError::UnexpectedPath { path: path.clone() });
    }
    expected
        .iter()
        .map(|(relative, expected_bytes)| {
            let path = root.join(relative);
            let bytes = fs::read(&path).map_err(|source| CookPublishError::ProductRead {
                path: path.clone(),
                source,
            })?;
            if &bytes != expected_bytes {
                return Err(CookPublishError::ProductRead {
                    path,
                    source: std::io::Error::other("product readback bytes changed"),
                });
            }
            Ok((relative.clone(), bytes))
        })
        .collect()
}

fn scan_tree(
    directory: &Path,
    relative: &Path,
    files: &mut BTreeSet<PathBuf>,
    directories: &mut BTreeSet<PathBuf>,
) -> Result<(), CookPublishError> {
    for entry in fs::read_dir(directory).map_err(|source| CookPublishError::ProductRead {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CookPublishError::ProductRead {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let child = relative.join(entry.file_name());
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| CookPublishError::ProductRead {
                path: path.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(CookPublishError::InvalidPathRole { path });
        }
        if metadata.is_dir() {
            directories.insert(child.clone());
            scan_tree(&path, &child, files, directories)?;
        } else if metadata.is_file() {
            files.insert(child);
        } else {
            return Err(CookPublishError::InvalidPathRole { path });
        }
    }
    Ok(())
}

fn canonical_catalog(
    catalog: &RuntimeAssetCatalog,
    entry_scene_bytes: &[u8],
    product_bytes: &BTreeMap<AssetId, Vec<u8>>,
) -> Result<Vec<u8>, CookPublishError> {
    let text = catalog.to_ron().map_err(CookPublishError::CatalogEncode)?;
    let reopened = RuntimeAssetCatalog::from_ron(&text).map_err(CookPublishError::CatalogReopen)?;
    if &reopened != catalog {
        return Err(CookPublishError::CatalogChanged);
    }
    reopened
        .verify_generation(entry_scene_bytes, product_bytes)
        .map_err(CookPublishError::CatalogVerify)?;
    Ok(text.into_bytes())
}

fn commit_catalog(root: &Path, bytes: &[u8]) -> Result<(), CookPublishError> {
    let path = root.join("runtime_catalog.ron");
    let mut file = atomic_write_file::AtomicWriteFile::open(&path).map_err(|source| {
        CookPublishError::CatalogCommitOpen {
            path: path.clone(),
            source,
        }
    })?;
    file.write_all(bytes)
        .map_err(|source| CookPublishError::CatalogCommitWrite {
            path: path.clone(),
            source,
        })?;
    file.commit()
        .map_err(|source| CookPublishError::CatalogCommit { path, source })
}

fn runtime_path(path: &RuntimeProductPath) -> PathBuf {
    path.as_str().split('/').collect()
}

struct TempGeneration {
    path: PathBuf,
    armed: bool,
}

struct CreatedDirectory {
    path: PathBuf,
    remove_if_empty: bool,
}

impl CreatedDirectory {
    fn new(path: PathBuf, remove_if_empty: bool) -> Self {
        Self {
            path,
            remove_if_empty,
        }
    }
}

impl Drop for CreatedDirectory {
    fn drop(&mut self) {
        if self.remove_if_empty {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

impl TempGeneration {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempGeneration {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

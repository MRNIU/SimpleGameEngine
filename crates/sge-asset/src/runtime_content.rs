// Copyright The SimpleGameEngine Contributors

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use sge_reflect::{KeyError, TypeKey};

use crate::{AssetId, RuntimeAssetCatalog, RuntimeCatalogError, RuntimeProductPath};

const CATALOG_PATH: &str = "runtime_catalog.ron";

pub struct RuntimeContentRoot {
    root: PathBuf,
}

pub struct RuntimeGeneration {
    catalog: RuntimeAssetCatalog,
    entry_scene_bytes: Vec<u8>,
    product_bytes: BTreeMap<AssetId, Vec<u8>>,
}

impl RuntimeContentRoot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeContentError> {
        let path = path.as_ref();
        let metadata = fs::symlink_metadata(path).map_err(|source| RuntimeContentError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RuntimeContentError::Symlink {
                path: path.to_path_buf(),
            });
        }
        if !metadata.is_dir() {
            return Err(RuntimeContentError::NotDirectory {
                path: path.to_path_buf(),
            });
        }
        let root = fs::canonicalize(path).map_err(|source| RuntimeContentError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self { root })
    }

    pub fn load_current(
        &self,
        expected_game_id: &str,
    ) -> Result<RuntimeGeneration, RuntimeContentError> {
        let catalog_path = self.root.join(CATALOG_PATH);
        let catalog_bytes = read_regular_file(&catalog_path)?;
        let catalog_text = std::str::from_utf8(&catalog_bytes).map_err(|source| {
            RuntimeContentError::CatalogText {
                path: catalog_path.clone(),
                source,
            }
        })?;
        let catalog = RuntimeAssetCatalog::from_ron(catalog_text)
            .map_err(|source| RuntimeContentError::Catalog { source })?;
        let expected = TypeKey::new(expected_game_id.to_owned()).map_err(|source| {
            RuntimeContentError::InvalidExpectedGameId {
                value: expected_game_id.to_owned(),
                source,
            }
        })?;
        if catalog.game_id() != &expected {
            return Err(RuntimeContentError::GameMismatch {
                expected,
                actual: catalog.game_id().clone(),
            });
        }

        validate_root_roles(&self.root)?;

        let generation_dir = self
            .root
            .join("generations")
            .join(catalog.generation().as_str());
        ensure_regular_directory(&self.root.join("generations"))?;
        ensure_regular_directory(&generation_dir)?;

        let entry_relative = runtime_path(catalog.entry_scene());
        let mut expected_files = BTreeSet::from([entry_relative.clone()]);
        let mut expected_directories = parent_directories(&entry_relative);
        let mut product_paths = BTreeMap::new();
        for record in catalog.assets() {
            let relative = runtime_path(record.product());
            expected_directories.extend(parent_directories(&relative));
            expected_files.insert(relative.clone());
            product_paths.insert(*record.id(), relative);
        }

        let mut actual_files = BTreeSet::new();
        let mut actual_directories = BTreeSet::new();
        scan_tree(
            &generation_dir,
            Path::new(""),
            &mut actual_files,
            &mut actual_directories,
        )?;
        if let Some(path) = expected_files.difference(&actual_files).next() {
            return Err(RuntimeContentError::MissingPath { path: path.clone() });
        }
        if let Some(path) = actual_files.difference(&expected_files).next() {
            return Err(RuntimeContentError::UnexpectedPath { path: path.clone() });
        }
        if let Some(path) = expected_directories.difference(&actual_directories).next() {
            return Err(RuntimeContentError::MissingPath { path: path.clone() });
        }
        if let Some(path) = actual_directories.difference(&expected_directories).next() {
            return Err(RuntimeContentError::UnexpectedPath { path: path.clone() });
        }

        let entry_scene_bytes = read_regular_file(&generation_dir.join(&entry_relative))?;
        let product_bytes = product_paths
            .into_iter()
            .map(|(id, path)| Ok((id, read_regular_file(&generation_dir.join(path))?)))
            .collect::<Result<BTreeMap<_, _>, RuntimeContentError>>()?;
        RuntimeGeneration::verify_owned(catalog, entry_scene_bytes, product_bytes)
    }
}

fn validate_root_roles(root: &Path) -> Result<(), RuntimeContentError> {
    let expected = BTreeSet::from([PathBuf::from(CATALOG_PATH), PathBuf::from("generations")]);
    let mut actual = BTreeSet::new();
    let entries = fs::read_dir(root).map_err(|source| RuntimeContentError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RuntimeContentError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| RuntimeContentError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RuntimeContentError::Symlink { path });
        }
        actual.insert(PathBuf::from(entry.file_name()));
    }
    if let Some(path) = expected.difference(&actual).next() {
        return Err(RuntimeContentError::MissingPath { path: path.clone() });
    }
    if let Some(path) = actual.difference(&expected).next() {
        return Err(RuntimeContentError::UnexpectedPath { path: path.clone() });
    }
    Ok(())
}

impl RuntimeGeneration {
    pub fn verify_owned(
        catalog: RuntimeAssetCatalog,
        entry_scene_bytes: Vec<u8>,
        product_bytes: BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<Self, RuntimeContentError> {
        catalog
            .verify_generation(&entry_scene_bytes, &product_bytes)
            .map_err(|source| RuntimeContentError::Catalog { source })?;
        Ok(Self {
            catalog,
            entry_scene_bytes,
            product_bytes,
        })
    }

    #[must_use]
    pub const fn catalog(&self) -> &RuntimeAssetCatalog {
        &self.catalog
    }

    #[must_use]
    pub fn entry_scene_bytes(&self) -> &[u8] {
        &self.entry_scene_bytes
    }

    pub(crate) fn product_bytes(&self, id: &AssetId) -> Option<&[u8]> {
        self.product_bytes.get(id).map(Vec::as_slice)
    }
}

fn runtime_path(path: &RuntimeProductPath) -> PathBuf {
    path.as_str().split('/').collect()
}

fn parent_directories(path: &Path) -> BTreeSet<PathBuf> {
    path.ancestors()
        .skip(1)
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect()
}

fn ensure_regular_directory(path: &Path) -> Result<(), RuntimeContentError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            RuntimeContentError::MissingPath {
                path: path.to_path_buf(),
            }
        } else {
            RuntimeContentError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RuntimeContentError::Symlink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_dir() {
        return Err(RuntimeContentError::NotDirectory {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, RuntimeContentError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            RuntimeContentError::MissingPath {
                path: path.to_path_buf(),
            }
        } else {
            RuntimeContentError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RuntimeContentError::Symlink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_file() {
        return Err(RuntimeContentError::NotFile {
            path: path.to_path_buf(),
        });
    }
    fs::read(path).map_err(|source| RuntimeContentError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn scan_tree(
    directory: &Path,
    relative: &Path,
    files: &mut BTreeSet<PathBuf>,
    directories: &mut BTreeSet<PathBuf>,
) -> Result<(), RuntimeContentError> {
    let entries = fs::read_dir(directory).map_err(|source| RuntimeContentError::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RuntimeContentError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let child_relative = relative.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path).map_err(|source| RuntimeContentError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RuntimeContentError::Symlink { path });
        }
        if metadata.is_dir() {
            directories.insert(child_relative.clone());
            scan_tree(&path, &child_relative, files, directories)?;
        } else if metadata.is_file() {
            files.insert(child_relative);
        } else {
            return Err(RuntimeContentError::NotFile { path });
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeContentError {
    #[error("runtime content IO failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("runtime content path is a symlink: {path}")]
    Symlink { path: PathBuf },
    #[error("runtime content path is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("runtime content path is not a regular file: {path}")]
    NotFile { path: PathBuf },
    #[error("runtime content path is missing: {path}")]
    MissingPath { path: PathBuf },
    #[error("runtime content path is unexpected: {path}")]
    UnexpectedPath { path: PathBuf },
    #[error("runtime catalog is not UTF-8 at {path}: {source}")]
    CatalogText {
        path: PathBuf,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("invalid runtime catalog: {source}")]
    Catalog {
        #[source]
        source: RuntimeCatalogError,
    },
    #[error("invalid expected game ID {value:?}: {source}")]
    InvalidExpectedGameId {
        value: String,
        #[source]
        source: KeyError,
    },
    #[error("runtime game mismatch: expected {expected}, found {actual}")]
    GameMismatch { expected: TypeKey, actual: TypeKey },
}

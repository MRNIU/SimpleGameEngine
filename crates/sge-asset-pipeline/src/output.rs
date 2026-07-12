// Copyright The SimpleGameEngine Contributors

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sge_asset::{RuntimeAssetStoreError, RuntimeCatalogError, RuntimeContentError};
use sge_scene::{RuntimeSceneFormatError, SceneInstantiationError, SceneValidationError};

#[derive(Debug)]
pub struct CookOutputRoot {
    root: PathBuf,
}

impl CookOutputRoot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CookPublishError> {
        let requested = path.as_ref();
        let metadata =
            fs::symlink_metadata(requested).map_err(|source| CookPublishError::RootAccess {
                path: requested.to_path_buf(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(CookPublishError::RootSymlink {
                path: requested.to_path_buf(),
            });
        }
        if !metadata.is_dir() {
            return Err(CookPublishError::RootNotDirectory {
                path: requested.to_path_buf(),
            });
        }
        let root = fs::canonicalize(requested).map_err(|source| CookPublishError::RootAccess {
            path: requested.to_path_buf(),
            source,
        })?;
        Ok(Self { root })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CookPublishError {
    #[error("cannot access Cook output root {path}: {source}")]
    RootAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cook output root must not be a symlink: {path}")]
    RootSymlink { path: PathBuf },
    #[error("Cook output root is not a directory: {path}")]
    RootNotDirectory { path: PathBuf },
    #[error("cannot prepare generation directory {path}: {source}")]
    GenerationDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot create unpublished generation {path}: {source}")]
    TempCreate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot write unpublished product {path}: {source}")]
    ProductWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot read runtime generation product {path}: {source}")]
    ProductRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("runtime generation contains unexpected path {path}")]
    UnexpectedPath { path: PathBuf },
    #[error("runtime generation is missing path {path}")]
    MissingPath { path: PathBuf },
    #[error("runtime generation path is a symlink or unsupported file: {path}")]
    InvalidPathRole { path: PathBuf },
    #[error("cannot verify unpublished runtime generation: {0}")]
    GenerationVerify(#[source] RuntimeContentError),
    #[error("cannot load unpublished runtime asset store: {0}")]
    Store(#[source] RuntimeAssetStoreError),
    #[error("unpublished runtime scene is not UTF-8: {0}")]
    SceneText(#[source] std::str::Utf8Error),
    #[error("cannot decode unpublished runtime scene: {0}")]
    SceneDecode(#[source] RuntimeSceneFormatError),
    #[error("cannot prepare unpublished runtime scene: {0}")]
    ScenePrepare(#[source] Box<SceneValidationError>),
    #[error("unpublished runtime scene cannot instantiate in the candidate World: {0}")]
    ScenePreflight(#[source] SceneInstantiationError),
    #[error("cannot publish immutable generation from {from} to {to}: {source}")]
    GenerationRename {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot encode runtime catalog: {0}")]
    CatalogEncode(#[source] RuntimeCatalogError),
    #[error("cannot reopen runtime catalog: {0}")]
    CatalogReopen(#[source] RuntimeCatalogError),
    #[error("reopened runtime catalog differs from the validated catalog")]
    CatalogChanged,
    #[error("cannot verify reopened runtime catalog: {0}")]
    CatalogVerify(#[source] RuntimeCatalogError),
    #[error("cannot open atomic runtime catalog commit at {path}: {source}")]
    CatalogCommitOpen {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot write atomic runtime catalog commit at {path}: {source}")]
    CatalogCommitWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot commit runtime catalog at {path}: {source}")]
    CatalogCommit {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

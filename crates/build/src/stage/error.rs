// Copyright The SimpleGameEngine Contributors

use std::{collections::BTreeSet, io, path::PathBuf};

use sge_asset::{RuntimeAssetStoreError, RuntimeContentError, RuntimeGenerationId};

use crate::StageManifestError;

#[derive(Debug, thiserror::Error)]
pub enum StageRootError {
    #[error("cannot create Stage root {path}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot access Stage root {path}: {source}")]
    Access {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Stage root is not a regular directory: {0}")]
    NotRegular(PathBuf),
    #[error("Stage root contains an unexpected path: {0}")]
    UnexpectedPath(PathBuf),
    #[error("cannot create Stage generations directory {path}: {source}")]
    GenerationsCreate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot access Stage generations directory {path}: {source}")]
    GenerationsAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Stage generations path is not a regular directory: {0}")]
    GenerationsNotRegular(PathBuf),
    #[error("cannot read Stage manifest {path}: {source}")]
    ManifestRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Stage manifest is not UTF-8: {0}")]
    ManifestText(#[source] std::str::Utf8Error),
    #[error(transparent)]
    Manifest(#[from] StageManifestError),
    #[error("Stage game ID mismatch: expected {expected}, found {actual}")]
    GameMismatch { expected: String, actual: String },
    #[error("cannot verify current Stage generation: {0}")]
    Verify(#[source] Box<StagePublishError>),
}

#[derive(Debug, thiserror::Error)]
pub enum StagePublishError {
    #[error(transparent)]
    Root(#[from] StageRootError),
    #[error(transparent)]
    Manifest(#[from] StageManifestError),
    #[error("cannot create unpublished Stage generation {path}: {source}")]
    TempCreate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot remove unpublished Stage generation {path}: {source}")]
    TempRemove {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot access Player executable {path}: {source}")]
    ExecutableAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Player executable has no UTF-8 leaf name: {0}")]
    ExecutableName(PathBuf),
    #[error("cannot copy Player executable from {from} to {to}: {source}")]
    ExecutableCopy {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot read back Player executable {path}: {source}")]
    ExecutableReadback {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Player executable changed during Stage publication")]
    ExecutableChanged,
    #[error(transparent)]
    RuntimeContent(#[from] RuntimeContentError),
    #[error(transparent)]
    RuntimeStore(#[from] RuntimeAssetStoreError),
    #[error("runtime generation mismatch: expected {expected}, found {actual}")]
    RuntimeGenerationMismatch {
        expected: RuntimeGenerationId,
        actual: RuntimeGenerationId,
    },
    #[error("cannot scan Stage generation {path}: {source}")]
    GenerationScan {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unexpected Stage generation roles: expected {expected:?}, found {actual:?}")]
    UnexpectedGenerationRoles {
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
    #[error("Stage manifest executable path has no leaf")]
    ManifestExecutableName,
    #[error("Stage generation does not match its manifest")]
    GenerationManifestMismatch,
    #[error("Stage generation is not a regular directory: {0}")]
    GenerationNotRegular(PathBuf),
    #[error("cannot access Stage generation {path}: {source}")]
    GenerationAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot publish immutable Stage generation from {from} to {to}: {source}")]
    GenerationRename {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("encoded Stage manifest is not UTF-8: {0}")]
    ManifestText(#[source] std::str::Utf8Error),
    #[error("reopened Stage manifest differs from the validated manifest")]
    ManifestChanged,
    #[error("Stage manifest path is a symlink: {0}")]
    ManifestSymlink(PathBuf),
    #[error("cannot open atomic Stage manifest commit at {path}: {source}")]
    ManifestCommitOpen {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot write atomic Stage manifest commit at {path}: {source}")]
    ManifestCommitWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot commit Stage manifest at {path}: {source}")]
    ManifestCommit {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

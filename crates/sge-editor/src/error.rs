// Copyright The SimpleGameEngine Contributors

use sge_app::{EngineBuildError, InitializationError};
use sge_asset_pipeline::{ObjImportError, ProjectAssetImportError};
use sge_project::{ManifestError, ProjectFormatError, ProjectIoError, ProjectPathError};
use sge_reflect::{KeyError, ReflectError};
use sge_render::{RenderExtractionError, RenderViewError};
use sge_scene::{
    SceneEntityId, SceneFormatError, SceneInstantiationError, SceneSnapshotError,
    SceneValidationError,
};

#[derive(Debug, thiserror::Error)]
pub enum EditorOpenError {
    #[error(transparent)]
    Project(#[from] ProjectIoError),
    #[error(transparent)]
    Descriptor(#[from] ProjectFormatError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Import(#[from] ProjectAssetImportError),
    #[error("authoring scene is not UTF-8: {0}")]
    SceneText(#[from] std::str::Utf8Error),
    #[error(transparent)]
    SceneFormat(#[from] SceneFormatError),
    #[error(transparent)]
    App(#[from] EngineBuildError),
    #[error("authoring scene validation failed: {0}")]
    SceneValidation(#[from] Box<SceneValidationError>),
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    Instantiation(#[from] SceneInstantiationError),
}

impl From<SceneValidationError> for EditorOpenError {
    fn from(source: SceneValidationError) -> Self {
        Self::SceneValidation(Box::new(source))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EditorPreviewError {
    #[error(transparent)]
    Extraction(#[from] RenderExtractionError),
    #[error(transparent)]
    View(#[from] RenderViewError),
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("scene entity does not exist: {entity}")]
    MissingEntity { entity: SceneEntityId },
    #[error("scene entity {entity} has no component {component}")]
    MissingComponent {
        entity: SceneEntityId,
        component: String,
    },
    #[error("scene entity already exists: {entity}")]
    DuplicateEntity { entity: SceneEntityId },
    #[error("scene entity {entity} already has component {component}")]
    DuplicateComponent {
        entity: SceneEntityId,
        component: String,
    },
    #[error("scene entity has children and cannot be removed: {entity}")]
    EntityHasChildren { entity: SceneEntityId },
    #[error("there is no command to undo")]
    NothingToUndo,
    #[error("there is no command to redo")]
    NothingToRedo,
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    Reflect(#[from] ReflectError),
    #[error(transparent)]
    Snapshot(#[from] SceneSnapshotError),
    #[error("candidate scene validation failed: {0}")]
    Validation(#[from] Box<SceneValidationError>),
    #[error(transparent)]
    App(#[from] EngineBuildError),
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    Instantiation(#[from] SceneInstantiationError),
    #[error(transparent)]
    SceneFormat(#[from] SceneFormatError),
    #[error("authoring scene is not UTF-8: {0}")]
    SceneText(#[from] std::str::Utf8Error),
    #[error(transparent)]
    Project(#[from] ProjectIoError),
    #[error(transparent)]
    ProjectPath(#[from] ProjectPathError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("cannot read source asset {path:?}: {source}")]
    SourceRead {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Import(#[from] ProjectAssetImportError),
    #[error(transparent)]
    ObjImport(#[from] ObjImportError),
    #[error(
        "asset import failed and source rollback also failed for {path}: operation: {operation}; rollback: {rollback}"
    )]
    AssetImportRollback {
        path: sge_project::ProjectPath,
        #[source]
        operation: Box<EditError>,
        rollback: ProjectIoError,
    },
}

impl From<SceneValidationError> for EditError {
    fn from(source: SceneValidationError) -> Self {
        Self::Validation(Box::new(source))
    }
}

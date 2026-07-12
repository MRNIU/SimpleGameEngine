// Copyright The SimpleGameEngine Contributors
//
//! Strict authoring scene data and validation.

mod document;
mod id;
mod runtime;
mod snapshot;
mod transfer;
mod validation;

pub use document::{
    AUTHORING_SCENE_FORMAT_VERSION, AuthoringEntity, AuthoringScene, SceneFormatError,
};
pub use id::{
    Parent, SceneEntityId, SceneEntityIdError, parent_descriptor, scene_entity_id_descriptor,
};
pub use runtime::{
    RUNTIME_SCENE_FORMAT_VERSION, RuntimeEntity, RuntimeScene, RuntimeSceneBuild,
    RuntimeSceneBuildError, RuntimeSceneFormatError, build_runtime_scene, prepare_runtime,
};
pub use snapshot::{SceneSnapshotError, snapshot};
pub use transfer::{SceneInstance, SceneInstantiationError, instantiate, preflight_instantiation};
pub use validation::{PreparedScene, SceneValidationError, prepare};

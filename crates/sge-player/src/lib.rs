// Copyright The SimpleGameEngine Contributors

//! Source-free runtime session ownership and frame extraction.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use sge_app::{AdvanceError, EngineApp, EngineBuildError, GameDescriptor, InitializationError};
use sge_asset::{
    RuntimeAssetStore, RuntimeAssetStoreError, RuntimeContentError, RuntimeContentRoot,
};
use sge_input::InputFrame;
use sge_render::{RenderExtractionError, RenderSnapshot, RenderView, RenderViewError, extract};
use sge_scene::{
    RuntimeScene, RuntimeSceneFormatError, SceneInstantiationError, SceneValidationError,
    instantiate, prepare_runtime,
};

mod host;
mod input;

pub use host::{PlayerRunError, RunOptions, RunReport, run, run_session};
pub use sge_render::RenderBackend;

pub fn staged_runtime_root() -> Result<PathBuf, StagedRuntimeRootError> {
    let executable = std::env::current_exe().map_err(StagedRuntimeRootError::CurrentExecutable)?;
    runtime_root_for_executable(&executable)
}

pub fn runtime_root_for_executable(executable: &Path) -> Result<PathBuf, StagedRuntimeRootError> {
    executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.join("runtime"))
        .ok_or_else(|| StagedRuntimeRootError::MissingParent(executable.to_path_buf()))
}

#[derive(Debug, thiserror::Error)]
pub enum StagedRuntimeRootError {
    #[error("cannot locate the current Player executable: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("Player executable has no parent directory: {0}")]
    MissingParent(PathBuf),
}

pub struct PlayerSession {
    game_id: &'static str,
    app: EngineApp,
    assets: RuntimeAssetStore,
}

impl PlayerSession {
    pub fn load(
        game: GameDescriptor,
        cooked_root: impl AsRef<Path>,
    ) -> Result<Self, PlayerLoadError> {
        let content = RuntimeContentRoot::open(cooked_root)?;
        let generation = content.load_current(game.game_id())?;
        let assets = RuntimeAssetStore::load(&generation)?;
        let scene_text = std::str::from_utf8(generation.entry_scene_bytes())?;
        let scene = RuntimeScene::from_ron(scene_text)?;
        let mut app = game.create_app()?;
        let prepared = prepare_runtime(&scene, app.type_registry(), &assets)?;
        instantiate(prepared, app.world_initializer()?)?;
        Ok(Self {
            game_id: game.game_id(),
            app,
            assets,
        })
    }

    pub fn advance(&mut self, delta: Duration, input: InputFrame) -> Result<(), AdvanceError> {
        self.app.advance(delta, input)
    }

    pub fn render_frame(&self) -> Result<(RenderSnapshot, RenderView), PlayerFrameError> {
        let snapshot = extract(self.app.world(), &self.assets)?;
        let view = RenderView::from_active_camera(&snapshot)?;
        Ok((snapshot, view))
    }

    #[must_use]
    pub const fn game_id(&self) -> &'static str {
        self.game_id
    }

    #[must_use]
    pub const fn assets(&self) -> &RuntimeAssetStore {
        &self.assets
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerLoadError {
    #[error(transparent)]
    Content(#[from] RuntimeContentError),
    #[error(transparent)]
    Assets(#[from] RuntimeAssetStoreError),
    #[error("runtime entry scene is not UTF-8: {0}")]
    SceneText(#[from] std::str::Utf8Error),
    #[error(transparent)]
    SceneFormat(#[from] RuntimeSceneFormatError),
    #[error(transparent)]
    App(#[from] EngineBuildError),
    #[error(transparent)]
    SceneValidation(Box<SceneValidationError>),
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    Instantiation(#[from] SceneInstantiationError),
}

impl From<SceneValidationError> for PlayerLoadError {
    fn from(source: SceneValidationError) -> Self {
        Self::SceneValidation(Box::new(source))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerFrameError {
    #[error(transparent)]
    Extraction(#[from] RenderExtractionError),
    #[error(transparent)]
    View(#[from] RenderViewError),
}

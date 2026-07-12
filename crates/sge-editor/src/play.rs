// Copyright The SimpleGameEngine Contributors

use std::{sync::Arc, time::Duration};

use sge_app::{AdvanceError, EngineApp, EngineBuildError, InitializationError};
use sge_asset::RuntimeAssetStore;
use sge_input::InputFrame;
use sge_render::{RenderSnapshot, RenderView, extract};
use sge_scene::{
    RuntimeSceneBuildError, SceneEntityId, SceneInstance, SceneInstantiationError,
    SceneValidationError, build_runtime_scene, instantiate, prepare_runtime,
};

use crate::{EditError, EditSession, EditorPreviewError, PreviewFrame};

pub struct PlaySession {
    game_id: &'static str,
    app: EngineApp,
    instance: SceneInstance,
    assets: Arc<RuntimeAssetStore>,
}

impl PlaySession {
    pub(crate) fn start(edit: &EditSession) -> Result<Self, PlayStartError> {
        let authoring = edit.snapshot()?;
        let mut app = edit.game().create_app()?;
        let runtime = build_runtime_scene(&authoring, app.type_registry(), edit.assets())?;
        let prepared = prepare_runtime(runtime.scene(), app.type_registry(), edit.assets())?;
        let instance = instantiate(prepared, app.world_initializer()?)?;
        Ok(Self {
            game_id: edit.game().game_id(),
            app,
            instance,
            assets: Arc::clone(edit.assets_arc()),
        })
    }

    pub fn advance(&mut self, delta: Duration, input: InputFrame) -> Result<(), AdvanceError> {
        self.app.advance(delta, input)
    }

    pub fn render_frame(&self) -> Result<(RenderSnapshot, RenderView), EditorPreviewError> {
        let snapshot = extract(self.app.world(), self.assets.as_ref())?;
        let view = RenderView::from_active_camera(&snapshot)?;
        Ok((snapshot, view))
    }

    pub(crate) fn preview_frame(&self) -> Result<PreviewFrame, EditorPreviewError> {
        let (snapshot, view) = self.render_frame()?;
        Ok(PreviewFrame {
            snapshot,
            view,
            assets: Arc::clone(&self.assets),
        })
    }

    #[must_use]
    pub fn component<T: 'static>(&self, entity: SceneEntityId) -> Option<&T> {
        self.app.world().get(self.instance.entity(&entity)?)
    }

    #[must_use]
    pub fn resource<R: 'static>(&self) -> Option<&R> {
        self.app.world().resource::<R>()
    }

    #[must_use]
    pub const fn game_id(&self) -> &'static str {
        self.game_id
    }

    #[must_use]
    pub fn assets(&self) -> &RuntimeAssetStore {
        self.assets.as_ref()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlayStartError {
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error(transparent)]
    App(#[from] EngineBuildError),
    #[error(transparent)]
    RuntimeScene(#[from] RuntimeSceneBuildError),
    #[error("Play scene validation failed: {0}")]
    Validation(#[from] Box<SceneValidationError>),
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    Instantiation(#[from] SceneInstantiationError),
}

impl From<SceneValidationError> for PlayStartError {
    fn from(source: SceneValidationError) -> Self {
        Self::Validation(Box::new(source))
    }
}

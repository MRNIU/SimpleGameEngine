// Copyright The SimpleGameEngine Contributors

use std::{path::Path, sync::Arc};

use sge_app::{EngineApp, EngineBuildError, GameDescriptor, InitializationError};
use sge_asset::RuntimeAssetStore;
use sge_asset_pipeline::{ProjectAssetImportError, import_project_assets};
use sge_project::{
    AuthoringAssetManifest, ManifestError, ProjectDescriptor, ProjectFormatError, ProjectIoError,
    ProjectRoot,
};
use sge_render::{RenderExtractionError, RenderSnapshot, RenderView, RenderViewError, extract};
use sge_scene::{
    AuthoringScene, SceneFormatError, SceneInstantiationError, SceneValidationError, instantiate,
    prepare,
};

pub struct EditorSession {
    project: ProjectRoot,
    descriptor: ProjectDescriptor,
    manifest: AuthoringAssetManifest,
    scene: AuthoringScene,
    app: EngineApp,
    assets: Arc<RuntimeAssetStore>,
}

#[derive(Clone)]
pub struct PreviewFrame {
    pub snapshot: RenderSnapshot,
    pub view: RenderView,
    pub assets: Arc<RuntimeAssetStore>,
}

#[derive(Default)]
pub struct EditorWorkspace {
    live: Option<EditorSession>,
}

impl EditorSession {
    pub fn open(
        game: GameDescriptor,
        project_root: impl AsRef<Path>,
    ) -> Result<Self, EditorOpenError> {
        let project = ProjectRoot::open(project_root)?;
        let descriptor = ProjectDescriptor::load(&project)?;
        descriptor.validate_for_game(game.game_id())?;
        let manifest = AuthoringAssetManifest::load(&project)?;
        let imported = import_project_assets(&project, &manifest)?;
        let assets = Arc::new(imported.into_parts().0);
        let scene_bytes = project.read(descriptor.default_authoring_scene())?;
        let scene_text = std::str::from_utf8(&scene_bytes)?;
        let scene = AuthoringScene::from_ron(scene_text)?;
        let mut app = game.create_app()?;
        let prepared = prepare(&scene, app.type_registry(), assets.as_ref())?;
        instantiate(prepared, app.world_initializer()?)?;
        let session = Self {
            project,
            descriptor,
            manifest,
            scene,
            app,
            assets,
        };
        let _ = session.preview_frame()?;
        Ok(session)
    }

    pub fn preview_frame(&self) -> Result<PreviewFrame, EditorOpenError> {
        let snapshot = extract(self.app.world(), self.assets.as_ref())?;
        let view = RenderView::from_active_camera(&snapshot)?;
        Ok(PreviewFrame {
            snapshot,
            view,
            assets: Arc::clone(&self.assets),
        })
    }

    #[must_use]
    pub const fn descriptor(&self) -> &ProjectDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn manifest(&self) -> &AuthoringAssetManifest {
        &self.manifest
    }

    #[must_use]
    pub const fn scene(&self) -> &AuthoringScene {
        &self.scene
    }

    #[must_use]
    pub const fn project(&self) -> &ProjectRoot {
        &self.project
    }
}

impl EditorWorkspace {
    pub fn replace(
        &mut self,
        game: GameDescriptor,
        project_root: impl AsRef<Path>,
    ) -> Result<(), EditorOpenError> {
        let candidate = EditorSession::open(game, project_root)?;
        self.live = Some(candidate);
        Ok(())
    }

    #[must_use]
    pub const fn live(&self) -> Option<&EditorSession> {
        self.live.as_ref()
    }
}

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
    #[error(transparent)]
    Extraction(#[from] RenderExtractionError),
    #[error(transparent)]
    View(#[from] RenderViewError),
}

impl From<SceneValidationError> for EditorOpenError {
    fn from(source: SceneValidationError) -> Self {
        Self::SceneValidation(Box::new(source))
    }
}

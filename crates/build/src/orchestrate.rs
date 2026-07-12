// Copyright The SimpleGameEngine Contributors

use std::path::PathBuf;

use sge_app::{EngineBuildError, GameDescriptor};
use sge_asset_pipeline::{CookError, CookOutputRoot, CookPublishError, CookReport, full_cook};
use sge_project::{ProjectDescriptor, ProjectFormatError, ProjectIoError, ProjectRoot};

use crate::{
    BuildProfile, CargoBuildError, CargoTool, StageManifest, StagePublishError,
    StagePublishRequest, StageRoot, StageRootError,
};

pub struct BuildRequest {
    project: PathBuf,
    workspace: PathBuf,
    stage: PathBuf,
    target_dir: PathBuf,
    profile: BuildProfile,
    cargo_program: PathBuf,
}

impl BuildRequest {
    #[must_use]
    pub fn new(
        project: impl Into<PathBuf>,
        workspace: impl Into<PathBuf>,
        stage: impl Into<PathBuf>,
        target_dir: impl Into<PathBuf>,
        profile: BuildProfile,
    ) -> Self {
        Self {
            project: project.into(),
            workspace: workspace.into(),
            stage: stage.into(),
            target_dir: target_dir.into(),
            profile,
            cargo_program: PathBuf::from("cargo"),
        }
    }

    #[must_use]
    pub fn with_cargo_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.cargo_program = program.into();
        self
    }
}

pub struct BuildReport {
    cook: CookReport,
    stage: StageManifest,
}

impl BuildReport {
    #[must_use]
    pub const fn cook(&self) -> &CookReport {
        &self.cook
    }

    #[must_use]
    pub const fn stage(&self) -> &StageManifest {
        &self.stage
    }
}

pub fn build(
    game: GameDescriptor,
    expected_build_package: &str,
    request: &BuildRequest,
) -> Result<BuildReport, BuildError> {
    let project = ProjectRoot::open(&request.project)?;
    let descriptor = ProjectDescriptor::load(&project)?;
    descriptor.validate_for_game(game.game_id())?;
    if descriptor.build_package().as_str() != expected_build_package {
        return Err(BuildError::BuildPackageMismatch {
            expected: expected_build_package.to_owned(),
            actual: descriptor.build_package().to_string(),
        });
    }
    let app = game.create_app()?;
    let stage = StageRoot::create(&request.stage)?;
    let unpublished = stage.begin()?;
    let cook_root = CookOutputRoot::open(unpublished.runtime_root())?;
    let cook = full_cook(
        &project,
        game.game_id(),
        app.type_registry(),
        app.world(),
        &cook_root,
    )?;
    let artifact = CargoTool::new(&request.cargo_program).build_player(
        &request.workspace,
        &request.target_dir,
        descriptor.player_package().as_str(),
        request.profile,
    )?;
    let staged = unpublished.publish(StagePublishRequest::new(
        game.game_id(),
        descriptor.player_package().as_str(),
        request.profile,
        artifact,
        cook.generation().clone(),
    ))?;
    Ok(BuildReport {
        cook,
        stage: staged,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("cannot open project root: {0}")]
    Project(#[from] ProjectIoError),
    #[error("cannot load or validate project descriptor: {0}")]
    Descriptor(#[from] ProjectFormatError),
    #[error("project build package mismatch: expected {expected}, found {actual}")]
    BuildPackageMismatch { expected: String, actual: String },
    #[error("cannot create game app for Build: {0}")]
    App(#[from] EngineBuildError),
    #[error("cannot open Stage root: {0}")]
    StageRoot(#[from] StageRootError),
    #[error("cannot open unpublished Cook output: {0}")]
    CookRoot(#[from] CookPublishError),
    #[error("full Cook failed: {0}")]
    Cook(#[from] CookError),
    #[error("Cargo Player build failed: {0}")]
    Cargo(#[from] CargoBuildError),
    #[error("Stage publication failed: {0}")]
    StagePublish(#[from] StagePublishError),
}

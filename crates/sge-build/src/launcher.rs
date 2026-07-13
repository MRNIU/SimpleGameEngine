// Copyright The SimpleGameEngine Contributors

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use sge_project::{ProjectBootstrap, ProjectFormatError, ProjectIoError, ProjectRoot};

use crate::BuildProfile;

pub struct BuildLauncher {
    cargo_program: PathBuf,
}

impl BuildLauncher {
    #[must_use]
    pub fn new(cargo_program: impl Into<PathBuf>) -> Self {
        Self {
            cargo_program: cargo_program.into(),
        }
    }

    pub fn run(
        &self,
        project: &Path,
        workspace: &Path,
        stage: &Path,
        target_dir: &Path,
        profile: BuildProfile,
    ) -> Result<(), BuildLaunchError> {
        let project_path =
            fs::canonicalize(project).map_err(|source| BuildLaunchError::ProjectAccess {
                path: project.to_path_buf(),
                source,
            })?;
        let project = ProjectRoot::open(&project_path)?;
        let bootstrap = ProjectBootstrap::load(&project)?;
        let workspace = canonical_workspace(workspace)?;
        let status = Command::new(&self.cargo_program)
            .current_dir(&workspace)
            .args([
                "run",
                "--package",
                bootstrap.build_package().as_str(),
                "--bin",
                bootstrap.build_package().as_str(),
                "--profile",
                profile.as_str(),
                "--target-dir",
            ])
            .arg(target_dir)
            .arg("--")
            .arg("--project")
            .arg(&project_path)
            .arg("--workspace")
            .arg(&workspace)
            .arg("--stage")
            .arg(stage)
            .arg("--target-dir")
            .arg(target_dir)
            .args(["--profile", profile.as_str()])
            .status()
            .map_err(|source| BuildLaunchError::Spawn {
                program: self.cargo_program.clone(),
                source,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(BuildLaunchError::Failed(status.code()))
        }
    }
}

impl Default for BuildLauncher {
    fn default() -> Self {
        Self::new("cargo")
    }
}

fn canonical_workspace(path: &Path) -> Result<PathBuf, BuildLaunchError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| BuildLaunchError::WorkspaceAccess {
            path: path.to_path_buf(),
            source,
        })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BuildLaunchError::WorkspaceNotRegular(path.to_path_buf()));
    }
    let manifest = path.join("Cargo.toml");
    let manifest_metadata =
        fs::symlink_metadata(&manifest).map_err(|source| BuildLaunchError::WorkspaceAccess {
            path: manifest.clone(),
            source,
        })?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(BuildLaunchError::WorkspaceNotRegular(manifest));
    }
    fs::canonicalize(path).map_err(|source| BuildLaunchError::WorkspaceAccess {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum BuildLaunchError {
    #[error("cannot access project root {path}: {source}")]
    ProjectAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Project(#[from] ProjectIoError),
    #[error("cannot load project bootstrap: {0}")]
    Bootstrap(#[from] ProjectFormatError),
    #[error("cannot access Cargo workspace {path}: {source}")]
    WorkspaceAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cargo workspace path is not a regular directory or manifest: {0}")]
    WorkspaceNotRegular(PathBuf),
    #[error("cannot start Cargo program {program}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("game-specific Build target failed with status {0:?}")]
    Failed(Option<i32>),
}

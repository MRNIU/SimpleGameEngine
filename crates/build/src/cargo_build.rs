// Copyright The SimpleGameEngine Contributors

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use sge_project::{PackageName, PackageNameError};

use crate::BuildProfile;

pub struct CargoTool {
    program: PathBuf,
}

impl CargoTool {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    pub fn build_player(
        &self,
        workspace: &Path,
        target_dir: &Path,
        player_package: &str,
        profile: BuildProfile,
    ) -> Result<PathBuf, CargoBuildError> {
        let package = PackageName::new(player_package)?;
        let workspace = canonical_workspace(workspace)?;
        let output = Command::new(&self.program)
            .current_dir(&workspace)
            .args([
                "build",
                "--package",
                package.as_str(),
                "--bin",
                package.as_str(),
                "--profile",
                profile.as_str(),
                "--target-dir",
            ])
            .arg(target_dir)
            .args(["--message-format", "json-render-diagnostics"])
            .output()
            .map_err(|source| CargoBuildError::Spawn {
                program: self.program.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(CargoBuildError::Failed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let artifact = matching_artifact(&output.stdout, package.as_str())?;
        let metadata =
            fs::symlink_metadata(&artifact).map_err(|source| CargoBuildError::ArtifactAccess {
                path: artifact.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CargoBuildError::ArtifactNotRegular(artifact));
        }
        Ok(artifact)
    }
}

impl Default for CargoTool {
    fn default() -> Self {
        Self::new("cargo")
    }
}

fn canonical_workspace(path: &Path) -> Result<PathBuf, CargoBuildError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| CargoBuildError::WorkspaceAccess {
            path: path.to_path_buf(),
            source,
        })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CargoBuildError::WorkspaceNotRegular(path.to_path_buf()));
    }
    let manifest = path.join("Cargo.toml");
    let manifest_metadata =
        fs::symlink_metadata(&manifest).map_err(|source| CargoBuildError::WorkspaceManifest {
            path: manifest.clone(),
            source,
        })?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(CargoBuildError::WorkspaceManifestNotRegular(manifest));
    }
    fs::canonicalize(path).map_err(|source| CargoBuildError::WorkspaceAccess {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    package_id: Option<String>,
    target: Option<CargoTarget>,
    executable: Option<PathBuf>,
}

#[derive(Deserialize)]
struct CargoTarget {
    kind: Vec<String>,
    name: String,
}

fn matching_artifact(output: &[u8], expected: &str) -> Result<PathBuf, CargoBuildError> {
    let text = std::str::from_utf8(output).map_err(CargoBuildError::OutputText)?;
    let mut matches = Vec::new();
    for line in text.lines() {
        let message: CargoMessage =
            serde_json::from_str(line).map_err(|source| CargoBuildError::OutputJson {
                line: line.to_owned(),
                source,
            })?;
        let Some(target) = message.target else {
            continue;
        };
        if message.reason == "compiler-artifact"
            && message
                .package_id
                .as_deref()
                .and_then(package_name)
                .is_some_and(|package| package == expected)
            && target.name == expected
            && target.kind.iter().any(|kind| kind == "bin")
            && let Some(executable) = message.executable
        {
            matches.push(executable);
        }
    }
    match matches.as_slice() {
        [artifact] => Ok(artifact.clone()),
        [] => Err(CargoBuildError::MissingArtifact(expected.to_owned())),
        _ => Err(CargoBuildError::MultipleArtifacts {
            package: expected.to_owned(),
            paths: matches,
        }),
    }
}

fn package_name(package_id: &str) -> Option<&str> {
    let fragment = package_id
        .rsplit_once('#')
        .map_or(package_id, |(_, tail)| tail);
    fragment
        .split_once('@')
        .map_or(fragment, |(name, _)| name)
        .split_whitespace()
        .next()
}

#[derive(Debug, thiserror::Error)]
pub enum CargoBuildError {
    #[error("invalid Player package: {0}")]
    Package(#[from] PackageNameError),
    #[error("cannot access Cargo workspace {path}: {source}")]
    WorkspaceAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cargo workspace is not a regular directory: {0}")]
    WorkspaceNotRegular(PathBuf),
    #[error("cannot access Cargo workspace manifest {path}: {source}")]
    WorkspaceManifest {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cargo workspace manifest is not a regular file: {0}")]
    WorkspaceManifestNotRegular(PathBuf),
    #[error("cannot start Cargo program {program}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cargo Player build failed with status {status:?}: {stderr}")]
    Failed { status: Option<i32>, stderr: String },
    #[error("Cargo JSON output is not UTF-8: {0}")]
    OutputText(#[source] std::str::Utf8Error),
    #[error("invalid Cargo JSON output {line:?}: {source}")]
    OutputJson {
        line: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("Cargo emitted no executable artifact for {0}")]
    MissingArtifact(String),
    #[error("Cargo emitted multiple executable artifacts for {package}: {paths:?}")]
    MultipleArtifacts {
        package: String,
        paths: Vec<PathBuf>,
    },
    #[error("cannot access Cargo executable artifact {path}: {source}")]
    ArtifactAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Cargo executable artifact is not a regular file: {0}")]
    ArtifactNotRegular(PathBuf),
}

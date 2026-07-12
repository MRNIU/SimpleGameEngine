// Copyright The SimpleGameEngine Contributors

use std::fmt;

use serde::{Deserialize, Serialize};
use sge_reflect::{KeyError, TypeKey};

use crate::{ProjectIoError, ProjectPath, ProjectPathError, ProjectRoot, canonical_pretty_config};

pub const PROJECT_FORMAT_VERSION: u32 = 1;
pub const PROJECT_DESCRIPTOR_PATH: &str = "project.sge.ron";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageName(String);

impl PackageName {
    pub fn new(value: impl Into<String>) -> Result<Self, PackageNameError> {
        let value = value.into();
        let mut bytes = value.bytes();
        let valid = (1..=64).contains(&value.len())
            && bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
        if !valid {
            return Err(PackageNameError);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("package name must match [A-Za-z][A-Za-z0-9_-]{{0,63}}")]
pub struct PackageNameError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDescriptor {
    game_id: TypeKey,
    game_package: PackageName,
    player_package: PackageName,
    build_package: PackageName,
    default_authoring_scene: ProjectPath,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectDescriptorWire {
    format_version: u32,
    game_id: String,
    game_package: String,
    player_package: String,
    build_package: String,
    default_authoring_scene: String,
}

impl ProjectDescriptor {
    pub fn new(
        game_id: impl Into<String>,
        game_package: impl Into<String>,
        player_package: impl Into<String>,
        build_package: impl Into<String>,
        default_authoring_scene: ProjectPath,
    ) -> Result<Self, ProjectFormatError> {
        let game_id = game_id.into();
        let game_id =
            TypeKey::new(game_id.clone()).map_err(|source| ProjectFormatError::InvalidGameId {
                value: game_id,
                source,
            })?;
        let game_package = checked_package("game_package", game_package.into())?;
        let player_package = checked_package("player_package", player_package.into())?;
        let build_package = checked_package("build_package", build_package.into())?;
        if !default_authoring_scene.as_str().ends_with(".scene.ron") {
            return Err(ProjectFormatError::InvalidDefaultScene {
                path: default_authoring_scene,
            });
        }
        Ok(Self {
            game_id,
            game_package,
            player_package,
            build_package,
            default_authoring_scene,
        })
    }

    #[must_use]
    pub fn game_id(&self) -> &TypeKey {
        &self.game_id
    }

    #[must_use]
    pub fn game_package(&self) -> &PackageName {
        &self.game_package
    }

    #[must_use]
    pub fn player_package(&self) -> &PackageName {
        &self.player_package
    }

    #[must_use]
    pub fn build_package(&self) -> &PackageName {
        &self.build_package
    }

    #[must_use]
    pub fn default_authoring_scene(&self) -> &ProjectPath {
        &self.default_authoring_scene
    }

    pub fn validate_for_game(&self, expected: &str) -> Result<(), ProjectFormatError> {
        let expected = TypeKey::new(expected.to_owned()).map_err(|source| {
            ProjectFormatError::InvalidExpectedGameId {
                value: expected.to_owned(),
                source,
            }
        })?;
        if self.game_id != expected {
            return Err(ProjectFormatError::GameMismatch {
                expected,
                actual: self.game_id.clone(),
            });
        }
        Ok(())
    }

    pub fn from_ron(input: &str) -> Result<Self, ProjectFormatError> {
        Self::from_bytes(input.as_bytes())
    }

    fn from_bytes(input: &[u8]) -> Result<Self, ProjectFormatError> {
        let path = ProjectPath::new(PROJECT_DESCRIPTOR_PATH)?;
        let wire: ProjectDescriptorWire =
            ron::de::from_bytes(input).map_err(|source| ProjectFormatError::Parse {
                path: path.clone(),
                source: Box::new(source),
            })?;
        if wire.format_version != PROJECT_FORMAT_VERSION {
            return Err(ProjectFormatError::VersionMismatch {
                path,
                expected: PROJECT_FORMAT_VERSION,
                found: wire.format_version,
            });
        }
        let default_authoring_scene =
            ProjectPath::new(&wire.default_authoring_scene).map_err(|source| {
                ProjectFormatError::AtPath {
                    path: path.clone(),
                    source: Box::new(ProjectFormatError::InvalidDefaultProjectPath {
                        value: wire.default_authoring_scene.clone(),
                        source,
                    }),
                }
            })?;
        Self::new(
            wire.game_id,
            wire.game_package,
            wire.player_package,
            wire.build_package,
            default_authoring_scene,
        )
        .map_err(|source| ProjectFormatError::AtPath {
            path,
            source: Box::new(source),
        })
    }

    pub fn to_ron(&self) -> Result<String, ProjectFormatError> {
        let path = ProjectPath::new(PROJECT_DESCRIPTOR_PATH)?;
        self.validate()
            .map_err(|source| ProjectFormatError::AtPath {
                path: path.clone(),
                source: Box::new(source),
            })?;
        let wire = ProjectDescriptorWire {
            format_version: PROJECT_FORMAT_VERSION,
            game_id: self.game_id.to_string(),
            game_package: self.game_package.to_string(),
            player_package: self.player_package.to_string(),
            build_package: self.build_package.to_string(),
            default_authoring_scene: self.default_authoring_scene.to_string(),
        };
        ron::ser::to_string_pretty(&wire, canonical_pretty_config())
            .map_err(|source| ProjectFormatError::Serialize { path, source })
    }

    pub fn load(root: &ProjectRoot) -> Result<Self, ProjectFormatError> {
        let path = ProjectPath::new(PROJECT_DESCRIPTOR_PATH)?;
        Self::from_bytes(&root.read(&path)?)
    }

    pub fn save(&self, root: &ProjectRoot) -> Result<(), ProjectFormatError> {
        let path = ProjectPath::new(PROJECT_DESCRIPTOR_PATH)?;
        let encoded = self.to_ron()?;
        root.write_atomic(&path, encoded.as_bytes())?;
        Ok(())
    }

    fn validate(&self) -> Result<(), ProjectFormatError> {
        Self::new(
            self.game_id.to_string(),
            self.game_package.to_string(),
            self.player_package.to_string(),
            self.build_package.to_string(),
            self.default_authoring_scene.clone(),
        )
        .map(|_| ())
    }
}

fn checked_package(field: &'static str, value: String) -> Result<PackageName, ProjectFormatError> {
    PackageName::new(value.clone()).map_err(|source| ProjectFormatError::InvalidPackage {
        field,
        value,
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectFormatError {
    #[error("invalid built-in project format path: {0}")]
    FormatPath(#[from] ProjectPathError),
    #[error("cannot parse project descriptor {path}: {source}")]
    Parse {
        path: ProjectPath,
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize project descriptor {path}: {source}")]
    Serialize {
        path: ProjectPath,
        #[source]
        source: ron::Error,
    },
    #[error(transparent)]
    Io(#[from] ProjectIoError),
    #[error("unsupported project descriptor version at {path}: expected {expected}, found {found}")]
    VersionMismatch {
        path: ProjectPath,
        expected: u32,
        found: u32,
    },
    #[error("invalid project descriptor {path}: {source}")]
    AtPath {
        path: ProjectPath,
        #[source]
        source: Box<ProjectFormatError>,
    },
    #[error("invalid default authoring scene project path {value:?}: {source}")]
    InvalidDefaultProjectPath {
        value: String,
        #[source]
        source: ProjectPathError,
    },
    #[error("invalid project game id {value:?}: {source}")]
    InvalidGameId {
        value: String,
        #[source]
        source: KeyError,
    },
    #[error("invalid expected game id {value:?}: {source}")]
    InvalidExpectedGameId {
        value: String,
        #[source]
        source: KeyError,
    },
    #[error("project game id mismatch: expected {expected}, found {actual}")]
    GameMismatch { expected: TypeKey, actual: TypeKey },
    #[error("invalid {field} package {value:?}: {source}")]
    InvalidPackage {
        field: &'static str,
        value: String,
        #[source]
        source: PackageNameError,
    },
    #[error("default authoring scene must end in .scene.ron: {path}")]
    InvalidDefaultScene { path: ProjectPath },
}

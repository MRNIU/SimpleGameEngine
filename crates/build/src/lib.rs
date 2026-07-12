// Copyright The SimpleGameEngine Contributors

//! Game-specific Cook、Cargo Player build 与 loose Stage publication。

mod cargo_build;
mod launcher;
mod stage_manifest;

pub use cargo_build::{CargoBuildError, CargoTool};
pub use launcher::{BuildLaunchError, BuildLauncher};
pub use stage_manifest::{
    BuildProfile, STAGE_MANIFEST_FORMAT_VERSION, StageId, StageManifest, StageManifestError,
};

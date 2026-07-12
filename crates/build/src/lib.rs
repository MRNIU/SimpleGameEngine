// Copyright The SimpleGameEngine Contributors

//! Game-specific Cook、Cargo Player build 与 loose Stage publication。

mod stage_manifest;

pub use stage_manifest::{
    BuildProfile, STAGE_MANIFEST_FORMAT_VERSION, StageId, StageManifest, StageManifestError,
};

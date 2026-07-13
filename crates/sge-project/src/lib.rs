// Copyright The SimpleGameEngine Contributors
//
//! Project identity、portable path 与 authoring data 的 durable 边界。

mod descriptor;
mod io;
mod manifest;
mod path;

pub use descriptor::{
    PROJECT_DESCRIPTOR_PATH, PROJECT_FORMAT_VERSION, PackageName, PackageNameError,
    ProjectBootstrap, ProjectDescriptor, ProjectFormatError,
};
pub use io::{ProjectIoError, ProjectRoot};
pub use manifest::{
    AUTHORING_ASSET_MANIFEST_FORMAT_VERSION, AUTHORING_ASSET_MANIFEST_PATH, AuthoringAssetManifest,
    ManifestError, ObjImportSettings, SourceAssetRecord, SourceImporter,
};
pub use path::{ProjectPath, ProjectPathError};

pub(crate) fn canonical_pretty_config() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new().new_line("\n")
}

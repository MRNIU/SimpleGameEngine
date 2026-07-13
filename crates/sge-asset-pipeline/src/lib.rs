// Copyright The SimpleGameEngine Contributors

//! Source import、disposable cache 与 full Cook 产品管线。

use sge_asset::{AssetId, MeshAsset, RuntimeAssetStore, RuntimeAssetStoreError};
use sge_project::{AuthoringAssetManifest, ProjectRoot};

mod cache;
mod closure;
mod cook;
mod obj;
mod output;
mod publish;

pub use cache::{CacheEntryError, CacheIssue, CacheStatus, ImportCacheError};
pub use cook::{CookError, CookReport, full_cook};
pub use obj::ObjImportError;
pub use output::{CookOutputRoot, CookPublishError};

pub struct ImportedAssetSet {
    store: RuntimeAssetStore,
    outcomes: Vec<(AssetId, CacheStatus)>,
}

pub fn validate_obj_source(
    record: &sge_project::SourceAssetRecord,
    bytes: &[u8],
) -> Result<(), ObjImportError> {
    obj::parse_obj(record, bytes).map(|_| ())
}

impl ImportedAssetSet {
    #[must_use]
    pub const fn store(&self) -> &RuntimeAssetStore {
        &self.store
    }

    #[must_use]
    pub fn outcomes(&self) -> &[(AssetId, CacheStatus)] {
        &self.outcomes
    }

    #[must_use]
    pub fn into_parts(self) -> (RuntimeAssetStore, Vec<(AssetId, CacheStatus)>) {
        (self.store, self.outcomes)
    }
}

pub fn import_project_assets(
    project: &ProjectRoot,
    manifest: &AuthoringAssetManifest,
) -> Result<ImportedAssetSet, ProjectAssetImportError> {
    let mut meshes = Vec::<(AssetId, MeshAsset)>::with_capacity(manifest.records().len());
    let mut outcomes = Vec::with_capacity(manifest.records().len());
    for record in manifest.records() {
        let imported = cache::import_obj(project, record).map_err(|source| {
            ProjectAssetImportError::Import {
                asset: record.id(),
                source: Box::new(source),
            }
        })?;
        outcomes.push((imported.asset_id, imported.cache_status));
        meshes.push((imported.asset_id, imported.mesh));
    }
    let store = RuntimeAssetStore::from_meshes(meshes)?;
    Ok(ImportedAssetSet { store, outcomes })
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectAssetImportError {
    #[error("cannot import source asset {asset}: {source}")]
    Import {
        asset: AssetId,
        #[source]
        source: Box<ImportCacheError>,
    },
    #[error("cannot build imported runtime asset store: {0}")]
    Store(#[from] RuntimeAssetStoreError),
}

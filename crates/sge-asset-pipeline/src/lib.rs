// Copyright The SimpleGameEngine Contributors

//! Source import、disposable cache 与 full Cook 产品管线。

use sge_asset::{AssetId, MeshAsset, RuntimeAssetStore, RuntimeAssetStoreError, TextureAsset};
use sge_project::{AuthoringAssetManifest, ProjectIoError, ProjectRoot, SourceImporter};

mod cache;
mod closure;
mod cook;
mod obj;
mod output;
mod png;
mod publish;

pub use cache::{CacheEntryError, CacheIssue, CacheStatus, ImportCacheError};
pub use cook::{CookError, CookReport, full_cook};
pub use obj::ObjImportError;
pub use output::{CookOutputRoot, CookPublishError};
pub use png::PngImportError;

pub(crate) enum ImportedAsset {
    Mesh(MeshAsset),
    Texture(TextureAsset),
}

pub(crate) struct ImportedProduct {
    pub(crate) asset_id: AssetId,
    pub(crate) asset: ImportedAsset,
    pub(crate) cache_status: CacheStatus,
}

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

pub fn validate_png_source(
    record: &sge_project::SourceAssetRecord,
    bytes: &[u8],
) -> Result<(), PngImportError> {
    png::parse_png(record, bytes).map(|_| ())
}

pub(crate) fn import_source_asset(
    project: &ProjectRoot,
    record: &sge_project::SourceAssetRecord,
) -> Result<ImportedProduct, SourceImportError> {
    match record.importer() {
        SourceImporter::Obj(_) => {
            let imported = cache::import_obj(project, record)?;
            Ok(ImportedProduct {
                asset_id: imported.asset_id,
                asset: ImportedAsset::Mesh(imported.mesh),
                cache_status: imported.cache_status,
            })
        }
        SourceImporter::Png => {
            let bytes =
                project
                    .read(record.source())
                    .map_err(|source| SourceImportError::SourceRead {
                        asset: record.id(),
                        source,
                    })?;
            let texture = png::parse_png(record, &bytes)?;
            Ok(ImportedProduct {
                asset_id: record.id(),
                asset: ImportedAsset::Texture(texture),
                cache_status: CacheStatus::Rebuilt,
            })
        }
    }
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
    let mut meshes = Vec::<(AssetId, MeshAsset)>::new();
    let mut textures = Vec::<(AssetId, TextureAsset)>::new();
    let mut outcomes = Vec::with_capacity(manifest.records().len());
    for record in manifest.records() {
        let imported = import_source_asset(project, record).map_err(|source| {
            ProjectAssetImportError::Import {
                asset: record.id(),
                source: Box::new(source),
            }
        })?;
        outcomes.push((imported.asset_id, imported.cache_status));
        match imported.asset {
            ImportedAsset::Mesh(mesh) => meshes.push((imported.asset_id, mesh)),
            ImportedAsset::Texture(texture) => textures.push((imported.asset_id, texture)),
        }
    }
    let store = RuntimeAssetStore::from_assets(meshes, textures)?;
    Ok(ImportedAssetSet { store, outcomes })
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectAssetImportError {
    #[error("cannot import source asset {asset}: {source}")]
    Import {
        asset: AssetId,
        #[source]
        source: Box<SourceImportError>,
    },
    #[error("cannot build imported runtime asset store: {0}")]
    Store(#[from] RuntimeAssetStoreError),
}

#[derive(Debug, thiserror::Error)]
pub enum SourceImportError {
    #[error(transparent)]
    Obj(#[from] ImportCacheError),
    #[error(transparent)]
    Png(#[from] PngImportError),
    #[error("cannot read source asset {asset}: {source}")]
    SourceRead {
        asset: AssetId,
        #[source]
        source: ProjectIoError,
    },
}

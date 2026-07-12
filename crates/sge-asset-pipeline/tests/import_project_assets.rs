// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetRef, RuntimeAssetStoreError};
use sge_asset_pipeline::{CacheStatus, ProjectAssetImportError, import_project_assets};
use sge_project::AuthoringAssetManifest;

use support::FullCookFixture;

#[test]
fn imports_every_manifest_asset_into_one_store_with_ordered_outcomes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("editor-import")?;
    let project = fixture.project()?;
    let manifest = AuthoringAssetManifest::load(&project)?;

    let imported = import_project_assets(&project, &manifest)?;

    assert_eq!(
        imported.outcomes(),
        [
            (fixture.used, CacheStatus::Rebuilt),
            (fixture.unused, CacheStatus::Rebuilt),
        ]
    );
    assert!(imported.store().mesh(AssetRef::new(fixture.used)).is_ok());
    assert!(imported.store().mesh(AssetRef::new(fixture.unused)).is_ok());
    assert!(matches!(
        imported.store().mesh(AssetRef::new(
            "10000000-0000-4000-8000-000000000003".parse()?
        )),
        Err(RuntimeAssetStoreError::MissingMesh { .. })
    ));
    Ok(())
}

#[test]
fn failed_full_import_returns_no_partial_public_store() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("editor-import-atomic")?;
    fixture.corrupt_unused_source()?;
    let project = fixture.project()?;
    let manifest = AuthoringAssetManifest::load(&project)?;

    let error = match import_project_assets(&project, &manifest) {
        Ok(_) => panic!("invalid second source returned a partial imported set"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        ProjectAssetImportError::Import { asset, .. } if asset == fixture.unused
    ));
    Ok(())
}

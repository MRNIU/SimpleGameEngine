// Copyright The SimpleGameEngine Contributors

mod support;

use std::fs;

use sge_asset::{RuntimeContentRoot, RuntimeGenerationId};
use sge_asset_pipeline::{CookError, CookOutputRoot, CookPublishError, full_cook};
use sge_scene::{RuntimeSceneBuildError, SceneValidationError};

use support::{FullCookFixture, GAME_ID, registry, world};

struct ValidPrior {
    fixture: FullCookFixture,
    catalog_bytes: Vec<u8>,
    generation: RuntimeGenerationId,
}

impl ValidPrior {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let fixture = FullCookFixture::new(name)?;
        let report = full_cook(
            &fixture.project()?,
            GAME_ID,
            &registry(true)?,
            &world(true, true)?,
            &CookOutputRoot::open(fixture.output_path())?,
        )?;
        Ok(Self {
            catalog_bytes: fs::read(fixture.output_path().join("runtime_catalog.ron"))?,
            generation: report.generation().clone(),
            fixture,
        })
    }

    fn assert_preserved(&self) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            fs::read(self.fixture.output_path().join("runtime_catalog.ron"))?,
            self.catalog_bytes
        );
        let loaded = RuntimeContentRoot::open(self.fixture.output_path())?.load_current(GAME_ID)?;
        assert_eq!(loaded.catalog().generation(), &self.generation);
        Ok(())
    }
}

#[test]
fn malformed_scene_preserves_valid_prior() -> Result<(), Box<dyn std::error::Error>> {
    let prior = ValidPrior::new("prior-scene")?;
    prior.fixture.corrupt_scene()?;

    let error = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )
    .expect_err("malformed scene must fail");

    assert!(matches!(error, CookError::SceneFormat { .. }));
    prior.assert_preserved()
}

#[test]
fn missing_source_preserves_valid_prior_even_with_cache() -> Result<(), Box<dyn std::error::Error>>
{
    let prior = ValidPrior::new("prior-source")?;
    prior.fixture.remove_used_source()?;

    let error = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )
    .expect_err("missing source must fail despite cache");

    assert!(matches!(error, CookError::Import { .. }));
    prior.assert_preserved()
}

#[test]
fn corrupt_manifest_preserves_valid_prior() -> Result<(), Box<dyn std::error::Error>> {
    let prior = ValidPrior::new("prior-manifest")?;
    prior.fixture.corrupt_manifest()?;

    let error = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )
    .expect_err("corrupt manifest must fail");

    assert!(matches!(error, CookError::Manifest(_)));
    prior.assert_preserved()
}

#[test]
fn reserved_structural_alias_preserves_valid_prior() -> Result<(), Box<dyn std::error::Error>> {
    let prior = ValidPrior::new("prior-alias")?;
    let alias_registry = prior.fixture.write_reserved_alias_scene()?;

    let error = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &alias_registry,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )
    .expect_err("reserved structural alias must fail");

    assert!(matches!(
        error,
        CookError::RuntimeSceneBuild(RuntimeSceneBuildError::Validation(source))
            if matches!(*source, SceneValidationError::ReservedStructuralComponent { .. })
    ));
    prior.assert_preserved()
}

#[test]
fn existing_mismatched_generation_is_rejected_and_prior_remains_loadable()
-> Result<(), Box<dyn std::error::Error>> {
    let prior = ValidPrior::new("prior-collision")?;
    prior.fixture.change_used_source()?;
    let secondary = prior
        .fixture
        .output_path()
        .with_file_name("secondary-output");
    fs::create_dir(&secondary)?;
    let candidate = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(&secondary)?,
    )?;
    let collision = prior
        .fixture
        .output_path()
        .join("generations")
        .join(candidate.generation().as_str());
    fs::create_dir(&collision)?;
    fs::write(collision.join("mismatch"), b"not the candidate tree")?;

    let error = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )
    .expect_err("existing mismatched generation must fail");

    assert!(matches!(
        error,
        CookError::Publish(source)
            if matches!(*source, CookPublishError::ExistingGeneration { .. })
    ));
    prior.assert_preserved()
}

#[test]
fn real_atomic_commit_replaces_catalog_and_keeps_old_generation_sibling()
-> Result<(), Box<dyn std::error::Error>> {
    let prior = ValidPrior::new("real-replace")?;
    let old_generation_path = prior
        .fixture
        .output_path()
        .join("generations")
        .join(prior.generation.as_str());
    prior.fixture.change_used_source()?;

    let report = full_cook(
        &prior.fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &CookOutputRoot::open(prior.fixture.output_path())?,
    )?;

    assert_ne!(report.generation(), &prior.generation);
    assert!(old_generation_path.is_dir());
    assert_ne!(
        fs::read(prior.fixture.output_path().join("runtime_catalog.ron"))?,
        prior.catalog_bytes
    );
    let loaded = RuntimeContentRoot::open(prior.fixture.output_path())?.load_current(GAME_ID)?;
    assert_eq!(loaded.catalog().generation(), report.generation());
    Ok(())
}

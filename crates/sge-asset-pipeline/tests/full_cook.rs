// Copyright The SimpleGameEngine Contributors

mod support;

use sge_asset::{AssetRef, RuntimeAssetStore, RuntimeContentRoot};
use sge_asset_pipeline::{CacheStatus, CookError, CookOutputRoot, CookPublishError, full_cook};
use sge_scene::{RuntimeScene, prepare_runtime};

use support::{FullCookFixture, GAME_ID, registry, world};

#[test]
fn full_cook_imports_every_source_and_publishes_only_the_entry_closure()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("success")?;
    let project = fixture.project()?;
    let registry = registry(true)?;
    let world = world(true, true)?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let report = full_cook(&project, GAME_ID, &registry, &world, &output)?;

    assert_eq!(
        report.entry_scene().as_str(),
        "Scenes/entry.runtime-scene.ron"
    );
    assert_eq!(report.published_assets(), [fixture.used]);
    assert_eq!(
        report.import_statuses(),
        [
            (fixture.used, CacheStatus::Rebuilt),
            (fixture.unused, CacheStatus::Rebuilt),
        ]
    );

    let content = RuntimeContentRoot::open(fixture.output_path())?;
    let generation = content.load_current(GAME_ID)?;
    assert_eq!(generation.catalog().generation(), report.generation());
    assert_eq!(generation.catalog().entry_scene(), report.entry_scene());
    assert_eq!(generation.catalog().assets().len(), 1);
    assert_eq!(*generation.catalog().assets()[0].id(), fixture.used);
    assert!(
        !fixture
            .output_path()
            .join("generations")
            .join(report.generation().as_str())
            .join(format!("Content/{}.mesh.ron", fixture.unused))
            .exists()
    );

    let store = RuntimeAssetStore::load(&generation)?;
    let _mesh = store.mesh(AssetRef::new(fixture.used))?;
    let runtime = RuntimeScene::from_ron(std::str::from_utf8(generation.entry_scene_bytes())?)?;
    let prepared = prepare_runtime(&runtime, &registry, &store)?;
    assert_eq!(runtime.entities().count(), 2);
    assert!(runtime.entities().any(|entity| {
        entity
            .components()
            .any(|component| component.type_key().as_str() == "demo.mesh_consumer")
    }));
    sge_scene::preflight_instantiation(&prepared, &world)?;
    Ok(())
}

#[test]
fn wrong_game_identity_wins_over_corrupt_manifest_and_missing_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("wrong-game-first")?;
    fixture.corrupt_manifest_and_remove_source()?;
    fixture.seed_prior_catalog()?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let error = full_cook(
        &fixture.project()?,
        "other.game",
        &registry(true)?,
        &world(true, true)?,
        &output,
    )
    .expect_err("wrong game identity was not rejected");

    assert!(matches!(error, CookError::GameIdentity(_)));
    fixture.assert_output_untouched()?;
    Ok(())
}

#[test]
fn full_cook_rejects_an_unfrozen_registry_without_touching_output()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("unfrozen-registry")?;
    fixture.seed_prior_catalog()?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let error = full_cook(
        &fixture.project()?,
        GAME_ID,
        &registry(false)?,
        &world(true, true)?,
        &output,
    )
    .expect_err("unfrozen registry was accepted");

    assert!(matches!(error, CookError::RegistryNotFrozen));
    fixture.assert_output_untouched()?;
    Ok(())
}

#[test]
fn full_cook_rejects_an_unfinished_world_without_touching_output()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("unfinished-world")?;
    fixture.seed_prior_catalog()?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let error = full_cook(
        &fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, false)?,
        &output,
    )
    .expect_err("unfinished World registration was accepted");

    assert!(matches!(error, CookError::WorldRegistrationNotFinished));
    fixture.assert_output_untouched()?;
    Ok(())
}

#[test]
fn full_cook_rejects_invalid_unused_source_before_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("invalid-unused-source")?;
    fixture.corrupt_unused_source()?;
    fixture.seed_prior_catalog()?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let error = full_cook(
        &fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(true, true)?,
        &output,
    )
    .expect_err("invalid unused source escaped deterministic full import");

    assert!(matches!(error, CookError::Import { asset, .. } if asset == fixture.unused));
    fixture.assert_output_untouched()?;
    Ok(())
}

#[test]
fn finished_world_missing_custom_registration_fails_consumer_preflight_without_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("missing-consumer-registration")?;
    fixture.seed_prior_catalog()?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let error = full_cook(
        &fixture.project()?,
        GAME_ID,
        &registry(true)?,
        &world(false, true)?,
        &output,
    )
    .expect_err("missing component registration passed consumer preflight");

    assert!(matches!(
        error,
        CookError::Publish(source)
            if matches!(*source, CookPublishError::ScenePreflight(_))
    ));
    fixture.assert_output_untouched()?;
    Ok(())
}

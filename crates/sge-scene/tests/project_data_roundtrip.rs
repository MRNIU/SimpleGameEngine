// Copyright The SimpleGameEngine Contributors

mod project_data_support;
#[allow(dead_code)]
mod support;

use sge_asset::AssetId;
use sge_ecs::World;
use sge_project::{
    AUTHORING_ASSET_MANIFEST_PATH, AuthoringAssetManifest, PROJECT_DESCRIPTOR_PATH,
    ProjectDescriptor, ProjectPath,
};

use project_data_support::{
    TestProject, game_descriptor, invalid_factory_calls, invalid_guard_game_descriptor,
    mismatch_factory_calls, mismatch_guard_game_descriptor, missing_parent_game_descriptor,
    open_all, reload, save_scene, save_world, signature,
};
use support::{Probe, probe_registry};

#[test]
fn project_data_happy_path_reopens_through_a_second_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("happy")?;
    let first = open_all(fixture.path(), game_descriptor())?;
    let first_signature = signature(&first, &fixture)?;
    assert_eq!(first_signature.count, fixture.count);
    assert_eq!(first_signature.target, fixture.root_id);
    assert_eq!(first_signature.asset, fixture.asset_id);

    let saved = save_scene(&first)?;
    assert!(!saved.is_empty());
    let second = open_all(fixture.path(), game_descriptor())?;
    assert_eq!(signature(&second, &fixture)?, first_signature);
    Ok(())
}

#[test]
fn invalid_project_game_id_does_not_replace_live_or_call_factory()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("invalid-game-id")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;
    let root = fixture.root()?;
    let path = ProjectPath::new(PROJECT_DESCRIPTOR_PATH)?;
    let valid = String::from_utf8(root.read(&path)?)?;
    let invalid = valid.replacen("demo.game", "demo/game", 1);
    assert_ne!(invalid, valid);
    root.write_atomic(&path, invalid.as_bytes())?;
    let guarded_game = invalid_guard_game_descriptor();

    assert!(reload(&mut live, fixture.path(), guarded_game).is_err());
    assert_eq!(invalid_factory_calls(), 0);
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn mismatched_project_game_id_does_not_replace_live_or_call_factory()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("mismatched-game-id")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;
    let root = fixture.root()?;
    ProjectDescriptor::new(
        "other.game",
        "demo-game",
        "demo-player",
        "demo-build",
        ProjectPath::new("scenes/main.scene.ron")?,
    )?
    .save(&root)?;
    let guarded_game = mismatch_guard_game_descriptor();

    assert!(reload(&mut live, fixture.path(), guarded_game).is_err());
    assert_eq!(mismatch_factory_calls(), 0);
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn corrupt_manifest_does_not_replace_live() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("corrupt-manifest")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;
    fixture.root()?.write_atomic(
        &ProjectPath::new(AUTHORING_ASSET_MANIFEST_PATH)?,
        b"(format_version: 1, assets: [",
    )?;

    assert!(reload(&mut live, fixture.path(), game_descriptor()).is_err());
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn corrupt_scene_does_not_replace_live() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("corrupt-scene")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;
    let root = fixture.root()?;
    let descriptor = ProjectDescriptor::load(&root)?;
    root.write_atomic(
        descriptor.default_authoring_scene(),
        b"(format_version: 1, entities: [",
    )?;

    assert!(reload(&mut live, fixture.path(), game_descriptor()).is_err());
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn prepare_failure_does_not_replace_live() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("prepare-failure")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;
    let invalid = fixture.scene_with_probe(AssetId::new_v4(), fixture.root_id)?;
    fixture.write_scene(&invalid)?;

    assert!(reload(&mut live, fixture.path(), game_descriptor()).is_err());
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn instantiate_preflight_failure_does_not_replace_live() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("instantiate-failure")?;
    let mut live = open_all(fixture.path(), game_descriptor())?;
    let before = signature(&live, &fixture)?;

    assert!(reload(&mut live, fixture.path(), missing_parent_game_descriptor()).is_err());
    assert_eq!(signature(&live, &fixture)?, before);
    Ok(())
}

#[test]
fn precommit_save_failure_preserves_prior_scene_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestProject::new("save-failure")?;
    let root = fixture.root()?;
    let descriptor = ProjectDescriptor::load(&root)?;
    let manifest = AuthoringAssetManifest::load(&root)?;
    let old = root.read(descriptor.default_authoring_scene())?;
    let registry = probe_registry()?;
    let mut invalid_world = World::new();
    invalid_world.register_component::<Probe>()?;
    invalid_world.finish_registration();
    let _runtime = invalid_world.spawn();

    assert!(
        save_world(
            &root,
            descriptor.default_authoring_scene(),
            &invalid_world,
            &registry,
            &manifest,
        )
        .is_err()
    );
    assert_eq!(root.read(descriptor.default_authoring_scene())?, old);
    Ok(())
}

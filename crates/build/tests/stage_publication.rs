// Copyright The SimpleGameEngine Contributors
//
//! M6 immutable Stage generation publication contract.

use std::{collections::BTreeMap, fs, path::PathBuf};

use sge_asset::{RuntimeAssetCatalog, RuntimeProductPath};
use sge_build::{BuildProfile, StagePublishRequest, StageRoot};
use sge_reflect::TypeKey;

const GAME_ID: &str = "demo.game";
const PLAYER: &str = "demo-game-player";
const SCENE: &[u8] = b"runtime scene";

#[test]
fn publishes_and_reuses_an_exact_immutable_generation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("publish")?;
    let stage = StageRoot::create(&fixture.stage)?;
    let first = stage.begin()?;
    let generation = seed_runtime(first.runtime_root())?;
    let artifact = fixture.artifact(b"player-v1")?;

    let manifest = first.publish(StagePublishRequest::new(
        GAME_ID,
        PLAYER,
        BuildProfile::Dev,
        &artifact,
        generation.clone(),
    ))?;
    let reopened = stage.load_current(GAME_ID)?;
    let reopened_root = StageRoot::create(&fixture.stage)?;

    assert_eq!(reopened, manifest);
    assert_eq!(reopened_root.load_current(GAME_ID)?, manifest);
    assert_eq!(reopened.runtime_generation(), &generation);
    assert!(
        fixture
            .stage
            .join(reopened.executable_path().as_str())
            .is_file()
    );

    let second = stage.begin()?;
    seed_runtime(second.runtime_root())?;
    let repeated = second.publish(StagePublishRequest::new(
        GAME_ID,
        PLAYER,
        BuildProfile::Dev,
        &artifact,
        generation,
    ))?;
    assert_eq!(repeated.stage_id(), manifest.stage_id());
    assert_eq!(fs::read_dir(fixture.stage.join("generations"))?.count(), 1);
    Ok(())
}

#[test]
fn corrupt_candidate_preserves_the_current_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("preserve")?;
    let stage = StageRoot::create(&fixture.stage)?;
    let first = stage.begin()?;
    let generation = seed_runtime(first.runtime_root())?;
    let artifact = fixture.artifact(b"player-v1")?;
    first.publish(StagePublishRequest::new(
        GAME_ID,
        PLAYER,
        BuildProfile::Dev,
        &artifact,
        generation,
    ))?;
    let old_manifest = fs::read(fixture.stage.join("stage_manifest.ron"))?;

    let candidate = stage.begin()?;
    let next_generation = seed_runtime(candidate.runtime_root())?;
    fs::write(
        candidate.runtime_root().join("runtime_catalog.ron"),
        b"corrupt",
    )?;
    let error = candidate
        .publish(StagePublishRequest::new(
            GAME_ID,
            PLAYER,
            BuildProfile::Dev,
            &artifact,
            next_generation,
        ))
        .expect_err("corrupt runtime was published");

    assert!(error.to_string().contains("runtime"));
    assert_eq!(
        fs::read(fixture.stage.join("stage_manifest.ron"))?,
        old_manifest
    );
    assert!(
        fs::read_dir(fixture.stage.join("generations"))?.all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".unpublished-"))
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn roots_and_executable_sources_reject_symlinks() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("symlink")?;
    fs::create_dir(&fixture.stage)?;
    let link = fixture.root.join("stage-link");
    symlink(&fixture.stage, &link)?;
    assert!(StageRoot::open(&link).is_err());

    let stage = StageRoot::open(&fixture.stage)?;
    let candidate = stage.begin()?;
    let generation = seed_runtime(candidate.runtime_root())?;
    let artifact = fixture.artifact(b"player")?;
    let artifact_link = fixture.root.join("player-link");
    symlink(&artifact, &artifact_link)?;
    assert!(
        candidate
            .publish(StagePublishRequest::new(
                GAME_ID,
                PLAYER,
                BuildProfile::Dev,
                artifact_link,
                generation,
            ))
            .is_err()
    );
    assert!(!fixture.stage.join("stage_manifest.ron").exists());
    Ok(())
}

#[test]
fn create_builds_a_missing_regular_parent_chain() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("parents")?;
    let nested = fixture.root.join("one/two/Stage");
    let _stage = StageRoot::create(&nested)?;
    assert!(nested.join("generations").is_dir());
    Ok(())
}

fn seed_runtime(
    root: &std::path::Path,
) -> Result<sge_asset::RuntimeGenerationId, Box<dyn std::error::Error>> {
    let products = BTreeMap::new();
    let catalog = RuntimeAssetCatalog::build(
        TypeKey::new(GAME_ID)?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        Vec::new(),
        SCENE,
        &products,
    )?;
    let generation = catalog.generation().clone();
    let generation_root = root.join("generations").join(generation.as_str());
    fs::create_dir_all(generation_root.join("Scenes"))?;
    fs::write(root.join("runtime_catalog.ron"), catalog.to_ron()?)?;
    fs::write(
        generation_root.join("Scenes/entry.runtime-scene.ron"),
        SCENE,
    )?;
    Ok(generation)
}

struct Fixture {
    root: PathBuf,
    stage: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/stage-publication")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)?;
        Ok(Self {
            stage: root.join("Stage"),
            root,
        })
    }

    fn artifact(&self, bytes: &[u8]) -> Result<PathBuf, std::io::Error> {
        let path = self.root.join(PLAYER);
        fs::write(&path, bytes)?;
        Ok(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

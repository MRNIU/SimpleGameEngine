// Copyright The SimpleGameEngine Contributors

use std::{collections::BTreeMap, fs, io};

use sge_asset::{RuntimeAssetCatalog, RuntimeProductPath};
use sge_reflect::TypeKey;

use super::{BuildProfile, StagePublishError, StagePublishRequest, StageRoot};

const GAME_ID: &str = "demo.game";
const PLAYER: &str = "demo-game-player";

#[test]
fn manifest_commit_is_the_last_fallible_step_and_preserves_old_current()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "sge-stage-commit-{}-{}",
        std::process::id(),
        super::NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _cleanup = Cleanup(root.clone());
    fs::create_dir(&root)?;
    let stage = StageRoot::open(&root)?;
    let artifact = root
        .parent()
        .unwrap()
        .join(format!("{PLAYER}-artifact-{}", std::process::id()));
    let _artifact_cleanup = Cleanup(artifact.clone());

    fs::write(&artifact, b"old player")?;
    let first = stage.begin()?;
    let generation = seed_runtime(first.runtime_root(), b"old scene")?;
    first.publish(StagePublishRequest::new(
        GAME_ID,
        PLAYER,
        BuildProfile::Dev,
        &artifact,
        generation,
    ))?;
    let old = fs::read(root.join(super::MANIFEST_NAME))?;

    fs::write(&artifact, b"new player")?;
    let candidate = stage.begin()?;
    let next_generation = seed_runtime(candidate.runtime_root(), b"new scene")?;
    let error = candidate
        .publish_with_commit(
            StagePublishRequest::new(
                GAME_ID,
                PLAYER,
                BuildProfile::Dev,
                &artifact,
                next_generation,
            ),
            |stage_root, _| {
                assert_eq!(stage_root, root);
                assert_eq!(
                    fs::read_dir(root.join(super::GENERATIONS_NAME))
                        .expect("generation directory must remain readable")
                        .count(),
                    2
                );
                Err(StagePublishError::ManifestCommit {
                    path: root.join(super::MANIFEST_NAME),
                    source: io::Error::other("injected commit failure"),
                })
            },
        )
        .expect_err("injected commit failure returned success");

    assert!(matches!(error, StagePublishError::ManifestCommit { .. }));
    assert_eq!(fs::read(root.join(super::MANIFEST_NAME))?, old);
    assert_eq!(stage.load_current(GAME_ID)?.to_ron()?.into_bytes(), old);
    Ok(())
}

fn seed_runtime(
    root: &std::path::Path,
    scene: &[u8],
) -> Result<sge_asset::RuntimeGenerationId, Box<dyn std::error::Error>> {
    let products = BTreeMap::new();
    let catalog = RuntimeAssetCatalog::build(
        TypeKey::new(GAME_ID)?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        Vec::new(),
        scene,
        &products,
    )?;
    let generation = catalog.generation().clone();
    let generation_root = root.join("generations").join(generation.as_str());
    fs::create_dir_all(generation_root.join("Scenes"))?;
    fs::write(root.join("runtime_catalog.ron"), catalog.to_ron()?)?;
    fs::write(
        generation_root.join("Scenes/entry.runtime-scene.ron"),
        scene,
    )?;
    Ok(generation)
}

struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.0.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}

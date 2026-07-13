// Copyright The SimpleGameEngine Contributors
//
//! M6 Stage current manifest public contract.

use std::str::FromStr;

use sge_asset::RuntimeGenerationId;
use sge_build::{BuildProfile, STAGE_MANIFEST_FORMAT_VERSION, StageManifest};

const RUNTIME_GENERATION: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn stage_manifest_is_canonical_strict_and_idempotent() {
    let runtime = RuntimeGenerationId::from_str(RUNTIME_GENERATION).unwrap();
    let manifest = StageManifest::build(
        "demo.game",
        "demo-game-player",
        BuildProfile::Dev,
        "demo-game-player",
        b"player-v1",
        runtime,
    )
    .unwrap();
    let encoded = manifest.to_ron().unwrap();
    let reopened = StageManifest::from_ron(&encoded).unwrap();

    assert_eq!(reopened, manifest);
    assert_eq!(reopened.to_ron().unwrap(), encoded);
    assert_eq!(reopened.profile(), BuildProfile::Dev);
    assert_eq!(reopened.game_id(), "demo.game");
    assert_eq!(reopened.player_package(), "demo-game-player");
    assert_eq!(reopened.runtime_generation().as_str(), RUNTIME_GENERATION);
    assert_eq!(
        reopened.executable_path().as_str(),
        format!(
            "generations/{}/demo-game-player",
            reopened.stage_id().as_str()
        )
    );
    assert_eq!(
        reopened.runtime_root().as_str(),
        format!("generations/{}/runtime", reopened.stage_id().as_str())
    );
    assert!(encoded.ends_with('\n'));
}

#[test]
fn stage_identity_changes_for_every_shipping_input() {
    let runtime = RuntimeGenerationId::from_str(RUNTIME_GENERATION).unwrap();
    let baseline = StageManifest::build(
        "demo.game",
        "demo-game-player",
        BuildProfile::Dev,
        "demo-game-player",
        b"player-v1",
        runtime.clone(),
    )
    .unwrap();
    let cases = [
        StageManifest::build(
            "other.game",
            "demo-game-player",
            BuildProfile::Dev,
            "demo-game-player",
            b"player-v1",
            runtime.clone(),
        )
        .unwrap(),
        StageManifest::build(
            "demo.game",
            "other-player",
            BuildProfile::Dev,
            "other-player",
            b"player-v1",
            runtime.clone(),
        )
        .unwrap(),
        StageManifest::build(
            "demo.game",
            "demo-game-player",
            BuildProfile::Release,
            "demo-game-player",
            b"player-v1",
            runtime.clone(),
        )
        .unwrap(),
        StageManifest::build(
            "demo.game",
            "demo-game-player",
            BuildProfile::Dev,
            "demo-game-player",
            b"player-v2",
            runtime.clone(),
        )
        .unwrap(),
        StageManifest::build(
            "demo.game",
            "demo-game-player",
            BuildProfile::Dev,
            "demo-game-player",
            b"player-v1",
            RuntimeGenerationId::from_str(
                "2222222222222222222222222222222222222222222222222222222222222222",
            )
            .unwrap(),
        )
        .unwrap(),
    ];
    for candidate in cases {
        assert_ne!(candidate.stage_id(), baseline.stage_id());
    }
}

#[test]
fn stage_manifest_rejects_version_unknown_paths_and_tampered_digest() {
    let manifest = StageManifest::build(
        "demo.game",
        "demo-game-player",
        BuildProfile::Dev,
        "demo-game-player",
        b"player-v1",
        RuntimeGenerationId::from_str(RUNTIME_GENERATION).unwrap(),
    )
    .unwrap();
    let encoded = manifest.to_ron().unwrap();

    assert!(
        StageManifest::from_ron(&encoded.replace(
            &format!("format_version: {STAGE_MANIFEST_FORMAT_VERSION}"),
            "format_version: 99"
        ))
        .is_err()
    );
    assert!(
        StageManifest::from_ron(&encoded.replace("runtime_root:", "unknown: 1, runtime_root:"))
            .is_err()
    );
    assert!(
        StageManifest::from_ron(&encoded.replace(manifest.runtime_root().as_str(), "../outside"))
            .is_err()
    );
    assert!(
        StageManifest::from_ron(&encoded.replace(
            manifest.stage_id().as_str(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ))
        .is_err()
    );
}

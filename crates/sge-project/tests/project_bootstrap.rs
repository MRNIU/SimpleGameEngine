// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf};

use sge_project::{
    PackageNameError, ProjectBootstrap, ProjectDescriptor, ProjectFormatError, ProjectIoError,
    ProjectRoot,
};

#[test]
fn bootstrap_exposes_only_the_checked_build_package() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = ProjectBootstrap::from_ron(&descriptor_ron(
        1,
        "demo.game",
        "demo-game",
        "demo-game-player",
        "demo-game-build",
        "scenes/main.scene.ron",
    ))?;

    assert_eq!(bootstrap.build_package().as_str(), "demo-game-build");
    Ok(())
}

#[test]
fn bootstrap_defers_non_build_descriptor_semantics() -> Result<(), Box<dyn std::error::Error>> {
    for input in [
        descriptor_ron(
            1,
            "invalid game id",
            "demo-game",
            "demo-game-player",
            "demo-game-build",
            "scenes/main.scene.ron",
        ),
        descriptor_ron(
            1,
            "demo.game",
            "bad.game.package",
            "demo-game-player",
            "demo-game-build",
            "scenes/main.scene.ron",
        ),
        descriptor_ron(
            1,
            "demo.game",
            "demo-game",
            "bad.player.package",
            "demo-game-build",
            "scenes/main.scene.ron",
        ),
        descriptor_ron(
            1,
            "demo.game",
            "demo-game",
            "demo-game-player",
            "demo-game-build",
            "../outside.ron",
        ),
    ] {
        let bootstrap = ProjectBootstrap::from_ron(&input)?;
        assert_eq!(bootstrap.build_package().as_str(), "demo-game-build");
        assert!(ProjectDescriptor::from_ron(&input).is_err());
    }
    Ok(())
}

#[test]
fn bootstrap_rejects_invalid_build_package_with_file_context() {
    let input = descriptor_ron(
        1,
        "demo.game",
        "demo-game",
        "demo-game-player",
        "bad/build",
        "scenes/main.scene.ron",
    );

    assert!(matches!(
        ProjectBootstrap::from_ron(&input),
        Err(ProjectFormatError::AtPath { path, source })
            if path.as_str() == "project.sge.ron"
                && matches!(source.as_ref(), ProjectFormatError::InvalidPackage {
                    field: "build_package",
                    value,
                    source: PackageNameError,
                } if value == "bad/build")
    ));
}

#[test]
fn bootstrap_uses_the_descriptor_version_and_strict_wire() {
    let wrong_version = descriptor_ron(
        2,
        "demo.game",
        "demo-game",
        "demo-game-player",
        "demo-game-build",
        "scenes/main.scene.ron",
    );
    assert!(matches!(
        ProjectBootstrap::from_ron(&wrong_version),
        Err(ProjectFormatError::VersionMismatch { path, expected: 1, found: 2 })
            if path.as_str() == "project.sge.ron"
    ));

    let valid = descriptor_ron(
        1,
        "demo.game",
        "demo-game",
        "demo-game-player",
        "demo-game-build",
        "scenes/main.scene.ron",
    );
    for invalid in [
        valid.replace("\n)", "\n    future_field: true,\n)"),
        valid.replace("game_id: \"demo.game\"", "game_id: 7"),
        valid.replace("game_package: \"demo-game\"", "game_package: 7"),
        valid.replace("player_package: \"demo-game-player\"", "player_package: 7"),
        valid.replace("build_package: \"demo-game-build\"", "build_package: 7"),
        valid.replace(
            "default_authoring_scene: \"scenes/main.scene.ron\"",
            "default_authoring_scene: 7",
        ),
    ] {
        assert!(matches!(
            ProjectBootstrap::from_ron(&invalid),
            Err(ProjectFormatError::Parse { path, .. }) if path.as_str() == "project.sge.ron"
        ));
    }
}

#[test]
fn bootstrap_load_preserves_the_project_file_path() -> Result<(), Box<dyn std::error::Error>> {
    let path = temporary_project_path();
    let _stale_cleanup = fs::remove_dir_all(&path);
    fs::create_dir(&path)?;
    let root = ProjectRoot::open(&path)?;

    let error = ProjectBootstrap::load(&root).expect_err("missing descriptor was accepted");
    assert!(matches!(
        error,
        ProjectFormatError::Io(ProjectIoError::Read { path, source })
            if path.as_str() == "project.sge.ron"
                && source.kind() == std::io::ErrorKind::NotFound
    ));

    fs::remove_dir_all(path)?;
    Ok(())
}

fn descriptor_ron(
    format_version: u32,
    game_id: &str,
    game_package: &str,
    player_package: &str,
    build_package: &str,
    default_authoring_scene: &str,
) -> String {
    format!(
        "(\n    format_version: {format_version},\n    game_id: \"{game_id}\",\n    game_package: \"{game_package}\",\n    player_package: \"{player_package}\",\n    build_package: \"{build_package}\",\n    default_authoring_scene: \"{default_authoring_scene}\",\n)"
    )
}

fn temporary_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp/project-bootstrap-{}",
        std::process::id()
    ))
}

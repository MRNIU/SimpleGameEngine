// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn editor_cli_has_stable_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg("--help")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Usage: demo-game-editor PROJECT_ROOT [--play] [--max-frames N]\n"
    );
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn game_specific_editor_plays_and_paints_preview() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--play", "--max-frames", "6"])
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("preview_prepare="));
    assert!(stdout.contains("preview_paint="));
    let play_frames = stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("play_frames="))
        .ok_or("missing play_frames report")?
        .parse::<u64>()?;
    assert!(play_frames > 0);
    Ok(())
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new() -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/tmp/demo_game_editor")
            .join(std::process::id().to_string());
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Content/Meshes"))?;
        fs::create_dir_all(root.join("Scenes"))?;
        for relative in [
            "project.sge.ron",
            "Content/asset_manifest.ron",
            "Content/Meshes/demo.obj",
            "Scenes/main.scene.ron",
        ] {
            fs::copy(demo_root().join(relative), root.join(relative))?;
        }
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

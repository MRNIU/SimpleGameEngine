// Copyright The SimpleGameEngine Contributors

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use sge_build::{BuildProfile, BuildRequest, StageRoot, build};

#[test]
#[ignore = "builds a real Player and requires a window system; run with xvfb-run"]
fn game_build_produces_a_copied_source_free_stage_that_runs()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = workspace.join("examples/demo_game");
    let root = workspace
        .join("target/tmp/m6-stage-product")
        .join(std::process::id().to_string());
    let _ = fs::remove_dir_all(&root);
    let stage = root.join("Stage");
    let copied = root.join("CopiedStage");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let request = BuildRequest::new(
        &project,
        &workspace,
        &stage,
        workspace.join("target"),
        BuildProfile::Dev,
    )
    .with_cargo_program(cargo);

    let first = build(demo_game::GAME, env!("CARGO_PKG_NAME"), &request)?;
    let second = build(demo_game::GAME, env!("CARGO_PKG_NAME"), &request)?;
    assert_eq!(first.stage().stage_id(), second.stage().stage_id());
    copy_tree(&stage, &copied)?;
    assert!(!copied.join("project.sge.ron").exists());
    assert!(!contains_extension(&copied, "obj")?);
    assert!(!contains_name(&copied, "asset_manifest.ron")?);

    let manifest = StageRoot::open(&copied)?.load_current(demo_game::GAME_ID)?;
    let executable = copied.join(manifest.executable_path().as_str());
    let output = Command::new(executable)
        .args(["--max-frames", "2"])
        .output()?;
    assert!(
        output.status.success(),
        "staged Player stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "presented_frames=2 input_frames=0\n"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::other("Stage contains a symlink"));
        }
        if metadata.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path)?;
            fs::set_permissions(&destination_path, metadata.permissions())?;
        } else {
            return Err(std::io::Error::other("Stage contains an unsupported path"));
        }
    }
    Ok(())
}

fn contains_extension(root: &Path, extension: &str) -> Result<bool, std::io::Error> {
    any_path(root, &|path| {
        path.extension().and_then(|value| value.to_str()) == Some(extension)
    })
}

fn contains_name(root: &Path, name: &str) -> Result<bool, std::io::Error> {
    any_path(root, &|path| {
        path.file_name().and_then(|value| value.to_str()) == Some(name)
    })
}

fn any_path(root: &Path, predicate: &dyn Fn(&Path) -> bool) -> Result<bool, std::io::Error> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if predicate(&path) || (path.is_dir() && any_path(&path, predicate)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

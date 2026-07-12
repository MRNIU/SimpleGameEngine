// Copyright The SimpleGameEngine Contributors

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
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
    let _window_manager = WindowManager::start()?;
    let child = Command::new(executable)
        .args(["--max-frames", "300"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let window = find_window(demo_game::GAME_ID)?;
    let injection = Command::new("xdotool")
        .args([
            "windowactivate",
            "--sync",
            &window,
            "keydown",
            "w",
            "sleep",
            "0.1",
            "keyup",
            "w",
        ])
        .output()?;
    assert!(
        injection.status.success(),
        "xdotool stderr: {}",
        String::from_utf8_lossy(&injection.stderr)
    );
    let output = child.wait_with_output()?;
    assert!(
        output.status.success(),
        "staged Player stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(report_value(&stdout, "presented_frames")? > 0);
    assert!(report_value(&stdout, "input_frames")? > 0);
    fs::remove_dir_all(root)?;
    Ok(())
}

struct WindowManager(std::process::Child);

impl WindowManager {
    fn start() -> Result<Self, std::io::Error> {
        let child = Command::new("openbox")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        thread::sleep(Duration::from_millis(100));
        Ok(Self(child))
    }
}

impl Drop for WindowManager {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn find_window(title: &str) -> Result<String, Box<dyn std::error::Error>> {
    for _ in 0..200 {
        let output = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--name", title])
            .output()?;
        if output.status.success()
            && let Some(window) = String::from_utf8(output.stdout)?
                .lines()
                .next()
                .map(str::to_owned)
        {
            return Ok(window);
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!("window did not appear: {title}").into())
}

fn report_value(output: &str, name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    output
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .ok_or_else(|| format!("missing {name} report").into())
        .and_then(|value| Ok(value.parse()?))
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

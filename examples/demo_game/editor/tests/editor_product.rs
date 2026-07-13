// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn editor_cli_has_stable_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg("--help")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Usage: demo-game-editor PROJECT_ROOT [--play] [--max-frames N] [--screenshot PATH] [--ui-action ACTION]...\n"
    );
    assert!(output.stderr.is_empty());
    let conflict = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .args([".", "--max-frames", "1", "--screenshot", "editor.png"])
        .output()?;
    assert!(!conflict.status.success());
    assert!(String::from_utf8(conflict.stderr)?.contains("cannot be combined"));
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn game_specific_editor_plays_and_paints_preview() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let _window_manager = WindowManager::start()?;
    let child = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--play", "--max-frames", "300"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let window = find_window("SimpleGameEngine Demo Editor")?;
    let injection = Command::new("xdotool")
        .args([
            "windowactivate",
            "--sync",
            &window,
            "mousemove",
            "--window",
            &window,
            "640",
            "400",
            "click",
            "1",
            "sleep",
            "0.1",
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
    let input_frames = stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("gameplay_input_frames="))
        .ok_or("missing gameplay_input_frames report")?
        .parse::<u64>()?;
    assert!(input_frames > 0);
    let key_w_frames = stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("gameplay_key_w_frames="))
        .ok_or("missing gameplay_key_w_frames report")?
        .parse::<u64>()?;
    assert!(key_w_frames > 0);
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn game_specific_editor_paints_the_authoring_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let screenshot = project.path().join("editor.png");
    let _window_manager = WindowManager::start()?;
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .arg("--screenshot")
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    for field in ["preview_prepare=", "preview_paint="] {
        let count = stdout
            .split_whitespace()
            .find_map(|value| value.strip_prefix(field))
            .ok_or("missing authoring preview report")?
            .parse::<u64>()?;
        assert!(count > 0);
    }
    assert!(stdout.contains("play_frames=0"));
    let screenshot = image::open(screenshot)?.to_rgba8();
    assert_eq!(screenshot.dimensions(), (1280, 720));
    let first = *screenshot.get_pixel(0, 0);
    assert!(screenshot.pixels().any(|pixel| *pixel != first));
    assert_eq!(grid_holes_inside_material(&screenshot), 0);
    Ok(())
}

fn grid_holes_inside_material(image: &image::RgbaImage) -> usize {
    let material = |pixel: &image::Rgba<u8>| {
        pixel[0] > 200
            && pixel[0] > pixel[1].saturating_add(100)
            && pixel[0] > pixel[2].saturating_add(100)
    };
    (0..image.height())
        .map(|y| {
            let material_x = (0..image.width())
                .filter(|x| material(image.get_pixel(*x, y)))
                .collect::<Vec<_>>();
            let (Some(first), Some(last)) = (material_x.first(), material_x.last()) else {
                return 0;
            };
            ((*first + 1)..*last)
                .filter(|x| {
                    let pixel = image.get_pixel(*x, y);
                    pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100
                })
                .count()
        })
        .sum()
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_selects_hierarchy_and_reads_back_inspector()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let screenshot = project.path().join("selected.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--ui-action", "select:1", "--screenshot"])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout)?.contains("ui_actions=1"));
    let screenshot = image::open(screenshot)?.to_rgba8();
    let dark_inspector_pixels = screenshot
        .enumerate_pixels()
        .filter(|(x, y, pixel)| *x >= 980 && *y >= 40 && pixel.0[..3].iter().all(|v| *v < 200))
        .count();
    assert!(dark_inspector_pixels > 2_000, "Inspector remained empty");
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_edits_saves_plays_stops_and_reads_back()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let scene = project.path().join("Scenes/main.scene.ron");
    let before = fs::read(&scene)?;
    let screenshot = project.path().join("workflow.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args([
            "--ui-action",
            "create-entity",
            "--ui-action",
            "undo",
            "--ui-action",
            "redo",
            "--ui-action",
            "save",
            "--ui-action",
            "play",
            "--ui-action",
            "stop",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout)?.contains("ui_actions=6"));
    let after = fs::read(&scene)?;
    assert_ne!(after, before);
    assert!(String::from_utf8(after)?.contains("Entity"));
    assert_eq!(
        image::open(screenshot)?.to_rgba8().dimensions(),
        (1280, 720)
    );
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_rejects_authoring_mutation_during_play()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let manifest = project.path().join("Content/asset_manifest.ron");
    let before_manifest = fs::read(&manifest)?;
    let meshes = project.path().join("Content/Meshes");
    let before_meshes = fs::read_dir(&meshes)?.count();
    let screenshot = project.path().join("play-rejected.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--play", "--ui-action", "create-cube", "--screenshot"])
        .arg(&screenshot)
        .output()?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("UiActionsIncomplete { expected: 1, completed: 0 }")
    );
    assert_eq!(fs::read(manifest)?, before_manifest);
    assert_eq!(fs::read_dir(meshes)?.count(), before_meshes);
    assert_eq!(
        image::open(screenshot)?.to_rgba8().dimensions(),
        (1280, 720)
    );
    Ok(())
}

#[test]
#[ignore = "runs a real Build and requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_waits_for_build_before_readback() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let screenshot = project.path().join("built.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .current_dir(std::env::temp_dir())
        .arg(project.path())
        .args(["--ui-action", "build", "--screenshot"])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout)?.contains("ui_actions=1"));
    assert_eq!(
        image::open(screenshot)?.to_rgba8().dimensions(),
        (1280, 720)
    );
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
            "Scenes/main.scene.ron",
        ] {
            fs::copy(demo_root().join(relative), root.join(relative))?;
        }
        for entry in fs::read_dir(demo_root().join("Content/Meshes"))? {
            let entry = entry?;
            fs::copy(
                entry.path(),
                root.join("Content/Meshes").join(entry.file_name()),
            )?;
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

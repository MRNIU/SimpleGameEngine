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
        "Usage: demo-game-editor PROJECT_ROOT [--language en|zh-CN] [--play] [--max-frames N] [--screenshot PATH] [--ui-action ACTION]...\n"
    );
    assert!(output.stderr.is_empty());
    let conflict = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .args([".", "--max-frames", "1", "--screenshot", "editor.png"])
        .output()?;
    assert!(!conflict.status.success());
    assert!(String::from_utf8(conflict.stderr)?.contains("cannot be combined"));
    let invalid_language = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .args([".", "--language", "zh"])
        .output()?;
    assert!(!invalid_language.status.success());
    assert!(String::from_utf8(invalid_language.stderr)?.contains("must be en or zh-CN"));
    Ok(())
}

#[test]
#[ignore = "requires a window system and an installed CJK font; run with xvfb-run"]
fn simplified_chinese_editor_paints_localized_chrome() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let screenshot = project.path().join("editor-zh-cn.png");
    let _window_manager = WindowManager::start()?;
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args([
            "--language",
            "zh-CN",
            "--ui-action",
            "language:en",
            "--ui-action",
            "language:zh-CN",
            "--ui-action",
            "select:1",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        report_value(&String::from_utf8(output.stdout)?, "ui_actions")?,
        3
    );
    let screenshot = image::open(screenshot)?.to_rgba8();
    assert_editor_screenshot_size(&screenshot);
    let first = *screenshot.get_pixel(0, 0);
    assert!(screenshot.pixels().any(|pixel| *pixel != first));
    let visible_viewport_pixels = (80..screenshot.height())
        .flat_map(|y| (230..970).map(move |x| (x, y)))
        .filter(|(x, y)| {
            screenshot.get_pixel(*x, *y).0[..3]
                .iter()
                .copied()
                .max()
                .is_some_and(|value| value > 30)
        })
        .count();
    assert!(visible_viewport_pixels > 1_000);
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
    assert_editor_screenshot_size(&screenshot);
    let first = *screenshot.get_pixel(0, 0);
    assert!(screenshot.pixels().any(|pixel| *pixel != first));
    assert_eq!(grid_holes_inside_material(&screenshot), 0);
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn editor_switches_from_wgpu_to_cpu_without_changing_scene_data()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let scene = project.path().join("Scenes/main.scene.ron");
    let before = fs::read(&scene)?;
    let screenshot = project.path().join("cpu-backend.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--ui-action", "backend:cpu", "--screenshot"])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        report_value(&stdout, "preview_wgpu_prepare")? > 0,
        "{stdout}"
    );
    assert!(
        report_value(&stdout, "preview_cpu_prepare")? > 0,
        "{stdout}"
    );
    assert_eq!(report_value(&stdout, "ui_actions")?, 1);
    assert_eq!(fs::read(scene)?, before);
    let image = image::open(screenshot)?.to_rgba8();
    assert_editor_screenshot_size(&image);
    let first = *image.get_pixel(0, 0);
    assert!(image.pixels().any(|pixel| *pixel != first));
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn editor_switches_all_render_modes_without_changing_scene_data()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let scene = project.path().join("Scenes/main.scene.ron");
    let manifest = project.path().join("Content/asset_manifest.ron");
    let descriptor = project.path().join("project.sge.ron");
    let before = [
        fs::read(&scene)?,
        fs::read(&manifest)?,
        fs::read(&descriptor)?,
    ];
    let screenshot = project.path().join("lit-wireframe.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args([
            "--ui-action",
            "mode:unlit",
            "--ui-action",
            "mode:wireframe",
            "--ui-action",
            "mode:lit-wireframe",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "editor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        report_value(&String::from_utf8(output.stdout)?, "ui_actions")?,
        3
    );
    assert_eq!(fs::read(scene)?, before[0]);
    assert_eq!(fs::read(manifest)?, before[1]);
    assert_eq!(fs::read(descriptor)?, before[2]);
    assert!(!project.path().join("Cook").exists());
    assert!(!project.path().join("Stage").exists());
    assert_editor_screenshot_size(&image::open(screenshot)?.to_rgba8());
    Ok(())
}

#[test]
#[ignore = "requires a real window manager; run with xvfb-run"]
fn dirty_native_window_close_waits_for_user_confirmation() -> Result<(), Box<dyn std::error::Error>>
{
    let project = TestProject::new()?;
    let scene = project.path().join("Scenes/main.scene.ron");
    let before = fs::read(&scene)?;
    let _window_manager = WindowManager::start()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--ui-action", "create-empty-actor"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let window = find_window("SimpleGameEngine Demo Editor")?;
    thread::sleep(Duration::from_millis(200));
    let close = Command::new("xdotool")
        .args(["windowactivate", "--sync", &window, "key", "alt+F4"])
        .output()?;
    assert!(
        close.status.success(),
        "xdotool stderr: {}",
        String::from_utf8_lossy(&close.stderr)
    );
    thread::sleep(Duration::from_millis(200));
    let premature_exit = child.try_wait()?;
    if premature_exit.is_none() {
        child.kill()?;
    }
    let output = child.wait_with_output()?;

    assert!(
        premature_exit.is_none(),
        "dirty Editor exited without confirmation: {premature_exit:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(scene)?, before);
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
                    (20..100).contains(&pixel[0])
                        && pixel[0].abs_diff(pixel[1]) <= 4
                        && pixel[0].abs_diff(pixel[2]) <= 4
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
    assert_editor_screenshot_size(&screenshot);
    let scale = screenshot.width() as f32 / 1280.0;
    let inspector_left = screenshot
        .width()
        .saturating_sub((300.0 * scale).round() as u32);
    let inspector_top = (40.0 * scale).round() as u32;
    let dark_inspector_pixels = screenshot
        .enumerate_pixels()
        .filter(|(x, y, pixel)| {
            *x >= inspector_left && *y >= inspector_top && pixel.0[..3].iter().all(|v| *v < 200)
        })
        .count();
    assert!(
        dark_inspector_pixels > (2_000.0 * scale * scale) as usize,
        "Inspector remained empty"
    );
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
            "create-empty-actor",
            "--ui-action",
            "create-cube",
            "--ui-action",
            "duplicate",
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
    assert!(String::from_utf8(output.stdout)?.contains("ui_actions=8"));
    let after = fs::read(&scene)?;
    assert_ne!(after, before);
    let after = String::from_utf8(after)?;
    assert!(after.contains("Empty Actor"));
    assert!(after.contains("Cube"));
    assert!(after.contains("Cube Copy"));
    assert!(after.contains("sge.material"));
    assert_editor_screenshot_size(&image::open(screenshot)?.to_rgba8());
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_rejects_authoring_and_build_actions_during_play()
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
    assert_editor_screenshot_size(&image::open(screenshot)?.to_rgba8());

    let build_screenshot = project.path().join("play-build-rejected.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--play", "--ui-action", "build", "--screenshot"])
        .arg(&build_screenshot)
        .output()?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("UiActionsIncomplete { expected: 1, completed: 0 }")
    );
    assert_editor_screenshot_size(&image::open(build_screenshot)?.to_rgba8());
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
    assert_editor_screenshot_size(&image::open(screenshot)?.to_rgba8());
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_cannot_report_a_dirty_unconfirmed_build_as_complete()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new()?;
    let scene = project.path().join("Scenes/main.scene.ron");
    let before = fs::read(&scene)?;
    let screenshot = project.path().join("dirty-build-confirmation.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args([
            "--ui-action",
            "create-empty-actor",
            "--ui-action",
            "build",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()?;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("UiActionsIncomplete { expected: 2, completed: 1 }")
    );
    assert_eq!(fs::read(scene)?, before);
    assert_editor_screenshot_size(&image::open(screenshot)?.to_rgba8());
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn internal_ui_tape_paints_error_feedback_before_readback() -> Result<(), Box<dyn std::error::Error>>
{
    let project = TestProject::new()?;
    let screenshot = project.path().join("error-feedback.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-editor"))
        .arg(project.path())
        .args(["--ui-action", "undo", "--screenshot"])
        .arg(&screenshot)
        .output()?;
    assert!(!output.status.success());
    let screenshot = image::open(screenshot)?.to_rgba8();
    assert_editor_screenshot_size(&screenshot);
    let scale = screenshot.height() as f32 / 720.0;
    let error_top = screenshot
        .height()
        .saturating_sub((60.0 * scale).round() as u32);
    let red_pixels = screenshot
        .enumerate_pixels()
        .filter(|(x, y, pixel)| {
            *x <= (300.0 * scale).round() as u32
                && *y >= error_top
                && pixel[0] > 170
                && pixel[1] < 150
                && pixel[2] < 150
        })
        .count();
    assert!(
        red_pixels > (20.0 * scale * scale) as usize,
        "error feedback is not visible before screenshot readback"
    );
    Ok(())
}

fn assert_editor_screenshot_size(image: &image::RgbaImage) {
    let (width, height) = image.dimensions();
    assert!(
        width >= 1280 && height >= 720,
        "unexpected size: {width}x{height}"
    );
    assert_eq!(
        u64::from(width) * 720,
        u64::from(height) * 1280,
        "unexpected aspect ratio: {width}x{height}"
    );
}

fn report_value(output: &str, name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    output
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .ok_or_else(|| format!("missing {name} report").into())
        .and_then(|value| Ok(value.parse()?))
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
    for _ in 0..1_000 {
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
        fs::create_dir_all(root.join("Scenes"))?;
        for relative in ["project.sge.ron", "Scenes/main.scene.ron"] {
            fs::copy(demo_root().join(relative), root.join(relative))?;
        }
        copy_tree(&demo_root().join("Content"), &root.join("Content"))?;
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

fn copy_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination_path)?;
        } else {
            fs::copy(entry.path(), destination_path)?;
        }
    }
    Ok(())
}

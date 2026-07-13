// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    thread,
    time::Duration,
};

use sge_asset::{AssetId, AssetRef};
use sge_asset_pipeline::{CookOutputRoot, full_cook, import_project_assets};
use sge_input::{Button, InputFrame, KeyCode};
use sge_player::PlayerSession;
use sge_project::{AuthoringAssetManifest, ProjectDescriptor, ProjectRoot};

const DEMO_ASSET: &str = "40000000-0000-4000-8000-000000000001";

#[test]
fn demo_project_import_cook_and_player_use_the_same_game_and_mesh()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = CookedDemo::new("headless")?;
    let project = fixture.project()?;
    let descriptor = ProjectDescriptor::load(&project)?;
    descriptor.validate_for_game(demo_game::GAME_ID)?;
    let manifest = AuthoringAssetManifest::load(&project)?;
    let imported = import_project_assets(&project, &manifest)?;
    let asset = AssetId::from_str(DEMO_ASSET)?;
    let imported_mesh = imported.store().mesh(AssetRef::new(asset))?.clone();
    assert_eq!(imported_mesh.indices().len(), 36);
    for axis in 0..3 {
        let bounds = imported_mesh.vertices().iter().fold(
            [f32::INFINITY, f32::NEG_INFINITY],
            |[minimum, maximum], vertex| {
                [
                    minimum.min(vertex.position()[axis]),
                    maximum.max(vertex.position()[axis]),
                ]
            },
        );
        assert_eq!(bounds, [-0.5, 0.5]);
    }

    fixture.cook(&project)?;
    fixture.delete_source()?;
    let mut session = PlayerSession::load(demo_game::GAME, fixture.path())?;
    let cooked_mesh = session.assets().mesh(AssetRef::new(asset))?;
    assert_eq!(cooked_mesh, &imported_mesh);
    let (snapshot, view) = session.render_frame()?;
    let before = snapshot.meshes()[0].transform();
    let mut input = InputFrame::new();
    input.hold(Button::Key(KeyCode::KeyW));
    session.advance(Duration::from_millis(20), input)?;
    let (after_snapshot, _) = session.render_frame()?;
    let after = after_snapshot.meshes()[0].transform();

    assert_eq!(snapshot.cameras().len(), 1);
    assert_eq!(snapshot.meshes().len(), 1);
    assert_eq!(snapshot.lights().len(), 1);
    assert!(view.camera().active());
    assert!(after.translation[2] < before.translation[2]);
    assert_ne!(after.rotation, before.rotation);
    Ok(())
}

#[test]
fn player_cli_has_stable_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-player"))
        .arg("--help")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Usage: demo-game-player [COOKED_ROOT] [--max-frames N] [--screenshot PATH]\n"
    );
    assert!(output.stderr.is_empty());
    let conflict = Command::new(env!("CARGO_BIN_EXE_demo-game-player"))
        .args(["--max-frames", "1", "--screenshot", "player.png"])
        .output()?;
    assert!(!conflict.status.success());
    assert!(String::from_utf8(conflict.stderr)?.contains("cannot be combined"));
    Ok(())
}

#[test]
#[ignore = "requires a real WGPU window; run with xvfb-run"]
fn game_specific_player_reads_back_presented_surface() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = CookedDemo::new("readback")?;
    fixture.cook(&fixture.project()?)?;
    fixture.delete_source()?;
    let screenshot = fixture.base.join("player.png");
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-player"))
        .arg(fixture.path())
        .arg("--screenshot")
        .arg(&screenshot)
        .output()?;
    assert!(
        output.status.success(),
        "player stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        report_value(&String::from_utf8(output.stdout)?, "presented_frames")?,
        1
    );
    let screenshot = image::open(screenshot)?.to_rgba8();
    let (width, height) = screenshot.dimensions();
    assert!(width >= 1280 && height >= 720);
    assert_eq!(u64::from(width) * 720, u64::from(height) * 1280);
    let corner = *screenshot.get_pixel(0, 0);
    let visible_pixels = screenshot
        .pixels()
        .filter(|pixel| {
            pixel.0[..3]
                .iter()
                .zip(corner.0[..3].iter())
                .any(|(channel, background)| channel.abs_diff(*background) > 16)
        })
        .count();
    assert!(visible_pixels > (u64::from(width) * u64::from(height) / 100) as usize);
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn game_specific_player_routes_input_and_presents_from_cooked_content()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = CookedDemo::new("window")?;
    fixture.cook(&fixture.project()?)?;
    fixture.delete_source()?;
    let _window_manager = WindowManager::start()?;
    let child = Command::new(env!("CARGO_BIN_EXE_demo-game-player"))
        .arg(fixture.path())
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
        "player stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    let presented = report_value(&stdout, "presented_frames")?;
    let input_frames = report_value(&stdout, "input_frames")?;
    assert!(presented > 0);
    assert!(input_frames > 0);
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

fn report_value(output: &str, name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    output
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .ok_or_else(|| format!("missing {name} report").into())
        .and_then(|value| Ok(value.parse()?))
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

struct CookedDemo {
    base: PathBuf,
    source: PathBuf,
    cooked: PathBuf,
}

impl CookedDemo {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/tmp/demo_game_m4")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let source = base.join("source");
        let cooked = base.join("cooked");
        fs::create_dir_all(source.join("Content/Meshes"))?;
        fs::create_dir_all(source.join("Scenes"))?;
        fs::create_dir(&cooked)?;
        for relative in [
            "project.sge.ron",
            "Content/asset_manifest.ron",
            "Scenes/main.scene.ron",
        ] {
            fs::copy(demo_root().join(relative), source.join(relative))?;
        }
        for entry in fs::read_dir(demo_root().join("Content/Meshes"))? {
            let entry = entry?;
            fs::copy(
                entry.path(),
                source.join("Content/Meshes").join(entry.file_name()),
            )?;
        }
        Ok(Self {
            base,
            source,
            cooked,
        })
    }

    fn cook(&self, project: &ProjectRoot) -> Result<(), Box<dyn std::error::Error>> {
        let app = demo_game::GAME.create_app()?;
        full_cook(
            project,
            demo_game::GAME_ID,
            app.type_registry(),
            app.world(),
            &CookOutputRoot::open(&self.cooked)?,
        )?;
        Ok(())
    }

    fn project(&self) -> Result<ProjectRoot, sge_project::ProjectIoError> {
        ProjectRoot::open(&self.source)
    }

    fn delete_source(&self) -> Result<(), std::io::Error> {
        fs::remove_dir_all(&self.source)
    }

    fn path(&self) -> &Path {
        &self.cooked
    }
}

impl Drop for CookedDemo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

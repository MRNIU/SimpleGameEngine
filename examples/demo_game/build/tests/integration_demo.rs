// Copyright The SimpleGameEngine Contributors
//
//! M7 single-chain integration demo across authoring, Play, Build, Stage, and Player.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use demo_game::{GameRuntimeState, PlayerController, Rotator};
use sge_asset::RuntimeContentRoot;
use sge_build::StageRoot;
use sge_editor::EditSession;
use sge_input::{Button, InputFrame, KeyCode};
use sge_math::Transform;
use sge_player::PlayerSession;
use sge_reflect::Value;
use sge_render::{Camera, Light, MeshRenderer, RenderSnapshot};
use sge_scene::{AuthoringEntity, RuntimeScene, SceneEntityId};

const ASSET_ID: &str = "40000000-0000-4000-8000-000000000001";
const CAMERA_ID: &str = "50000000-0000-4000-8000-000000000001";
const MESH_ID: &str = "50000000-0000-4000-8000-000000000002";
const LIGHT_ID: &str = "50000000-0000-4000-8000-000000000003";
const CHILD_ID: &str = "60000000-0000-4000-8000-000000000001";

#[test]
#[ignore = "runs real sge build and staged X11/WGPU Player; run with xvfb-run"]
fn independent_demo_closes_the_complete_engine_spine() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let mesh = MESH_ID.parse::<SceneEntityId>()?;
    let child = CHILD_ID.parse::<SceneEntityId>()?;
    let mut edit = EditSession::open(demo_game::GAME, &fixture.project)?;

    assert_eq!(
        edit.component::<MeshRenderer>(mesh)
            .ok_or("missing MeshRenderer")?
            .mesh()
            .id()
            .to_string(),
        ASSET_ID
    );
    assert!(
        edit.component::<Camera>(CAMERA_ID.parse::<SceneEntityId>()?)
            .is_some()
    );
    assert!(
        edit.component::<Light>(LIGHT_ID.parse::<SceneEntityId>()?)
            .is_some()
    );
    edit.add_entity(AuthoringEntity::new(child, Some(mesh), Vec::new())?)?;
    edit.select(Some(mesh))?;
    let inspector = edit.inspector()?;
    assert!(
        inspector
            .iter()
            .any(|component| component.type_key().as_str() == "demo.player_controller")
    );
    let rotator = inspector
        .iter()
        .find(|component| component.type_key().as_str() == "demo.rotator")
        .ok_or("Inspector did not restore Rotator")?;
    let field = rotator
        .fields()
        .iter()
        .find(|field| field.field_key().as_str() == "radians_per_second")
        .ok_or("Inspector did not restore Rotator field")?;
    let Value::F32(current_speed) = field.value() else {
        return Err("Rotator Inspector field has the wrong value kind".into());
    };
    let edited_speed = *current_speed + 0.5;
    edit.set_field(
        mesh,
        rotator.type_key().as_str(),
        field.field_key().as_str(),
        Value::F32(edited_speed),
    )?;
    edit.undo()?;
    assert_eq!(
        edit.component::<Rotator>(mesh)
            .ok_or("missing Rotator after undo")?
            .radians_per_second(),
        *current_speed
    );
    edit.redo()?;
    edit.save()?;
    drop(edit);

    let edit = EditSession::open(demo_game::GAME, &fixture.project)?;
    let authoring = edit.snapshot()?;
    assert!(
        authoring
            .entities()
            .any(|entity| entity.id() == child && entity.parent() == Some(mesh))
    );
    assert_eq!(
        edit.component::<Rotator>(mesh)
            .ok_or("missing reopened Rotator")?
            .radians_per_second(),
        edited_speed
    );
    assert!(edit.component::<PlayerController>(mesh).is_some());
    let edit_before_play = authoring.to_ron()?;
    let mut play = edit.start_play()?;
    let (play_initial, play_view) = play.render_frame()?;
    assert!(play_view.camera().active());
    let before_transform = *play
        .component::<Transform>(mesh)
        .ok_or("PlayWorld is missing mesh Transform")?;
    let mut input = InputFrame::new();
    input.hold(Button::Key(KeyCode::KeyW));
    play.advance(Duration::from_millis(20), input)?;
    let after_transform = *play
        .component::<Transform>(mesh)
        .ok_or("advanced PlayWorld is missing mesh Transform")?;
    assert!(after_transform.translation[2] < before_transform.translation[2]);
    assert_ne!(after_transform.rotation, before_transform.rotation);
    let state = play
        .resource::<GameRuntimeState>()
        .ok_or("PlayWorld is missing GameRuntimeState")?;
    assert_eq!(state.startup_runs(), 1);
    assert!(state.fixed_updates() > 0);
    assert!(state.updates() > 0);
    assert!(state.post_updates() > 0);
    drop(play);
    assert_eq!(edit.snapshot()?.to_ron()?, edit_before_play);
    drop(edit);

    fixture.run_sge_build()?;
    copy_tree(&fixture.stage, &fixture.copied_stage)?;
    assert!(!contains_extension(&fixture.copied_stage, "obj")?);
    assert!(!contains_name(&fixture.copied_stage, "asset_manifest.ron")?);
    fs::remove_dir_all(&fixture.project)?;

    let manifest = StageRoot::open(&fixture.copied_stage)?.load_current(demo_game::GAME_ID)?;
    let runtime = fixture.copied_stage.join(manifest.runtime_root().as_str());
    assert_cooked_authoring_changes(&runtime, child, mesh, edited_speed)?;
    let player = PlayerSession::load(demo_game::GAME, &runtime)?;
    let (player_snapshot, player_view) = player.render_frame()?;
    assert!(player_view.camera().active());
    assert_semantic_snapshot_eq(&play_initial, &player_snapshot);

    let executable = fixture
        .copied_stage
        .join(manifest.executable_path().as_str());
    run_staged_player_with_input(&executable)?;
    Ok(())
}

fn assert_cooked_authoring_changes(
    runtime_root: &Path,
    child: SceneEntityId,
    mesh: SceneEntityId,
    edited_speed: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = RuntimeContentRoot::open(runtime_root)?;
    let generation = content.load_current(demo_game::GAME_ID)?;
    let scene = RuntimeScene::from_ron(std::str::from_utf8(generation.entry_scene_bytes())?)?;
    assert!(
        scene
            .entities()
            .any(|entity| entity.id() == child && entity.parent() == Some(mesh))
    );
    let mesh_entity = scene
        .entities()
        .find(|entity| entity.id() == mesh)
        .ok_or("cooked scene is missing mesh entity")?;
    let rotator = mesh_entity
        .components()
        .find(|component| component.type_key().as_str() == "demo.rotator")
        .ok_or("cooked scene is missing Rotator")?;
    assert_eq!(
        rotator.fields().get("radians_per_second"),
        Some(&Value::F32(edited_speed))
    );
    assert!(
        mesh_entity
            .components()
            .any(|component| component.type_key().as_str() == "demo.player_controller")
    );
    Ok(())
}

fn assert_semantic_snapshot_eq(play: &RenderSnapshot, player: &RenderSnapshot) {
    assert_eq!(play.cameras().len(), player.cameras().len());
    assert_eq!(play.meshes().len(), player.meshes().len());
    assert_eq!(play.lights().len(), player.lights().len());
    for (left, right) in play.cameras().iter().zip(player.cameras()) {
        assert_eq!(left.transform(), right.transform());
        assert_eq!(left.camera(), right.camera());
    }
    for (left, right) in play.meshes().iter().zip(player.meshes()) {
        assert_eq!(left.transform(), right.transform());
        assert_eq!(left.mesh().id(), right.mesh().id());
        assert_eq!(left.material(), right.material());
    }
    for (left, right) in play.lights().iter().zip(player.lights()) {
        assert_eq!(left.transform(), right.transform());
        assert_eq!(left.light(), right.light());
    }
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    project: PathBuf,
    stage: PathBuf,
    copied_stage: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, std::io::Error> {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = workspace
            .join("target/tmp/m7-integration-demo")
            .join(std::process::id().to_string());
        let _ = fs::remove_dir_all(&root);
        let project = root.join("Project");
        fs::create_dir_all(project.join("Content/Meshes"))?;
        fs::create_dir_all(project.join("Scenes"))?;
        let source = workspace.join("examples/demo_game");
        for relative in [
            "project.sge.ron",
            "Content/asset_manifest.ron",
            "Content/Meshes/demo.obj",
            "Scenes/main.scene.ron",
        ] {
            fs::copy(source.join(relative), project.join(relative))?;
        }
        Ok(Self {
            stage: root.join("Stage"),
            copied_stage: root.join("CopiedStage"),
            root,
            workspace,
            project,
        })
    }

    fn run_sge_build(&self) -> Result<(), Box<dyn std::error::Error>> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let output = Command::new(cargo)
            .current_dir(&self.workspace)
            .args([
                "run",
                "--package",
                "sge-build",
                "--bin",
                "sge",
                "--",
                "build",
            ])
            .arg(&self.project)
            .arg("--workspace")
            .arg(&self.workspace)
            .arg("--stage")
            .arg(&self.stage)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "sge build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into())
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
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

fn run_staged_player_with_input(executable: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let _window_manager = WindowManager::start()?;
    let child = ChildGuard::new(
        Command::new(executable)
            .args(["--max-frames", "300"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
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
    if !injection.status.success() {
        return Err(format!(
            "xdotool failed: {}",
            String::from_utf8_lossy(&injection.stderr)
        )
        .into());
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "staged Player failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    assert!(report_value(&stdout, "presented_frames")? > 0);
    assert!(report_value(&stdout, "input_frames")? > 0);
    Ok(())
}

struct ChildGuard(Option<std::process::Child>);

impl ChildGuard {
    fn new(child: std::process::Child) -> Self {
        Self(Some(child))
    }

    fn wait_with_output(mut self) -> Result<std::process::Output, std::io::Error> {
        self.0
            .take()
            .ok_or_else(|| std::io::Error::other("child process was already consumed"))?
            .wait_with_output()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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

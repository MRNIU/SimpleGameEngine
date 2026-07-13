// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_app::{EngineApp, EngineBuildError, GameDescriptor};
use sge_editor::{EditSession, EditorOpenError, EditorPreviewError, EditorWorkspace};

static FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

#[test]
fn demo_project_opens_as_a_preview_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("open")?;
    let session = EditSession::open(demo_game::GAME, project.path())?;
    let frame = session.preview_frame()?;

    assert_eq!(session.descriptor().game_id().as_str(), demo_game::GAME_ID);
    assert_eq!(session.manifest().records().len(), 1);
    assert_eq!(session.snapshot()?.entities().count(), 3);
    assert_eq!(frame.snapshot.meshes().len(), 1);
    assert_eq!(frame.snapshot.lights().len(), 1);
    Ok(())
}

#[test]
fn identity_failure_wins_and_preserves_live_session() -> Result<(), Box<dyn std::error::Error>> {
    let valid = TestProject::new("live")?;
    let invalid = TestProject::new("candidate")?;
    fs::write(
        invalid.path().join("Content/asset_manifest.ron"),
        b"corrupt",
    )?;
    FACTORY_CALLS.store(0, Ordering::SeqCst);
    let mut workspace = EditorWorkspace::default();
    workspace.replace(demo_game::GAME, valid.path())?;
    let previous_game = workspace
        .live()
        .ok_or("missing live session")?
        .descriptor()
        .game_id()
        .as_str()
        .to_owned();

    let error = workspace
        .replace(
            GameDescriptor::new("wrong.game", counted_app),
            invalid.path(),
        )
        .expect_err("identity mismatch must fail");

    assert!(matches!(
        error,
        EditorOpenError::Descriptor(sge_project::ProjectFormatError::GameMismatch { .. })
    ));
    assert_eq!(FACTORY_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(
        workspace
            .live()
            .ok_or("live session was replaced")?
            .descriptor()
            .game_id()
            .as_str(),
        previous_game
    );
    Ok(())
}

#[test]
fn every_candidate_stage_failure_preserves_the_live_session()
-> Result<(), Box<dyn std::error::Error>> {
    let live = TestProject::new("stage-live")?;
    let mut workspace = EditorWorkspace::default();
    workspace.replace(demo_game::GAME, live.path())?;
    let live_address = workspace.live().ok_or("missing live session")? as *const _ as usize;

    let manifest = TestProject::new("bad-manifest")?;
    fs::write(manifest.path().join("Content/asset_manifest.ron"), b"bad")?;
    assert_preserved(&mut workspace, manifest.path(), live_address, |error| {
        matches!(error, EditorOpenError::Manifest(_))
    })?;

    let source = TestProject::new("bad-source")?;
    fs::write(source.path().join("Content/Meshes/demo.obj"), b"bad obj")?;
    assert_preserved(&mut workspace, source.path(), live_address, |error| {
        matches!(error, EditorOpenError::Import(_))
    })?;

    let format = TestProject::new("bad-scene-format")?;
    fs::write(format.path().join("Scenes/main.scene.ron"), b"bad scene")?;
    assert_preserved(&mut workspace, format.path(), live_address, |error| {
        matches!(error, EditorOpenError::SceneFormat(_))
    })?;

    let prepare = TestProject::new("bad-prepare")?;
    prepare.replace_scene(
        "40000000-0000-4000-8000-000000000001",
        "40000000-0000-4000-8000-000000000099",
    )?;
    assert_preserved(&mut workspace, prepare.path(), live_address, |error| {
        matches!(error, EditorOpenError::SceneValidation(_))
    })?;

    Ok(())
}

#[test]
fn extraction_errors_remain_typed_while_scene_camera_is_optional_for_authoring()
-> Result<(), Box<dyn std::error::Error>> {
    let extraction = TestProject::new("bad-extract")?;
    extraction.replace_scene(MATERIAL_COMPONENT, "")?;
    let extraction_session = EditSession::open(demo_game::GAME, extraction.path())?;
    assert!(matches!(
        extraction_session.preview_frame(),
        Err(EditorPreviewError::Extraction(_))
    ));

    let view = TestProject::new("bad-view")?;
    view.replace_scene("\"active\": Bool(true)", "\"active\": Bool(false)")?;
    let view_session = EditSession::open(demo_game::GAME, view.path())?;
    assert!(view_session.preview_frame().is_ok());
    Ok(())
}

fn assert_preserved(
    workspace: &mut EditorWorkspace,
    candidate: &Path,
    live_address: usize,
    expected: impl FnOnce(&EditorOpenError) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = workspace
        .replace(demo_game::GAME, candidate)
        .expect_err("candidate must fail");
    assert!(expected(&error), "unexpected candidate error: {error}");
    assert_eq!(
        workspace.live().ok_or("live session disappeared")? as *const _ as usize,
        live_address
    );
    Ok(())
}

fn counted_app() -> Result<EngineApp, EngineBuildError> {
    FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    demo_game::GAME.create_app()
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_editor")
            .join(format!("{name}-{}", std::process::id()));
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

    fn replace_scene(&self, from: &str, to: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.root.join("Scenes/main.scene.ron");
        let scene = fs::read_to_string(&path)?;
        if !scene.contains(from) {
            return Err(format!("scene replacement source is missing: {from:?}").into());
        }
        fs::write(path, scene.replacen(from, to, 1))?;
        Ok(())
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game")
}

const MATERIAL_COMPONENT: &str = r#"                (
                    type_key: "sge.material",
                    schema_version: 1,
                    fields: ({
                        "base_color": Color((0.9, 0.25, 0.1, 1.0)),
                    }),
                ),
"#;

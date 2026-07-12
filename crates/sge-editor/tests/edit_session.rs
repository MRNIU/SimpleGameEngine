// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use sge_editor::{EditError, EditSession, EditorPreviewError};
use sge_input::{Button, InputFrame, KeyCode};
use sge_math::Transform;
use sge_reflect::{FieldKind, Value};
use sge_scene::{AuthoringEntity, SceneEntityId};

const CAMERA: &str = "50000000-0000-4000-8000-000000000001";
const MESH: &str = "50000000-0000-4000-8000-000000000002";
const NEW_PARENT: &str = "60000000-0000-4000-8000-000000000001";
const NEW_CHILD: &str = "60000000-0000-4000-8000-000000000002";
const DEMO_ASSET: &str = "40000000-0000-4000-8000-000000000001";

#[test]
fn inspector_field_edit_is_validated_and_undoable() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("field-history")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;
    let mesh = id(MESH)?;
    session.select(Some(mesh))?;

    let inspector = session.inspector()?;
    let rotator = inspector
        .iter()
        .find(|component| component.type_key().as_str() == "demo.rotator")
        .ok_or("missing Rotator inspector")?;
    assert_eq!(rotator.display_name(), "Rotator");
    let speed = rotator
        .fields()
        .iter()
        .find(|field| field.field_key().as_str() == "radians_per_second")
        .ok_or("missing Rotator speed field")?;
    assert_eq!(speed.display_name(), "Radians Per Second");
    assert_eq!(speed.kind(), &FieldKind::F32);

    session.set_field(mesh, "demo.rotator", "radians_per_second", Value::F32(2.5))?;
    assert!(session.is_dirty());
    assert_eq!(session.history_cursor(), 1);
    assert_eq!(
        field(&session, mesh, "demo.rotator", "radians_per_second")?,
        Value::F32(2.5)
    );

    session.undo()?;
    assert!(!session.is_dirty());
    assert_eq!(session.history_cursor(), 0);
    assert_eq!(
        field(&session, mesh, "demo.rotator", "radians_per_second")?,
        Value::F32(1.0)
    );

    session.redo()?;
    assert_eq!(
        field(&session, mesh, "demo.rotator", "radians_per_second")?,
        Value::F32(2.5)
    );
    let before = session.snapshot()?.to_ron()?;
    let cursor = session.history_cursor();
    assert!(
        session
            .set_field(
                mesh,
                "demo.player_controller",
                "movement_speed",
                Value::F32(0.0),
            )
            .is_err()
    );
    assert_eq!(session.snapshot()?.to_ron()?, before);
    assert_eq!(session.history_cursor(), cursor);
    Ok(())
}

#[test]
fn component_and_leaf_entity_snapshots_share_generic_history()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("snapshot-history")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;
    let mesh = id(MESH)?;

    session.add_component(mesh, "sge.light")?;
    assert!(has_component(&session, mesh, "sge.light")?);
    session.undo()?;
    assert!(!has_component(&session, mesh, "sge.light")?);
    session.redo()?;
    session.remove_component(mesh, "sge.light")?;
    assert!(!has_component(&session, mesh, "sge.light")?);
    session.undo()?;
    assert!(has_component(&session, mesh, "sge.light")?);

    let parent = id(NEW_PARENT)?;
    let child = id(NEW_CHILD)?;
    session.add_entity(AuthoringEntity::new(parent, None, Vec::new())?)?;
    let cursor = session.history_cursor();
    assert!(session.add_component(parent, "sge.mesh_renderer").is_err());
    assert_eq!(session.history_cursor(), cursor);
    let draft = session.component_draft("sge.mesh_renderer")?;
    let draft = session.set_component_draft_field(
        &draft,
        "mesh",
        Value::Reference(DEMO_ASSET.to_owned()),
    )?;
    session.add_component_value(parent, draft)?;
    assert!(has_component(&session, parent, "sge.mesh_renderer")?);
    session.undo()?;
    assert!(!has_component(&session, parent, "sge.mesh_renderer")?);
    session.redo()?;
    session.add_entity(AuthoringEntity::new(child, Some(parent), Vec::new())?)?;
    session.select(Some(parent))?;
    let cursor = session.history_cursor();
    assert!(matches!(
        session.remove_entity(parent),
        Err(EditError::EntityHasChildren { entity }) if entity == parent
    ));
    assert_eq!(session.history_cursor(), cursor);
    assert_eq!(session.selection(), Some(parent));

    session.select(Some(child))?;
    session.remove_entity(child)?;
    assert_eq!(session.selection(), None);
    session.undo()?;
    assert!(has_entity(&session, child)?);
    assert_eq!(session.selection(), None);
    Ok(())
}

#[test]
fn saved_cursor_and_atomic_save_follow_committed_history() -> Result<(), Box<dyn std::error::Error>>
{
    let project = TestProject::new("save")?;
    let mesh = id(MESH)?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;
    session.set_field(mesh, "demo.rotator", "radians_per_second", Value::F32(4.0))?;
    session.save()?;
    assert!(!session.is_dirty());
    assert_eq!(session.saved_cursor(), Some(1));
    session.undo()?;
    session.set_field(
        mesh,
        "sge.material",
        "base_color",
        Value::Color([0.2, 0.3, 0.4, 1.0]),
    )?;
    assert_eq!(session.saved_cursor(), None);
    drop(session);

    let mut reopened = EditSession::open(demo_game::GAME, project.path())?;
    assert_eq!(
        field(&reopened, mesh, "demo.rotator", "radians_per_second")?,
        Value::F32(4.0)
    );
    reopened.set_field(mesh, "demo.rotator", "radians_per_second", Value::F32(5.0))?;
    assert_eq!(reopened.saved_cursor(), Some(0));

    fs::remove_dir_all(project.path().join("Scenes"))?;
    let cursor = reopened.history_cursor();
    assert!(reopened.save().is_err());
    assert_eq!(reopened.history_cursor(), cursor);
    assert_eq!(reopened.saved_cursor(), Some(0));
    assert!(reopened.is_dirty());
    Ok(())
}

#[test]
fn valid_authoring_without_active_camera_reports_preview_diagnostic()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("preview-diagnostic")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;

    session.remove_component(id(CAMERA)?, "sge.camera")?;

    assert!(matches!(
        session.preview_frame(),
        Err(EditorPreviewError::View(
            sge_render::RenderViewError::MissingActiveCamera
        ))
    ));
    assert!(session.is_dirty());
    Ok(())
}

#[test]
fn play_uses_a_fresh_world_and_drop_preserves_edit_world() -> Result<(), Box<dyn std::error::Error>>
{
    let project = TestProject::new("play-isolation")?;
    let edit = EditSession::open(demo_game::GAME, project.path())?;
    let mesh = id(MESH)?;
    let before = edit.snapshot()?.to_ron()?;
    let edit_translation = edit
        .component::<Transform>(mesh)
        .ok_or("missing EditWorld Transform")?
        .translation;
    let mut play = edit.start_play()?;
    let mut input = InputFrame::new();
    input.hold(Button::Key(KeyCode::KeyW));

    play.advance(std::time::Duration::from_millis(20), input)?;

    let play_translation = play
        .component::<Transform>(mesh)
        .ok_or("missing PlayWorld Transform")?
        .translation;
    assert!(play_translation[2] < edit_translation[2]);
    let state = play
        .resource::<demo_game::GameRuntimeState>()
        .ok_or("missing Play runtime state")?;
    assert_eq!(state.startup_runs(), 1);
    assert_eq!(state.fixed_updates(), 1);
    assert_eq!(state.updates(), 1);
    assert_eq!(state.post_updates(), 1);
    let (snapshot, view) = play.render_frame()?;
    assert_eq!(snapshot.meshes().len(), 1);
    assert!(view.camera().active());
    drop(play);

    assert_eq!(edit.snapshot()?.to_ron()?, before);
    assert_eq!(
        edit.component::<Transform>(mesh)
            .ok_or("missing preserved EditWorld Transform")?
            .translation,
        edit_translation
    );
    Ok(())
}

fn field(
    session: &EditSession,
    entity: SceneEntityId,
    component: &str,
    field: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let scene = session.snapshot()?;
    scene
        .entities()
        .find(|candidate| candidate.id() == entity)
        .and_then(|entity| {
            entity
                .components()
                .find(|candidate| candidate.type_key().as_str() == component)
        })
        .and_then(|component| component.fields().get(field))
        .cloned()
        .ok_or_else(|| format!("missing {entity}/{component}/{field}").into())
}

fn has_component(
    session: &EditSession,
    entity: SceneEntityId,
    component: &str,
) -> Result<bool, EditError> {
    Ok(session
        .snapshot()?
        .entities()
        .find(|candidate| candidate.id() == entity)
        .is_some_and(|entity| {
            entity
                .components()
                .any(|candidate| candidate.type_key().as_str() == component)
        }))
}

fn has_entity(session: &EditSession, entity: SceneEntityId) -> Result<bool, EditError> {
    Ok(session
        .snapshot()?
        .entities()
        .any(|candidate| candidate.id() == entity))
}

fn id(value: &str) -> Result<SceneEntityId, Box<dyn std::error::Error>> {
    Ok(value.parse()?)
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_editor_edit")
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
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game")
}

// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use sge_editor::{EditError, EditSession, EditorPreviewError};
use sge_reflect::{FieldKind, Value};
use sge_scene::{AuthoringEntity, SceneEntityId};

const CAMERA: &str = "50000000-0000-4000-8000-000000000001";
const MESH: &str = "50000000-0000-4000-8000-000000000002";
const NEW_PARENT: &str = "60000000-0000-4000-8000-000000000001";
const NEW_CHILD: &str = "60000000-0000-4000-8000-000000000002";

#[test]
fn inspector_field_edit_is_validated_and_undoable() -> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("field-history")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;
    let mesh = id(MESH)?;
    session.select(Some(mesh))?;

    let inspector = session.inspector()?;
    let transform = inspector
        .iter()
        .find(|component| component.type_key().as_str() == "sge.transform")
        .ok_or("missing Transform inspector")?;
    assert_eq!(transform.display_name(), "Transform");
    let translation = transform
        .fields()
        .iter()
        .find(|field| field.field_key().as_str() == "translation")
        .ok_or("missing translation field")?;
    assert_eq!(translation.display_name(), "Translation");
    assert_eq!(translation.kind(), &FieldKind::Vec3);

    session.set_field(
        mesh,
        "sge.transform",
        "translation",
        Value::Vec3([2.0, 0.0, 2.0].into()),
    )?;
    assert!(session.is_dirty());
    assert_eq!(session.history_cursor(), 1);
    assert_eq!(
        field(&session, mesh, "sge.transform", "translation")?,
        Value::Vec3([2.0, 0.0, 2.0].into())
    );

    session.undo()?;
    assert!(!session.is_dirty());
    assert_eq!(session.history_cursor(), 0);
    assert_eq!(
        field(&session, mesh, "sge.transform", "translation")?,
        Value::Vec3([0.0, 0.0, 2.0].into())
    );

    session.redo()?;
    assert_eq!(
        field(&session, mesh, "sge.transform", "translation")?,
        Value::Vec3([2.0, 0.0, 2.0].into())
    );
    let before = session.snapshot()?.to_ron()?;
    let cursor = session.history_cursor();
    assert!(
        session
            .set_field(
                mesh,
                "sge.transform",
                "scale",
                Value::Vec3([0.0, 1.0, 1.0].into()),
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
    session.set_field(
        mesh,
        "sge.material",
        "base_color",
        Value::Color([0.1, 0.2, 0.3, 1.0]),
    )?;
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
        field(&reopened, mesh, "sge.material", "base_color")?,
        Value::Color([0.1, 0.2, 0.3, 1.0])
    );
    reopened.set_field(
        mesh,
        "sge.material",
        "base_color",
        Value::Color([0.3, 0.4, 0.5, 1.0]),
    )?;
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

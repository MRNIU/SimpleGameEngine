// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use sge_app::{EngineApp, EngineBuildError, GameDescriptor};
use sge_asset_pipeline::{CookOutputRoot, full_cook};
use sge_editor::{EditError, EditSession};
use sge_project::ProjectRoot;
use sge_scene::SceneEntityId;

static PREPARE_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

#[test]
fn basic_primitive_reuses_one_formal_asset_across_history_reopen_and_cook()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("primitive-reuse")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;

    let first = session.create_cube()?;
    let records_after_first = session.manifest().records().len();
    let sources_after_first = mesh_source_names(project.path())?;
    let second = session.create_cube()?;

    assert_eq!(second.asset, first.asset);
    assert_ne!(second.entity, first.entity);
    assert_eq!(session.manifest().records().len(), records_after_first);
    assert_eq!(mesh_source_names(project.path())?, sources_after_first);
    session.undo()?;
    assert!(!has_entity(&session, second.entity)?);
    assert!(has_entity(&session, first.entity)?);
    session.redo()?;
    session.save()?;
    drop(session);

    let project_root = ProjectRoot::open(project.path())?;
    let cooked = project.path().join("CookedPrimitiveReuse");
    fs::create_dir(&cooked)?;
    let app = demo_game::GAME.create_app()?;
    full_cook(
        &project_root,
        demo_game::GAME_ID,
        app.type_registry(),
        app.world(),
        &CookOutputRoot::open(&cooked)?,
    )?;

    let mut reopened = EditSession::open(demo_game::GAME, project.path())?;
    let third = reopened.create_cube()?;
    assert_eq!(third.asset, first.asset);
    assert_eq!(reopened.manifest().records().len(), records_after_first);
    assert_eq!(mesh_source_names(project.path())?, sources_after_first);
    reopened.preview_frame()?;
    Ok(())
}

#[test]
fn import_and_cache_failures_remove_new_source_and_preserve_live_session()
-> Result<(), Box<dyn std::error::Error>> {
    let source_project = TestProject::new("import-rollback")?;
    let incoming = incoming_obj(&source_project, "incoming.obj")?;
    let existing_source = source_project
        .path()
        .join("Content/Meshes/Kenney/conveyor-bars-stripe.obj");
    let original_source = fs::read(&existing_source)?;
    let mut session = EditSession::open(demo_game::GAME, source_project.path())?;
    let before_scene = session.snapshot()?.to_ron()?;
    let before_sources = mesh_source_names(source_project.path())?;
    fs::write(&existing_source, b"not an obj")?;

    assert!(matches!(
        session.import_obj(&incoming),
        Err(EditError::Import(_))
    ));
    assert_eq!(mesh_source_names(source_project.path())?, before_sources);
    assert_eq!(session.snapshot()?.to_ron()?, before_scene);
    fs::write(existing_source, original_source)?;
    let still_live = session.create_entity("Still Live After Import Failure")?;
    assert!(has_entity(&session, still_live)?);

    let cache_project = TestProject::new("cache-rollback")?;
    let incoming = incoming_obj(&cache_project, "incoming.obj")?;
    let mut session = EditSession::open(demo_game::GAME, cache_project.path())?;
    let before_scene = session.snapshot()?.to_ron()?;
    let before_sources = mesh_source_names(cache_project.path())?;
    fs::remove_dir_all(cache_project.path().join("Cache"))?;
    fs::write(cache_project.path().join("Cache"), b"blocked")?;

    assert!(matches!(
        session.import_obj(&incoming),
        Err(EditError::Import(_))
    ));
    assert_eq!(mesh_source_names(cache_project.path())?, before_sources);
    assert_eq!(session.snapshot()?.to_ron()?, before_scene);
    fs::remove_file(cache_project.path().join("Cache"))?;
    fs::create_dir(cache_project.path().join("Cache"))?;
    let still_live = session.create_entity("Still Live After Cache Failure")?;
    assert!(has_entity(&session, still_live)?);
    Ok(())
}

#[test]
fn prepare_and_manifest_failures_remove_new_source_and_preserve_live_session()
-> Result<(), Box<dyn std::error::Error>> {
    let prepare_project = TestProject::new("prepare-rollback")?;
    let incoming = incoming_obj(&prepare_project, "incoming.obj")?;
    PREPARE_FACTORY_CALLS.store(0, Ordering::SeqCst);
    let game = GameDescriptor::new(demo_game::GAME_ID, app_that_fails_prepare_once);
    let mut session = EditSession::open(game, prepare_project.path())?;
    let before_scene = session.snapshot()?.to_ron()?;
    let before_sources = mesh_source_names(prepare_project.path())?;

    assert!(matches!(
        session.import_obj(&incoming),
        Err(EditError::Validation(_))
    ));
    assert_eq!(mesh_source_names(prepare_project.path())?, before_sources);
    assert_eq!(session.snapshot()?.to_ron()?, before_scene);
    let still_live = session.create_entity("Still Live After Prepare Failure")?;
    assert!(has_entity(&session, still_live)?);

    let manifest_project = TestProject::new("manifest-rollback")?;
    let incoming = incoming_obj(&manifest_project, "incoming.obj")?;
    let mut session = EditSession::open(demo_game::GAME, manifest_project.path())?;
    let before_scene = session.snapshot()?.to_ron()?;
    let before_sources = mesh_source_names(manifest_project.path())?;
    let manifest_path = manifest_project.path().join("Content/asset_manifest.ron");
    let manifest_bytes = fs::read(&manifest_path)?;
    fs::remove_file(&manifest_path)?;
    fs::create_dir(&manifest_path)?;

    assert!(matches!(
        session.import_obj(&incoming),
        Err(EditError::Manifest(_))
    ));
    assert_eq!(mesh_source_names(manifest_project.path())?, before_sources);
    assert_eq!(session.snapshot()?.to_ron()?, before_scene);
    fs::remove_dir(&manifest_path)?;
    fs::write(manifest_path, manifest_bytes)?;
    let still_live = session.create_entity("Still Live After Manifest Failure")?;
    assert!(has_entity(&session, still_live)?);
    Ok(())
}

#[test]
fn png_import_commits_manifest_without_dirty_history_and_corrupt_png_rolls_back()
-> Result<(), Box<dyn std::error::Error>> {
    let project = TestProject::new("png-import")?;
    let valid = project.path().join("incoming.png");
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(&[255, 0, 0, 255], 1, 1, ExtendedColorType::Rgba8)?;
    fs::write(&valid, bytes)?;
    let invalid = project.path().join("corrupt.png");
    fs::write(&invalid, b"not png")?;
    let mut session = EditSession::open(demo_game::GAME, project.path())?;
    let before_cursor = session.history_cursor();
    let before_records = session.manifest().records().len();

    let texture = session.import_png(&valid)?;
    assert_eq!(session.history_cursor(), before_cursor);
    assert_eq!(session.manifest().records().len(), before_records + 1);
    assert!(
        session
            .manifest()
            .records()
            .iter()
            .any(|record| record.id() == texture)
    );
    let committed_sources = texture_source_names(project.path())?;

    assert!(session.import_png(&invalid).is_err());
    assert_eq!(session.history_cursor(), before_cursor);
    assert_eq!(session.manifest().records().len(), before_records + 1);
    assert_eq!(texture_source_names(project.path())?, committed_sources);
    Ok(())
}

fn app_that_fails_prepare_once() -> Result<EngineApp, EngineBuildError> {
    if PREPARE_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst) != 1 {
        return demo_game::GAME.create_app();
    }
    let mut app = EngineApp::new();
    app.finish()?;
    Ok(app)
}

fn incoming_obj(project: &TestProject, name: &str) -> Result<PathBuf, std::io::Error> {
    let incoming = project.path().join(name);
    fs::copy(
        project
            .path()
            .join("Content/Meshes/Kenney/conveyor-bars-stripe.obj"),
        &incoming,
    )?;
    Ok(incoming)
}

fn mesh_source_names(project: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let root = project.join("Content/Meshes");
    let mut names = Vec::new();
    collect_relative_files(&root, &root, &mut names)?;
    names.sort();
    Ok(names)
}

fn collect_relative_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &entry.path(), files)?;
        } else {
            files.push(
                entry
                    .path()
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    Ok(())
}

fn texture_source_names(project: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = project.join("Content/Textures");
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    Ok(names)
}

fn has_entity(session: &EditSession, entity: SceneEntityId) -> Result<bool, EditError> {
    Ok(session
        .snapshot()?
        .entities()
        .any(|candidate| candidate.id() == entity))
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_editor_asset_lifecycle")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)?;
        copy_tree(&demo_root().join("Content"), &root.join("Content"))?;
        copy_tree(&demo_root().join("Scenes"), &root.join("Scenes"))?;
        fs::copy(
            demo_root().join("project.sge.ron"),
            root.join("project.sge.ron"),
        )?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn demo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game")
}

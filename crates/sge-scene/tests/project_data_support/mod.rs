// Copyright The SimpleGameEngine Contributors

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_app::{EngineApp, EngineBuildError, GameDescriptor};
use sge_asset::{AssetId, AssetRef};
use sge_ecs::{Entity, World};
use sge_project::{
    AuthoringAssetManifest, ProjectDescriptor, ProjectPath, ProjectRoot, SourceAssetRecord,
};
use sge_reflect::{TypeKey, TypeRegistry};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, SceneEntityId, SceneInstance, instantiate,
    parent_descriptor, prepare, scene_entity_id_descriptor, snapshot,
};

use crate::support::{MeshAsset, Probe, probe_descriptor, probe_registry, scene_id};

pub const GAME_ID: &str = "demo.game";

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);
static INVALID_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
static MISMATCH_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

pub struct TestProject {
    path: PathBuf,
    pub root_id: SceneEntityId,
    pub child_id: SceneEntityId,
    pub asset_id: AssetId,
    pub count: i64,
}

impl TestProject {
    pub fn new(name: &str) -> Result<Self, Box<dyn Error>> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_scene_project_data");
        fs::create_dir_all(&base)?;
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{name}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path)?;
        fs::create_dir(path.join("Content"))?;
        fs::create_dir(path.join("scenes"))?;
        fs::write(path.join("Content/mesh.obj"), b"o triangle\n")?;

        let root = ProjectRoot::open(&path)?;
        let descriptor = ProjectDescriptor::new(
            GAME_ID,
            "demo-game",
            "demo-player",
            "demo-build",
            ProjectPath::new("scenes/main.scene.ron")?,
        )?;
        let asset_id = AssetId::new_v4();
        let manifest = AuthoringAssetManifest::new(vec![SourceAssetRecord::new(
            asset_id,
            TypeKey::new("asset.mesh")?,
            ProjectPath::new("Content/mesh.obj")?,
        )])?;
        let registry = probe_registry()?;
        let reflected = registry.encode(&Probe {
            count: 17,
            target: scene_id(1)?,
            mesh: AssetRef::<MeshAsset>::new(asset_id),
        })?;
        let root_id = scene_id(1)?;
        let child_id = scene_id(2)?;
        let scene = AuthoringScene::new(vec![
            AuthoringEntity::new(child_id, Some(root_id), vec![reflected])?,
            AuthoringEntity::new(root_id, None, Vec::new())?,
        ])?;

        descriptor.save(&root)?;
        manifest.save(&root)?;
        root.write_atomic(
            descriptor.default_authoring_scene(),
            scene.to_ron()?.as_bytes(),
        )?;

        Ok(Self {
            path,
            root_id,
            child_id,
            asset_id,
            count: 17,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> Result<ProjectRoot, Box<dyn Error>> {
        Ok(ProjectRoot::open(&self.path)?)
    }

    pub fn scene_with_probe(
        &self,
        asset: AssetId,
        target: SceneEntityId,
    ) -> Result<AuthoringScene, Box<dyn Error>> {
        let registry = probe_registry()?;
        let reflected = registry.encode(&Probe {
            count: self.count,
            target,
            mesh: AssetRef::<MeshAsset>::new(asset),
        })?;
        Ok(AuthoringScene::new(vec![
            AuthoringEntity::new(self.child_id, Some(self.root_id), vec![reflected])?,
            AuthoringEntity::new(self.root_id, None, Vec::new())?,
        ])?)
    }

    pub fn write_scene(&self, scene: &AuthoringScene) -> Result<(), Box<dyn Error>> {
        let root = self.root()?;
        let descriptor = ProjectDescriptor::load(&root)?;
        root.write_atomic(
            descriptor.default_authoring_scene(),
            scene.to_ron()?.as_bytes(),
        )?;
        Ok(())
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _cleanup_result = fs::remove_dir_all(&self.path);
    }
}

pub struct OpenProject {
    pub app: EngineApp,
    pub instance: SceneInstance,
    pub descriptor: ProjectDescriptor,
    pub manifest: AuthoringAssetManifest,
    pub root: ProjectRoot,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LiveSignature {
    pub runtime_entity: Entity,
    pub count: i64,
    pub target: SceneEntityId,
    pub asset: AssetId,
    pub game_id: String,
    pub manifest_len: usize,
}

pub fn game_descriptor() -> GameDescriptor {
    GameDescriptor::new(GAME_ID, create_game_app)
}

pub fn missing_parent_game_descriptor() -> GameDescriptor {
    GameDescriptor::new(GAME_ID, create_missing_parent_app)
}

pub fn invalid_guard_game_descriptor() -> GameDescriptor {
    INVALID_FACTORY_CALLS.store(0, Ordering::SeqCst);
    GameDescriptor::new(GAME_ID, create_invalid_guard_app)
}

pub fn invalid_factory_calls() -> usize {
    INVALID_FACTORY_CALLS.load(Ordering::SeqCst)
}

pub fn mismatch_guard_game_descriptor() -> GameDescriptor {
    MISMATCH_FACTORY_CALLS.store(0, Ordering::SeqCst);
    GameDescriptor::new(GAME_ID, create_mismatch_guard_app)
}

pub fn mismatch_factory_calls() -> usize {
    MISMATCH_FACTORY_CALLS.load(Ordering::SeqCst)
}

fn create_game_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.register_reflected_component::<SceneEntityId>(
        scene_entity_id_descriptor().expect("static SceneEntityId descriptor is valid"),
    )?;
    app.register_reflected_component::<Parent>(
        parent_descriptor().expect("static Parent descriptor is valid"),
    )?;
    app.register_reflected_component::<Probe>(
        probe_descriptor().expect("static Probe descriptor is valid"),
    )?;
    app.finish()?;
    Ok(app)
}

fn create_missing_parent_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.register_reflected_component::<SceneEntityId>(
        scene_entity_id_descriptor().expect("static SceneEntityId descriptor is valid"),
    )?;
    app.register_reflected_component::<Probe>(
        probe_descriptor().expect("static Probe descriptor is valid"),
    )?;
    app.finish()?;
    Ok(app)
}

fn create_invalid_guard_app() -> Result<EngineApp, EngineBuildError> {
    INVALID_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    create_game_app()
}

fn create_mismatch_guard_app() -> Result<EngineApp, EngineBuildError> {
    MISMATCH_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    create_game_app()
}

pub fn open_all(path: &Path, game: GameDescriptor) -> Result<OpenProject, Box<dyn Error>> {
    let root = ProjectRoot::open(path)?;
    let descriptor = ProjectDescriptor::load(&root)?;
    descriptor.validate_for_game(game.game_id())?;
    let manifest = AuthoringAssetManifest::load(&root)?;
    let scene_bytes = root.read(descriptor.default_authoring_scene())?;
    let scene = AuthoringScene::from_ron(std::str::from_utf8(&scene_bytes)?)?;

    let mut app = game.create_app()?;
    let prepared = prepare(&scene, app.type_registry(), &manifest)?;
    let instance = instantiate(prepared, app.world_initializer()?)?;

    let snapshot_scene = snapshot(app.world(), app.type_registry(), &manifest)?;
    let encoded = snapshot_scene.to_ron()?;
    let reopened = AuthoringScene::from_ron(&encoded)?;
    let _prepared = prepare(&reopened, app.type_registry(), &manifest)?;

    Ok(OpenProject {
        app,
        instance,
        descriptor,
        manifest,
        root,
    })
}

pub fn save_scene(project: &OpenProject) -> Result<Vec<u8>, Box<dyn Error>> {
    save_world(
        &project.root,
        project.descriptor.default_authoring_scene(),
        project.app.world(),
        project.app.type_registry(),
        &project.manifest,
    )
}

pub fn save_world(
    root: &ProjectRoot,
    scene_path: &ProjectPath,
    world: &World,
    registry: &TypeRegistry,
    manifest: &AuthoringAssetManifest,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let scene = snapshot(world, registry, manifest)?;
    let bytes = scene.to_ron()?.into_bytes();
    let reopened = AuthoringScene::from_ron(std::str::from_utf8(&bytes)?)?;
    let _prepared = prepare(&reopened, registry, manifest)?;
    root.write_atomic(scene_path, &bytes)?;
    let readback = root.read(scene_path)?;
    let readback_scene = AuthoringScene::from_ron(std::str::from_utf8(&readback)?)?;
    let _prepared = prepare(&readback_scene, registry, manifest)?;
    Ok(readback)
}

pub fn reload(
    live: &mut OpenProject,
    path: &Path,
    game: GameDescriptor,
) -> Result<(), Box<dyn Error>> {
    let candidate = open_all(path, game)?;
    *live = candidate;
    Ok(())
}

pub fn signature(
    project: &OpenProject,
    expected: &TestProject,
) -> Result<LiveSignature, Box<dyn Error>> {
    let runtime_entity = project
        .instance
        .entity(&expected.child_id)
        .ok_or("live child SceneInstance mapping is missing")?;
    let probe = project
        .app
        .world()
        .get::<Probe>(runtime_entity)
        .ok_or("live child Probe is missing")?;
    Ok(LiveSignature {
        runtime_entity,
        count: probe.count,
        target: probe.target,
        asset: *probe.mesh.id(),
        game_id: project.descriptor.game_id().to_string(),
        manifest_len: project.manifest.records().len(),
    })
}

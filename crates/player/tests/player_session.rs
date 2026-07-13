// Copyright The SimpleGameEngine Contributors

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use sge_app::{EngineApp, EngineBuildError, GameDescriptor, ScheduleLabel, System, SystemBuilder};
use sge_asset::{
    AssetId, AssetRef, MESH_ASSET_TYPE_KEY, RuntimeAssetCatalog, RuntimeAssetStoreError,
    RuntimeContentError,
};
use sge_asset_pipeline::{CookOutputRoot, full_cook};
use sge_input::InputFrame;
use sge_player::{PlayerLoadError, PlayerSession, RunOptions, run, runtime_root_for_executable};
use sge_project::{
    AuthoringAssetManifest, ObjImportSettings, ProjectDescriptor, ProjectPath, ProjectRoot,
    SourceAssetRecord, SourceImporter,
};
use sge_reflect::TypeKey;
use sge_render::{Camera, Light, Material, MeshRenderer, Projection, RenderPlugin};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, RuntimeSceneFormatError, SceneEntityId,
    parent_descriptor, scene_entity_id_descriptor,
};

const GAME_ID: &str = "test.player";
static FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
static ADVANCES: AtomicUsize = AtomicUsize::new(0);

#[test]
fn staged_runtime_root_is_sibling_of_the_player_executable() {
    assert_eq!(
        runtime_root_for_executable(Path::new("Stage/generations/id/demo-player")).unwrap(),
        PathBuf::from("Stage/generations/id/runtime")
    );
    assert!(runtime_root_for_executable(Path::new("/")).is_err());
}

#[test]
fn copied_runtime_loads_advances_and_extracts_without_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("source-free")?;
    fixture.cook()?;
    fixture.delete_source()?;

    let mut session = PlayerSession::load(game(), fixture.cooked())?;
    session.advance(Duration::from_millis(16), InputFrame::new())?;
    let (snapshot, view) = session.render_frame()?;

    assert_eq!(snapshot.meshes().len(), 1);
    assert_eq!(snapshot.lights().len(), 1);
    assert_eq!(view.camera().projection(), Projection::Perspective);
    Ok(())
}

#[test]
fn wrong_identity_wins_before_generation_and_factory() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("identity-first")?;
    fixture.cook()?;
    let catalog = fs::read_to_string(fixture.cooked().join("runtime_catalog.ron"))?;
    let generation = catalog
        .split("generation: \"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .ok_or("generation not found")?;
    fs::remove_dir_all(fixture.cooked().join("generations").join(generation))?;
    FACTORY_CALLS.store(0, Ordering::SeqCst);

    let error = match PlayerSession::load(other_game(), fixture.cooked()) {
        Err(error) => error,
        Ok(_) => panic!("wrong identity must be rejected"),
    };
    assert!(matches!(
        error,
        PlayerLoadError::Content(RuntimeContentError::GameMismatch { .. })
    ));
    assert_eq!(FACTORY_CALLS.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn verified_corrupt_products_and_scene_decode_to_typed_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let scene_fixture = Fixture::new("corrupt-scene")?;
    scene_fixture.cook()?;
    scene_fixture.rewrite_generation(Some(b"not a runtime scene"), None)?;
    assert!(matches!(
        PlayerSession::load(game(), scene_fixture.cooked()),
        Err(PlayerLoadError::SceneFormat(
            RuntimeSceneFormatError::Parse { .. }
        ))
    ));

    let product_fixture = Fixture::new("corrupt-product")?;
    product_fixture.cook()?;
    product_fixture.rewrite_generation(None, Some(b"not a mesh product"))?;
    assert!(matches!(
        PlayerSession::load(game(), product_fixture.cooked()),
        Err(PlayerLoadError::Assets(
            RuntimeAssetStoreError::MeshDecode { .. }
        ))
    ));
    Ok(())
}

#[test]
fn factory_failure_remains_typed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("factory-failure")?;
    fixture.cook()?;
    let descriptor = GameDescriptor::new(GAME_ID, unfinished_app);
    assert!(matches!(
        PlayerSession::load(descriptor, fixture.cooked()),
        Err(PlayerLoadError::App(
            EngineBuildError::FactoryReturnedUnfinishedApp
        ))
    ));
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn real_window_advances_extracts_renders_and_presents_before_exit()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("window-smoke")?;
    fixture.cook()?;
    fixture.delete_source()?;
    ADVANCES.store(0, Ordering::SeqCst);

    let report = run(
        game(),
        fixture.cooked(),
        RunOptions {
            max_frames: Some(2),
            initial_size: [320, 240],
            screenshot: None,
        },
    )?;

    assert_eq!(report.presented_frames(), 2);
    assert!(ADVANCES.load(Ordering::SeqCst) >= 2);
    Ok(())
}

fn game() -> GameDescriptor {
    GameDescriptor::new(GAME_ID, create_app)
}

fn other_game() -> GameDescriptor {
    GameDescriptor::new("other.player", counted_app)
}

fn counted_app() -> Result<EngineApp, EngineBuildError> {
    FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    create_app()
}

fn unfinished_app() -> Result<EngineApp, EngineBuildError> {
    Ok(EngineApp::new())
}

fn create_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.register_reflected_component::<SceneEntityId>(
        scene_entity_id_descriptor().expect("built-in scene identity descriptor must be valid"),
    )?;
    app.register_reflected_component::<Parent>(
        parent_descriptor().expect("built-in parent descriptor must be valid"),
    )?;
    app.add_plugin(RenderPlugin)?;
    app.add_system(ScheduleLabel::Update, advance_probe())?;
    app.finish()?;
    Ok(app)
}

fn advance_probe() -> System {
    SystemBuilder::new().build(|_| {
        ADVANCES.fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
}

struct Fixture {
    base: PathBuf,
    source: PathBuf,
    cooked: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_player")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let source = base.join("source");
        let cooked = base.join("cooked");
        fs::create_dir_all(source.join("Content"))?;
        fs::create_dir_all(source.join("Scenes"))?;
        fs::create_dir(&cooked)?;
        let asset = AssetId::from_str("20000000-0000-4000-8000-000000000001")?;
        fs::write(
            source.join("Content/demo.obj"),
            b"o demo\nv -0.8 -0.8 0\nv 0.8 -0.8 0\nv 0 0.8 0\nf 1 2 3\n",
        )?;
        let root = ProjectRoot::open(&source)?;
        let descriptor = ProjectDescriptor::new(
            GAME_ID,
            "test-game",
            "test-player",
            "test-build",
            ProjectPath::new("Scenes/main.scene.ron")?,
        )?;
        descriptor.save(&root)?;
        AuthoringAssetManifest::new(vec![SourceAssetRecord::new(
            asset,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            ProjectPath::new("Content/demo.obj")?,
            SourceImporter::Obj(ObjImportSettings::new(false)),
        )?])?
        .save(&root)?;
        let app = game().create_app()?;
        let camera_id = scene_id("30000000-0000-4000-8000-000000000001")?;
        let mesh_id = scene_id("30000000-0000-4000-8000-000000000002")?;
        let light_id = scene_id("30000000-0000-4000-8000-000000000003")?;
        let scene = AuthoringScene::new(vec![
            AuthoringEntity::new(
                camera_id,
                None,
                vec![
                    app.type_registry()
                        .encode(&sge_math::Transform::identity())?,
                    app.type_registry().encode(&Camera::new(
                        true,
                        Projection::Perspective,
                        std::f32::consts::FRAC_PI_3,
                        10.0,
                        0.1,
                        100.0,
                    ))?,
                ],
            )?,
            AuthoringEntity::new(
                mesh_id,
                None,
                vec![
                    app.type_registry()
                        .encode(&sge_math::Transform::from_translation([0.0, 0.0, 2.0]))?,
                    app.type_registry()
                        .encode(&MeshRenderer::new(AssetRef::new(asset)))?,
                    app.type_registry().encode(&Material::default())?,
                ],
            )?,
            AuthoringEntity::new(
                light_id,
                None,
                vec![
                    app.type_registry()
                        .encode(&sge_math::Transform::identity())?,
                    app.type_registry().encode(&Light::default())?,
                ],
            )?,
        ])?;
        root.write_atomic(
            descriptor.default_authoring_scene(),
            scene.to_ron()?.as_bytes(),
        )?;
        Ok(Self {
            base,
            source,
            cooked,
        })
    }

    fn cook(&self) -> Result<(), Box<dyn std::error::Error>> {
        let project = ProjectRoot::open(&self.source)?;
        let app = game().create_app()?;
        full_cook(
            &project,
            GAME_ID,
            app.type_registry(),
            app.world(),
            &CookOutputRoot::open(&self.cooked)?,
        )?;
        Ok(())
    }

    fn delete_source(&self) -> Result<(), std::io::Error> {
        fs::remove_dir_all(&self.source)
    }

    fn cooked(&self) -> &Path {
        &self.cooked
    }

    fn rewrite_generation(
        &self,
        entry_override: Option<&[u8]>,
        product_override: Option<&[u8]>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let catalog_path = self.cooked.join("runtime_catalog.ron");
        let catalog = RuntimeAssetCatalog::from_ron(&fs::read_to_string(&catalog_path)?)?;
        let old_generation = self
            .cooked
            .join("generations")
            .join(catalog.generation().as_str());
        let entry_path = runtime_path(catalog.entry_scene().as_str());
        let entry = entry_override.map_or_else(
            || fs::read(old_generation.join(&entry_path)),
            |bytes| Ok(bytes.to_vec()),
        )?;
        let mut products = catalog
            .assets()
            .iter()
            .map(|record| {
                Ok((
                    *record.id(),
                    fs::read(old_generation.join(runtime_path(record.product().as_str())))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, std::io::Error>>()?;
        if let Some(bytes) = product_override {
            let id = *catalog.assets()[0].id();
            products.insert(id, bytes.to_vec());
        }
        let rewritten = RuntimeAssetCatalog::build(
            catalog.game_id().clone(),
            catalog.entry_scene().clone(),
            catalog.assets().to_vec(),
            &entry,
            &products,
        )?;
        let new_generation = self
            .cooked
            .join("generations")
            .join(rewritten.generation().as_str());
        fs::rename(&old_generation, &new_generation)?;
        fs::write(new_generation.join(entry_path), entry)?;
        for record in rewritten.assets() {
            fs::write(
                new_generation.join(runtime_path(record.product().as_str())),
                &products[record.id()],
            )?;
        }
        fs::write(catalog_path, rewritten.to_ron()?)?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn scene_id(value: &str) -> Result<SceneEntityId, Box<dyn std::error::Error>> {
    Ok(SceneEntityId::from_str(value)?)
}

fn runtime_path(value: &str) -> PathBuf {
    value.split('/').collect()
}

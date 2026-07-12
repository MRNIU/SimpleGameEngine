// Copyright The SimpleGameEngine Contributors

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_asset::{AssetId, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset};
use sge_ecs::World;
use sge_project::{
    AuthoringAssetManifest, ObjImportSettings, ProjectDescriptor, ProjectPath, ProjectRoot,
    SourceAssetRecord, SourceImporter,
};
use sge_reflect::{FieldKey, FieldRegistration, TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{AuthoringEntity, AuthoringScene, Parent, SceneEntityId};

pub const GAME_ID: &str = "demo.game";
pub const PRIOR_CATALOG: &[u8] = b"prior catalog bytes\n";

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, PartialEq)]
pub struct MeshConsumer {
    pub mesh: AssetRef<MeshAsset>,
}

pub struct FullCookFixture {
    root: PathBuf,
    output: PathBuf,
    pub used: AssetId,
    pub unused: AssetId,
}

impl FullCookFixture {
    pub fn new(name: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_asset_pipeline_full_cook")
            .join(format!("{name}-{}-{sequence}", std::process::id()));
        let output = root.join("output");
        fs::create_dir_all(root.join("Content"))?;
        fs::create_dir_all(root.join("Scenes"))?;
        fs::create_dir(&output)?;

        let used = AssetId::from_str("10000000-0000-4000-8000-000000000001")?;
        let unused = AssetId::from_str("10000000-0000-4000-8000-000000000002")?;
        fs::write(root.join("Content/used.obj"), triangle_obj("used", 1.0))?;
        fs::write(root.join("Content/unused.obj"), triangle_obj("unused", 2.0))?;

        let project = ProjectRoot::open(&root)?;
        let descriptor = ProjectDescriptor::new(
            GAME_ID,
            "demo-game",
            "demo-player",
            "demo-build",
            ProjectPath::new("Scenes/main.scene.ron")?,
        )?;
        descriptor.save(&project)?;
        AuthoringAssetManifest::new(vec![
            source_record(used, "Content/used.obj", false)?,
            source_record(unused, "Content/unused.obj", true)?,
        ])?
        .save(&project)?;

        let registry = registry(true)?;
        let root_entity = scene_id(1)?;
        let child_entity = scene_id(2)?;
        let scene = AuthoringScene::new(vec![
            AuthoringEntity::new(root_entity, None, Vec::new())?,
            AuthoringEntity::new(
                child_entity,
                Some(root_entity),
                vec![registry.encode(&MeshConsumer {
                    mesh: AssetRef::new(used),
                })?],
            )?,
        ])?;
        project.write_atomic(
            descriptor.default_authoring_scene(),
            scene.to_ron()?.as_bytes(),
        )?;

        Ok(Self {
            root,
            output,
            used,
            unused,
        })
    }

    pub fn project(&self) -> Result<ProjectRoot, Box<dyn Error>> {
        Ok(ProjectRoot::open(&self.root)?)
    }

    pub fn output_path(&self) -> &Path {
        &self.output
    }

    pub fn corrupt_manifest_and_remove_source(&self) -> Result<(), Box<dyn Error>> {
        fs::write(self.root.join("Content/asset_manifest.ron"), b"not ron")?;
        fs::remove_file(self.root.join("Content/used.obj"))?;
        Ok(())
    }

    pub fn corrupt_unused_source(&self) -> Result<(), Box<dyn Error>> {
        fs::write(self.root.join("Content/unused.obj"), b"v invalid 0 0\n")?;
        Ok(())
    }

    pub fn seed_prior_catalog(&self) -> Result<(), Box<dyn Error>> {
        fs::write(self.output.join("runtime_catalog.ron"), PRIOR_CATALOG)?;
        Ok(())
    }

    pub fn assert_output_untouched(&self) -> Result<(), Box<dyn Error>> {
        let mut names = fs::read_dir(&self.output)?
            .map(|entry| Ok(entry?.file_name()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        names.sort();
        assert_eq!(names, ["runtime_catalog.ron"]);
        assert_eq!(
            fs::read(self.output.join("runtime_catalog.ron"))?,
            PRIOR_CATALOG
        );
        Ok(())
    }
}

impl Drop for FullCookFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn registry(frozen: bool) -> Result<TypeRegistry, Box<dyn Error>> {
    let mut registry = TypeRegistry::new();
    registry.register(mesh_consumer_descriptor()?)?;
    if frozen {
        registry.freeze()?;
    }
    Ok(registry)
}

pub fn world(include_consumer: bool, finished: bool) -> Result<World, Box<dyn Error>> {
    let mut world = World::new();
    world.register_component::<SceneEntityId>()?;
    world.register_component::<Parent>()?;
    if include_consumer {
        world.register_component::<MeshConsumer>()?;
    }
    if finished {
        world.finish_registration();
    }
    Ok(world)
}

fn mesh_consumer_descriptor() -> Result<TypeDescriptor, Box<dyn Error>> {
    Ok(TypeDescriptor::builder::<MeshConsumer>(
        TypeKey::new("demo.mesh_consumer")?,
        1,
        "Mesh Consumer",
        || MeshConsumer {
            mesh: AssetRef::new(AssetId::new_v4()),
        },
    )
    .field(FieldRegistration::reference(
        FieldKey::new("mesh")?,
        "Mesh",
        |consumer: &MeshConsumer| &consumer.mesh,
        |consumer: &mut MeshConsumer, mesh| consumer.mesh = mesh,
    )?)
    .scene_saveable()
    .build()?)
}

fn source_record(
    id: AssetId,
    source: &str,
    flip_texcoord_v: bool,
) -> Result<SourceAssetRecord, Box<dyn Error>> {
    Ok(SourceAssetRecord::new(
        id,
        TypeKey::new(MESH_ASSET_TYPE_KEY)?,
        ProjectPath::new(source)?,
        SourceImporter::Obj(ObjImportSettings::new(flip_texcoord_v)),
    )?)
}

fn scene_id(index: u64) -> Result<SceneEntityId, Box<dyn Error>> {
    Ok(SceneEntityId::from_str(&format!(
        "00000000-0000-0000-0000-{index:012x}"
    ))?)
}

fn triangle_obj(name: &str, width: f32) -> String {
    format!("o {name}\nv 0 0 0\nv {width} 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3\n")
}

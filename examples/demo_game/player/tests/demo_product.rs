// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use sge_asset::{AssetId, AssetRef};
use sge_asset_pipeline::{CookOutputRoot, full_cook, import_project_assets};
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

    fixture.cook(&project)?;
    fixture.delete_source()?;
    let session = PlayerSession::load(demo_game::GAME, fixture.path())?;
    let cooked_mesh = session.assets().mesh(AssetRef::new(asset))?;
    let (snapshot, view) = session.render_frame()?;

    assert_eq!(cooked_mesh, &imported_mesh);
    assert_eq!(snapshot.cameras().len(), 1);
    assert_eq!(snapshot.meshes().len(), 1);
    assert_eq!(snapshot.lights().len(), 1);
    assert!(view.camera().active());
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
        "Usage: demo-game-player COOKED_ROOT [--max-frames N]\n"
    );
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
#[ignore = "requires a window system; run with xvfb-run"]
fn game_specific_player_presents_two_frames_from_cooked_content()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = CookedDemo::new("window")?;
    fixture.cook(&fixture.project()?)?;
    fixture.delete_source()?;
    let output = Command::new(env!("CARGO_BIN_EXE_demo-game-player"))
        .arg(fixture.path())
        .args(["--max-frames", "2"])
        .output()?;
    assert!(
        output.status.success(),
        "player stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout)?, "presented_frames=2\n");
    Ok(())
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
            "Content/Meshes/demo.obj",
            "Scenes/main.scene.ron",
        ] {
            fs::copy(demo_root().join(relative), source.join(relative))?;
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

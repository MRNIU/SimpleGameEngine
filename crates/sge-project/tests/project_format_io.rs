// Copyright The SimpleGameEngine Contributors

mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_project::{
    AUTHORING_ASSET_MANIFEST_PATH, AuthoringAssetManifest, ManifestError, PROJECT_DESCRIPTOR_PATH,
    ProjectDescriptor, ProjectFormatError, ProjectIoError, ProjectPath, ProjectRoot,
};

use support::{asset_id, source_record};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> std::io::Result<Self> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/project_m2");
        fs::create_dir_all(&base)?;
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("format-{name}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _cleanup_result = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_format_loads_fail_without_creating_anything() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("missing")?;
    let root = ProjectRoot::open(temp.path())?;

    assert!(matches!(
        ProjectDescriptor::load(&root),
        Err(ProjectFormatError::Io(ProjectIoError::Read { path, source }))
            if path.as_str() == PROJECT_DESCRIPTOR_PATH
                && source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(matches!(
        AuthoringAssetManifest::load(&root),
        Err(ManifestError::Io(ProjectIoError::Read { path, source }))
            if path.as_str() == AUTHORING_ASSET_MANIFEST_PATH
                && source.kind() == std::io::ErrorKind::NotFound
    ));
    assert_eq!(fs::read_dir(temp.path())?.count(), 0);
    Ok(())
}

#[test]
fn corrupt_format_loads_preserve_the_exact_existing_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = TestDir::new("corrupt")?;
    fs::create_dir(temp.path().join("Content"))?;
    let descriptor_bytes = b"(format_version: 1, broken";
    let manifest_bytes = [0xff, 0xfe, 0xfd];
    fs::write(temp.path().join(PROJECT_DESCRIPTOR_PATH), descriptor_bytes)?;
    fs::write(
        temp.path().join(AUTHORING_ASSET_MANIFEST_PATH),
        manifest_bytes,
    )?;
    let root = ProjectRoot::open(temp.path())?;

    assert!(matches!(
        ProjectDescriptor::load(&root),
        Err(ProjectFormatError::Parse { path, .. }) if path.as_str() == PROJECT_DESCRIPTOR_PATH
    ));
    assert!(matches!(
        AuthoringAssetManifest::load(&root),
        Err(ManifestError::Parse { path, .. }) if path.as_str() == AUTHORING_ASSET_MANIFEST_PATH
    ));
    assert_eq!(
        fs::read(temp.path().join(PROJECT_DESCRIPTOR_PATH))?,
        descriptor_bytes
    );
    assert_eq!(
        fs::read(temp.path().join(AUTHORING_ASSET_MANIFEST_PATH))?,
        manifest_bytes
    );
    Ok(())
}

#[test]
fn valid_formats_save_and_load_only_at_the_constant_paths() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = TestDir::new("save-load")?;
    fs::create_dir(temp.path().join("Content"))?;
    let root = ProjectRoot::open(temp.path())?;
    let descriptor = descriptor()?;
    let manifest = manifest()?;

    descriptor.save(&root)?;
    manifest.save(&root)?;

    assert_eq!(
        fs::read_to_string(temp.path().join(PROJECT_DESCRIPTOR_PATH))?,
        descriptor.to_ron()?
    );
    assert_eq!(
        fs::read_to_string(temp.path().join(AUTHORING_ASSET_MANIFEST_PATH))?,
        manifest.to_ron()?
    );
    assert_eq!(ProjectDescriptor::load(&root)?, descriptor);
    assert_eq!(AuthoringAssetManifest::load(&root)?, manifest);
    Ok(())
}

#[test]
fn manifest_save_does_not_create_a_missing_content_directory()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("missing-content")?;
    let root = ProjectRoot::open(temp.path())?;
    let error = manifest()?
        .save(&root)
        .expect_err("manifest save created its missing parent");

    assert!(matches!(
        error,
        ManifestError::Io(ProjectIoError::Write { path, source })
            if path.as_str() == AUTHORING_ASSET_MANIFEST_PATH
                && source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(!temp.path().join("Content").exists());
    Ok(())
}

fn descriptor() -> Result<ProjectDescriptor, Box<dyn std::error::Error>> {
    Ok(ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "demo-player",
        "demo-build",
        ProjectPath::new("scenes/main.scene.ron")?,
    )?)
}

fn manifest() -> Result<AuthoringAssetManifest, Box<dyn std::error::Error>> {
    let id = asset_id("10000000-0000-4000-8000-000000000001")?;
    Ok(AuthoringAssetManifest::new(vec![source_record(
        id,
        "Content/mesh.obj",
        false,
    )?])?)
}

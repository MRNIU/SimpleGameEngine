// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use sge_project::{ProjectPath, ProjectRoot};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> std::io::Result<Self> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/project_m2");
        fs::create_dir_all(&base)?;
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{name}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _cleanup_result = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn project_path_accepts_only_canonical_portable_relative_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let path = ProjectPath::new("scenes/main.scene.ron")?;
    assert_eq!(path.as_str(), "scenes/main.scene.ron");

    for invalid in [
        "",
        "/abs",
        "C:/abs",
        "//server/share",
        ".",
        "..",
        "a/./b",
        "a/../b",
        "a//b",
        "a/",
        r"a\b",
        r"a:\b",
        "a\0b",
        "a:b",
    ] {
        assert!(ProjectPath::new(invalid).is_err(), "{invalid:?}");
    }
    Ok(())
}

#[test]
fn project_path_traits_keep_the_canonical_string() -> Result<(), Box<dyn std::error::Error>> {
    let from_borrowed = ProjectPath::try_from("scenes/b.scene.ron")?;
    let from_owned = ProjectPath::try_from(String::from("scenes/a.scene.ron"))?;
    let mut paths = [from_borrowed, from_owned];
    paths.sort();

    assert_eq!(paths[0].to_string(), "scenes/a.scene.ron");
    assert_eq!(paths[1].as_ref(), "scenes/b.scene.ron");
    Ok(())
}

#[test]
fn project_path_serde_uses_and_validates_the_canonical_string()
-> Result<(), Box<dyn std::error::Error>> {
    let path = ProjectPath::new("scenes/main.scene.ron")?;
    let encoded = ron::to_string(&path)?;

    assert_eq!(encoded, r#""scenes/main.scene.ron""#);
    assert_eq!(ron::from_str::<ProjectPath>(&encoded)?, path);
    assert!(ron::from_str::<ProjectPath>(r#""a/../b""#).is_err());
    Ok(())
}

#[test]
fn project_root_canonicalizes_an_existing_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("root-canonical")?;
    let nested = temp.path().join("nested");
    fs::create_dir(&nested)?;
    fs::create_dir(nested.join("scenes"))?;
    fs::write(nested.join("scenes/existing.scene.ron"), b"existing")?;
    let root = ProjectRoot::open(nested.join("..").join("nested"))?;

    let existing = ProjectPath::new("scenes/existing.scene.ron")?;
    assert_eq!(root.read(&existing)?, b"existing");
    let created = ProjectPath::new("created.scene.ron")?;
    root.write_atomic(&created, b"created")?;
    assert_eq!(fs::read(nested.join("created.scene.ron"))?, b"created");

    let file = temp.path().join("not-a-directory");
    fs::write(&file, b"file")?;
    assert!(matches!(
        ProjectRoot::open(file),
        Err(sge_project::ProjectIoError::RootNotDirectory(_))
    ));
    Ok(())
}

#[test]
fn project_root_atomically_creates_reads_and_replaces_a_file()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("write-replace")?;
    fs::create_dir(temp.path().join("scenes"))?;
    let root = ProjectRoot::open(temp.path())?;
    let path = ProjectPath::new("scenes/main.scene.ron")?;

    root.write_atomic(&path, b"old complete bytes")?;
    assert_eq!(root.read(&path)?, b"old complete bytes");

    root.write_atomic(&path, b"new complete bytes")?;
    assert_eq!(root.read(&path)?, b"new complete bytes");

    let root_file = ProjectPath::new("project.sge.ron")?;
    root.write_atomic(&root_file, b"root file")?;
    assert_eq!(root.read(&root_file)?, b"root file");
    assert_eq!(fs::read_dir(temp.path().join("scenes"))?.count(), 1);
    Ok(())
}

#[test]
fn project_root_reports_atomic_commit_phase_failure() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("commit-failure")?;
    let scenes = temp.path().join("scenes");
    fs::create_dir(&scenes)?;
    let target = scenes.join("existing.scene.ron");
    fs::create_dir(&target)?;
    let root = ProjectRoot::open(temp.path())?;
    let path = ProjectPath::new("scenes/existing.scene.ron")?;
    let error = root
        .write_atomic(&path, b"bytes")
        .err()
        .ok_or_else(|| std::io::Error::other("directory target commit unexpectedly succeeded"))?;

    assert!(matches!(
        error,
        sge_project::ProjectIoError::Commit { path: actual, .. } if actual == path
    ));
    assert!(target.is_dir());
    Ok(())
}

#[test]
fn project_root_reports_a_missing_read_without_creating_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("missing-read")?;
    let root = ProjectRoot::open(temp.path())?;
    let path = ProjectPath::new("scenes/missing.scene.ron")?;
    let error = root
        .read(&path)
        .err()
        .ok_or_else(|| std::io::Error::other("missing read unexpectedly succeeded"))?;

    assert!(matches!(
        error,
        sge_project::ProjectIoError::Read { path: actual, source }
            if actual == path && source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(!temp.path().join("scenes").exists());
    Ok(())
}

#[test]
fn project_root_write_does_not_create_a_missing_parent() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TestDir::new("missing-write-parent")?;
    let root = ProjectRoot::open(temp.path())?;
    let path = ProjectPath::new("scenes/main.scene.ron")?;
    let error = root
        .write_atomic(&path, b"bytes")
        .err()
        .ok_or_else(|| std::io::Error::other("write unexpectedly created a missing parent"))?;

    assert!(matches!(
        error,
        sge_project::ProjectIoError::Write { path: actual, source }
            if actual == path && source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(!temp.path().join("scenes").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn project_root_rejects_normal_and_dangling_final_symlinks()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let temp = TestDir::new("final-symlink")?;
    let scenes = temp.path().join("scenes");
    fs::create_dir(&scenes)?;
    let root = ProjectRoot::open(temp.path())?;
    let path = ProjectPath::new("scenes/main.scene.ron")?;
    let target = temp.path().join("target.scene.ron");
    fs::write(&target, b"target bytes")?;
    symlink(&target, scenes.join("main.scene.ron"))?;

    let normal_error = root
        .write_atomic(&path, b"replacement")
        .err()
        .ok_or_else(|| std::io::Error::other("normal final symlink was accepted"))?;
    assert!(matches!(
        normal_error,
        sge_project::ProjectIoError::TargetSymlink { path: actual } if actual == path
    ));
    assert_eq!(fs::read(&target)?, b"target bytes");

    fs::remove_file(scenes.join("main.scene.ron"))?;
    symlink(
        temp.path().join("missing-target.scene.ron"),
        scenes.join("main.scene.ron"),
    )?;
    let dangling_error = root
        .write_atomic(&path, b"replacement")
        .err()
        .ok_or_else(|| std::io::Error::other("dangling final symlink was accepted"))?;
    assert!(matches!(
        dangling_error,
        sge_project::ProjectIoError::TargetSymlink { path: actual } if actual == path
    ));
    assert_eq!(fs::read_dir(&scenes)?.count(), 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn project_root_rejects_parent_symlink_escape_for_read_and_write()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let temp = TestDir::new("parent-symlink")?;
    let project = temp.path().join("project");
    let outside = temp.path().join("outside");
    fs::create_dir(&project)?;
    fs::create_dir(&outside)?;
    fs::write(outside.join("main.scene.ron"), b"outside bytes")?;
    symlink(&outside, project.join("scenes"))?;
    let root = ProjectRoot::open(&project)?;
    let path = ProjectPath::new("scenes/main.scene.ron")?;

    let read_error = root
        .read(&path)
        .err()
        .ok_or_else(|| std::io::Error::other("parent symlink read escape was accepted"))?;
    assert!(matches!(
        read_error,
        sge_project::ProjectIoError::OutsideRoot { path: actual } if actual == path
    ));

    let write_error = root
        .write_atomic(&path, b"replacement")
        .err()
        .ok_or_else(|| std::io::Error::other("parent symlink write escape was accepted"))?;
    assert!(matches!(
        write_error,
        sge_project::ProjectIoError::OutsideRoot { path: actual } if actual == path
    ));
    assert_eq!(fs::read(outside.join("main.scene.ron"))?, b"outside bytes");
    Ok(())
}

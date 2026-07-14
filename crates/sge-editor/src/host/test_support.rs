// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

fn copy_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

pub(super) struct TestProject {
    root: PathBuf,
}

impl TestProject {
    pub(super) fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_editor_host")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Scenes"))?;
        let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game");
        for relative in ["project.sge.ron", "Scenes/main.scene.ron"] {
            fs::copy(demo.join(relative), root.join(relative))?;
        }
        copy_tree(&demo.join("Content"), &root.join("Content"))?;
        Ok(Self { root })
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

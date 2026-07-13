// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf};

pub(super) struct TestProject {
    root: PathBuf,
}

impl TestProject {
    pub(super) fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_editor_host")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Content/Meshes"))?;
        fs::create_dir_all(root.join("Scenes"))?;
        let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game");
        for relative in [
            "project.sge.ron",
            "Content/asset_manifest.ron",
            "Scenes/main.scene.ron",
        ] {
            fs::copy(demo.join(relative), root.join(relative))?;
        }
        for entry in fs::read_dir(demo.join("Content/Meshes"))? {
            let entry = entry?;
            fs::copy(
                entry.path(),
                root.join("Content/Meshes").join(entry.file_name()),
            )?;
        }
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

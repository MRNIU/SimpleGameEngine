// Copyright The SimpleGameEngine Contributors

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use sge_editor::{EditorBuildLauncher, EditorFileDialogs, EditorRunOptions, run};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let report = run(
        demo_game::GAME,
        arguments.project_root,
        EditorRunOptions {
            max_frames: arguments.max_frames,
            start_in_play: arguments.start_in_play,
            build_launcher: Some(EditorBuildLauncher::new(
                "cargo",
                ["run", "--package", "sge-build", "--bin", "sge", "--"],
            )),
            file_dialogs: Some(EditorFileDialogs {
                new_project: new_project_dialog,
                open_project: open_project_dialog,
                open_scene: open_scene_dialog,
                save_scene: save_scene_dialog,
                import_obj: import_obj_dialog,
            }),
            ..EditorRunOptions::default()
        },
    )?;
    println!(
        "preview_prepare={} preview_paint={} play_frames={} gameplay_input_frames={} gameplay_key_w_frames={}",
        report.preview.prepare_count,
        report.preview.paint_count,
        report.play_frames,
        report.gameplay_input_frames,
        report.gameplay_key_w_frames
    );
    Ok(())
}

fn new_project_dialog() -> Result<Option<PathBuf>, String> {
    rfd::FileDialog::new()
        .set_title("Choose a parent folder for DemoGame")
        .pick_folder()
        .map(|parent| create_demo_project(&parent))
        .transpose()
}

fn open_project_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("SimpleGameEngine Project", &["ron"])
        .set_file_name("project.sge.ron")
        .pick_file()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn open_scene_dialog(root: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_directory(root)
        .add_filter("Authoring Scene", &["ron"])
        .pick_file()
}

fn save_scene_dialog(root: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_directory(root)
        .set_file_name("scene.scene.ron")
        .save_file()
}

fn import_obj_dialog(_: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Wavefront OBJ", &["obj"])
        .pick_file()
}

fn create_demo_project(parent: &Path) -> Result<PathBuf, String> {
    let root = parent.join("DemoGame");
    fs::create_dir(&root).map_err(|error| {
        format!(
            "cannot create new project folder {}: {error}",
            root.display()
        )
    })?;
    match populate_demo_project(&root) {
        Ok(()) => Ok(root),
        Err(error) => {
            let _ = fs::remove_dir_all(&root);
            Err(error)
        }
    }
}

fn populate_demo_project(root: &Path) -> Result<(), String> {
    let manifest = sge_project::AuthoringAssetManifest::from_ron(include_str!(
        "../../Content/asset_manifest.ron"
    ))
    .and_then(|manifest| {
        sge_project::AuthoringAssetManifest::new(
            manifest
                .records()
                .iter()
                .filter(|record| record.source().as_str() == "Content/Meshes/demo.obj")
                .cloned()
                .collect(),
        )
    })
    .and_then(|manifest| manifest.to_ron())
    .map_err(|error| error.to_string())?;
    let files = [
        (
            "project.sge.ron",
            include_bytes!("../../project.sge.ron").as_slice(),
        ),
        ("Content/asset_manifest.ron", manifest.as_bytes()),
        (
            "Content/Meshes/demo.obj",
            include_bytes!("../../Content/Meshes/demo.obj").as_slice(),
        ),
        (
            "Scenes/main.scene.ron",
            include_bytes!("../../Scenes/main.scene.ron").as_slice(),
        ),
    ];
    fs::create_dir_all(root.join("Content/Meshes")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("Scenes")).map_err(|error| error.to_string())?;
    for (path, bytes) in files {
        fs::write(root.join(path), bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct Arguments {
    project_root: PathBuf,
    max_frames: Option<u64>,
    start_in_play: bool,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1);
    let Some(first) = values.next() else {
        return Err(usage("missing PROJECT_ROOT"));
    };
    if first == "--help" || first == "-h" {
        println!("{}", usage(""));
        return Ok(None);
    }
    let project_root = PathBuf::from(first);
    let mut max_frames = None;
    let mut start_in_play = false;
    while let Some(argument) = values.next() {
        if argument == "--play" {
            start_in_play = true;
            continue;
        }
        if argument != "--max-frames" {
            return Err(usage(&format!(
                "unknown argument: {}",
                argument.to_string_lossy()
            )));
        }
        let value = values
            .next()
            .ok_or_else(|| usage("--max-frames requires a value"))?;
        let value = value
            .to_str()
            .ok_or_else(|| usage("--max-frames must be UTF-8"))?;
        max_frames = Some(
            value
                .parse()
                .map_err(|_| usage("--max-frames must be an unsigned integer"))?,
        );
    }
    Ok(Some(Arguments {
        project_root,
        max_frames,
        start_in_play,
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!("{prefix}Usage: demo-game-editor PROJECT_ROOT [--play] [--max-frames N]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_creation_owns_a_new_complete_root() -> Result<(), Box<dyn Error>> {
        let parent = std::env::temp_dir().join(format!(
            "sge-demo-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir(&parent)?;
        let root = create_demo_project(&parent)?;
        assert!(root.join("project.sge.ron").is_file());
        assert!(root.join("Content/asset_manifest.ron").is_file());
        assert!(root.join("Content/Meshes/demo.obj").is_file());
        assert!(root.join("Scenes/main.scene.ron").is_file());
        let _session = sge_editor::EditSession::open(demo_game::GAME, &root)?;
        assert!(create_demo_project(&parent).is_err());
        fs::remove_dir_all(parent)?;
        Ok(())
    }
}

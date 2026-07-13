// Copyright The SimpleGameEngine Contributors

use std::{
    error::Error,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use sge_editor::{
    EditorBuildLauncher, EditorFileDialogs, EditorLanguage, EditorRunOptions, EditorTranslations,
    run,
};

mod localization;

use localization::{DemoText, text};

fn main() -> Result<(), Box<dyn Error>> {
    localization::validate_catalogs().map_err(std::io::Error::other)?;
    let translations =
        EditorTranslations::from_json(localization::ENGLISH, localization::SIMPLIFIED_CHINESE)
            .map_err(std::io::Error::other)?;
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let report = run(
        demo_game::GAME,
        arguments.project_root,
        EditorRunOptions {
            max_frames: arguments.max_frames,
            start_in_play: arguments.start_in_play,
            language: arguments.language,
            screenshot: arguments.screenshot,
            ui_actions: arguments.ui_actions,
            build_launcher: Some(build_launcher()),
            file_dialogs: Some(EditorFileDialogs {
                new_project: new_project_dialog,
                open_project: open_project_dialog,
                open_scene: open_scene_dialog,
                save_scene: save_scene_dialog,
                import_obj: import_obj_dialog,
            }),
            translations: Some(translations),
            ..EditorRunOptions::default()
        },
    )?;
    println!(
        "preview_prepare={} preview_paint={} preview_wgpu_prepare={} preview_cpu_prepare={} play_frames={} gameplay_input_frames={} gameplay_key_w_frames={} ui_actions={}",
        report.preview.prepare_count,
        report.preview.paint_count,
        report.preview.wgpu_prepare_count,
        report.preview.cpu_prepare_count,
        report.play_frames,
        report.gameplay_input_frames,
        report.gameplay_key_w_frames,
        report.ui_actions
    );
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn build_launcher() -> EditorBuildLauncher {
    let workspace = workspace_root();
    EditorBuildLauncher::new(
        "cargo",
        [
            OsString::from("run"),
            OsString::from("--manifest-path"),
            workspace.join("Cargo.toml").into_os_string(),
            OsString::from("--package"),
            OsString::from("sge-build"),
            OsString::from("--bin"),
            OsString::from("sge"),
            OsString::from("--"),
        ],
    )
    .with_build_args([OsString::from("--workspace"), workspace.into_os_string()])
}

fn new_project_dialog(language: EditorLanguage) -> Result<Option<PathBuf>, String> {
    rfd::FileDialog::new()
        .set_title(text(language, DemoText::ChooseProjectParent))
        .pick_folder()
        .map(|parent| create_demo_project(&parent))
        .transpose()
}

fn open_project_dialog(language: EditorLanguage) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(text(language, DemoText::OpenProject))
        .add_filter(text(language, DemoText::ProjectFilter), &["ron"])
        .set_file_name("project.sge.ron")
        .pick_file()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn open_scene_dialog(language: EditorLanguage, root: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(text(language, DemoText::OpenScene))
        .set_directory(root)
        .add_filter(text(language, DemoText::SceneFilter), &["ron"])
        .pick_file()
}

fn save_scene_dialog(language: EditorLanguage, root: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(text(language, DemoText::SaveScene))
        .set_directory(root)
        .set_file_name("scene.scene.ron")
        .save_file()
}

fn import_obj_dialog(language: EditorLanguage, _: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(text(language, DemoText::ImportObj))
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
    language: EditorLanguage,
    screenshot: Option<PathBuf>,
    ui_actions: Vec<sge_editor::EditorUiAction>,
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
    let mut language = EditorLanguage::English;
    let mut screenshot = None;
    let mut ui_actions = Vec::new();
    while let Some(argument) = values.next() {
        if argument == "--play" {
            start_in_play = true;
            continue;
        }
        if argument == "--screenshot" {
            screenshot = Some(PathBuf::from(
                values
                    .next()
                    .ok_or_else(|| usage("--screenshot requires a path"))?,
            ));
            continue;
        }
        if argument == "--language" {
            let value = values
                .next()
                .ok_or_else(|| usage("--language requires en or zh-CN"))?;
            let value = value
                .to_str()
                .ok_or_else(|| usage("--language must be UTF-8"))?;
            language = EditorLanguage::from_code(value)
                .ok_or_else(|| usage("--language must be en or zh-CN"))?;
            continue;
        }
        if argument == "--ui-action" {
            let value = values
                .next()
                .ok_or_else(|| usage("--ui-action requires a value"))?;
            let value = value
                .to_str()
                .ok_or_else(|| usage("--ui-action must be UTF-8"))?;
            ui_actions.push(parse_ui_action(value)?);
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
    if screenshot.is_some() && max_frames.is_some() {
        return Err(usage(
            "--screenshot cannot be combined with --max-frames because capture controls window exit",
        ));
    }
    Ok(Some(Arguments {
        project_root,
        max_frames,
        start_in_play,
        language,
        screenshot,
        ui_actions,
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!(
        "{prefix}Usage: demo-game-editor PROJECT_ROOT [--language en|zh-CN] [--play] [--max-frames N] [--screenshot PATH] [--ui-action ACTION]..."
    )
}

fn parse_ui_action(value: &str) -> Result<sge_editor::EditorUiAction, String> {
    use sge_editor::{EditorUiAction, PrimitiveKind};

    let action = match value {
        "create-empty-actor" => EditorUiAction::CreateEmptyActor,
        "create-cube" => EditorUiAction::CreatePrimitive(PrimitiveKind::Cube),
        "create-sphere" => EditorUiAction::CreatePrimitive(PrimitiveKind::Sphere),
        "create-cone" => EditorUiAction::CreatePrimitive(PrimitiveKind::Cone),
        "create-cylinder" => EditorUiAction::CreatePrimitive(PrimitiveKind::Cylinder),
        "duplicate" => EditorUiAction::DuplicateSelection,
        "save" => EditorUiAction::Save,
        "undo" => EditorUiAction::Undo,
        "redo" => EditorUiAction::Redo,
        "play" => EditorUiAction::StartPlay,
        "stop" => EditorUiAction::StopPlay,
        "build" => EditorUiAction::Build,
        "backend:wgpu" => EditorUiAction::SetRenderBackend(sge_editor::RenderBackend::Wgpu),
        "backend:cpu" => EditorUiAction::SetRenderBackend(sge_editor::RenderBackend::Cpu),
        "mode:lit" => EditorUiAction::SetRenderMode(sge_editor::RenderMode::Lit),
        "mode:unlit" => EditorUiAction::SetRenderMode(sge_editor::RenderMode::Unlit),
        "mode:wireframe" => EditorUiAction::SetRenderMode(sge_editor::RenderMode::Wireframe),
        "mode:lit-wireframe" => EditorUiAction::SetRenderMode(sge_editor::RenderMode::LitWireframe),
        "language:en" => EditorUiAction::SetLanguage(EditorLanguage::English),
        "language:zh-CN" => EditorUiAction::SetLanguage(EditorLanguage::SimplifiedChinese),
        _ => value
            .strip_prefix("select:")
            .and_then(|index| index.parse().ok())
            .map(EditorUiAction::SelectHierarchyIndex)
            .ok_or_else(|| usage(&format!("unknown --ui-action: {value}")))?,
    };
    Ok(action)
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

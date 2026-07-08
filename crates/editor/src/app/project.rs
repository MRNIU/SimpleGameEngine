// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(super) const PROJECT_FILE_NAME: &str = "project.sge.ron";
pub(super) const DEFAULT_SCENE_PATH: &str = "scenes/main.scene.ron";
const PROJECT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ProjectDocument {
    pub version: u32,
    pub name: String,
    pub default_scene: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProjectContext {
    pub root: PathBuf,
    pub document: ProjectDocument,
    pub current_scene: PathBuf,
}

#[derive(Debug, Error)]
pub(super) enum ProjectError {
    #[error("project name cannot be empty")]
    EmptyName,
    #[error("project path must be relative to the project root")]
    InvalidRelativePath,
    #[error("scene path must end with .scene.ron")]
    InvalidSceneExtension,
    #[error("repository root cannot be used as a user project")]
    RepositoryRoot,
    #[error("project directory is not empty")]
    NonEmptyDirectory,
    #[error("missing project file")]
    MissingProjectFile,
    #[error("unsupported project version: {0}")]
    UnsupportedVersion(u32),
    #[error("path is outside the current project")]
    OutsideProject,
    #[error("failed to read or write project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to serialize project file: {0}")]
    Serialize(#[from] ron::Error),
    #[error("failed to parse project file: {0}")]
    Deserialize(#[from] ron::error::SpannedError),
    #[error("failed to write default scene: {0}")]
    Scene(#[from] scene::SceneError),
    #[error("failed to write asset manifest: {0}")]
    Asset(#[from] asset::AssetError),
}

pub(super) fn validate_relative_scene_path(path: &Path) -> Result<PathBuf, ProjectError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ProjectError::InvalidRelativePath);
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ProjectError::InvalidRelativePath);
    }
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".scene.ron"))
    {
        return Err(ProjectError::InvalidSceneExtension);
    }
    Ok(path.to_path_buf())
}

pub(super) fn path_inside_project(
    project_root: &Path,
    absolute_path: &Path,
) -> Result<PathBuf, ProjectError> {
    let root = project_root.canonicalize()?;
    let path = absolute_path.canonicalize()?;
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| ProjectError::OutsideProject)?;
    validate_relative_scene_path(relative)
}

pub(super) fn save_as_relative_path(
    project_root: &Path,
    absolute_path: &Path,
) -> Result<PathBuf, ProjectError> {
    let parent = absolute_path.parent().ok_or(ProjectError::OutsideProject)?;
    let file_name = absolute_path
        .file_name()
        .ok_or(ProjectError::InvalidRelativePath)?;
    let root = project_root.canonicalize()?;
    let parent = parent.canonicalize()?;
    let relative_parent = parent
        .strip_prefix(&root)
        .map_err(|_| ProjectError::OutsideProject)?;
    validate_relative_scene_path(&relative_parent.join(file_name))
}

pub(super) fn is_repository_root(path: &Path) -> bool {
    path.join("Cargo.toml").is_file()
        && path.join("crates").is_dir()
        && path.join("AGENTS.md").is_file()
        && path.join("assets/primitives").is_dir()
}

pub(super) fn create_project(root: &Path) -> Result<ProjectContext, ProjectError> {
    if is_repository_root(root) {
        return Err(ProjectError::RepositoryRoot);
    }
    if root.exists() {
        if root.join(PROJECT_FILE_NAME).exists() {
            return Err(ProjectError::NonEmptyDirectory);
        }
        if fs::read_dir(root)?.next().is_some() {
            return Err(ProjectError::NonEmptyDirectory);
        }
    }
    fs::create_dir_all(root.join("scenes"))?;
    fs::create_dir_all(root.join("assets/imported"))?;

    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or(ProjectError::EmptyName)?
        .to_owned();
    let document = ProjectDocument {
        version: PROJECT_VERSION,
        name,
        default_scene: PathBuf::from(DEFAULT_SCENE_PATH),
    };
    write_document(root, &document)?;

    let model = crate::model::EditorModel::default();
    fs::write(root.join(DEFAULT_SCENE_PATH), model.save_scene_to_string()?)?;
    asset::AssetManifest::default().save_to_project_root(root)?;

    Ok(ProjectContext {
        root: root.to_path_buf(),
        document,
        current_scene: PathBuf::from(DEFAULT_SCENE_PATH),
    })
}

pub(super) fn open_project(selection: &Path) -> Result<ProjectContext, ProjectError> {
    let root = if selection
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == PROJECT_FILE_NAME)
    {
        selection
            .parent()
            .ok_or(ProjectError::MissingProjectFile)?
            .to_path_buf()
    } else {
        selection.to_path_buf()
    };
    if is_repository_root(&root) {
        return Err(ProjectError::RepositoryRoot);
    }

    let document_path = root.join(PROJECT_FILE_NAME);
    if !document_path.exists() {
        return Err(ProjectError::MissingProjectFile);
    }
    let document: ProjectDocument = ron::from_str(&fs::read_to_string(&document_path)?)?;
    validate_document(&document)?;
    let current_scene = document.default_scene.clone();
    let scene_path = root.join(&current_scene);
    if !scene_path.exists() {
        if let Some(parent) = scene_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let model = crate::model::EditorModel::default();
        fs::write(&scene_path, model.save_scene_to_string()?)?;
    }
    if !asset::manifest_path(&root).exists() {
        asset::AssetManifest::default().save_to_project_root(&root)?;
    }

    Ok(ProjectContext {
        root,
        document,
        current_scene,
    })
}

fn validate_document(document: &ProjectDocument) -> Result<(), ProjectError> {
    if document.version != PROJECT_VERSION {
        return Err(ProjectError::UnsupportedVersion(document.version));
    }
    if document.name.trim().is_empty() {
        return Err(ProjectError::EmptyName);
    }
    let _ = validate_relative_scene_path(&document.default_scene)?;
    Ok(())
}

fn write_document(root: &Path, document: &ProjectDocument) -> Result<(), ProjectError> {
    let config = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true)
        .enumerate_arrays(true);
    fs::write(
        root.join(PROJECT_FILE_NAME),
        ron::ser::to_string_pretty(document, config)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_document_roundtrips_minimal_fields() {
        let root = temp_root("project_document_roundtrips_minimal_fields");
        let context = create_project(&root).unwrap();

        assert_eq!(context.root, root);
        assert_eq!(context.document.version, 1);
        assert_eq!(
            context.document.name,
            "project_document_roundtrips_minimal_fields"
        );
        assert_eq!(
            context.document.default_scene,
            PathBuf::from(DEFAULT_SCENE_PATH)
        );
        assert_eq!(context.current_scene, PathBuf::from(DEFAULT_SCENE_PATH));

        let opened = open_project(&context.root).unwrap();

        assert_eq!(opened.document, context.document);
        assert_eq!(opened.current_scene, PathBuf::from(DEFAULT_SCENE_PATH));
    }

    #[test]
    fn create_project_writes_required_layout() {
        let root = temp_root("create_project_writes_required_layout");

        create_project(&root).unwrap();

        assert!(root.join(PROJECT_FILE_NAME).exists());
        assert!(root.join("scenes/main.scene.ron").exists());
        assert!(root.join("assets/asset_manifest.ron").exists());
        assert!(root.join("assets/imported").is_dir());
    }

    #[test]
    fn create_project_rejects_non_empty_non_project_directory() {
        let root = temp_root("create_project_rejects_non_empty_non_project_directory");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("notes.txt"), "not a project").unwrap();

        assert!(matches!(
            create_project(&root),
            Err(ProjectError::NonEmptyDirectory)
        ));
    }

    #[test]
    fn project_relative_scene_path_rejects_escape_and_absolute_paths() {
        assert!(validate_relative_scene_path(Path::new("scenes/main.scene.ron")).is_ok());
        assert!(matches!(
            validate_relative_scene_path(Path::new("../main.scene.ron")),
            Err(ProjectError::InvalidRelativePath)
        ));
        assert!(matches!(
            validate_relative_scene_path(Path::new("/tmp/main.scene.ron")),
            Err(ProjectError::InvalidRelativePath)
        ));
        assert!(matches!(
            validate_relative_scene_path(Path::new("scenes/main.ron")),
            Err(ProjectError::InvalidSceneExtension)
        ));
    }

    #[test]
    fn absolute_dialog_path_converts_to_project_relative_path() {
        let root = temp_root("absolute_dialog_path_converts_to_project_relative_path");
        fs::create_dir_all(root.join("scenes")).unwrap();
        fs::write(root.join("scenes/alt.scene.ron"), "").unwrap();

        let relative = path_inside_project(&root, &root.join("scenes/alt.scene.ron")).unwrap();

        assert_eq!(relative, PathBuf::from("scenes/alt.scene.ron"));
    }

    #[test]
    fn save_as_allows_new_file_under_project_and_rejects_outside_path() {
        let root = temp_root("save_as_allows_new_file_under_project_and_rejects_outside_path");
        let scenes = root.join("scenes");
        fs::create_dir_all(&scenes).unwrap();

        let relative = save_as_relative_path(&root, &scenes.join("new.scene.ron")).unwrap();
        let outside = root.parent().unwrap().join("outside.scene.ron");

        assert_eq!(relative, PathBuf::from("scenes/new.scene.ron"));
        assert!(matches!(
            save_as_relative_path(&root, &outside),
            Err(ProjectError::OutsideProject)
        ));
    }

    #[test]
    fn repository_root_guard_rejects_current_repo_shape() {
        let root = temp_root("repository_root_guard_rejects_current_repo_shape");
        fs::create_dir_all(root.join("crates")).unwrap();
        fs::create_dir_all(root.join("assets/primitives")).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("AGENTS.md"), "# rules\n").unwrap();

        assert!(is_repository_root(&root));
        assert!(matches!(
            create_project(&root),
            Err(ProjectError::RepositoryRoot)
        ));
        assert!(matches!(
            open_project(&root),
            Err(ProjectError::RepositoryRoot)
        ));
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/project_file_tests")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        root
    }
}

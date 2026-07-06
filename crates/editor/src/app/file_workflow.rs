// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::model::{EditorModel, EditorSmokeReport};

use super::{EditorApp, PendingFileAction};

const PATH_EMPTY_STATUS: &str = "Path is empty";
const UNSAVED_CHANGES_STATUS: &str = "Unsaved changes: save or discard first";

impl EditorApp {
    pub(super) fn new_scene(&mut self) {
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::New);
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        self.replace_with_new_scene();
    }

    pub(super) fn open_scene(&mut self) {
        let Some(path) = self.path_from_input() else {
            return;
        };
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::Open(path));
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        self.open_scene_path(&path);
    }

    pub(super) fn save_scene(&mut self) {
        let (path, sync_input) = if let Some(path) = self.current_path.clone() {
            (path, false)
        } else {
            let Some(path) = self.path_from_input() else {
                return;
            };
            (path, true)
        };
        let _ = self.save_scene_path(&path, sync_input);
    }

    pub(super) fn save_scene_as(&mut self) {
        let Some(path) = self.path_from_input() else {
            return;
        };
        let _ = self.save_scene_path(&path, true);
    }

    pub(super) fn discard_pending_action(&mut self) {
        match self.pending_action.take() {
            Some(PendingFileAction::New) => self.replace_with_new_scene(),
            Some(PendingFileAction::Open(path)) => self.open_scene_path(&path),
            None => self.status.clear(),
        }
    }

    pub(super) fn run_smoke_file_workflow(
        &mut self,
        path: &Path,
    ) -> anyhow::Result<EditorSmokeReport> {
        self.model.run_smoke_actions_in_place()?;
        self.path_input = path.display().to_string();
        self.save_scene_path(path, true)?;
        self.load_scene_from_path(path)?;
        self.model.smoke_report()
    }

    fn path_from_input(&mut self) -> Option<PathBuf> {
        let input = self.path_input.trim();
        if input.is_empty() {
            self.status = PATH_EMPTY_STATUS.to_owned();
            return None;
        }
        Some(PathBuf::from(input))
    }

    fn replace_with_new_scene(&mut self) {
        self.model = EditorModel::default();
        self.current_path = None;
        self.pending_action = None;
        self.status = "New scene".to_owned();
    }

    fn open_scene_path(&mut self, path: &Path) {
        match self.load_scene_from_path(path) {
            Ok(()) => self.status = "Opened".to_owned(),
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn load_scene_from_path(&mut self, path: &Path) -> anyhow::Result<()> {
        let input = fs::read_to_string(path)?;
        self.model.reopen_scene_from_str(&input)?;
        self.current_path = Some(path.to_path_buf());
        self.path_input = path.display().to_string();
        Ok(())
    }

    fn save_scene_path(&mut self, path: &Path, sync_input: bool) -> anyhow::Result<()> {
        match self.write_scene_to_path(path) {
            Ok(()) => {
                self.current_path = Some(path.to_path_buf());
                if sync_input {
                    self.path_input = path.display().to_string();
                }
                self.pending_action = None;
                self.status = "Saved".to_owned();
                Ok(())
            }
            Err(error) => {
                self.status = format!("Save failed: {error}");
                Err(error)
            }
        }
    }

    fn write_scene_to_path(&mut self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.model.save_scene_to_string()?)?;
        self.model.mark_saved();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::super::{EditorApp, PendingFileAction};

    #[test]
    fn dirty_new_is_blocked_and_sets_pending_action() {
        let mut app = EditorApp::default();
        app.model.create_cube();

        app.new_scene();

        assert_eq!(app.pending_action, Some(PendingFileAction::New));
        assert_eq!(app.status, "Unsaved changes: save or discard first");
        assert!(app.model.world().entity("cube").is_some());
        assert!(app.model.is_dirty());
    }

    #[test]
    fn empty_open_path_reports_error_without_pending_action() {
        let mut app = EditorApp {
            path_input: "   ".to_owned(),
            ..Default::default()
        };

        app.open_scene();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.status, "Path is empty");
    }

    #[test]
    fn dirty_open_is_blocked_and_sets_pending_action() {
        let mut app = EditorApp::default();
        app.model.create_cube();
        let path = temp_scene_path("dirty_open_is_blocked");
        app.path_input = path.display().to_string();

        app.open_scene();

        assert_eq!(
            app.pending_action,
            Some(PendingFileAction::Open(path.clone()))
        );
        assert_eq!(app.status, "Unsaved changes: save or discard first");
        assert!(app.model.world().entity("cube").is_some());
    }

    #[test]
    fn save_success_clears_pending_action() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_success_clears_pending_action");
        app.model.create_cube();
        app.path_input = path.display().to_string();
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.current_path, Some(path.clone()));
        assert!(!app.model.is_dirty());
        assert_eq!(app.status, "Saved");
        assert!(path.exists());
    }

    #[test]
    fn save_as_success_clears_pending_action() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_as_success_clears_pending_action");
        app.model.create_cube();
        app.path_input = path.display().to_string();
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene_as();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.current_path, Some(path.clone()));
        assert!(!app.model.is_dirty());
        assert!(path.exists());
    }

    #[test]
    fn discard_runs_pending_new() {
        let mut app = EditorApp::default();
        app.model.create_cube();
        app.pending_action = Some(PendingFileAction::New);

        app.discard_pending_action();

        assert_eq!(app.pending_action, None);
        assert!(app.model.world().entity("cube").is_none());
        assert_eq!(app.current_path, None);
        assert!(!app.model.is_dirty());
    }

    #[test]
    fn discard_runs_pending_open() {
        let path = temp_scene_path("discard_runs_pending_open");
        write_scene_with_cube(&path);
        let mut app = EditorApp::default();
        app.model.create_cube();
        app.pending_action = Some(PendingFileAction::Open(path.clone()));

        app.discard_pending_action();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.current_path, Some(path));
        assert_eq!(app.status, "Opened");
        assert!(app.model.world().entity("cube").is_some());
        assert!(!app.model.is_dirty());
    }

    #[test]
    fn save_without_current_path_uses_path_input() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_without_current_path_uses_path_input");
        app.model.create_cube();
        app.path_input = path.display().to_string();

        app.save_scene();

        assert_eq!(app.current_path, Some(path.clone()));
        assert!(path.exists());
    }

    #[test]
    fn save_as_updates_current_path() {
        let mut app = EditorApp::default();
        let old_path = temp_scene_path("save_as_updates_current_path_old");
        let new_path = temp_scene_path("save_as_updates_current_path_new");
        app.current_path = Some(old_path);
        app.path_input = new_path.display().to_string();

        app.save_scene_as();

        assert_eq!(app.current_path, Some(new_path));
    }

    #[test]
    fn editor_smoke_uses_file_workflow_to_save_open_and_report() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("editor_smoke_uses_file_workflow");

        let report = app.run_smoke_file_workflow(&path).unwrap();

        assert_eq!(report.mesh_count, 3);
        assert!(report.has_camera);
        assert_eq!(report.viewport_index_count, 108);
        assert_eq!(app.current_path, Some(path.clone()));
        assert!(!app.model.is_dirty());
        assert!(path.exists());
    }

    fn temp_scene_path(name: &str) -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("{name}_{}.scene.ron", std::process::id()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let _ = fs::remove_file(&path);
        path
    }

    fn write_scene_with_cube(path: &Path) {
        let mut model = crate::EditorModel::default();
        model.create_cube();
        fs::write(path, model.save_scene_to_string().unwrap()).unwrap();
    }
}

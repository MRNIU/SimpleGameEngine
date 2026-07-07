// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, Projection};
use eframe::egui;
use math::Transform;

use crate::{
    model::EditorModel,
    viewport::{GizmoDrag, GizmoHandle, ViewportAction, transform_for_gizmo_drag},
};

use super::{EditorApp, PendingFileAction};

const UNSAVED_CHANGES_STATUS: &str = "Unsaved changes: save or discard first";

#[derive(Debug, Clone, PartialEq)]
struct SemanticSmokeState {
    target: EntityId,
    expected_transform: Transform,
    imported_asset: asset::AssetUuid,
    transform_undo_redo_ok: bool,
}

impl EditorApp {
    pub(super) fn new_scene(&mut self) {
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::New);
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        self.replace_with_new_scene();
    }

    pub(super) fn open_scene_dialog(&mut self) {
        let Some(path) = self.pick_open_scene_path() else {
            return;
        };
        self.open_scene_path_or_defer(path);
    }

    fn open_scene_path_or_defer(&mut self, path: PathBuf) {
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::Open(path));
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        self.open_scene_path(&path);
    }

    pub(super) fn save_scene(&mut self) {
        if let Some(path) = self.current_path.clone() {
            let _ = self.save_scene_path(&path);
        } else {
            self.save_scene_as_dialog();
        }
    }

    pub(super) fn save_scene_as_dialog(&mut self) {
        let Some(path) = self.pick_save_scene_path() else {
            return;
        };
        let _ = self.save_scene_path(&path);
    }

    pub(super) fn import_obj_dialog(&mut self) {
        let Some(path) = self.pick_import_obj_path() else {
            return;
        };
        match self.import_obj_path(&path) {
            Ok(_) => {}
            Err(error) => self.status = format!("Import OBJ failed: {error}"),
        }
    }

    pub(crate) fn import_obj_path(
        &mut self,
        source_path: &Path,
    ) -> anyhow::Result<asset::AssetUuid> {
        let is_obj = source_path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("obj"));
        if !is_obj {
            anyhow::bail!("expected .obj");
        }

        let parsed = asset::load_obj_mesh(source_path)?;
        let uuid = asset::AssetUuid::new_v4();
        let existing_paths = self
            .asset_manifest
            .assets
            .iter()
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();
        let relative_destination =
            asset::unique_import_path(&self.project_root, source_path, existing_paths)?;
        let absolute_destination = self.project_root.join(&relative_destination);
        if let Some(parent) = absolute_destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_path, &absolute_destination)?;

        let asset_name = self.next_asset_display_name(&source_asset_name(source_path));
        let mut next_manifest = self.asset_manifest.clone();
        next_manifest.upsert(asset::AssetRecord {
            uuid: uuid.clone(),
            name: asset_name.clone(),
            kind: asset::AssetKind::Mesh,
            path: relative_destination,
            importer: asset::AssetImporter::Obj,
            source_name: source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("imported.obj")
                .to_owned(),
        });
        next_manifest.save_to_project_root(&self.project_root)?;
        self.asset_manifest = next_manifest;
        self.imported_meshes.insert(uuid.clone(), parsed);
        self.asset_load_status
            .insert(uuid.clone(), super::AssetLoadStatus::Loaded);
        let entity = self.model.create_imported_mesh(&uuid, &asset_name)?;
        self.model.select(entity);
        self.status = format!("Imported {asset_name}");
        Ok(uuid)
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
    ) -> anyhow::Result<super::EditorAppSmokeReport> {
        self.project_root = smoke_project_root(path);
        self.reload_asset_cache();
        let semantic_state = self.run_semantic_smoke_actions()?;
        self.save_scene_path(path)?;
        self.load_scene_from_path(path)?;

        let content_reopen_ok = self.semantic_smoke_content_reopened(&semantic_state);
        let view = self.viewport_camera.to_viewport_view();
        let semantic = self.model.smoke_report_for_view_with_checks(
            &view,
            semantic_state.transform_undo_redo_ok,
            content_reopen_ok,
        )?;
        let app = self.app_smoke_checks(&semantic_state);

        anyhow::ensure!(
            semantic.transform_undo_redo_ok,
            "semantic smoke transform undo/redo failed"
        );
        anyhow::ensure!(
            semantic.content_reopen_ok,
            "semantic smoke content did not survive reopen"
        );
        anyhow::ensure!(
            app.history_cleared_after_reopen,
            "smoke history survived reopen"
        );
        anyhow::ensure!(
            app.gizmo_drag_cleared_after_reopen,
            "smoke gizmo drag survived reopen"
        );
        anyhow::ensure!(
            app.pilot_camera_cleared_after_reopen,
            "smoke pilot camera survived reopen"
        );
        anyhow::ensure!(app.asset_count >= 1, "smoke imported asset missing");
        anyhow::ensure!(
            app.imported_mesh_count >= 1,
            "smoke imported mesh cache missing"
        );
        anyhow::ensure!(
            app.imported_asset_reopened,
            "smoke imported asset ref did not survive reopen"
        );
        anyhow::ensure!(
            app.imported_viewport_span,
            "smoke imported viewport span missing"
        );

        Ok(super::EditorAppSmokeReport { semantic, app })
    }

    fn run_semantic_smoke_actions(&mut self) -> anyhow::Result<SemanticSmokeState> {
        let _first = self.model.create_cube();
        let target = self.model.create_cube();
        self.model.select(target.clone());
        self.model.mark_saved();
        self.model.clear_history();

        let before = self
            .model
            .world()
            .entity(target.as_str())
            .ok_or_else(|| anyhow::anyhow!("smoke target cube missing"))?
            .transform;
        let start_pointer = egui::pos2(10.0, 10.0);
        let end_pointer = egui::pos2(60.0, 10.0);
        let after =
            transform_for_gizmo_drag(GizmoHandle::MoveX, before, start_pointer, end_pointer);

        self.handle_viewport_action(ViewportAction::PreviewTransform {
            target: target.clone(),
            transform: after,
        });
        anyhow::ensure!(!self.model.is_dirty(), "gizmo preview dirtied the scene");
        anyhow::ensure!(!self.model.can_undo(), "gizmo preview wrote history");

        self.handle_viewport_action(ViewportAction::CommitTransform {
            target: target.clone(),
            before,
            after,
        });
        anyhow::ensure!(
            self.model.is_dirty(),
            "gizmo commit did not dirty the scene"
        );
        anyhow::ensure!(self.model.can_undo(), "gizmo commit did not write history");

        self.model.undo()?;
        anyhow::ensure!(
            self.model
                .world()
                .entity(target.as_str())
                .is_some_and(|entity| entity.transform == before),
            "gizmo undo did not restore the start transform"
        );
        self.model.redo()?;
        anyhow::ensure!(
            self.model
                .world()
                .entity(target.as_str())
                .is_some_and(|entity| entity.transform == after),
            "gizmo redo did not restore the committed transform"
        );

        self.model.set_material_override(
            &target,
            Some(MaterialOverride {
                base_color: [0.4, 0.9, 0.5, 1.0],
            }),
        )?;
        self.model.set_light(
            &EntityId::new("directional_light"),
            Light {
                kind: LightKind::Directional,
                color: [0.8, 0.9, 1.0],
                intensity: 1.25,
            },
        )?;
        self.model.set_camera(
            &EntityId::new("camera"),
            Camera::new(Projection::Perspective {
                fov_y_degrees: 55.0,
            }),
        )?;

        self.transform_gizmo.start_drag(GizmoDrag {
            target: target.clone(),
            handle: GizmoHandle::MoveX,
            start_pointer,
            start_transform: after,
        });
        anyhow::ensure!(
            self.transform_gizmo.has_drag(),
            "smoke gizmo drag was not set"
        );

        self.model.select(EntityId::new("camera"));
        self.toggle_pilot_camera();
        anyhow::ensure!(self.pilot_camera, "smoke pilot camera was not enabled");

        let source = self.project_root.join("source/smoke_triangle.obj");
        write_smoke_obj(&source)?;
        let imported_asset = self.import_obj_path(&source)?;

        Ok(SemanticSmokeState {
            target,
            expected_transform: after,
            imported_asset,
            transform_undo_redo_ok: true,
        })
    }

    fn semantic_smoke_content_reopened(&self, state: &SemanticSmokeState) -> bool {
        let Some(target) = self.model.world().entity(state.target.as_str()) else {
            return false;
        };
        let material_ok = target
            .material_override
            .as_ref()
            .is_some_and(|material| material.base_color == [0.4, 0.9, 0.5, 1.0]);
        let transform_ok = target.transform == state.expected_transform;
        let light_ok = self
            .model
            .world()
            .entity("directional_light")
            .and_then(|entity| entity.light.as_ref())
            .is_some_and(|light| {
                light.kind == LightKind::Directional
                    && light.color == [0.8, 0.9, 1.0]
                    && light.intensity == 1.25
            });
        let camera_ok = self
            .model
            .world()
            .entity("camera")
            .and_then(|entity| entity.camera.as_ref())
            .is_some_and(|camera| {
                camera.projection
                    == (Projection::Perspective {
                        fov_y_degrees: 55.0,
                    })
            });

        let imported_asset_ref = state.imported_asset.to_asset_ref();
        let imported_asset_ok = self.model.world().entities().any(|entity| {
            entity
                .mesh
                .as_ref()
                .is_some_and(|mesh| mesh.asset == imported_asset_ref)
        });

        material_ok && transform_ok && light_ok && camera_ok && imported_asset_ok
    }

    fn app_smoke_checks(&self, state: &SemanticSmokeState) -> super::EditorAppSmokeChecks {
        let imported_asset_ref = state.imported_asset.to_asset_ref();
        let imported_viewport_span = render::viewport_draw_call_with_view_and_meshes(
            &self.model.render_scene(),
            self.model.selected(),
            &self.viewport_camera.to_viewport_view(),
            &self.imported_meshes,
        )
        .is_some_and(|draw| {
            draw.mesh_spans.iter().any(|span| {
                self.model
                    .world()
                    .entity(span.entity.as_str())
                    .and_then(|entity| entity.mesh.as_ref())
                    .is_some_and(|mesh| mesh.asset == imported_asset_ref)
            })
        });
        super::EditorAppSmokeChecks {
            history_cleared_after_reopen: !self.model.can_undo() && !self.model.can_redo(),
            gizmo_drag_cleared_after_reopen: !self.transform_gizmo.has_drag(),
            pilot_camera_cleared_after_reopen: !self.pilot_camera,
            asset_count: self.asset_manifest.assets.len(),
            imported_mesh_count: self.imported_meshes.len(),
            imported_asset_reopened: self.model.world().entities().any(|entity| {
                entity
                    .mesh
                    .as_ref()
                    .is_some_and(|mesh| mesh.asset == imported_asset_ref)
            }),
            imported_viewport_span,
        }
    }

    pub(super) fn replace_with_new_scene(&mut self) {
        self.model = EditorModel::default();
        self.model.clear_history();
        self.transform_gizmo.clear_drag();
        self.pilot_camera = false;
        self.clear_content_edit_sessions();
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
        self.model.clear_history();
        self.transform_gizmo.clear_drag();
        self.pilot_camera = false;
        self.clear_content_edit_sessions();
        self.current_path = Some(path.to_path_buf());
        self.reload_asset_cache();
        Ok(())
    }

    fn save_scene_path(&mut self, path: &Path) -> anyhow::Result<()> {
        match self.write_scene_to_path(path) {
            Ok(()) => {
                self.current_path = Some(path.to_path_buf());
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

    fn pick_open_scene_path(&mut self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(path) = self.test_dialog_paths.open_scene.take() {
            return path;
        }

        rfd::FileDialog::new()
            .add_filter("Scene", &["scene.ron", "ron"])
            .pick_file()
    }

    fn pick_save_scene_path(&mut self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(path) = self.test_dialog_paths.save_scene.take() {
            return path;
        }

        rfd::FileDialog::new()
            .add_filter("Scene", &["scene.ron", "ron"])
            .set_file_name("scene.scene.ron")
            .save_file()
    }

    fn pick_import_obj_path(&mut self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(path) = self.test_dialog_paths.import_obj.take() {
            return path;
        }

        rfd::FileDialog::new()
            .add_filter("OBJ", &["obj"])
            .pick_file()
    }

    pub(super) fn reload_asset_cache(&mut self) {
        self.asset_manifest =
            asset::AssetManifest::load_from_project_root(&self.project_root).unwrap_or_default();
        self.imported_meshes.clear();
        self.asset_load_status.clear();
        for record in &self.asset_manifest.assets {
            let path = self.project_root.join(&record.path);
            if !path.exists() {
                self.asset_load_status
                    .insert(record.uuid.clone(), super::AssetLoadStatus::MissingFile);
                continue;
            }
            match asset::load_obj_mesh(&path) {
                Ok(mesh) => {
                    self.imported_meshes.insert(record.uuid.clone(), mesh);
                    self.asset_load_status
                        .insert(record.uuid.clone(), super::AssetLoadStatus::Loaded);
                }
                Err(error) => {
                    self.asset_load_status.insert(
                        record.uuid.clone(),
                        super::AssetLoadStatus::LoadFailed(error.to_string()),
                    );
                }
            }
        }
    }

    fn next_asset_display_name(&self, base: &str) -> String {
        if self
            .asset_manifest
            .assets
            .iter()
            .all(|record| record.name != base)
        {
            return base.to_owned();
        }
        for index in 2..=u32::MAX {
            let name = format!("{base} {index}");
            if self
                .asset_manifest
                .assets
                .iter()
                .all(|record| record.name != name)
            {
                return name;
            }
        }
        base.to_owned()
    }

    #[cfg(test)]
    pub(super) fn set_next_open_scene_dialog_path_for_test(&mut self, path: Option<PathBuf>) {
        self.test_dialog_paths.open_scene = Some(path);
    }

    #[cfg(test)]
    pub(super) fn set_next_save_scene_dialog_path_for_test(&mut self, path: Option<PathBuf>) {
        self.test_dialog_paths.save_scene = Some(path);
    }

    #[cfg(test)]
    pub(super) fn set_next_import_obj_dialog_path_for_test(&mut self, path: Option<PathBuf>) {
        self.test_dialog_paths.import_obj = Some(path);
    }
}

fn source_asset_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Imported Asset")
        .to_owned()
}

fn smoke_project_root(scene_path: &Path) -> PathBuf {
    scene_path
        .parent()
        .unwrap_or_else(|| Path::new("target/tmp"))
        .join(format!("editor_smoke_project_{}", std::process::id()))
}

fn write_smoke_obj(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n")?;
    Ok(())
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
    fn dirty_open_is_blocked_and_sets_pending_action() {
        let mut app = EditorApp::default();
        app.model.create_cube();
        let path = temp_scene_path("dirty_open_is_blocked");
        app.set_next_open_scene_dialog_path_for_test(Some(path.clone()));

        app.open_scene_dialog();

        assert_eq!(
            app.pending_action,
            Some(PendingFileAction::Open(path.clone()))
        );
        assert_eq!(app.status, "Unsaved changes: save or discard first");
        assert!(app.model.world().entity("cube").is_some());
    }

    #[test]
    fn save_after_dirty_guard_clears_pending_without_running_new() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_after_dirty_guard_clears_pending_without_running_new");
        let cube = app.model.create_cube();
        app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene();

        assert_eq!(app.pending_action, None);
        assert!(app.model.world().entity(cube.as_str()).is_some());
        assert_eq!(app.current_path, Some(path.clone()));
        assert!(!app.model.is_dirty());
        assert_eq!(app.status, "Saved");
        assert!(path.exists());
    }

    #[test]
    fn save_as_after_dirty_guard_clears_pending_without_running_new() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_as_after_dirty_guard_clears_pending_without_running_new");
        let cube = app.model.create_cube();
        app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene_as_dialog();

        assert_eq!(app.pending_action, None);
        assert!(app.model.world().entity(cube.as_str()).is_some());
        assert_eq!(app.current_path, Some(path.clone()));
        assert!(!app.model.is_dirty());
        assert_eq!(app.status, "Saved");
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
    fn save_without_current_path_uses_save_as_dialog() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("save_without_current_path_uses_save_as_dialog");
        app.model.create_cube();
        app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));

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
        app.set_next_save_scene_dialog_path_for_test(Some(new_path.clone()));

        app.save_scene_as_dialog();

        assert_eq!(app.current_path, Some(new_path));
    }

    #[test]
    fn new_scene_clears_history() {
        let mut app = EditorApp::default();
        app.model.create_cube();
        app.model.mark_saved();
        assert!(app.model.can_undo());

        app.new_scene();

        assert!(!app.model.can_undo());
        assert!(!app.model.can_redo());
    }

    #[test]
    fn open_scene_clears_history() {
        let path = temp_scene_path("open_scene_clears_history");
        write_scene_with_cube(&path);
        let mut app = EditorApp::default();
        app.model.create_cube();
        app.model.mark_saved();
        app.set_next_open_scene_dialog_path_for_test(Some(path));
        assert!(app.model.can_undo());

        app.open_scene_dialog();

        assert!(!app.model.can_undo());
        assert!(!app.model.can_redo());
        assert_eq!(app.status, "Opened");
    }

    #[test]
    fn editor_smoke_uses_file_workflow_to_save_open_and_report() {
        let mut app = EditorApp::default();
        let path = temp_scene_path("editor_smoke_uses_file_workflow");

        let report = app.run_smoke_file_workflow(&path).unwrap();

        assert_eq!(report.semantic.mesh_count, 3);
        assert!(report.semantic.has_camera);
        assert!(report.semantic.has_light);
        assert_eq!(report.semantic.viewport_index_count, 72);
        assert!(report.semantic.transform_undo_redo_ok);
        assert!(report.semantic.content_reopen_ok);
        assert!(report.app.history_cleared_after_reopen);
        assert!(report.app.gizmo_drag_cleared_after_reopen);
        assert!(report.app.pilot_camera_cleared_after_reopen);
        assert!(report.app.asset_count >= 1);
        assert!(report.app.imported_mesh_count >= 1);
        assert!(report.app.imported_asset_reopened);
        assert!(report.app.imported_viewport_span);
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

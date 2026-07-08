// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, Projection};
use eframe::egui;
use math::Transform;

use crate::{
    model::{EditorModel, PrimitiveKind},
    viewport::{GizmoDrag, GizmoHandle, ViewportAction, transform_for_gizmo_drag},
};

use super::{EditorApp, PendingFileAction, project};

const UNSAVED_CHANGES_STATUS: &str = "Unsaved changes: save or discard first";

#[derive(Debug, Clone, PartialEq)]
struct SemanticSmokeState {
    target: EntityId,
    expected_transform: Transform,
    imported_asset: asset::AssetUuid,
    transform_undo_redo_ok: bool,
}

impl EditorApp {
    pub(super) fn new_project_dialog(&mut self) {
        let Some(path) = self.pick_new_project_path() else {
            self.pending_action = None;
            return;
        };
        self.new_project_path_or_defer(path);
    }

    pub(super) fn open_project_dialog(&mut self) {
        let Some(path) = self.pick_open_project_path() else {
            self.pending_action = None;
            return;
        };
        self.open_project_path_or_defer(path);
    }

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
        let Some(project_root) = self.require_project_root() else {
            return;
        };
        let relative = match project::path_inside_project(&project_root, &path) {
            Ok(path) => path,
            Err(error) => {
                self.status = format!("Open failed: {error}");
                return;
            }
        };
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::Open(relative));
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        self.open_scene_relative_path(&relative);
    }

    pub(super) fn save_scene(&mut self) {
        if self.require_project_root().is_none() {
            return;
        }
        if let Some(path) = self.current_path.clone() {
            let _ = self.save_scene_path(&path);
        } else {
            self.save_scene_as_dialog();
        }
    }

    pub(super) fn save_scene_as_dialog(&mut self) {
        let Some(project_root) = self.require_project_root() else {
            return;
        };
        let Some(path) = self.pick_save_scene_path() else {
            return;
        };
        let relative = match project::save_as_relative_path(&project_root, &path) {
            Ok(path) => path,
            Err(error) => {
                self.status = format!("Save failed: {error}");
                return;
            }
        };
        let _ = self.save_scene_path(&relative);
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
        let Some(project_root) = self.require_project_root() else {
            anyhow::bail!("Open or create a project first");
        };
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
            asset::unique_import_path(&project_root, source_path, existing_paths)?;
        let absolute_destination = project_root.join(&relative_destination);
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
        next_manifest.save_to_project_root(&project_root)?;
        let transform = imported_mesh_transform(&parsed);
        self.asset_manifest = next_manifest;
        self.imported_meshes.insert(uuid.clone(), parsed);
        self.asset_load_status
            .insert(uuid.clone(), super::AssetLoadStatus::Loaded);
        let entity = self
            .model
            .create_imported_mesh(&uuid, &asset_name, transform)?;
        self.model.select(entity);
        self.status = format!("Imported {asset_name}");
        Ok(uuid)
    }

    pub(super) fn discard_pending_action(&mut self) {
        match self.pending_action.take() {
            Some(PendingFileAction::New) => self.replace_with_new_scene(),
            Some(PendingFileAction::Open(path)) => self.open_scene_relative_path(&path),
            Some(PendingFileAction::NewProject(path)) => {
                if let Err(error) = self.new_project_path(&path) {
                    self.status = format!("New project failed: {error}");
                }
            }
            Some(PendingFileAction::OpenProject(path)) => {
                if let Err(error) = self.open_project_path(&path) {
                    self.status = format!("Open project failed: {error}");
                }
            }
            None => self.status.clear(),
        }
    }

    fn new_project_path_or_defer(&mut self, path: PathBuf) {
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::NewProject(path));
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        if let Err(error) = self.new_project_path(&path) {
            self.status = format!("New project failed: {error}");
        }
    }

    fn open_project_path_or_defer(&mut self, path: PathBuf) {
        if self.model.is_dirty() {
            self.pending_action = Some(PendingFileAction::OpenProject(path));
            self.status = UNSAVED_CHANGES_STATUS.to_owned();
            return;
        }
        if let Err(error) = self.open_project_path(&path) {
            self.status = format!("Open project failed: {error}");
        }
    }

    fn new_project_path(&mut self, path: &Path) -> anyhow::Result<()> {
        let context = project::create_project(path)?;
        self.install_project_context(context, "Project created")
    }

    fn open_project_path(&mut self, path: &Path) -> anyhow::Result<()> {
        let context = project::open_project(path)?;
        self.install_project_context(context, "Project opened")
    }

    fn install_project_context(
        &mut self,
        context: project::ProjectContext,
        status: &str,
    ) -> anyhow::Result<()> {
        let scene_path = context.root.join(&context.current_scene);
        let input = fs::read_to_string(&scene_path)?;
        self.model.reopen_scene_from_str(&input)?;
        self.model.clear_history();
        self.model.mark_saved();
        self.transform_gizmo.clear_drag();
        self.pilot_camera = false;
        self.clear_content_edit_sessions();
        self.current_path = Some(context.current_scene.clone());
        self.current_project = Some(context);
        self.pending_action = None;
        self.reload_asset_cache();
        self.status = status.to_owned();
        Ok(())
    }

    pub(super) fn run_smoke_file_workflow(
        &mut self,
        path: &Path,
    ) -> anyhow::Result<super::EditorAppSmokeReport> {
        let project_root = smoke_project_root(path);
        let _ = fs::remove_dir_all(&project_root);
        self.new_project_path(&project_root)?;
        let semantic_state = self.run_semantic_smoke_actions()?;
        let scene_path = PathBuf::from(project::DEFAULT_SCENE_PATH);
        self.save_scene_path(&scene_path)?;
        self.load_scene_from_relative_path(&scene_path)?;

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
        let _sphere = self.model.create_primitive(PrimitiveKind::Sphere);
        let _cone = self.model.create_primitive(PrimitiveKind::Cone);
        let _cylinder = self.model.create_primitive(PrimitiveKind::Cylinder);
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
        let moved =
            transform_for_gizmo_drag(GizmoHandle::MoveX, before, start_pointer, end_pointer);

        self.handle_viewport_action(ViewportAction::PreviewTransform {
            target: target.clone(),
            transform: moved,
        });
        anyhow::ensure!(!self.model.is_dirty(), "gizmo preview dirtied the scene");
        anyhow::ensure!(!self.model.can_undo(), "gizmo preview wrote history");

        self.handle_viewport_action(ViewportAction::CommitTransform {
            target: target.clone(),
            before,
            after: moved,
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
                .is_some_and(|entity| entity.transform == moved),
            "gizmo redo did not restore the committed transform"
        );
        self.model.mark_saved();
        self.model.clear_history();

        let rotate_pointer = egui::pos2(60.0, -40.0);
        let rotate_preview =
            transform_for_gizmo_drag(GizmoHandle::RotateZ, moved, start_pointer, rotate_pointer);
        self.handle_viewport_action(ViewportAction::PreviewTransform {
            target: target.clone(),
            transform: rotate_preview,
        });
        let rotated = self
            .model
            .world()
            .entity(target.as_str())
            .ok_or_else(|| anyhow::anyhow!("smoke target cube missing after rotate preview"))?
            .transform;
        anyhow::ensure!(
            rotated.translation == moved.translation
                && rotated.scale == moved.scale
                && rotated.rotation != moved.rotation,
            "gizmo rotate preview did not update the transform"
        );
        anyhow::ensure!(
            !self.model.is_dirty(),
            "gizmo rotate preview dirtied the scene"
        );
        anyhow::ensure!(!self.model.can_undo(), "gizmo rotate preview wrote history");
        self.handle_viewport_action(ViewportAction::CommitTransform {
            target: target.clone(),
            before: moved,
            after: rotated,
        });
        anyhow::ensure!(
            self.model.is_dirty(),
            "gizmo rotate commit did not dirty the scene"
        );
        anyhow::ensure!(
            self.model.can_undo(),
            "gizmo rotate commit did not write history"
        );
        self.model.undo()?;
        anyhow::ensure!(
            self.model
                .world()
                .entity(target.as_str())
                .is_some_and(|entity| entity.transform == moved),
            "gizmo rotate undo did not restore the moved transform"
        );
        self.model.redo()?;
        anyhow::ensure!(
            self.model
                .world()
                .entity(target.as_str())
                .is_some_and(|entity| entity.transform == rotated),
            "gizmo rotate redo did not restore the rotated transform"
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
            start_transform: rotated,
        });
        anyhow::ensure!(
            self.transform_gizmo.has_drag(),
            "smoke gizmo drag was not set"
        );

        self.model.select(EntityId::new("camera"));
        self.toggle_pilot_camera();
        anyhow::ensure!(self.pilot_camera, "smoke pilot camera was not enabled");

        let source = self
            .current_project_root()
            .ok_or_else(|| anyhow::anyhow!("Open or create a project first"))?
            .join("source/smoke_triangle.obj");
        write_smoke_obj(&source)?;
        let imported_asset = self.import_obj_path(&source)?;

        Ok(SemanticSmokeState {
            target,
            expected_transform: rotated,
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

    fn open_scene_relative_path(&mut self, path: &Path) {
        match self.load_scene_from_relative_path(path) {
            Ok(()) => self.status = "Opened".to_owned(),
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn load_scene_from_relative_path(&mut self, relative_path: &Path) -> anyhow::Result<()> {
        let project_root = self
            .current_project_root()
            .ok_or_else(|| anyhow::anyhow!("Open or create a project first"))?;
        let input = fs::read_to_string(project_root.join(relative_path))?;
        self.model.reopen_scene_from_str(&input)?;
        self.model.clear_history();
        self.transform_gizmo.clear_drag();
        self.pilot_camera = false;
        self.clear_content_edit_sessions();
        self.current_path = Some(relative_path.to_path_buf());
        if let Some(project) = &mut self.current_project {
            project.current_scene = relative_path.to_path_buf();
        }
        self.reload_asset_cache();
        Ok(())
    }

    fn save_scene_path(&mut self, relative_path: &Path) -> anyhow::Result<()> {
        match self.write_scene_to_relative_path(relative_path) {
            Ok(()) => {
                self.current_path = Some(relative_path.to_path_buf());
                if let Some(project) = &mut self.current_project {
                    project.current_scene = relative_path.to_path_buf();
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

    fn write_scene_to_relative_path(&mut self, relative_path: &Path) -> anyhow::Result<()> {
        let project_root = self
            .current_project_root()
            .ok_or_else(|| anyhow::anyhow!("Open or create a project first"))?;
        let path = project_root.join(relative_path);
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

    fn pick_new_project_path(&mut self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(path) = self.test_dialog_paths.new_project.take() {
            return path;
        }

        rfd::FileDialog::new().pick_folder()
    }

    fn pick_open_project_path(&mut self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(path) = self.test_dialog_paths.open_project.take() {
            return path;
        }

        rfd::FileDialog::new()
            .add_filter("SimpleGameEngine Project", &["sge.ron"])
            .pick_file()
    }

    pub(super) fn reload_asset_cache(&mut self) {
        let Some(project_root) = self.current_project_root().map(Path::to_path_buf) else {
            self.asset_manifest = asset::AssetManifest::default();
            self.imported_meshes.clear();
            self.asset_load_status.clear();
            return;
        };
        self.asset_manifest =
            asset::AssetManifest::load_from_project_root(&project_root).unwrap_or_default();
        self.imported_meshes.clear();
        self.asset_load_status.clear();
        for record in &self.asset_manifest.assets {
            let path = project_root.join(&record.path);
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

    #[cfg(test)]
    pub(super) fn set_next_new_project_dialog_path_for_test(&mut self, path: Option<PathBuf>) {
        self.test_dialog_paths.new_project = Some(path);
    }

    #[cfg(test)]
    pub(super) fn set_next_open_project_dialog_path_for_test(&mut self, path: Option<PathBuf>) {
        self.test_dialog_paths.open_project = Some(path);
    }

    #[cfg(test)]
    pub(super) fn new_project_path_for_test(&mut self, path: &Path) -> anyhow::Result<()> {
        self.new_project_path(path)
    }

    #[cfg(test)]
    pub(super) fn open_project_path_for_test(&mut self, path: &Path) -> anyhow::Result<()> {
        self.open_project_path(path)
    }
}

fn source_asset_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Imported Asset")
        .to_owned()
}

fn imported_mesh_transform(mesh: &asset::ImportedMesh) -> Transform {
    let scale = imported_mesh_default_scale(mesh);
    Transform {
        scale: [scale; 3],
        ..Transform::identity()
    }
}

fn imported_mesh_default_scale(mesh: &asset::ImportedMesh) -> f32 {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    let max_extent = (0..3).map(|axis| max[axis] - min[axis]).fold(0.0, f32::max);
    if max_extent.is_finite() && max_extent > 2.0 {
        2.0 / max_extent
    } else {
        1.0
    }
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
        let root = open_test_project(&mut app, "dirty_open_is_blocked_project");
        app.model.create_cube();
        let relative = PathBuf::from("scenes/dirty_open.scene.ron");
        let path = root.join(&relative);
        write_scene_with_cube(&path);
        app.set_next_open_scene_dialog_path_for_test(Some(path.clone()));

        app.open_scene_dialog();

        assert_eq!(app.pending_action, Some(PendingFileAction::Open(relative)));
        assert_eq!(app.status, "Unsaved changes: save or discard first");
        assert!(app.model.world().entity("cube").is_some());
    }

    #[test]
    fn save_after_dirty_guard_clears_pending_without_running_new() {
        let mut app = EditorApp::default();
        let root = open_test_project(
            &mut app,
            "save_after_dirty_guard_clears_pending_without_running_new",
        );
        let cube = app.model.create_cube();
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene();

        assert_eq!(app.pending_action, None);
        assert!(app.model.world().entity(cube.as_str()).is_some());
        assert_eq!(
            app.current_path,
            Some(PathBuf::from("scenes/main.scene.ron"))
        );
        assert!(!app.model.is_dirty());
        assert_eq!(app.status, "Saved");
        assert!(root.join("scenes/main.scene.ron").exists());
    }

    #[test]
    fn save_as_after_dirty_guard_clears_pending_without_running_new() {
        let mut app = EditorApp::default();
        let root = open_test_project(
            &mut app,
            "save_as_after_dirty_guard_clears_pending_without_running_new",
        );
        let relative = PathBuf::from("scenes/save_as.scene.ron");
        let path = root.join(&relative);
        let cube = app.model.create_cube();
        app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));
        app.pending_action = Some(PendingFileAction::New);

        app.save_scene_as_dialog();

        assert_eq!(app.pending_action, None);
        assert!(app.model.world().entity(cube.as_str()).is_some());
        assert_eq!(app.current_path, Some(relative));
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
        let mut app = EditorApp::default();
        let root = open_test_project(&mut app, "discard_runs_pending_open_project");
        let relative = PathBuf::from("scenes/discard_open.scene.ron");
        let path = root.join(&relative);
        write_scene_with_cube(&path);
        app.model.create_cube();
        app.pending_action = Some(PendingFileAction::Open(relative.clone()));

        app.discard_pending_action();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.current_path, Some(relative));
        assert_eq!(app.status, "Opened");
        assert!(app.model.world().entity("cube").is_some());
        assert!(!app.model.is_dirty());
    }

    #[test]
    fn save_without_current_path_uses_save_as_dialog() {
        let mut app = EditorApp::default();
        let root = open_test_project(&mut app, "save_without_current_path_uses_save_as_dialog");
        app.current_path = None;
        let relative = PathBuf::from("scenes/saved.scene.ron");
        let path = root.join(&relative);
        app.model.create_cube();
        app.set_next_save_scene_dialog_path_for_test(Some(path.clone()));

        app.save_scene();

        assert_eq!(app.current_path, Some(relative));
        assert!(path.exists());
    }

    #[test]
    fn save_as_updates_current_path() {
        let mut app = EditorApp::default();
        let root = open_test_project(&mut app, "save_as_updates_current_path");
        let new_path = root.join("scenes/new.scene.ron");
        app.current_path = Some(PathBuf::from("scenes/old.scene.ron"));
        app.set_next_save_scene_dialog_path_for_test(Some(new_path.clone()));

        app.save_scene_as_dialog();

        assert_eq!(
            app.current_path,
            Some(PathBuf::from("scenes/new.scene.ron"))
        );
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
        let mut app = EditorApp::default();
        let root = open_test_project(&mut app, "open_scene_clears_history_project");
        let path = root.join("scenes/open.scene.ron");
        write_scene_with_cube(&path);
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

        assert_eq!(report.semantic.mesh_count, 7);
        assert!(report.semantic.has_camera);
        assert!(report.semantic.has_light);
        assert_eq!(report.semantic.viewport_index_count, 276);
        assert!(report.semantic.transform_undo_redo_ok);
        assert!(report.semantic.content_reopen_ok);
        assert!(report.app.history_cleared_after_reopen);
        assert!(report.app.gizmo_drag_cleared_after_reopen);
        assert!(report.app.pilot_camera_cleared_after_reopen);
        assert!(report.app.asset_count >= 1);
        assert!(report.app.imported_mesh_count >= 1);
        assert!(report.app.imported_asset_reopened);
        assert!(report.app.imported_viewport_span);
        assert_eq!(
            app.current_path,
            Some(PathBuf::from("scenes/main.scene.ron"))
        );
        assert!(!app.model.is_dirty());
        assert!(
            app.current_project
                .as_ref()
                .unwrap()
                .root
                .join("scenes/main.scene.ron")
                .exists()
        );
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut model = crate::EditorModel::default();
        model.create_cube();
        fs::write(path, model.save_scene_to_string().unwrap()).unwrap();
    }

    fn open_test_project(app: &mut EditorApp, name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/file_workflow_project_tests")
            .join(format!("{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        app.new_project_path_for_test(&root).unwrap();
        root
    }
}

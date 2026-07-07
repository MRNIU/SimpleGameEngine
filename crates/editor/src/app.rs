// Copyright The SimpleGameEngine Contributors

use std::path::PathBuf;

use ecs::{Camera, EntityId, Light, MaterialOverride};
use eframe::egui;
use math::Transform;

use crate::{
    model::{EditorError, EditorModel, EditorSmokeReport},
    viewport::{
        GizmoMode, TransformGizmoState, ViewCamera, ViewportAction, ViewportWgpuProbe,
        install_viewport_renderer,
    },
};

mod file_workflow;
mod fonts;
mod panels;

use fonts::install_cjk_font;

const SMOKE_MAX_VIEWPORT_FRAMES: u32 = 120;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorLaunchOptions {
    pub smoke_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EditorAppSmokeChecks {
    pub(crate) history_cleared_after_reopen: bool,
    pub(crate) gizmo_drag_cleared_after_reopen: bool,
    pub(crate) pilot_camera_cleared_after_reopen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorAppSmokeReport {
    pub(crate) semantic: EditorSmokeReport,
    pub(crate) app: EditorAppSmokeChecks,
}

impl EditorLaunchOptions {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut smoke_path = None;
        let mut args = args.into_iter();
        let _program = args.next();
        while let Some(arg) = args.next() {
            if arg == "--smoke" {
                smoke_path = Some(
                    args.next()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("target/tmp/editor_smoke.scene.ron")),
                );
            } else {
                anyhow::bail!("unknown editor argument: {arg}");
            }
        }
        Ok(Self { smoke_path })
    }
}

#[derive(Debug, Default)]
pub struct EditorApp {
    model: EditorModel,
    path_input: String,
    current_path: Option<PathBuf>,
    pending_action: Option<PendingFileAction>,
    status: String,
    options: EditorLaunchOptions,
    smoke_report: Option<EditorAppSmokeReport>,
    smoke_frame_count: u32,
    viewport_probe: ViewportWgpuProbe,
    wgpu_viewport_available: bool,
    viewport_camera: ViewCamera,
    transform_gizmo: TransformGizmoState,
    fit_view_requested: bool,
    name_edit: Option<NameEditSession>,
    transform_edit: Option<TransformEditSession>,
    pilot_camera: bool,
    material_edit: Option<MaterialEditSession>,
    light_edit: Option<LightEditSession>,
    camera_edit: Option<CameraEditSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingFileAction {
    New,
    Open(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorUiAction {
    NewScene,
    OpenScene,
    SaveScene,
    SaveSceneAs,
    DiscardPendingAction,
    Undo,
    Redo,
    CreateCube,
    DuplicateSelection,
    DeleteSelection,
    SetGizmoMode(GizmoMode),
    FitView,
    TogglePilotCamera,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NameEditSession {
    target: EntityId,
    before: String,
    buffer: String,
}

#[derive(Debug, Clone, PartialEq)]
struct TransformEditSession {
    target: EntityId,
    before: Transform,
    dirty_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct MaterialEditSession {
    target: EntityId,
    before: Option<MaterialOverride>,
    dirty_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct LightEditSession {
    target: EntityId,
    before: Light,
    dirty_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct CameraEditSession {
    target: EntityId,
    before: Camera,
    dirty_before: bool,
}

impl EditorApp {
    #[must_use]
    pub fn new(_creation_context: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_options(_creation_context, EditorLaunchOptions::default())
    }

    #[must_use]
    pub fn new_with_options(
        creation_context: &eframe::CreationContext<'_>,
        options: EditorLaunchOptions,
    ) -> Self {
        install_cjk_font(&creation_context.egui_ctx);
        let wgpu_viewport_available = install_viewport_renderer(creation_context);
        Self {
            options,
            wgpu_viewport_available,
            ..Self::default()
        }
    }

    fn target_is_current_selection(&self, target: &EntityId) -> bool {
        self.model
            .selected()
            .is_some_and(|selected| selected == target)
            && self.model.world().entity(target.as_str()).is_some()
    }

    fn preview_viewport_transform(&mut self, target: EntityId, transform: Transform) {
        if !self.target_is_current_selection(&target) {
            self.transform_gizmo.clear_drag();
            self.status = "Gizmo target changed".to_owned();
            return;
        }

        match self.model.preview_transform(&target, transform) {
            Ok(()) => self.status = "Gizmo preview".to_owned(),
            Err(error) => {
                self.transform_gizmo.clear_drag();
                self.status = format_editor_error("Gizmo failed", error);
            }
        }
    }

    fn commit_viewport_transform(&mut self, target: EntityId, before: Transform, after: Transform) {
        if !self.target_is_current_selection(&target) {
            self.transform_gizmo.clear_drag();
            self.status = "Gizmo target changed".to_owned();
            return;
        }

        match self.model.commit_transform_edit(&target, before, after) {
            Ok(true) => self.status = "Gizmo updated".to_owned(),
            Ok(false) => self.status = "Gizmo unchanged".to_owned(),
            Err(error) => {
                self.transform_gizmo.clear_drag();
                self.status = format_editor_error("Gizmo failed", error);
            }
        }
    }

    fn restore_viewport_transform(&mut self, target: EntityId, transform: Transform) {
        match self
            .model
            .restore_transform_preview(&target, transform, self.model.is_dirty())
        {
            Ok(()) => self.status = "Gizmo restored".to_owned(),
            Err(error) => self.status = format_editor_error("Gizmo restore failed", error),
        }
    }

    pub(crate) fn handle_viewport_action(&mut self, action: ViewportAction) {
        match action {
            ViewportAction::None => {}
            ViewportAction::Select(entity) => {
                self.model.select(entity);
                self.status = "Selected".to_owned();
            }
            ViewportAction::ClearSelection => {
                self.model.clear_selection();
                self.status = "Selection cleared".to_owned();
            }
            ViewportAction::PreviewTransform { target, transform } => {
                self.preview_viewport_transform(target, transform);
            }
            ViewportAction::CommitTransform {
                target,
                before,
                after,
            } => {
                self.commit_viewport_transform(target, before, after);
            }
            ViewportAction::RestoreTransform { target, transform } => {
                self.restore_viewport_transform(target, transform);
            }
            ViewportAction::Status(status) => {
                self.status = status;
            }
        }
    }

    pub(super) fn run_ui_action(&mut self, action: EditorUiAction) {
        match action {
            EditorUiAction::NewScene => self.new_scene(),
            EditorUiAction::OpenScene => self.open_scene(),
            EditorUiAction::SaveScene => self.save_scene(),
            EditorUiAction::SaveSceneAs => self.save_scene_as(),
            EditorUiAction::DiscardPendingAction => self.discard_pending_action(),
            EditorUiAction::Undo => {
                if self.model.undo().unwrap_or(false) {
                    self.status = "Undone".to_owned();
                }
            }
            EditorUiAction::Redo => {
                if self.model.redo().unwrap_or(false) {
                    self.status = "Redone".to_owned();
                }
            }
            EditorUiAction::CreateCube => {
                self.model.create_cube();
            }
            EditorUiAction::DuplicateSelection => match self.model.duplicate_selected() {
                Ok(_) => self.status = "Duplicated".to_owned(),
                Err(error) => self.status = format_editor_error("Duplicate failed", error),
            },
            EditorUiAction::DeleteSelection => match self.model.delete_selected() {
                Ok(()) => self.status = "Deleted".to_owned(),
                Err(error) => self.status = format_editor_error("Delete failed", error),
            },
            EditorUiAction::SetGizmoMode(mode) => {
                self.transform_gizmo.mode = mode;
            }
            EditorUiAction::FitView => {
                self.fit_view_requested = true;
                self.status = "Fit view requested".to_owned();
            }
            EditorUiAction::TogglePilotCamera => self.toggle_pilot_camera(),
        }
    }

    fn begin_name_edit(&mut self, target: EntityId, before: String) {
        self.name_edit = Some(NameEditSession {
            target,
            buffer: before.clone(),
            before,
        });
    }

    fn update_name_edit(&mut self, buffer: String) {
        if let Some(edit) = &mut self.name_edit {
            edit.buffer = buffer;
        }
    }

    fn finish_name_edit(&mut self, commit: bool) {
        let Some(edit) = self.name_edit.take() else {
            return;
        };
        if !commit {
            return;
        }
        match self.model.rename_entity(&edit.target, &edit.buffer) {
            Ok(()) => self.status = "Name updated".to_owned(),
            Err(error) => self.status = format_editor_error("Rename failed", error),
        }
    }

    fn begin_transform_edit(&mut self, target: EntityId, before: Transform) {
        self.transform_edit = Some(TransformEditSession {
            target,
            before,
            dirty_before: self.model.is_dirty(),
        });
    }

    fn preview_inspector_transform(&mut self, target: EntityId, transform: Transform) {
        if self.transform_edit.is_none() {
            let before = self
                .model
                .world()
                .entity(target.as_str())
                .map_or(Transform::identity(), |entity| entity.transform);
            self.begin_transform_edit(target.clone(), before);
        }
        match self.model.preview_transform(&target, transform) {
            Ok(()) => self.status = "Transform preview".to_owned(),
            Err(error) => self.status = format_editor_error("Edit failed", error),
        }
    }

    fn finish_transform_edit(&mut self, commit: bool) {
        let Some(edit) = self.transform_edit.take() else {
            return;
        };
        let current = self
            .model
            .world()
            .entity(edit.target.as_str())
            .map_or(edit.before, |entity| entity.transform);
        if commit {
            match self
                .model
                .commit_transform_edit(&edit.target, edit.before, current)
            {
                Ok(true) => self.status = "Transform updated".to_owned(),
                Ok(false) => self.status = "Transform unchanged".to_owned(),
                Err(error) => self.status = format_editor_error("Edit failed", error),
            }
        } else if let Err(error) =
            self.model
                .restore_transform_preview(&edit.target, edit.before, edit.dirty_before)
        {
            self.status = format_editor_error("Edit restore failed", error);
        }
    }

    fn begin_material_edit(&mut self, target: EntityId, before: Option<MaterialOverride>) {
        self.material_edit = Some(MaterialEditSession {
            target,
            before,
            dirty_before: self.model.is_dirty(),
        });
    }

    fn preview_material_edit(&mut self, target: EntityId, material: Option<MaterialOverride>) {
        if self.material_edit.is_none() {
            let before = self
                .model
                .world()
                .entity(target.as_str())
                .and_then(|entity| entity.material_override);
            self.begin_material_edit(target.clone(), before);
        }
        match self.model.preview_material_override(&target, material) {
            Ok(()) => self.status = "Material preview".to_owned(),
            Err(error) => self.status = format_editor_error("Material edit failed", error),
        }
    }

    fn finish_material_edit(&mut self, commit: bool) {
        let Some(edit) = self.material_edit.take() else {
            return;
        };
        let current = self
            .model
            .world()
            .entity(edit.target.as_str())
            .and_then(|entity| entity.material_override);
        if commit {
            match self
                .model
                .commit_material_override_edit(&edit.target, edit.before, current)
            {
                Ok(true) => self.status = "Material updated".to_owned(),
                Ok(false) => self.status = "Material unchanged".to_owned(),
                Err(error) => self.status = format_editor_error("Material edit failed", error),
            }
        } else if let Err(error) = self.model.restore_material_override_preview(
            &edit.target,
            edit.before,
            edit.dirty_before,
        ) {
            self.status = format_editor_error("Material restore failed", error);
        }
    }

    fn begin_light_edit(&mut self, target: EntityId, before: Light) {
        self.light_edit = Some(LightEditSession {
            target,
            before,
            dirty_before: self.model.is_dirty(),
        });
    }

    fn preview_light_edit(&mut self, target: EntityId, light: Light) {
        if self.light_edit.is_none()
            && let Some(before) = self
                .model
                .world()
                .entity(target.as_str())
                .and_then(|entity| entity.light.clone())
        {
            self.begin_light_edit(target.clone(), before);
        }
        match self.model.preview_light(&target, light) {
            Ok(()) => self.status = "Light preview".to_owned(),
            Err(error) => self.status = format_editor_error("Light edit failed", error),
        }
    }

    fn finish_light_edit(&mut self, commit: bool) {
        let Some(edit) = self.light_edit.take() else {
            return;
        };
        let current = self
            .model
            .world()
            .entity(edit.target.as_str())
            .and_then(|entity| entity.light.clone())
            .unwrap_or_else(|| edit.before.clone());
        if commit {
            match self
                .model
                .commit_light_edit(&edit.target, edit.before, current)
            {
                Ok(true) => self.status = "Light updated".to_owned(),
                Ok(false) => self.status = "Light unchanged".to_owned(),
                Err(error) => self.status = format_editor_error("Light edit failed", error),
            }
        } else if let Err(error) =
            self.model
                .restore_light_preview(&edit.target, edit.before, edit.dirty_before)
        {
            self.status = format_editor_error("Light restore failed", error);
        }
    }

    fn begin_camera_edit(&mut self, target: EntityId, before: Camera) {
        self.camera_edit = Some(CameraEditSession {
            target,
            before,
            dirty_before: self.model.is_dirty(),
        });
    }

    fn preview_camera_edit(&mut self, target: EntityId, camera: Camera) {
        if self.camera_edit.is_none()
            && let Some(before) = self
                .model
                .world()
                .entity(target.as_str())
                .and_then(|entity| entity.camera.clone())
        {
            self.begin_camera_edit(target.clone(), before);
        }
        match self.model.preview_camera(&target, camera) {
            Ok(()) => self.status = "Camera preview".to_owned(),
            Err(error) => self.status = format_editor_error("Camera edit failed", error),
        }
    }

    fn finish_camera_edit(&mut self, commit: bool) {
        let Some(edit) = self.camera_edit.take() else {
            return;
        };
        let current = self
            .model
            .world()
            .entity(edit.target.as_str())
            .and_then(|entity| entity.camera.clone())
            .unwrap_or_else(|| edit.before.clone());
        if commit {
            match self
                .model
                .commit_camera_edit(&edit.target, edit.before, current)
            {
                Ok(true) => self.status = "Camera updated".to_owned(),
                Ok(false) => self.status = "Camera unchanged".to_owned(),
                Err(error) => self.status = format_editor_error("Camera edit failed", error),
            }
        } else if let Err(error) =
            self.model
                .restore_camera_preview(&edit.target, edit.before, edit.dirty_before)
        {
            self.status = format_editor_error("Camera restore failed", error);
        }
    }

    fn can_pilot_selected_camera(&self) -> bool {
        self.model
            .selected()
            .and_then(|id| self.model.world().entity(id.as_str()))
            .is_some_and(|entity| entity.camera.is_some())
    }

    fn toggle_pilot_camera(&mut self) {
        if self.pilot_camera {
            self.pilot_camera = false;
            self.status = "Pilot camera off".to_owned();
        } else if self.can_pilot_selected_camera() {
            self.pilot_camera = true;
            self.status = "Pilot camera on".to_owned();
        } else {
            self.status = "Select a camera to pilot".to_owned();
        }
    }

    fn sync_pilot_camera_target(&mut self) {
        if self.pilot_camera && !self.can_pilot_selected_camera() {
            self.pilot_camera = false;
            self.status = "Pilot camera off".to_owned();
        }
    }

    fn clear_content_edit_sessions(&mut self) {
        self.material_edit = None;
        self.light_edit = None;
        self.camera_edit = None;
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(path) = self.options.smoke_path.clone() {
            if self.smoke_report.is_none() {
                match self.run_smoke_file_workflow(&path) {
                    Ok(report) => self.smoke_report = Some(report),
                    Err(error) => {
                        eprintln!("editor smoke failed: {error:#}");
                        std::process::exit(1);
                    }
                }
            }

            self.smoke_frame_count = self.smoke_frame_count.saturating_add(1);
            let viewport_report = self.viewport_probe.report();
            if viewport_report.completed {
                let report = self
                    .smoke_report
                    .as_ref()
                    .expect("smoke report is set before viewport completion");
                println!(
                    "editor smoke ok: meshes={}, camera={}, light={}, viewport_indices={}, transform_undo_redo={}, content_reopen={}, history_cleared={}, gizmo_drag_cleared={}, pilot_camera_cleared={}, viewport_prepare={}, viewport_paint={}",
                    report.semantic.mesh_count,
                    report.semantic.has_camera,
                    report.semantic.has_light,
                    report.semantic.viewport_index_count,
                    report.semantic.transform_undo_redo_ok,
                    report.semantic.content_reopen_ok,
                    report.app.history_cleared_after_reopen,
                    report.app.gizmo_drag_cleared_after_reopen,
                    report.app.pilot_camera_cleared_after_reopen,
                    viewport_report.prepare_count,
                    viewport_report.paint_count
                );
                self.options.smoke_path = None;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.smoke_frame_count > SMOKE_MAX_VIEWPORT_FRAMES {
                match self.smoke_report.as_ref() {
                    Some(report) => eprintln!(
                        "editor smoke failed: wgpu viewport path not reached after {} frames: meshes={}, camera={}, light={}, viewport_indices={}, transform_undo_redo={}, content_reopen={}, history_cleared={}, gizmo_drag_cleared={}, pilot_camera_cleared={}, viewport_prepare={}, viewport_paint={}",
                        self.smoke_frame_count,
                        report.semantic.mesh_count,
                        report.semantic.has_camera,
                        report.semantic.has_light,
                        report.semantic.viewport_index_count,
                        report.semantic.transform_undo_redo_ok,
                        report.semantic.content_reopen_ok,
                        report.app.history_cleared_after_reopen,
                        report.app.gizmo_drag_cleared_after_reopen,
                        report.app.pilot_camera_cleared_after_reopen,
                        viewport_report.prepare_count,
                        viewport_report.paint_count
                    ),
                    None => eprintln!("editor smoke failed: model smoke did not produce a report"),
                }
                std::process::exit(1);
            } else if !self.wgpu_viewport_available {
                eprintln!("editor smoke failed: eframe wgpu render state is unavailable");
                std::process::exit(1);
            } else {
                context.request_repaint();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("editor_toolbar").show(ui, |ui| {
            self.draw_top_toolbar(ui);
        });
        egui::Panel::bottom("editor_status_bar").show(ui, |ui| {
            self.draw_status_bar(ui);
        });
        self.draw_editor_body(ui);
    }
}

fn format_editor_error(action: &str, error: EditorError) -> String {
    format!("{action}: {error}")
}

#[cfg(test)]
mod tests;

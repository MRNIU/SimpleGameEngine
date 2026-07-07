// Copyright The SimpleGameEngine Contributors

use std::path::PathBuf;

use ecs::EntityId;
use eframe::egui;
use math::Transform;

use crate::{
    model::{EditorError, EditorModel, EditorSmokeReport},
    viewport::{TransformGizmoState, ViewCamera, ViewportWgpuProbe, install_viewport_renderer},
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
    smoke_report: Option<EditorSmokeReport>,
    smoke_frame_count: u32,
    viewport_probe: ViewportWgpuProbe,
    wgpu_viewport_available: bool,
    viewport_camera: ViewCamera,
    transform_gizmo: TransformGizmoState,
    name_edit: Option<NameEditSession>,
    transform_edit: Option<TransformEditSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingFileAction {
    New,
    Open(PathBuf),
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
                    "editor smoke ok: meshes={}, camera={}, viewport_indices={}, viewport_prepare={}, viewport_paint={}",
                    report.mesh_count,
                    report.has_camera,
                    report.viewport_index_count,
                    viewport_report.prepare_count,
                    viewport_report.paint_count
                );
                self.options.smoke_path = None;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.smoke_frame_count > SMOKE_MAX_VIEWPORT_FRAMES {
                match self.smoke_report.as_ref() {
                    Some(report) => eprintln!(
                        "editor smoke failed: wgpu viewport path not reached after {} frames: meshes={}, camera={}, viewport_indices={}, viewport_prepare={}, viewport_paint={}",
                        self.smoke_frame_count,
                        report.mesh_count,
                        report.has_camera,
                        report.viewport_index_count,
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
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_editor_body(ui);
        });
    }
}

fn format_editor_error(action: &str, error: EditorError) -> String {
    format!("{action}: {error}")
}

#[cfg(test)]
mod tests;

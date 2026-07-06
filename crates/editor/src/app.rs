// Copyright The SimpleGameEngine Contributors

use std::{
    fs,
    path::{Path, PathBuf},
};

use ecs::EntityId;
use eframe::egui;

use crate::{
    model::{EditorModel, EditorSmokeReport},
    viewport::{ViewportWgpuProbe, draw_viewport, install_viewport_renderer},
};

const DEFAULT_SCENE_PATH: &str = "target/tmp/editor_manual.scene.ron";
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
    status: String,
    options: EditorLaunchOptions,
    smoke_report: Option<EditorSmokeReport>,
    smoke_frame_count: u32,
    viewport_probe: ViewportWgpuProbe,
    wgpu_viewport_available: bool,
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
        let wgpu_viewport_available = install_viewport_renderer(creation_context);
        Self {
            options,
            wgpu_viewport_available,
            ..Self::default()
        }
    }

    fn save_default_scene(&mut self) {
        match self.save_scene_to_path(Path::new(DEFAULT_SCENE_PATH)) {
            Ok(()) => self.status = format!("Saved {DEFAULT_SCENE_PATH}"),
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn reopen_default_scene(&mut self) {
        match fs::read_to_string(DEFAULT_SCENE_PATH)
            .map_err(anyhow::Error::from)
            .and_then(|input| Self::model_from_scene(&input))
        {
            Ok(model) => {
                self.model = model;
                self.status = format!("Opened {DEFAULT_SCENE_PATH}");
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn save_scene_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.model.save_scene_to_string()?)?;
        Ok(())
    }

    fn model_from_scene(input: &str) -> anyhow::Result<EditorModel> {
        Ok(EditorModel::from_scene_str(input)?)
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(path) = self.options.smoke_path.clone() {
            if self.smoke_report.is_none() {
                match self.model.run_smoke_actions_in_place(&path) {
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
        ui.horizontal(|ui| {
            if ui.button("New Cube").clicked() {
                self.model.create_cube();
            }
            if ui.button("Save").clicked() {
                self.save_default_scene();
            }
            if ui.button("Reopen").clicked() {
                self.reopen_default_scene();
            }
            ui.label(&self.status);
        });

        ui.separator();
        ui.columns(3, |columns| {
            draw_hierarchy(&mut columns[0], &mut self.model);
            draw_inspector(&mut columns[1], &mut self.model, &mut self.status);
            let draw = self.model.viewport_draw_call();
            let wgpu_probe = self.wgpu_viewport_available.then_some(&self.viewport_probe);
            draw_viewport(&mut columns[2], draw.as_ref(), wgpu_probe);
        });
    }
}

fn draw_hierarchy(ui: &mut egui::Ui, model: &mut EditorModel) {
    ui.heading("Hierarchy");
    let rows: Vec<(EntityId, String)> = model
        .world()
        .entities()
        .map(|entity| (entity.id.clone(), entity.name.clone()))
        .collect();
    for (id, name) in rows {
        let selected = model.selected().is_some_and(|selected| selected == &id);
        if ui.selectable_label(selected, name).clicked() {
            model.select(id);
        }
    }
}

fn draw_inspector(ui: &mut egui::Ui, model: &mut EditorModel, status: &mut String) {
    ui.heading("Inspector");
    if let Some(selected) = model.selected().cloned()
        && let Some(entity) = model.world().entity(selected.as_str())
    {
        ui.label(entity.name.clone());
        let mut translation = entity.transform.translation;
        let changed = ui
            .horizontal(|ui| {
                ui.label("T");
                ui.add(egui::DragValue::new(&mut translation[0]).speed(0.1))
                    .changed()
                    | ui.add(egui::DragValue::new(&mut translation[1]).speed(0.1))
                        .changed()
                    | ui.add(egui::DragValue::new(&mut translation[2]).speed(0.1))
                        .changed()
            })
            .inner;
        if changed {
            match model.set_translation(&selected, translation) {
                Ok(()) => *status = "Transform updated".to_owned(),
                Err(error) => *status = format!("Edit failed: {error}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SCENE_PATH, EditorLaunchOptions};
    use std::path::PathBuf;

    #[test]
    fn parses_smoke_argument() {
        let options = EditorLaunchOptions::from_args([
            "editor".to_owned(),
            "--smoke".to_owned(),
            "target/tmp/smoke.scene.ron".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            options.smoke_path,
            Some(PathBuf::from("target/tmp/smoke.scene.ron"))
        );
    }

    #[test]
    fn manual_save_path_stays_out_of_tracked_assets() {
        assert_eq!(DEFAULT_SCENE_PATH, "target/tmp/editor_manual.scene.ron");
    }
}

// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf, sync::Arc};

use ecs::EntityId;
use eframe::egui;
use math::Transform;

use crate::{
    model::{EditorError, EditorModel, EditorSmokeReport},
    viewport::{
        TransformGizmoState, ViewCamera, ViewportAction, ViewportWgpuProbe, draw_viewport,
        install_viewport_renderer,
    },
};

mod file_workflow;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingFileAction {
    New,
    Open(PathBuf),
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

    fn apply_viewport_transform(&mut self, target: EntityId, transform: Transform, status: &str) {
        let selected_matches = self
            .model
            .selected()
            .is_some_and(|selected| selected == &target);
        let target_exists = self.model.world().entity(target.as_str()).is_some();
        if !selected_matches || !target_exists {
            self.transform_gizmo.clear_drag();
            self.status = "Gizmo target changed".to_owned();
            return;
        }

        match self.model.set_transform(&target, transform) {
            Ok(()) => self.status = status.to_owned(),
            Err(error) => {
                self.transform_gizmo.clear_drag();
                self.status = format_editor_error("Gizmo failed", error);
            }
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
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.new_scene();
            }
            if ui.button("Open").clicked() {
                self.open_scene();
            }
            if ui.button("Save").clicked() {
                self.save_scene();
            }
            if ui.button("Save As").clicked() {
                self.save_scene_as();
            }
            if ui.button("Discard").clicked() {
                self.discard_pending_action();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Path");
            ui.add(
                egui::TextEdit::singleline(&mut self.path_input)
                    .desired_width(ui.available_width()),
            );
        });

        ui.horizontal(|ui| {
            if ui.button("New Cube").clicked() {
                self.model.create_cube();
            }
            if ui.button("Duplicate").clicked() {
                match self.model.duplicate_selected() {
                    Ok(_) => self.status = "Duplicated".to_owned(),
                    Err(error) => self.status = format_editor_error("Duplicate failed", error),
                }
            }
            if ui.button("Delete").clicked() {
                match self.model.delete_selected() {
                    Ok(()) => self.status = "Deleted".to_owned(),
                    Err(error) => self.status = format_editor_error("Delete failed", error),
                }
            }
            if self.model.is_dirty() {
                ui.label("Unsaved");
            }
        });
        if !self.status.is_empty() {
            ui.add(egui::Label::new(&self.status).wrap());
        }

        ui.separator();
        ui.columns(3, |columns| {
            draw_hierarchy(&mut columns[0], &mut self.model);
            draw_inspector(&mut columns[1], &mut self.model, &mut self.status);
            let view = self.viewport_camera.to_viewport_view();
            let draw = self.model.viewport_draw_call_for_view(&view);
            let selected = self.model.selected().cloned();
            let selected_transform = selected
                .as_ref()
                .and_then(|id| self.model.world().entity(id.as_str()))
                .map(|entity| entity.transform);
            let wgpu_probe = self.wgpu_viewport_available.then_some(&self.viewport_probe);
            match draw_viewport(
                &mut columns[2],
                draw.as_ref(),
                selected.as_ref(),
                selected_transform,
                &mut self.viewport_camera,
                &mut self.transform_gizmo,
                wgpu_probe,
            ) {
                ViewportAction::None => {}
                ViewportAction::Select(entity) => {
                    self.model.select(entity);
                    self.status = "Selected".to_owned();
                }
                ViewportAction::ClearSelection => {
                    self.model.clear_selection();
                    self.status = "Selection cleared".to_owned();
                }
                ViewportAction::ApplyTransform { target, transform } => {
                    self.apply_viewport_transform(target, transform, "Gizmo updated");
                }
                ViewportAction::RestoreTransform { target, transform } => {
                    self.apply_viewport_transform(target, transform, "Gizmo restored");
                }
                ViewportAction::Status(status) => {
                    self.status = status;
                }
            }
        });
    }
}

fn draw_hierarchy(ui: &mut egui::Ui, model: &mut EditorModel) {
    ui.heading("Hierarchy");
    let roots: Vec<EntityId> = model
        .world()
        .entities()
        .filter(|entity| entity.parent.is_none())
        .map(|entity| entity.id.clone())
        .collect();
    for id in roots {
        draw_hierarchy_node(ui, model, &id);
    }
}

fn draw_hierarchy_node(ui: &mut egui::Ui, model: &mut EditorModel, id: &EntityId) {
    let Some(entity) = model.world().entity(id.as_str()).cloned() else {
        return;
    };
    let children = model.world().children_of(id.as_str());
    let selected = model.selected().is_some_and(|selected| selected == id);

    if children.is_empty() {
        if ui.selectable_label(selected, entity.name).clicked() {
            model.select(id.clone());
        }
        return;
    }

    let response = egui::CollapsingHeader::new(entity.name)
        .id_salt(id.as_str())
        .default_open(true)
        .show(ui, |ui| {
            for child in children {
                draw_hierarchy_node(ui, model, &child);
            }
        });
    if response.header_response.clicked() {
        model.select(id.clone());
    }
}

fn draw_inspector(ui: &mut egui::Ui, model: &mut EditorModel, status: &mut String) {
    ui.heading("Inspector");
    if let Some(selected) = model.selected().cloned()
        && let Some(entity) = model.world().entity(selected.as_str()).cloned()
    {
        let mut name = entity.name.clone();
        ui.horizontal(|ui| {
            ui.label("Name");
            if ui.text_edit_singleline(&mut name).changed() {
                match model.rename_entity(&selected, &name) {
                    Ok(()) => *status = "Name updated".to_owned(),
                    Err(error) => *status = format_editor_error("Rename failed", error),
                }
            }
        });

        let mut transform = entity.transform;
        let translation_changed = draw_vec3(ui, "Translation", &mut transform.translation);
        let rotation_changed = draw_vec4(ui, "Rotation", &mut transform.rotation);
        let fields = inspector_transform_fields(&entity);
        let scale_changed = fields.show_scale && draw_vec3(ui, "Scale", &mut transform.scale);
        let transform_changed = translation_changed || rotation_changed || scale_changed;
        if transform_changed {
            match model.set_transform(&selected, transform) {
                Ok(()) => *status = "Transform updated".to_owned(),
                Err(error) => *status = format_editor_error("Edit failed", error),
            }
        }

        if let Some(mesh) = &entity.mesh {
            ui.label(format!("Mesh: {}", mesh.asset));
            ui.label(format!("Material: {}", mesh.material));
        }
        if let Some(camera) = &entity.camera {
            ui.label(format!("Camera: {:?}", camera.projection));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InspectorTransformFields {
    show_translation: bool,
    show_rotation: bool,
    show_scale: bool,
}

fn inspector_transform_fields(entity: &ecs::EntityRecord) -> InspectorTransformFields {
    InspectorTransformFields {
        show_translation: true,
        show_rotation: true,
        show_scale: entity.camera.is_none(),
    }
}

fn draw_vec3(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3]) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let x_changed = ui
            .add(egui::DragValue::new(&mut values[0]).speed(0.1))
            .changed();
        let y_changed = ui
            .add(egui::DragValue::new(&mut values[1]).speed(0.1))
            .changed();
        let z_changed = ui
            .add(egui::DragValue::new(&mut values[2]).speed(0.1))
            .changed();
        x_changed || y_changed || z_changed
    })
    .inner
}

fn draw_vec4(ui: &mut egui::Ui, label: &str, values: &mut [f32; 4]) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let x_changed = ui
            .add(egui::DragValue::new(&mut values[0]).speed(0.1))
            .changed();
        let y_changed = ui
            .add(egui::DragValue::new(&mut values[1]).speed(0.1))
            .changed();
        let z_changed = ui
            .add(egui::DragValue::new(&mut values[2]).speed(0.1))
            .changed();
        let w_changed = ui
            .add(egui::DragValue::new(&mut values[3]).speed(0.1))
            .changed();
        x_changed || y_changed || z_changed || w_changed
    })
    .inner
}

fn format_editor_error(action: &str, error: EditorError) -> String {
    format!("{action}: {error}")
}

fn install_cjk_font(context: &egui::Context) {
    let Some(font_bytes) = cjk_font_candidates()
        .iter()
        .find_map(|candidate| fs::read(candidate).ok())
    else {
        return;
    };

    let font_name = "sge_system_cjk".to_owned();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(egui::FontData::from_owned(font_bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, font_name.clone());
    }
    context.set_fonts(fonts);
}

fn cjk_font_candidates() -> &'static [&'static str] {
    &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.otf",
        "/usr/share/fonts/opentype/source-han-sans/SourceHanSans-Regular.otf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
    ]
}

#[cfg(test)]
mod tests {
    use super::{EditorLaunchOptions, cjk_font_candidates, inspector_transform_fields};
    use ecs::{Camera, EntityId, EntityRecord, Projection};
    use math::Transform;
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
    fn camera_inspector_hides_scale_field() {
        let mut camera =
            EntityRecord::new(EntityId::new("camera"), "Camera", Transform::identity());
        camera.camera = Some(Camera::new(Projection::Perspective {
            fov_y_degrees: 60.0,
        }));
        let cube = EntityRecord::new(EntityId::new("cube"), "Cube", Transform::identity());

        assert!(inspector_transform_fields(&camera).show_translation);
        assert!(inspector_transform_fields(&camera).show_rotation);
        assert!(!inspector_transform_fields(&camera).show_scale);
        assert!(inspector_transform_fields(&cube).show_scale);
    }

    #[test]
    fn viewport_transform_action_writes_current_selected_target() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.mark_saved();

        app.apply_viewport_transform(
            cube.clone(),
            Transform::from_translation([3.0, 0.0, 0.0]),
            "Gizmo updated",
        );

        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [3.0, 0.0, 0.0]
        );
        assert!(app.model.is_dirty());
        assert_eq!(app.status, "Gizmo updated");
    }

    #[test]
    fn viewport_transform_action_drops_stale_target() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.select(EntityId::new("root"));
        app.transform_gizmo.start_drag(crate::viewport::GizmoDrag {
            target: cube.clone(),
            handle: crate::viewport::GizmoHandle::MoveX,
            start_pointer: egui::pos2(0.0, 0.0),
            start_transform: Transform::identity(),
        });

        app.apply_viewport_transform(
            cube.clone(),
            Transform::from_translation([3.0, 0.0, 0.0]),
            "Gizmo updated",
        );

        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [0.0, 0.0, 0.0]
        );
        assert_eq!(app.transform_gizmo.drag(), None);
        assert_eq!(app.status, "Gizmo target changed");
    }

    #[test]
    fn scene_replace_clears_active_gizmo_drag() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.transform_gizmo.start_drag(crate::viewport::GizmoDrag {
            target: cube,
            handle: crate::viewport::GizmoHandle::MoveX,
            start_pointer: egui::pos2(0.0, 0.0),
            start_transform: Transform::identity(),
        });

        app.replace_with_new_scene();

        assert_eq!(app.transform_gizmo.drag(), None);
    }

    #[test]
    fn cjk_font_candidates_cover_common_desktop_fonts() {
        let candidates = cjk_font_candidates();

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.ends_with("PingFang.ttc"))
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.contains("NotoSansCJK"))
        );
    }
}

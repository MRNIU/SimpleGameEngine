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
        self.draw_top_toolbar(ui);
        ui.separator();
        self.draw_editor_body(ui);
        ui.separator();
        self.draw_status_bar(ui);
    }
}

impl EditorApp {
    fn draw_top_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
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
            if ui
                .add_enabled(self.pending_action.is_some(), egui::Button::new("Discard"))
                .clicked()
            {
                self.discard_pending_action();
            }
            ui.separator();
            ui.label("Path");
            ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(260.0));
            ui.separator();
            if ui
                .add_enabled(self.model.can_undo(), egui::Button::new("Undo"))
                .clicked()
                && self.model.undo().unwrap_or(false)
            {
                self.status = "Undone".to_owned();
            }
            if ui
                .add_enabled(self.model.can_redo(), egui::Button::new("Redo"))
                .clicked()
                && self.model.redo().unwrap_or(false)
            {
                self.status = "Redone".to_owned();
            }
            ui.separator();
            if ui.button("New Cube").clicked() {
                self.model.create_cube();
            }
            let has_selection = self.model.selected().is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("Duplicate"))
                .clicked()
            {
                match self.model.duplicate_selected() {
                    Ok(_) => self.status = "Duplicated".to_owned(),
                    Err(error) => self.status = format_editor_error("Duplicate failed", error),
                }
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                match self.model.delete_selected() {
                    Ok(()) => self.status = "Deleted".to_owned(),
                    Err(error) => self.status = format_editor_error("Delete failed", error),
                }
            }
            ui.separator();
            ui.selectable_value(
                &mut self.transform_gizmo.mode,
                crate::viewport::GizmoMode::Move,
                "Move",
            );
            ui.selectable_value(
                &mut self.transform_gizmo.mode,
                crate::viewport::GizmoMode::Scale,
                "Scale",
            );
            if self.model.is_dirty() {
                ui.label("Unsaved");
            }
        });
    }

    fn draw_editor_body(&mut self, ui: &mut egui::Ui) {
        ui.columns(3, |columns| {
            draw_hierarchy(&mut columns[0], &mut self.model);
            self.draw_inspector_panel(&mut columns[1]);
            self.draw_viewport_column(&mut columns[2]);
        });
    }

    fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        let path = self
            .current_path
            .as_ref()
            .map_or_else(|| "No file".to_owned(), |path| path.display().to_string());
        let selection = self
            .model
            .selected()
            .map_or_else(|| "No selection".to_owned(), ToString::to_string);
        ui.horizontal_wrapped(|ui| {
            ui.label(path);
            ui.separator();
            ui.label(selection);
            ui.separator();
            ui.label(format!("{:?}", self.transform_gizmo.mode));
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }

    fn draw_viewport_column(&mut self, ui: &mut egui::Ui) {
        let view = self.viewport_camera.to_viewport_view();
        let draw = self.model.viewport_draw_call_for_view(&view);
        let selected = self.model.selected().cloned();
        let selected_transform = selected
            .as_ref()
            .and_then(|id| self.model.world().entity(id.as_str()))
            .map(|entity| entity.transform);
        let wgpu_probe = self.wgpu_viewport_available.then_some(&self.viewport_probe);
        match draw_viewport(
            ui,
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

    fn draw_inspector_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        let Some(selected) = self.model.selected().cloned() else {
            return;
        };
        let Some(entity) = self.model.world().entity(selected.as_str()).cloned() else {
            return;
        };

        let mut name = self
            .name_edit
            .as_ref()
            .filter(|edit| edit.target == selected)
            .map_or_else(|| entity.name.clone(), |edit| edit.buffer.clone());
        ui.horizontal(|ui| {
            ui.label("Name");
            let response = ui.text_edit_singleline(&mut name);
            if response.gained_focus() {
                self.begin_name_edit(selected.clone(), entity.name.clone());
            }
            if response.changed() {
                self.update_name_edit(name);
            }
            if response.lost_focus() || ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                self.finish_name_edit(true);
            }
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.finish_name_edit(false);
            }
        });

        let mut transform = entity.transform;
        let translation_changed = draw_vec3(ui, "Translation", &mut transform.translation);
        let rotation_changed = draw_vec4(ui, "Rotation", &mut transform.rotation);
        let fields = inspector_transform_fields(&entity);
        let scale_changed = fields.show_scale && draw_vec3(ui, "Scale", &mut transform.scale);
        let transform_changed = translation_changed || rotation_changed || scale_changed;
        if transform_changed {
            if self.transform_edit.is_none() {
                self.begin_transform_edit(selected.clone(), entity.transform);
            }
            self.preview_inspector_transform(selected.clone(), transform);
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.finish_transform_edit(false);
        } else if self.transform_edit.is_some() && !ui.input(|input| input.pointer.primary_down()) {
            self.finish_transform_edit(true);
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
    fn viewport_preview_transform_does_not_write_history_or_dirty() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.mark_saved();
        app.model.clear_history();

        app.preview_viewport_transform(cube.clone(), Transform::from_translation([3.0, 0.0, 0.0]));

        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [3.0, 0.0, 0.0]
        );
        assert!(!app.model.is_dirty());
        assert!(!app.model.can_undo());
    }

    #[test]
    fn viewport_commit_transform_writes_one_history_entry() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.mark_saved();
        app.model.clear_history();
        let before = Transform::identity();
        let after = Transform::from_translation([3.0, 0.0, 0.0]);

        app.preview_viewport_transform(cube.clone(), after);
        app.commit_viewport_transform(cube.clone(), before, after);

        assert!(app.model.is_dirty());
        assert!(app.model.can_undo());
        app.model.undo().unwrap();
        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [0.0, 0.0, 0.0]
        );
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

        app.preview_viewport_transform(cube.clone(), Transform::from_translation([3.0, 0.0, 0.0]));

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
    fn name_edit_session_commits_one_history_entry() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.mark_saved();
        app.model.clear_history();

        app.begin_name_edit(cube.clone(), "Cube".to_owned());
        app.update_name_edit("Cube A".to_owned());
        app.update_name_edit("Cube B".to_owned());
        assert!(!app.model.can_undo());

        app.finish_name_edit(true);

        assert_eq!(app.model.world().entity(&cube).unwrap().name, "Cube B");
        assert!(app.model.can_undo());
        app.model.undo().unwrap();
        assert_eq!(app.model.world().entity(&cube).unwrap().name, "Cube");
    }

    #[test]
    fn transform_edit_session_previews_then_commits_one_history_entry() {
        let mut app = super::EditorApp::default();
        let cube = app.model.create_cube();
        app.model.mark_saved();
        app.model.clear_history();
        let before = app.model.world().entity(&cube).unwrap().transform;

        app.begin_transform_edit(cube.clone(), before);
        app.preview_inspector_transform(cube.clone(), Transform::from_translation([1.0, 0.0, 0.0]));
        app.preview_inspector_transform(cube.clone(), Transform::from_translation([2.0, 0.0, 0.0]));
        assert!(!app.model.is_dirty());
        assert!(!app.model.can_undo());

        app.finish_transform_edit(true);

        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [2.0, 0.0, 0.0]
        );
        assert!(app.model.can_undo());
        app.model.undo().unwrap();
        assert_eq!(
            app.model
                .world()
                .entity(&cube)
                .unwrap()
                .transform
                .translation,
            [0.0, 0.0, 0.0]
        );
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

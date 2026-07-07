// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;

use crate::{
    model::EditorModel,
    viewport::{self, ViewportAction, draw_viewport},
};

use super::{EditorApp, format_editor_error};

type SidePanel = egui::Panel;

impl EditorApp {
    pub(super) fn draw_top_toolbar(&mut self, ui: &mut egui::Ui) {
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
                viewport::GizmoMode::Move,
                "Move",
            );
            ui.selectable_value(
                &mut self.transform_gizmo.mode,
                viewport::GizmoMode::Scale,
                "Scale",
            );
            ui.separator();
            if ui
                .add_enabled(
                    self.pilot_camera || self.can_pilot_selected_camera(),
                    egui::Button::new(if self.pilot_camera {
                        "Pilot Camera: On"
                    } else {
                        "Pilot Camera"
                    }),
                )
                .clicked()
            {
                self.toggle_pilot_camera();
            }
            if self.model.is_dirty() {
                ui.label("Unsaved");
            }
        });
    }

    pub(super) fn draw_editor_body(&mut self, ui: &mut egui::Ui) {
        SidePanel::left("hierarchy_panel")
            .resizable(false)
            .default_size(240.0)
            .size_range(220.0..=260.0)
            .show(ui, |ui| draw_hierarchy(ui, &mut self.model));

        SidePanel::right("inspector_panel")
            .resizable(false)
            .default_size(330.0)
            .size_range(300.0..=360.0)
            .show(ui, |ui| self.draw_inspector_panel(ui));

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_viewport_column(ui);
        });
    }

    pub(super) fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        let path = self
            .current_path
            .as_ref()
            .map_or_else(|| "No file".to_owned(), |path| path.display().to_string());
        let selection = status_bar_selection_text(&self.model);
        ui.horizontal_wrapped(|ui| {
            ui.label(path);
            ui.separator();
            ui.label(selection);
            ui.separator();
            ui.label(format!("{:?}", self.transform_gizmo.mode));
            ui.separator();
            ui.label(if self.pilot_camera {
                "Pilot"
            } else {
                "Editor camera"
            });
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }

    fn draw_viewport_column(&mut self, ui: &mut egui::Ui) {
        self.sync_pilot_camera_target();
        let piloted_view = self
            .pilot_camera
            .then(|| self.model.selected_camera_view())
            .flatten();
        let editor_view = self.viewport_camera.to_viewport_view();
        let view = piloted_view.as_ref().unwrap_or(&editor_view);
        let draw = self.model.viewport_draw_call_for_view(view);
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
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.finish_name_edit(false);
            } else if response.lost_focus() || ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                self.finish_name_edit(true);
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
            let mut material = entity.material_override.unwrap_or(ecs::MaterialOverride {
                base_color: [0.3, 0.64, 1.0, 1.0],
            });
            if ui
                .color_edit_button_rgba_unmultiplied(&mut material.base_color)
                .changed()
            {
                self.preview_material_edit(selected.clone(), Some(material));
            }
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.finish_material_edit(false);
            } else if self.material_edit.is_some()
                && !ui.input(|input| input.pointer.primary_down())
            {
                self.finish_material_edit(true);
            }
        }
        if let Some(light) = &entity.light {
            let mut edited = light.clone();
            ui.horizontal(|ui| {
                ui.label("Light Kind");
                ui.selectable_value(&mut edited.kind, ecs::LightKind::Directional, "Directional");
                ui.selectable_value(&mut edited.kind, ecs::LightKind::Point, "Point");
            });
            let color_changed = ui.color_edit_button_rgb(&mut edited.color).changed();
            let intensity_changed = ui
                .add(
                    egui::DragValue::new(&mut edited.intensity)
                        .speed(0.1)
                        .range(0.0..=100.0),
                )
                .changed();
            if color_changed || intensity_changed || edited.kind != light.kind {
                self.preview_light_edit(selected.clone(), edited);
            }
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.finish_light_edit(false);
            } else if self.light_edit.is_some() && !ui.input(|input| input.pointer.primary_down()) {
                self.finish_light_edit(true);
            }
        }
        if let Some(camera) = &entity.camera {
            let mut edited = camera.clone();
            let changed = match &mut edited.projection {
                ecs::Projection::Perspective { fov_y_degrees } => {
                    ui.label("Projection: Perspective");
                    ui.add(
                        egui::DragValue::new(fov_y_degrees)
                            .speed(1.0)
                            .range(1.0..=179.0),
                    )
                    .changed()
                }
                ecs::Projection::Orthographic { vertical_size } => {
                    ui.label("Projection: Orthographic");
                    ui.add(
                        egui::DragValue::new(vertical_size)
                            .speed(0.1)
                            .range(0.01..=100.0),
                    )
                    .changed()
                }
            };
            if changed {
                self.preview_camera_edit(selected.clone(), edited);
            }
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.finish_camera_edit(false);
            } else if self.camera_edit.is_some() && !ui.input(|input| input.pointer.primary_down())
            {
                self.finish_camera_edit(true);
            }
        }
    }
}

pub(super) fn status_bar_selection_text(model: &EditorModel) -> String {
    let Some(selected) = model.selected() else {
        return "No selection".to_owned();
    };
    model
        .world()
        .entity(selected.as_str())
        .map_or_else(|| selected.to_string(), |entity| entity.name.clone())
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
pub(super) struct InspectorTransformFields {
    pub(super) show_translation: bool,
    pub(super) show_rotation: bool,
    pub(super) show_scale: bool,
}

pub(super) fn inspector_transform_fields(entity: &ecs::EntityRecord) -> InspectorTransformFields {
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

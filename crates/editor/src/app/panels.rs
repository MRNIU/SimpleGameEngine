// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;

use crate::{
    model::EditorModel,
    viewport::{self, draw_viewport},
};

use super::{EditorApp, EditorUiAction};

type SidePanel = egui::Panel;

impl EditorApp {
    pub(super) fn draw_menu_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    self.run_ui_action(EditorUiAction::NewScene);
                    ui.close();
                }
                if ui.button("Open Scene").clicked() {
                    self.run_ui_action(EditorUiAction::OpenScene);
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    self.run_ui_action(EditorUiAction::SaveScene);
                    ui.close();
                }
                if ui.button("Save As").clicked() {
                    self.run_ui_action(EditorUiAction::SaveSceneAs);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(self.model.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::Undo);
                    ui.close();
                }
                if ui
                    .add_enabled(self.model.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::Redo);
                    ui.close();
                }
                let has_selection = self.model.selected().is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Duplicate"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::DuplicateSelection);
                    ui.close();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Delete"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::DeleteSelection);
                    ui.close();
                }
            });
            ui.menu_button("Create", |ui| {
                if ui.button("Cube").clicked() {
                    self.run_ui_action(EditorUiAction::CreateCube);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Fit View").clicked() {
                    self.run_ui_action(EditorUiAction::FitView);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.pilot_camera || self.can_pilot_selected_camera(),
                        egui::Button::new("Pilot Camera"),
                    )
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::TogglePilotCamera);
                    ui.close();
                }
            });
        });
    }

    pub(super) fn draw_top_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("File");
            if ui.button("New").clicked() {
                self.run_ui_action(EditorUiAction::NewScene);
            }
            if ui.button("Open").clicked() {
                self.run_ui_action(EditorUiAction::OpenScene);
            }
            if ui.button("Save").clicked() {
                self.run_ui_action(EditorUiAction::SaveScene);
            }
            if ui.button("Save As").clicked() {
                self.run_ui_action(EditorUiAction::SaveSceneAs);
            }
            ui.separator();

            ui.label("Edit");
            if ui
                .add_enabled(self.model.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::Undo);
            }
            if ui
                .add_enabled(self.model.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::Redo);
            }
            ui.separator();

            ui.label("Create");
            if ui.button("Cube").clicked() {
                self.run_ui_action(EditorUiAction::CreateCube);
            }
            let has_selection = self.model.selected().is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("Duplicate"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::DuplicateSelection);
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::DeleteSelection);
            }
            ui.separator();

            ui.label("Transform");
            if ui
                .selectable_label(
                    self.transform_gizmo.mode == viewport::GizmoMode::Move,
                    "Move",
                )
                .clicked()
            {
                self.run_ui_action(EditorUiAction::SetGizmoMode(viewport::GizmoMode::Move));
            }
            if ui
                .selectable_label(
                    self.transform_gizmo.mode == viewport::GizmoMode::Scale,
                    "Scale",
                )
                .clicked()
            {
                self.run_ui_action(EditorUiAction::SetGizmoMode(viewport::GizmoMode::Scale));
            }
            ui.separator();

            ui.label("View");
            if ui.button("Fit").clicked() {
                self.run_ui_action(EditorUiAction::FitView);
            }
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
                self.run_ui_action(EditorUiAction::TogglePilotCamera);
            }
            ui.separator();

            ui.label("State");
            if self.model.is_dirty() {
                ui.label("Unsaved");
                if ui
                    .add_enabled(self.pending_action.is_some(), egui::Button::new("Discard"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::DiscardPendingAction);
                }
            } else {
                ui.label("Saved");
            }
        });
    }

    pub(super) fn draw_editor_body(&mut self, ui: &mut egui::Ui) {
        SidePanel::left("hierarchy_panel")
            .resizable(true)
            .default_size(240.0)
            .size_range(220.0..=320.0)
            .show(ui, |ui| draw_hierarchy(ui, &mut self.model));

        SidePanel::right("inspector_panel")
            .resizable(true)
            .default_size(340.0)
            .size_range(300.0..=460.0)
            .show(ui, |ui| self.draw_inspector_panel(ui));

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_viewport_column(ui);
        });
    }

    pub(super) fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        let selection = status_bar_selection_text(&self.model);
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(360.0));
            if let Some(path) = &self.current_path {
                ui.label(path.display().to_string());
            } else {
                ui.label("No file");
            }
            ui.separator();
            ui.label(if self.model.is_dirty() {
                "Unsaved"
            } else {
                "Saved"
            });
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
        let keyboard_shortcuts_allowed = Self::keyboard_shortcuts_allowed(ui.ctx());
        let fit_view_requested = self.fit_view_requested;
        self.fit_view_requested = false;
        let view_mode_label = if self.pilot_camera {
            viewport::PILOT_CAMERA_LABEL
        } else {
            viewport::EDITOR_CAMERA_LABEL
        };
        let action = draw_viewport(
            ui,
            draw.as_ref(),
            selected.as_ref(),
            selected_transform,
            &mut self.viewport_camera,
            &mut self.transform_gizmo,
            viewport::ViewportUiOptions {
                keyboard_shortcuts_allowed,
                fit_view_requested,
                view_mode_label,
                wgpu_probe,
            },
        );
        self.handle_viewport_action(action);
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
            let color_changed = ui
                .horizontal(|ui| {
                    ui.label("Color");
                    ui.color_edit_button_rgb(&mut edited.color).changed()
                })
                .inner;
            let intensity_changed = ui
                .horizontal(|ui| {
                    ui.label("Intensity");
                    ui.add(
                        egui::DragValue::new(&mut edited.intensity)
                            .speed(0.1)
                            .range(0.0..=100.0),
                    )
                    .changed()
                })
                .inner;
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

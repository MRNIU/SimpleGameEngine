// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::Transform;

use crate::{
    model::{EditorModel, PrimitiveKind},
    viewport::{self, draw_viewport},
};

use super::{EditorApp, EditorUiAction};

type SidePanel = egui::Panel;

const VIEWPORT_MIN_WIDTH: f32 = 360.0;
const HIERARCHY_MIN_WIDTH: f32 = 160.0;
const INSPECTOR_MIN_WIDTH: f32 = 240.0;
const HIERARCHY_DEFAULT_RATIO: f32 = 0.20;
const INSPECTOR_DEFAULT_RATIO: f32 = 0.27;
const SIDE_PANEL_MAX_RATIO: f32 = 0.45;

#[derive(Clone, Copy, Debug)]
pub(super) struct SidePanelLayout {
    pub(super) hierarchy_default: f32,
    pub(super) hierarchy_min: f32,
    pub(super) hierarchy_max: f32,
    pub(super) inspector_default: f32,
    pub(super) inspector_min: f32,
    pub(super) inspector_max: f32,
    pub(super) viewport_min: f32,
}

pub(super) fn side_panel_layout(available_width: f32) -> SidePanelLayout {
    let width = available_width.max(0.0);
    let side_budget = (width - VIEWPORT_MIN_WIDTH).max(0.0);
    let inspector_min = INSPECTOR_MIN_WIDTH.min(side_budget * 0.6);
    let hierarchy_min = HIERARCHY_MIN_WIDTH.min((side_budget - inspector_min).max(0.0));
    let hierarchy_max = (width * SIDE_PANEL_MAX_RATIO)
        .min((width - VIEWPORT_MIN_WIDTH - inspector_min).max(0.0))
        .max(hierarchy_min);
    let inspector_max = (width * SIDE_PANEL_MAX_RATIO)
        .min((width - VIEWPORT_MIN_WIDTH - hierarchy_min).max(0.0))
        .max(inspector_min);
    let hierarchy_default = (width * HIERARCHY_DEFAULT_RATIO).clamp(hierarchy_min, hierarchy_max);
    let inspector_default = (width * INSPECTOR_DEFAULT_RATIO).clamp(inspector_min, inspector_max);

    SidePanelLayout {
        hierarchy_default,
        hierarchy_min,
        hierarchy_max,
        inspector_default,
        inspector_min,
        inspector_max,
        viewport_min: VIEWPORT_MIN_WIDTH,
    }
}

impl EditorApp {
    pub(super) fn draw_menu_bar(&mut self, ui: &mut egui::Ui) {
        let has_project = self.current_project.is_some();
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project...").clicked() {
                    self.run_ui_action(EditorUiAction::NewProjectDialog);
                    ui.close();
                }
                if ui.button("Open Project...").clicked() {
                    self.run_ui_action(EditorUiAction::OpenProjectDialog);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(has_project, egui::Button::new("New Scene"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::NewScene);
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Open Scene..."))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::OpenSceneDialog);
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Save"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::SaveScene);
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Save As..."))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::SaveSceneAsDialog);
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Import OBJ..."))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::ImportObjDialog);
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
                if ui
                    .add_enabled(has_project, egui::Button::new("Cube"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cube));
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Sphere"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Sphere));
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Cone"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cone));
                    ui.close();
                }
                if ui
                    .add_enabled(has_project, egui::Button::new("Cylinder"))
                    .clicked()
                {
                    self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cylinder));
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
        let has_project = self.current_project.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_project, egui::Button::new("Cube"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cube));
            }
            if ui
                .add_enabled(has_project, egui::Button::new("Sphere"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Sphere));
            }
            if ui
                .add_enabled(has_project, egui::Button::new("Cone"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cone));
            }
            if ui
                .add_enabled(has_project, egui::Button::new("Cylinder"))
                .clicked()
            {
                self.run_ui_action(EditorUiAction::CreatePrimitive(PrimitiveKind::Cylinder));
            }
            ui.separator();

            ui.label("Transform");
            if ui
                .selectable_label(
                    self.transform_gizmo.mode == viewport::GizmoMode::Move,
                    "Move (W)",
                )
                .clicked()
            {
                self.run_ui_action(EditorUiAction::SetGizmoMode(viewport::GizmoMode::Move));
            }
            if ui
                .selectable_label(
                    self.transform_gizmo.mode == viewport::GizmoMode::Rotate,
                    "Rotate (E)",
                )
                .clicked()
            {
                self.run_ui_action(EditorUiAction::SetGizmoMode(viewport::GizmoMode::Rotate));
            }
            if ui
                .selectable_label(
                    self.transform_gizmo.mode == viewport::GizmoMode::Scale,
                    "Scale (R)",
                )
                .clicked()
            {
                self.run_ui_action(EditorUiAction::SetGizmoMode(viewport::GizmoMode::Scale));
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
        let panel_layout = side_panel_layout(ui.available_width());

        SidePanel::left("hierarchy_panel")
            .resizable(true)
            .default_size(panel_layout.hierarchy_default)
            .size_range(panel_layout.hierarchy_min..=panel_layout.hierarchy_max)
            .show(ui, |ui| {
                ui.take_available_width();
                draw_hierarchy(ui, &mut self.model);
                ui.separator();
                self.draw_assets_panel(ui);
            });

        let inspector_max = panel_layout
            .inspector_max
            .min((ui.available_width() - panel_layout.viewport_min).max(0.0));
        let inspector_min = panel_layout.inspector_min.min(inspector_max);
        let inspector_default = panel_layout
            .inspector_default
            .clamp(inspector_min, inspector_max);

        SidePanel::right("inspector_panel")
            .resizable(true)
            .default_size(inspector_default)
            .size_range(inspector_min..=inspector_max)
            .show(ui, |ui| {
                ui.take_available_width();
                self.draw_inspector_panel(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_viewport_column(ui);
        });
    }

    pub(super) fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        let selection = status_bar_selection_text(&self.model);
        ui.horizontal_wrapped(|ui| {
            if let Some(project) = &self.current_project {
                ui.label(format!("Project: {}", project.document.name));
                ui.separator();
                ui.label(
                    self.current_path
                        .as_ref()
                        .map_or_else(|| "No file".to_owned(), |path| path.display().to_string()),
                );
            } else {
                ui.label("No Project");
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
        let render_scene = self.model.render_scene();
        let draw = render::viewport_draw_call_with_view_and_meshes(
            &render_scene,
            self.model.selected(),
            view,
            &self.imported_meshes,
        );
        let selected = self.model.selected().cloned();
        let selected_transform = selected
            .as_ref()
            .and_then(|id| self.model.world().entity(id.as_str()))
            .map(|entity| entity.transform);
        let wgpu_probe = self.wgpu_viewport_available.then_some(&self.viewport_probe);
        let keyboard_shortcuts_allowed = Self::keyboard_shortcuts_allowed(ui.ctx());
        let fit_view_requested = self.fit_view_requested;
        self.fit_view_requested = false;
        let action = if self.pilot_camera {
            let mut blocked_camera = self.viewport_camera;
            draw_viewport(
                ui,
                draw.as_ref(),
                selected.as_ref(),
                selected_transform,
                &mut blocked_camera,
                &mut self.transform_gizmo,
                viewport::ViewportUiOptions {
                    keyboard_shortcuts_allowed,
                    fit_view_requested,
                    navigation_enabled: false,
                    wgpu_probe,
                },
            )
        } else {
            draw_viewport(
                ui,
                draw.as_ref(),
                selected.as_ref(),
                selected_transform,
                &mut self.viewport_camera,
                &mut self.transform_gizmo,
                viewport::ViewportUiOptions {
                    keyboard_shortcuts_allowed,
                    fit_view_requested,
                    navigation_enabled: true,
                    wgpu_probe,
                },
            )
        };
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
            self.draw_mesh_asset_info(ui, &mesh.asset, entity.transform);
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

    fn draw_assets_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Assets");
        for record in &self.asset_manifest.assets {
            let marker = match self.asset_load_status.get(&record.uuid) {
                Some(super::AssetLoadStatus::Loaded) => "",
                Some(super::AssetLoadStatus::MissingFile) => " missing file",
                Some(super::AssetLoadStatus::LoadFailed(_)) => " load failed",
                None => " missing",
            };
            ui.label(format!(
                "{}  {:?}  {}{}",
                record.name,
                record.kind,
                record.path.display(),
                marker
            ));
        }
    }

    fn draw_mesh_asset_info(&self, ui: &mut egui::Ui, asset_ref: &str, transform: Transform) {
        if let Some(size) = primitive_mesh_size_for_display(asset_ref, transform) {
            ui.label(format!("Mesh: {asset_ref}"));
            draw_mesh_size(ui, size);
            return;
        }
        let Ok(uuid) = asset::AssetUuid::parse_asset_ref(asset_ref) else {
            ui.label(format!("Mesh: {asset_ref}"));
            return;
        };
        let Some(record) = self.asset_manifest.find(&uuid) else {
            ui.label("Asset: missing");
            return;
        };
        ui.label(format!("Asset: {}", record.name));
        ui.label(format!("Path: {}", record.path.display()));
        match self.asset_load_status.get(&uuid) {
            Some(super::AssetLoadStatus::Loaded) => {}
            Some(super::AssetLoadStatus::MissingFile) => {
                ui.label("Status: missing file");
            }
            Some(super::AssetLoadStatus::LoadFailed(error)) => {
                ui.label(format!("Status: load failed: {error}"));
            }
            None => {
                ui.label("Status: missing");
            }
        }
        if let Some(size) = self
            .imported_meshes
            .get(&uuid)
            .and_then(|mesh| mesh_size_for_display(mesh, transform))
        {
            draw_mesh_size(ui, size);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MeshSizeDisplay {
    pub(super) local: [f32; 3],
    pub(super) scaled: [f32; 3],
}

pub(super) fn mesh_size_for_display(
    mesh: &asset::ImportedMesh,
    transform: Transform,
) -> Option<MeshSizeDisplay> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    let local = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    if !local.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(MeshSizeDisplay {
        local,
        scaled: [
            local[0] * transform.scale[0].abs(),
            local[1] * transform.scale[1].abs(),
            local[2] * transform.scale[2].abs(),
        ],
    })
}

pub(super) fn primitive_mesh_size_for_display(
    asset_ref: &str,
    transform: Transform,
) -> Option<MeshSizeDisplay> {
    let local = match asset_ref {
        "primitive:cube" | "primitive:sphere" | "primitive:cone" | "primitive:cylinder" => {
            [2.0, 2.0, 2.0]
        }
        _ => return None,
    };
    Some(MeshSizeDisplay {
        local,
        scaled: [
            local[0] * transform.scale[0].abs(),
            local[1] * transform.scale[1].abs(),
            local[2] * transform.scale[2].abs(),
        ],
    })
}

fn draw_mesh_size(ui: &mut egui::Ui, size: MeshSizeDisplay) {
    ui.label(format!("Local size: {}", format_vec3(size.local)));
    ui.label(format!("Scaled size: {}", format_vec3(size.scaled)));
}

fn format_vec3(values: [f32; 3]) -> String {
    format!("{:.3} x {:.3} x {:.3}", values[0], values[1], values[2])
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

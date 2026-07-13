// Copyright The SimpleGameEngine Contributors

use eframe::egui;
use sge_scene::{AuthoringEntity, SceneEntityId, SceneName};

use super::EditorApp;
use crate::{inspector, inspector_ui};

const MIN_HIERARCHY_WIDTH: f32 = 160.0;
const MIN_INSPECTOR_WIDTH: f32 = 240.0;
const MIN_VIEWPORT_WIDTH: f32 = 240.0;

impl EditorApp {
    pub(super) fn hierarchy(&mut self, ui: &mut egui::Ui) {
        let max_width = side_panel_max_width(
            ui.available_width(),
            MIN_HIERARCHY_WIDTH,
            MIN_VIEWPORT_WIDTH,
        );
        egui::Panel::left("hierarchy")
            .resizable(true)
            .default_size(230.0)
            .min_size(MIN_HIERARCHY_WIDTH)
            .max_size(max_width)
            .show(ui, |ui| {
                ui.heading("Hierarchy");
                if self.play.is_none() && ui.button("New Entity").clicked() {
                    let _ = self.apply_ui_action(super::EditorUiAction::CreateEntity);
                }
                ui.add_enabled_ui(self.play.is_none(), |ui| {
                    ui.menu_button("Create Primitive", |ui| {
                        for (label, primitive) in [
                            ("Cube", crate::PrimitiveKind::Cube),
                            ("Sphere", crate::PrimitiveKind::Sphere),
                            ("Cone", crate::PrimitiveKind::Cone),
                            ("Cylinder", crate::PrimitiveKind::Cylinder),
                        ] {
                            if ui.button(label).clicked() {
                                ui.close();
                                let _ = self.apply_ui_action(
                                    super::EditorUiAction::CreatePrimitive(primitive),
                                );
                            }
                        }
                    });
                });
                let selection = self.session.selection();
                match self.session.snapshot() {
                    Ok(scene) => {
                        for entity in scene.entities() {
                            let label = self
                                .session
                                .component::<SceneName>(entity.id())
                                .map_or_else(
                                    || entity.id().to_string(),
                                    |name| name.as_str().to_owned(),
                                );
                            if ui
                                .selectable_label(selection == Some(entity.id()), label)
                                .clicked()
                            {
                                let _ = self.apply_ui_action(super::EditorUiAction::SelectEntity(
                                    entity.id(),
                                ));
                            }
                        }
                    }
                    Err(error) => self.last_error = Some(error.to_string()),
                }
                if self.play.is_none()
                    && let Some(selection) = self.session.selection()
                {
                    ui.horizontal(|ui| {
                        if ui.button("Duplicate").clicked() {
                            match self.session.duplicate_entity(selection) {
                                Ok(entity) => {
                                    let result = self.session.select(Some(entity));
                                    self.apply_edit(result);
                                }
                                Err(error) => self.last_error = Some(error.to_string()),
                            }
                        }
                        if ui.button("Delete Subtree").clicked() {
                            let result = self.session.remove_subtree(selection);
                            self.apply_edit(result);
                        }
                    });
                    let parents = self
                        .session
                        .snapshot()
                        .map(|scene| {
                            scene
                                .entities()
                                .filter(|entity| entity.id() != selection)
                                .map(AuthoringEntity::id)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    ui.menu_button("Reparent", |ui| {
                        if ui.button("Root").clicked() {
                            ui.close();
                            let result = self.session.reparent_entity(selection, None);
                            self.apply_edit(result);
                        }
                        for parent in parents {
                            if ui.button(parent.to_string()).clicked() {
                                ui.close();
                                let result = self.session.reparent_entity(selection, Some(parent));
                                self.apply_edit(result);
                            }
                        }
                    });
                }
                fill_resizable_panel(ui);
            });
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        let max_width = side_panel_max_width(
            ui.available_width(),
            MIN_INSPECTOR_WIDTH,
            MIN_HIERARCHY_WIDTH + MIN_VIEWPORT_WIDTH,
        );
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(300.0)
            .min_size(MIN_INSPECTOR_WIDTH)
            .max_size(max_width)
            .show(ui, |ui| {
                self.inspector_contents(ui);
                fill_resizable_panel(ui);
            });
    }

    fn inspector_contents(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        let mut components = match self.session.inspector() {
            Ok(components) => components,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        };
        let Some(entity) = self.session.selection() else {
            return;
        };
        if let Some((preview_entity, transform)) = self.viewport.drag_preview()
            && preview_entity == entity
        {
            inspector::apply_transform_preview(&mut components, transform);
        }
        self.component_picker(ui, &components);
        self.component_draft(ui, entity);
        let action = ui
            .add_enabled_ui(self.play.is_none(), |ui| {
                inspector_ui::draw(ui, entity, &components, &mut self.inspector_drafts, true)
            })
            .inner;
        let Some(action) = action else {
            return;
        };
        let result = match action {
            inspector_ui::InspectorAction::SetField {
                component,
                field,
                value,
            } => self
                .session
                .set_field(entity, component.as_str(), field.as_str(), value),
            inspector_ui::InspectorAction::RemoveComponent(component) => {
                self.session.remove_component(entity, component.as_str())
            }
        };
        self.apply_edit(result);
    }

    fn component_picker(&mut self, ui: &mut egui::Ui, components: &[crate::InspectorComponent]) {
        let available = self
            .session
            .component_types()
            .into_iter()
            .filter(|candidate| {
                !components
                    .iter()
                    .any(|component| component.type_key() == candidate.type_key())
            })
            .collect::<Vec<_>>();
        if !available
            .iter()
            .any(|candidate| self.component_to_add.as_ref() == Some(candidate.type_key()))
        {
            self.component_to_add = available
                .first()
                .map(|candidate| candidate.type_key().clone());
        }
        let before = self.component_to_add.clone();
        let mut configure = false;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("add_component_type")
                .selected_text(
                    self.component_to_add
                        .as_ref()
                        .and_then(|selected| {
                            available
                                .iter()
                                .find(|candidate| candidate.type_key() == selected)
                        })
                        .map_or("No component", |candidate| candidate.display_name()),
                )
                .show_ui(ui, |ui| {
                    for candidate in &available {
                        ui.selectable_value(
                            &mut self.component_to_add,
                            Some(candidate.type_key().clone()),
                            candidate.display_name(),
                        );
                    }
                });
            configure = self.play.is_none() && ui.button("Configure Component").clicked();
        });
        if self.component_to_add != before {
            self.component_draft = None;
            self.inspector_drafts.clear();
        }
        if configure && let Some(component) = self.component_to_add.as_ref() {
            match self.session.component_draft(component.as_str()) {
                Ok(draft) => {
                    self.component_draft = Some(draft);
                    self.inspector_drafts.clear();
                    self.last_error = None;
                }
                Err(error) => self.last_error = Some(error.to_string()),
            }
        }
    }

    fn component_draft(&mut self, ui: &mut egui::Ui, entity: SceneEntityId) {
        let Some(draft) = self.component_draft.clone() else {
            return;
        };
        ui.separator();
        ui.strong("New Component Draft");
        let component = match self.session.inspect_component_value(&draft) {
            Ok(component) => component,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        };
        if let Some(inspector_ui::InspectorAction::SetField { field, value, .. }) =
            inspector_ui::draw(
                ui,
                entity,
                std::slice::from_ref(&component),
                &mut self.inspector_drafts,
                false,
            )
        {
            match self
                .session
                .set_component_draft_field(&draft, field.as_str(), value)
            {
                Ok(updated) => {
                    self.component_draft = Some(updated);
                    self.inspector_drafts.clear();
                    self.last_error = None;
                }
                Err(error) => self.last_error = Some(error.to_string()),
            }
        }
        ui.horizontal(|ui| {
            if self.play.is_none() && ui.button("Commit Component").clicked() {
                let value = self.component_draft.clone().unwrap_or(draft.clone());
                let result = self.session.add_component_value(entity, value);
                self.apply_edit(result);
            }
            if ui.button("Cancel").clicked() {
                self.component_draft = None;
                self.inspector_drafts.clear();
            }
        });
        ui.separator();
    }
}

fn fill_resizable_panel(ui: &mut egui::Ui) {
    ui.take_available_space();
}

fn side_panel_max_width(available_width: f32, minimum: f32, reserved_width: f32) -> f32 {
    (available_width - reserved_width)
        .max(minimum)
        .min(available_width)
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_HIERARCHY_WIDTH, MIN_INSPECTOR_WIDTH, MIN_VIEWPORT_WIDTH, fill_resizable_panel,
        side_panel_max_width,
    };
    use eframe::egui;

    #[test]
    fn resizable_panel_keeps_its_configured_width_with_short_content() {
        let context = egui::Context::default();
        let mut width = 0.0;
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                let response = egui::Panel::left("resizable_panel_test")
                    .resizable(true)
                    .default_size(230.0)
                    .show(ui, |ui| {
                        ui.label("Short");
                        fill_resizable_panel(ui);
                    });
                width = response.response.rect.width();
            },
        );

        assert!((width - 230.0).abs() <= 1.0, "panel width was {width}");
    }

    #[test]
    fn side_panels_reserve_the_other_panel_and_viewport() {
        let window_width = 1_000.0;
        let inspector_max = side_panel_max_width(
            window_width,
            MIN_INSPECTOR_WIDTH,
            MIN_HIERARCHY_WIDTH + MIN_VIEWPORT_WIDTH,
        );
        let hierarchy_max = side_panel_max_width(
            window_width - inspector_max,
            MIN_HIERARCHY_WIDTH,
            MIN_VIEWPORT_WIDTH,
        );

        let viewport_width = window_width - inspector_max - hierarchy_max;
        assert!(viewport_width >= MIN_VIEWPORT_WIDTH);
        assert!(inspector_max >= MIN_INSPECTOR_WIDTH);
        assert!(hierarchy_max >= MIN_HIERARCHY_WIDTH);
    }
}

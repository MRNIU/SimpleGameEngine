// Copyright The SimpleGameEngine Contributors

use eframe::egui;
use sge_scene::{AuthoringEntity, SceneEntityId};

use super::EditorApp;
use crate::inspector_ui;

impl EditorApp {
    pub(super) fn hierarchy(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("hierarchy")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| {
                ui.heading("Hierarchy");
                if self.play.is_none() && ui.button("New Entity").clicked() {
                    let id = SceneEntityId::new_v4();
                    let result = AuthoringEntity::new(id, None, Vec::new())
                        .map_err(crate::EditError::from)
                        .and_then(|entity| self.session.add_entity(entity))
                        .and_then(|()| self.session.select(Some(id)));
                    self.apply_edit(result);
                }
                let selection = self.session.selection();
                match self.session.snapshot() {
                    Ok(scene) => {
                        for entity in scene.entities() {
                            if ui
                                .selectable_label(
                                    selection == Some(entity.id()),
                                    entity.id().to_string(),
                                )
                                .clicked()
                            {
                                let result = self.session.select(Some(entity.id()));
                                self.apply_edit(result);
                            }
                        }
                    }
                    Err(error) => self.last_error = Some(error.to_string()),
                }
                if self.play.is_none()
                    && let Some(selection) = self.session.selection()
                    && ui.button("Delete Selected").clicked()
                {
                    let result = self.session.remove_entity(selection);
                    self.apply_edit(result);
                }
            });
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| self.inspector_contents(ui));
    }

    fn inspector_contents(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        let components = match self.session.inspector() {
            Ok(components) => components,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        };
        let Some(entity) = self.session.selection() else {
            return;
        };
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

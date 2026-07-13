// Copyright The SimpleGameEngine Contributors
//
//! Inspector panel, component picker and component-draft controls.

use eframe::egui;
use sge_scene::SceneEntityId;

use crate::{
    host::EditorApp,
    inspector, inspector_ui,
    localization::{EditorText, reflect_type_name},
};

use super::{INSPECTOR_PANEL_ID, fill_resizable_panel};

impl EditorApp {
    pub(in crate::host) fn inspector(&mut self, ui: &mut egui::Ui) {
        let sizes = self.panel_layout.inspector_sizes();
        let response = egui::Panel::right(INSPECTOR_PANEL_ID)
            .resizable(true)
            .default_size(sizes.default)
            .min_size(sizes.minimum)
            .max_size(sizes.maximum)
            .show(ui, |ui| {
                self.inspector_contents(ui);
                fill_resizable_panel(ui);
            });
        self.panel_layout
            .record_inspector(response.response.rect.width());
    }

    fn inspector_contents(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        ui.heading(language.text(EditorText::Inspector));
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
        let authoring_enabled = self.authoring_enabled();
        ui.add_enabled_ui(authoring_enabled, |ui| {
            self.component_picker(ui, &components);
            self.component_draft(ui, entity);
        });
        let action = ui
            .add_enabled_ui(authoring_enabled, |ui| {
                inspector_ui::draw(
                    ui,
                    entity,
                    &components,
                    &mut self.inspector_drafts,
                    true,
                    language,
                    self.translations.as_ref(),
                )
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
        let language = self.language;
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
                        .map_or(language.text(EditorText::NoComponent), |candidate| {
                            reflect_type_name(
                                language,
                                candidate.type_key().as_str(),
                                candidate.display_name(),
                                self.translations.as_ref(),
                            )
                        }),
                )
                .show_ui(ui, |ui| {
                    for candidate in &available {
                        ui.selectable_value(
                            &mut self.component_to_add,
                            Some(candidate.type_key().clone()),
                            reflect_type_name(
                                language,
                                candidate.type_key().as_str(),
                                candidate.display_name(),
                                self.translations.as_ref(),
                            ),
                        );
                    }
                });
            configure = self.authoring_enabled()
                && ui
                    .button(language.text(EditorText::ConfigureComponent))
                    .clicked();
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
        let language = self.language;
        let Some(draft) = self.component_draft.clone() else {
            return;
        };
        ui.separator();
        ui.strong(language.text(EditorText::NewComponentDraft));
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
                language,
                self.translations.as_ref(),
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
            if self.authoring_enabled()
                && ui
                    .button(language.text(EditorText::CommitComponent))
                    .clicked()
            {
                let value = self.component_draft.clone().unwrap_or(draft.clone());
                let result = self.session.add_component_value(entity, value);
                self.apply_edit(result);
            }
            if ui.button(language.text(EditorText::Cancel)).clicked() {
                self.component_draft = None;
                self.inspector_drafts.clear();
            }
        });
        ui.separator();
    }
}

// Copyright The SimpleGameEngine Contributors
//
//! Hierarchy, actor placement and entity-tree authoring controls.

use eframe::egui;
use sge_scene::{AuthoringEntity, SceneName};

use crate::{
    host::{EditorApp, EditorUiAction},
    localization::{EditorText, scene_entity_name},
};

use super::{HIERARCHY_PANEL_ID, fill_resizable_panel};

impl EditorApp {
    pub(in crate::host) fn hierarchy(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        let sizes = self.panel_layout.hierarchy_sizes();
        let response = egui::Panel::left(HIERARCHY_PANEL_ID)
            .resizable(true)
            .default_size(sizes.default)
            .min_size(sizes.minimum)
            .max_size(sizes.maximum)
            .show(ui, |ui| {
                ui.heading(language.text(EditorText::Hierarchy));
                ui.add_enabled_ui(self.authoring_enabled(), |ui| {
                    ui.menu_button(language.text(EditorText::PlaceActors), |ui| {
                        if ui.button(language.text(EditorText::EmptyActor)).clicked() {
                            ui.close();
                            let _ = self.apply_ui_action(EditorUiAction::CreateEmptyActor);
                        }
                        ui.separator();
                        ui.label(language.text(EditorText::BasicShapes));
                        for (label, primitive) in [
                            (EditorText::Cube, crate::PrimitiveKind::Cube),
                            (EditorText::Sphere, crate::PrimitiveKind::Sphere),
                            (EditorText::Cone, crate::PrimitiveKind::Cone),
                            (EditorText::Cylinder, crate::PrimitiveKind::Cylinder),
                        ] {
                            if ui.button(language.text(label)).clicked() {
                                ui.close();
                                let _ = self
                                    .apply_ui_action(EditorUiAction::CreatePrimitive(primitive));
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
                            let entity_id = entity.id().to_string();
                            let label = scene_entity_name(
                                language,
                                &entity_id,
                                &label,
                                self.translations.as_ref(),
                            );
                            if ui
                                .selectable_label(selection == Some(entity.id()), label)
                                .clicked()
                            {
                                let _ =
                                    self.apply_ui_action(EditorUiAction::SelectEntity(entity.id()));
                            }
                        }
                    }
                    Err(error) => self.last_error = Some(error.to_string()),
                }
                if self.authoring_enabled()
                    && let Some(selection) = self.session.selection()
                {
                    ui.horizontal(|ui| {
                        if ui.button(language.text(EditorText::Duplicate)).clicked() {
                            let _ = self.apply_ui_action(EditorUiAction::DuplicateSelection);
                        }
                        if ui
                            .button(language.text(EditorText::DeleteSubtree))
                            .clicked()
                        {
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
                    ui.menu_button(language.text(EditorText::Reparent), |ui| {
                        if ui.button(language.text(EditorText::Root)).clicked() {
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
        self.panel_layout
            .record_hierarchy(response.response.rect.width());
    }
}

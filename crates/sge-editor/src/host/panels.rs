// Copyright The SimpleGameEngine Contributors

use eframe::egui;
use sge_scene::{AuthoringEntity, SceneEntityId, SceneName};

use super::EditorApp;
use crate::{inspector, inspector_ui};

const HIERARCHY_PANEL_ID: &str = "hierarchy";
const INSPECTOR_PANEL_ID: &str = "inspector";
const DEFAULT_HIERARCHY_FRACTION: f32 = 0.18;
const DEFAULT_INSPECTOR_FRACTION: f32 = 0.24;
const MIN_HIERARCHY_FRACTION: f32 = 0.12;
const MIN_INSPECTOR_FRACTION: f32 = 0.18;
const MIN_VIEWPORT_FRACTION: f32 = 0.40;
const MAX_SIDE_PANEL_FRACTION: f32 = 0.35;

pub(super) struct PanelLayout {
    total_width: Option<f32>,
    hierarchy_fraction: f32,
    inspector_fraction: f32,
}

#[derive(Clone, Copy)]
struct PanelSizes {
    default: f32,
    minimum: f32,
    maximum: f32,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            total_width: None,
            hierarchy_fraction: DEFAULT_HIERARCHY_FRACTION,
            inspector_fraction: DEFAULT_INSPECTOR_FRACTION,
        }
    }
}

impl PanelLayout {
    pub(super) fn begin_frame(&mut self, context: &egui::Context, total_width: f32) {
        if self
            .total_width
            .is_some_and(|previous| (previous - total_width).abs() > 0.5)
        {
            context.data_mut(|data| {
                data.remove::<egui::containers::PanelState>(egui::Id::new(HIERARCHY_PANEL_ID));
                data.remove::<egui::containers::PanelState>(egui::Id::new(INSPECTOR_PANEL_ID));
            });
        }
        self.total_width = Some(total_width.max(f32::EPSILON));
    }

    fn inspector_sizes(&self) -> PanelSizes {
        self.sizes(
            self.inspector_fraction,
            MIN_INSPECTOR_FRACTION,
            MAX_SIDE_PANEL_FRACTION.min(1.0 - MIN_HIERARCHY_FRACTION - MIN_VIEWPORT_FRACTION),
        )
    }

    fn hierarchy_sizes(&self) -> PanelSizes {
        self.sizes(
            self.hierarchy_fraction,
            MIN_HIERARCHY_FRACTION,
            MAX_SIDE_PANEL_FRACTION
                .min(1.0 - self.inspector_fraction - MIN_VIEWPORT_FRACTION)
                .max(MIN_HIERARCHY_FRACTION),
        )
    }

    fn sizes(&self, default: f32, minimum: f32, maximum: f32) -> PanelSizes {
        let total_width = self.total_width.unwrap_or(1.0);
        PanelSizes {
            default: total_width * default.clamp(minimum, maximum),
            minimum: total_width * minimum,
            maximum: total_width * maximum,
        }
    }

    fn record_inspector(&mut self, width: f32) {
        self.inspector_fraction = self
            .width_fraction(width)
            .clamp(MIN_INSPECTOR_FRACTION, MAX_SIDE_PANEL_FRACTION);
    }

    fn record_hierarchy(&mut self, width: f32) {
        let maximum = MAX_SIDE_PANEL_FRACTION
            .min(1.0 - self.inspector_fraction - MIN_VIEWPORT_FRACTION)
            .max(MIN_HIERARCHY_FRACTION);
        self.hierarchy_fraction = self
            .width_fraction(width)
            .clamp(MIN_HIERARCHY_FRACTION, maximum);
    }

    fn width_fraction(&self, width: f32) -> f32 {
        width / self.total_width.unwrap_or(1.0)
    }
}

impl EditorApp {
    pub(super) fn hierarchy(&mut self, ui: &mut egui::Ui) {
        let sizes = self.panel_layout.hierarchy_sizes();
        let response = egui::Panel::left(HIERARCHY_PANEL_ID)
            .resizable(true)
            .default_size(sizes.default)
            .min_size(sizes.minimum)
            .max_size(sizes.maximum)
            .show(ui, |ui| {
                ui.heading("Hierarchy");
                ui.add_enabled_ui(self.authoring_enabled(), |ui| {
                    ui.menu_button("Place Actors", |ui| {
                        if ui.button("Empty Actor").clicked() {
                            ui.close();
                            let _ = self.apply_ui_action(super::EditorUiAction::CreateEmptyActor);
                        }
                        ui.separator();
                        ui.label("Basic Shapes");
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
                if self.authoring_enabled()
                    && let Some(selection) = self.session.selection()
                {
                    ui.horizontal(|ui| {
                        if ui.button("Duplicate").clicked() {
                            let _ = self.apply_ui_action(super::EditorUiAction::DuplicateSelection);
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
        self.panel_layout
            .record_hierarchy(response.response.rect.width());
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
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
        let authoring_enabled = self.authoring_enabled();
        ui.add_enabled_ui(authoring_enabled, |ui| {
            self.component_picker(ui, &components);
            self.component_draft(ui, entity);
        });
        let action = ui
            .add_enabled_ui(authoring_enabled, |ui| {
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
            configure = self.authoring_enabled() && ui.button("Configure Component").clicked();
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
            if self.authoring_enabled() && ui.button("Commit Component").clicked() {
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

#[cfg(test)]
mod tests {
    use super::{HIERARCHY_PANEL_ID, INSPECTOR_PANEL_ID, PanelLayout, fill_resizable_panel};
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
    fn percent_layout_scales_all_columns_with_the_window() {
        let context = egui::Context::default();
        let mut layout = PanelLayout::default();
        let small = panel_widths(&context, &mut layout, 1_000.0);
        let large = panel_widths(&context, &mut layout, 2_000.0);

        assert!((large.0 - small.0 * 2.0).abs() <= 2.0);
        assert!((large.1 - small.1 * 2.0).abs() <= 2.0);
        assert!((large.2 - small.2 * 2.0).abs() <= 2.0);
        assert!(small.2 >= 400.0);
    }

    fn panel_widths(
        context: &egui::Context,
        layout: &mut PanelLayout,
        width: f32,
    ) -> (f32, f32, f32) {
        let mut widths = (0.0, 0.0, 0.0);
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                layout.begin_frame(ui.ctx(), ui.available_width());
                let inspector = layout.inspector_sizes();
                let response = egui::Panel::right(INSPECTOR_PANEL_ID)
                    .resizable(true)
                    .default_size(inspector.default)
                    .min_size(inspector.minimum)
                    .max_size(inspector.maximum)
                    .show(ui, fill_resizable_panel);
                widths.1 = response.response.rect.width();
                layout.record_inspector(widths.1);

                let hierarchy = layout.hierarchy_sizes();
                let response = egui::Panel::left(HIERARCHY_PANEL_ID)
                    .resizable(true)
                    .default_size(hierarchy.default)
                    .min_size(hierarchy.minimum)
                    .max_size(hierarchy.maximum)
                    .show(ui, fill_resizable_panel);
                widths.0 = response.response.rect.width();
                layout.record_hierarchy(widths.0);
                widths.2 = ui.available_width();
            },
        );
        widths
    }
}

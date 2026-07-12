// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use eframe::egui;
use sge_reflect::{FieldKey, FieldKind, TypeKey, Value};
use sge_scene::SceneEntityId;

use crate::InspectorComponent;

pub(crate) enum InspectorAction {
    SetField {
        component: TypeKey,
        field: FieldKey,
        value: Value,
    },
    RemoveComponent(TypeKey),
}

#[derive(Default)]
pub(crate) struct InspectorDrafts {
    references: BTreeMap<(SceneEntityId, TypeKey, FieldKey), String>,
}

impl InspectorDrafts {
    pub(crate) fn clear(&mut self) {
        self.references.clear();
    }
}

pub(crate) fn draw(
    ui: &mut egui::Ui,
    entity: SceneEntityId,
    components: &[InspectorComponent],
    drafts: &mut InspectorDrafts,
    allow_remove: bool,
) -> Option<InspectorAction> {
    let mut action = None;
    for component in components {
        egui::CollapsingHeader::new(component.display_name())
            .id_salt(component.type_key().as_str())
            .default_open(true)
            .show(ui, |ui| {
                for field in component.fields() {
                    ui.push_id(field.field_key().as_str(), |ui| {
                        ui.label(field.display_name());
                        if let Value::Reference(current) = field.value() {
                            let key = (
                                entity,
                                component.type_key().clone(),
                                field.field_key().clone(),
                            );
                            let draft = drafts
                                .references
                                .entry(key.clone())
                                .or_insert_with(|| current.clone());
                            let response = ui.text_edit_singleline(draft);
                            let commit = response.lost_focus()
                                || (response.has_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                            if commit {
                                if draft == current {
                                    drafts.references.remove(&key);
                                } else {
                                    action = Some(InspectorAction::SetField {
                                        component: component.type_key().clone(),
                                        field: field.field_key().clone(),
                                        value: Value::Reference(draft.clone()),
                                    });
                                }
                            }
                            return;
                        }
                        let mut value = field.value().clone();
                        if edit_value(ui, field.kind(), &mut value) {
                            action = Some(InspectorAction::SetField {
                                component: component.type_key().clone(),
                                field: field.field_key().clone(),
                                value,
                            });
                        }
                    });
                }
                if allow_remove && ui.small_button("Remove Component").clicked() {
                    action = Some(InspectorAction::RemoveComponent(
                        component.type_key().clone(),
                    ));
                }
            });
    }
    action
}

fn edit_value(ui: &mut egui::Ui, kind: &FieldKind, value: &mut Value) -> bool {
    match value {
        Value::Bool(value) => ui.checkbox(value, "").changed(),
        Value::I64(value) => ui.add(egui::DragValue::new(value)).changed(),
        Value::F32(value) => ui.add(egui::DragValue::new(value)).changed(),
        Value::String(value) => ui.text_edit_singleline(value).changed(),
        Value::Reference(_) => false,
        Value::Vec2(value) => {
            let mut fields = value.to_array();
            let changed = floats(ui, &mut fields);
            *value = fields.into();
            changed
        }
        Value::Vec3(value) => {
            let mut fields = value.to_array();
            let changed = floats(ui, &mut fields);
            *value = fields.into();
            changed
        }
        Value::Vec4(value) => {
            let mut fields = value.to_array();
            let changed = floats(ui, &mut fields);
            *value = fields.into();
            changed
        }
        Value::Quat(value) => {
            let mut fields = value.to_array();
            let changed = floats(ui, &mut fields);
            *value = sge_math::Quat::from_array(fields);
            changed
        }
        Value::Color(value) => ui.color_edit_button_rgba_unmultiplied(value).changed(),
        Value::Enum(value) => {
            let before = value.clone();
            if let FieldKind::Enum { options } = kind {
                egui::ComboBox::from_id_salt("enum")
                    .selected_text(value.as_str())
                    .show_ui(ui, |ui| {
                        for option in options {
                            ui.selectable_value(value, option.clone(), option);
                        }
                    });
            }
            *value != before
        }
    }
}

fn floats<const N: usize>(ui: &mut egui::Ui, values: &mut [f32; N]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for value in values {
            changed |= ui.add(egui::DragValue::new(value).speed(0.05)).changed();
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use sge_reflect::{FieldKey, TypeKey};
    use sge_scene::SceneEntityId;

    use super::InspectorDrafts;

    #[test]
    fn invalid_reference_text_remains_an_editor_draft_until_successful_commit() {
        let entity = "80000000-0000-4000-8000-000000000001"
            .parse::<SceneEntityId>()
            .expect("canonical ID");
        let key = (
            entity,
            TypeKey::new("demo.reference").expect("type key"),
            FieldKey::new("target").expect("field key"),
        );
        let mut drafts = InspectorDrafts::default();
        drafts.references.insert(key.clone(), "partial".to_owned());

        assert_eq!(
            drafts.references.get(&key).map(String::as_str),
            Some("partial")
        );
        drafts.clear();
        assert!(drafts.references.is_empty());
    }
}

// Copyright The SimpleGameEngine Contributors

use sge_math::{Quat, Transform, Vec3};
use sge_reflect::{FieldKey, FieldKind, ReflectedValue, TypeDescriptor, TypeKey, Value};

use crate::EditError;

#[derive(Debug, Clone, PartialEq)]
pub struct InspectorComponent {
    type_key: TypeKey,
    display_name: String,
    fields: Vec<InspectorField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectorField {
    field_key: FieldKey,
    display_name: String,
    kind: FieldKind,
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneComponentType {
    type_key: TypeKey,
    display_name: String,
}

impl InspectorComponent {
    pub(crate) fn from_reflected(
        descriptor: &TypeDescriptor,
        value: &ReflectedValue,
    ) -> Result<Self, EditError> {
        let fields = descriptor
            .fields()
            .map(|(field_key, metadata)| {
                let value = value
                    .fields()
                    .get(field_key.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        EditError::Reflect(sge_reflect::ReflectError::MissingField(
                            field_key.clone(),
                        ))
                    })?;
                Ok(InspectorField {
                    field_key: field_key.clone(),
                    display_name: metadata.display_name().to_owned(),
                    kind: metadata.kind().clone(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, EditError>>()?;
        Ok(Self {
            type_key: descriptor.type_key().clone(),
            display_name: descriptor.display_name().to_owned(),
            fields,
        })
    }

    #[must_use]
    pub const fn type_key(&self) -> &TypeKey {
        &self.type_key
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn fields(&self) -> &[InspectorField] {
        &self.fields
    }
}

impl InspectorField {
    #[must_use]
    pub const fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn kind(&self) -> &FieldKind {
        &self.kind
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

pub(crate) fn apply_transform_preview(components: &mut [InspectorComponent], transform: Transform) {
    let Some(component) = components
        .iter_mut()
        .find(|component| component.type_key.as_str() == "sge.transform")
    else {
        return;
    };
    for field in &mut component.fields {
        field.value = match field.field_key.as_str() {
            "translation" => Value::Vec3(Vec3::from_array(transform.translation)),
            "rotation" => Value::Quat(Quat::from_array(transform.rotation)),
            "scale" => Value::Vec3(Vec3::from_array(transform.scale)),
            _ => continue,
        };
    }
}

impl SceneComponentType {
    pub(crate) fn new(type_key: TypeKey, display_name: String) -> Self {
        Self {
            type_key,
            display_name,
        }
    }

    #[must_use]
    pub const fn type_key(&self) -> &TypeKey {
        &self.type_key
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_preview_replaces_all_inspector_values_without_committing() {
        let mut components = [InspectorComponent {
            type_key: TypeKey::new("sge.transform").unwrap(),
            display_name: "Transform".to_owned(),
            fields: vec![
                InspectorField {
                    field_key: FieldKey::new("translation").unwrap(),
                    display_name: "Translation".to_owned(),
                    kind: FieldKind::Vec3,
                    value: Value::Vec3(Vec3::ZERO),
                },
                InspectorField {
                    field_key: FieldKey::new("rotation").unwrap(),
                    display_name: "Rotation".to_owned(),
                    kind: FieldKind::Quat,
                    value: Value::Quat(Quat::IDENTITY),
                },
                InspectorField {
                    field_key: FieldKey::new("scale").unwrap(),
                    display_name: "Scale".to_owned(),
                    kind: FieldKind::Vec3,
                    value: Value::Vec3(Vec3::ONE),
                },
            ],
        }];
        let preview = Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: Quat::from_rotation_z(0.5).to_array(),
            scale: [2.0, 3.0, 4.0],
        };

        apply_transform_preview(&mut components, preview);

        let values = components[0]
            .fields()
            .iter()
            .map(InspectorField::value)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                Value::Vec3(Vec3::from_array(preview.translation)),
                Value::Quat(Quat::from_array(preview.rotation)),
                Value::Vec3(Vec3::from_array(preview.scale)),
            ]
        );
    }
}

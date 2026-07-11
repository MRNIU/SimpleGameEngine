// Copyright The SimpleGameEngine Contributors
//
//! `sge-reflect` public contract tests.

use sge_reflect::{
    FieldKey, FieldKind, FieldMetadata, FieldRegistration, FieldValues, ReferenceSemantic,
    ReflectError, ReflectedValue, RegistryError, TypeDescriptor, TypeKey, TypeRegistry,
    ValidationErrors, ValidationIssue, Value,
};

#[derive(Debug, Clone, PartialEq)]
struct Rotator {
    speed: f32,
    angle: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct RevisionedRotator {
    speed: f32,
    hidden_revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct ModeComponent {
    mode: String,
}

fn rotator_descriptor() -> TypeDescriptor {
    rotator_descriptor_with_key("demo.rotator")
}

fn rotator_descriptor_with_key(key: &str) -> TypeDescriptor {
    TypeDescriptor::builder::<Rotator>(TypeKey::new(key).unwrap(), 1, "Rotator", || Rotator {
        speed: 1.0,
        angle: 0.0,
    })
    .field(
        FieldRegistration::new(
            FieldKey::new("speed").unwrap(),
            FieldMetadata::new("Speed", FieldKind::F32),
            |value: &Rotator| Value::F32(value.speed),
            |value: &mut Rotator, field: &Value| match field {
                Value::F32(speed) => {
                    value.speed = *speed;
                    Ok(())
                }
                other => Err(ReflectError::value_kind("speed", "F32", other.kind())),
            },
        )
        .validator(|field: &Value| match field {
            Value::F32(speed) if *speed > 0.0 => Ok(()),
            Value::F32(_) => Err(ValidationIssue::field(
                FieldKey::new("speed").unwrap(),
                "speed must be positive",
            )),
            other => Err(ValidationIssue::field(
                FieldKey::new("speed").unwrap(),
                format!("expected F32, got {:?}", other.kind()),
            )),
        }),
    )
    .field(FieldRegistration::new(
        FieldKey::new("angle").unwrap(),
        FieldMetadata::new("Angle", FieldKind::F32),
        |value: &Rotator| Value::F32(value.angle),
        |value: &mut Rotator, field: &Value| match field {
            Value::F32(angle) => {
                value.angle = *angle;
                Ok(())
            }
            other => Err(ReflectError::value_kind("angle", "F32", other.kind())),
        },
    ))
    .validator(|value: &Rotator| {
        if value.angle.is_finite() {
            Ok(())
        } else {
            Err(ValidationErrors::one(ValidationIssue::component(
                "angle must be finite",
            )))
        }
    })
    .build()
    .unwrap()
}

#[test]
fn registered_component_roundtrips_clones_and_reads_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(rotator_descriptor()).unwrap();
    registry.freeze().unwrap();
    let value = Rotator {
        speed: 2.0,
        angle: 3.0,
    };

    registry.validate(&value).unwrap();
    assert!(matches!(
        registry.validate(&Rotator {
            speed: -1.0,
            angle: 0.0,
        }),
        Err(ReflectError::Validation(_))
    ));
    let encoded = registry.encode(&value).unwrap();
    assert_eq!(encoded.type_key().as_str(), "demo.rotator");
    assert_eq!(encoded.schema_version(), 1);
    assert_eq!(encoded.fields().get("speed"), Some(&Value::F32(2.0)));
    let decoded = registry.decode(&encoded).unwrap();
    assert_eq!(*decoded.downcast::<Rotator>().unwrap(), value);
    let mut cloned = registry
        .clone_value(&value)
        .unwrap()
        .downcast::<Rotator>()
        .unwrap();
    cloned.angle = 9.0;
    assert_eq!(value.angle, 3.0);
    assert_eq!(
        registry
            .field_value("demo.rotator", &value, &FieldKey::new("angle").unwrap(),)
            .unwrap(),
        Value::F32(3.0)
    );
}

#[test]
fn validated_field_write_is_atomic() {
    let mut registry = TypeRegistry::new();
    registry.register(rotator_descriptor()).unwrap();
    registry.freeze().unwrap();
    let mut value = Rotator {
        speed: 2.0,
        angle: 0.0,
    };
    let speed = FieldKey::new("speed").unwrap();
    let angle = FieldKey::new("angle").unwrap();

    registry
        .set_field_value("demo.rotator", &mut value, &speed, &Value::F32(4.0))
        .unwrap();
    assert_eq!(value.speed, 4.0);

    assert!(matches!(
        registry.set_field_value(
            "demo.rotator",
            &mut value,
            &speed,
            &Value::String(String::from("fast")),
        ),
        Err(ReflectError::ValueKindMismatch { .. })
    ));
    assert_eq!(value.speed, 4.0);

    let error = registry
        .set_field_value("demo.rotator", &mut value, &speed, &Value::F32(-1.0))
        .unwrap_err();
    assert!(matches!(error, ReflectError::Validation(_)));
    assert_eq!(value.speed, 4.0);

    assert!(matches!(
        registry.set_field_value("demo.rotator", &mut value, &angle, &Value::F32(f32::NAN),),
        Err(ReflectError::Validation(_))
    ));
    assert_eq!(value.angle, 0.0);
}

#[test]
fn encode_rejects_getter_values_that_disagree_with_metadata() {
    let descriptor = TypeDescriptor::builder::<Rotator>(
        TypeKey::new("demo.bad-rotator").unwrap(),
        1,
        "Bad Rotator",
        || Rotator {
            speed: 1.0,
            angle: 0.0,
        },
    )
    .field(FieldRegistration::new(
        FieldKey::new("speed").unwrap(),
        FieldMetadata::new("Speed", FieldKind::F32),
        |_value: &Rotator| Value::String(String::from("wrong")),
        |_value: &mut Rotator, _field: &Value| Ok(()),
    ))
    .build()
    .unwrap();
    let mut registry = TypeRegistry::new();
    registry.register(descriptor).unwrap();
    registry.freeze().unwrap();

    assert!(matches!(
        registry.encode(&Rotator {
            speed: 1.0,
            angle: 0.0
        }),
        Err(ReflectError::ValueKindMismatch { .. })
    ));
}

#[test]
fn decode_rejects_schema_and_field_mismatches() {
    let mut registry = TypeRegistry::new();
    registry.register(rotator_descriptor()).unwrap();
    registry.freeze().unwrap();
    let valid = registry
        .encode(&Rotator {
            speed: 1.0,
            angle: 0.0,
        })
        .unwrap();

    let wrong_version = ReflectedValue::new(valid.type_key().clone(), 2, valid.fields().clone());
    assert!(matches!(
        registry.decode(&wrong_version),
        Err(ReflectError::SchemaVersionMismatch { .. })
    ));

    let mut missing = valid.fields().clone();
    assert_eq!(missing.remove("speed"), Some(Value::F32(1.0)));
    assert!(matches!(
        registry.decode(&ReflectedValue::new(valid.type_key().clone(), 1, missing)),
        Err(ReflectError::MissingField(_))
    ));

    let mut unexpected = valid.fields().clone();
    assert_eq!(
        unexpected.insert(FieldKey::new("extra").unwrap(), Value::Bool(true)),
        None,
    );
    assert!(matches!(
        registry.decode(&ReflectedValue::new(
            valid.type_key().clone(),
            1,
            unexpected
        )),
        Err(ReflectError::UnexpectedField(_))
    ));
}

#[test]
fn duplicate_and_frozen_registry_operations_fail() {
    let mut registry = TypeRegistry::new();
    registry.register(rotator_descriptor()).unwrap();
    assert!(matches!(
        registry.encode(&Rotator {
            speed: 1.0,
            angle: 0.0
        }),
        Err(ReflectError::RegistryNotFrozen)
    ));
    assert!(matches!(
        registry.register(rotator_descriptor()),
        Err(RegistryError::DuplicateTypeKey(_))
    ));
    registry.freeze().unwrap();
    assert_eq!(registry.freeze().unwrap_err(), RegistryError::AlreadyFrozen);
    assert!(matches!(
        registry.register(rotator_descriptor()),
        Err(RegistryError::Frozen)
    ));
    assert!(matches!(
        registry.encode(&String::from("unknown")),
        Err(ReflectError::UnknownRustType(_))
    ));

    let unknown = ReflectedValue::new(
        TypeKey::new("demo.unknown").unwrap(),
        1,
        FieldValues::default(),
    );
    assert!(matches!(
        registry.decode(&unknown),
        Err(ReflectError::UnknownTypeKey(_))
    ));

    let mut duplicate_type = TypeRegistry::new();
    duplicate_type.register(rotator_descriptor()).unwrap();
    assert!(matches!(
        duplicate_type.register(rotator_descriptor_with_key("demo.other-rotator")),
        Err(RegistryError::DuplicateRustType(_))
    ));
}

#[test]
fn field_mutation_preserves_unreflected_state_and_commits_only_after_validation() {
    let descriptor = TypeDescriptor::builder::<RevisionedRotator>(
        TypeKey::new("demo.revisioned-rotator").unwrap(),
        1,
        "Revisioned Rotator",
        || RevisionedRotator {
            speed: 1.0,
            hidden_revision: 0,
        },
    )
    .field(
        FieldRegistration::new(
            FieldKey::new("speed").unwrap(),
            FieldMetadata::new("Speed", FieldKind::F32),
            |value: &RevisionedRotator| Value::F32(value.speed),
            |value: &mut RevisionedRotator, field: &Value| match field {
                Value::F32(speed) => {
                    value.speed = *speed;
                    Ok(())
                }
                other => Err(ReflectError::value_kind("speed", "F32", other.kind())),
            },
        )
        .validator(|field: &Value| match field {
            Value::F32(speed) if *speed > 0.0 => Ok(()),
            _ => Err(ValidationIssue::field(
                FieldKey::new("speed").unwrap(),
                "speed must be positive",
            )),
        }),
    )
    .build()
    .unwrap();
    let mut registry = TypeRegistry::new();
    registry.register(descriptor).unwrap();
    registry.freeze().unwrap();
    let speed = FieldKey::new("speed").unwrap();
    let mut value = RevisionedRotator {
        speed: 2.0,
        hidden_revision: 42,
    };

    assert!(matches!(
        registry.set_field_value(
            "demo.revisioned-rotator",
            &mut value,
            &speed,
            &Value::F32(-1.0),
        ),
        Err(ReflectError::Validation(_))
    ));
    assert_eq!(value.speed, 2.0);
    assert_eq!(value.hidden_revision, 42);

    registry
        .set_field_value(
            "demo.revisioned-rotator",
            &mut value,
            &speed,
            &Value::F32(3.0),
        )
        .unwrap();
    assert_eq!(value.speed, 3.0);
    assert_eq!(value.hidden_revision, 42);
}

#[test]
fn enum_options_are_validated_by_the_shared_metadata_contract() {
    let descriptor = TypeDescriptor::builder::<ModeComponent>(
        TypeKey::new("demo.mode").unwrap(),
        1,
        "Mode",
        || ModeComponent {
            mode: String::from("Idle"),
        },
    )
    .field(FieldRegistration::new(
        FieldKey::new("mode").unwrap(),
        FieldMetadata::new(
            "Mode",
            FieldKind::Enum {
                options: vec![String::from("Idle"), String::from("Active")],
            },
        ),
        |value: &ModeComponent| Value::Enum(value.mode.clone()),
        |value: &mut ModeComponent, field: &Value| match field {
            Value::Enum(mode) => {
                value.mode.clone_from(mode);
                Ok(())
            }
            other => Err(ReflectError::value_kind("mode", "Enum", other.kind())),
        },
    ))
    .build()
    .unwrap();
    let mut registry = TypeRegistry::new();
    registry.register(descriptor).unwrap();
    registry.freeze().unwrap();
    let mode = FieldKey::new("mode").unwrap();
    let mut value = ModeComponent {
        mode: String::from("Idle"),
    };

    assert!(matches!(
        registry.validate(&ModeComponent {
            mode: String::from("Unknown"),
        }),
        Err(ReflectError::Validation(_))
    ));
    assert!(matches!(
        registry.set_field_value(
            "demo.mode",
            &mut value,
            &mode,
            &Value::Enum(String::from("Unknown")),
        ),
        Err(ReflectError::Validation(_))
    ));
    assert_eq!(value.mode, "Idle");

    let mut fields = FieldValues::default();
    assert_eq!(
        fields.insert(mode, Value::Enum(String::from("Unknown"))),
        None
    );
    assert!(matches!(
        registry.decode(&ReflectedValue::new(
            TypeKey::new("demo.mode").unwrap(),
            1,
            fields,
        )),
        Err(ReflectError::Validation(_))
    ));
}

#[test]
fn empty_component_validation_error_still_fails_closed() {
    let descriptor = TypeDescriptor::builder::<Rotator>(
        TypeKey::new("demo.empty-validation-error").unwrap(),
        1,
        "Empty Validation Error",
        || Rotator {
            speed: 1.0,
            angle: 0.0,
        },
    )
    .validator(|_value: &Rotator| Err(ValidationErrors::new(Vec::new())))
    .build()
    .unwrap();
    let mut registry = TypeRegistry::new();
    registry.register(descriptor).unwrap();
    registry.freeze().unwrap();

    assert!(matches!(
        registry.validate(&Rotator {
            speed: 1.0,
            angle: 0.0,
        }),
        Err(ReflectError::Validation(errors)) if errors.is_empty()
    ));
}

#[test]
fn dependency_neutral_references_keep_only_semantic_and_string_payload() {
    let entity = Value::Reference(String::from("entity:root"));
    let asset = Value::Reference(String::from("asset:mesh/cube"));
    let entity_metadata =
        FieldMetadata::new("Parent", FieldKind::Reference(ReferenceSemantic::Entity));
    let asset_metadata = FieldMetadata::new(
        "Mesh",
        FieldKind::Reference(ReferenceSemantic::Asset {
            asset_type: TypeKey::new("asset.mesh").unwrap(),
        }),
    );

    assert_eq!(entity.kind(), sge_reflect::ValueKind::Reference);
    assert_eq!(asset.kind(), sge_reflect::ValueKind::Reference);
    assert!(matches!(
        entity_metadata.kind(),
        FieldKind::Reference(ReferenceSemantic::Entity)
    ));
    assert!(matches!(
        asset_metadata.kind(),
        FieldKind::Reference(ReferenceSemantic::Asset { asset_type })
            if asset_type.as_str() == "asset.mesh"
    ));
}

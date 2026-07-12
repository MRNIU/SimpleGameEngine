// Copyright The SimpleGameEngine Contributors
//
//! `sge-reflect` public contract tests.

use sge_reflect::{
    DescriptorError, FieldKey, FieldKind, FieldMetadata, FieldRegistration, FieldValues, KeyError,
    ReferenceSemantic, ReferenceValue, ReflectError, ReflectedValue, RegistryError, TypeDescriptor,
    TypeKey, TypeRegistry, ValidationErrors, ValidationIssue, Value,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundReference(String);

impl ReferenceValue for BoundReference {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Ok(ReferenceSemantic::Entity)
    }

    fn to_reference(&self) -> String {
        self.0.clone()
    }

    fn from_reference(value: &str) -> Result<Self, String> {
        (!value.is_empty())
            .then(|| Self(value.to_owned()))
            .ok_or_else(|| "reference cannot be empty".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceComponent {
    target: BoundReference,
}

#[derive(Debug, Clone)]
struct InvalidSemanticReference;

impl ReferenceValue for InvalidSemanticReference {
    fn semantic() -> Result<ReferenceSemantic, KeyError> {
        Err(KeyError::Empty)
    }

    fn to_reference(&self) -> String {
        String::new()
    }

    fn from_reference(_value: &str) -> Result<Self, String> {
        Ok(Self)
    }
}

#[derive(Debug, Clone)]
struct InvalidSemanticComponent {
    target: InvalidSemanticReference,
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

fn reference_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<ReferenceComponent>(
        TypeKey::new("demo.reference").unwrap(),
        1,
        "Reference",
        || ReferenceComponent {
            target: BoundReference(String::from("entity:default")),
        },
    )
    .field(
        FieldRegistration::reference(
            FieldKey::new("target").unwrap(),
            "Target",
            |value: &ReferenceComponent| &value.target,
            |value: &mut ReferenceComponent, target| value.target = target,
        )
        .unwrap(),
    )
    .build()
    .unwrap()
}

#[test]
fn typed_reference_roundtrips() {
    let mut registry = TypeRegistry::new();
    registry.register(reference_descriptor()).unwrap();
    registry.freeze().unwrap();
    let value = ReferenceComponent {
        target: BoundReference(String::from("entity:root")),
    };

    let encoded = registry.encode(&value).unwrap();
    assert_eq!(
        encoded.fields().get("target"),
        Some(&Value::Reference(String::from("entity:root")))
    );
    assert_eq!(
        *registry
            .decode(&encoded)
            .unwrap()
            .downcast::<ReferenceComponent>()
            .unwrap(),
        value
    );
}

#[test]
fn typed_reference_rejects_invalid_payload() {
    let mut registry = TypeRegistry::new();
    registry.register(reference_descriptor()).unwrap();
    registry.freeze().unwrap();
    let mut fields = FieldValues::default();
    assert_eq!(
        fields.insert(
            FieldKey::new("target").unwrap(),
            Value::Reference(String::new()),
        ),
        None
    );

    assert!(matches!(
        registry.decode(&ReflectedValue::new(
            TypeKey::new("demo.reference").unwrap(),
            1,
            fields,
        )),
        Err(ReflectError::InvalidReferencePayload { value, reason })
            if value.is_empty() && reason == "reference cannot be empty"
    ));
}

#[test]
fn invalid_reference_semantic_is_a_descriptor_error() {
    assert!(matches!(
        FieldRegistration::reference(
            FieldKey::new("target").unwrap(),
            "Target",
            |value: &InvalidSemanticComponent| &value.target,
            |value: &mut InvalidSemanticComponent, target| value.target = target,
        ),
        Err(DescriptorError::InvalidReferenceSemantic(KeyError::Empty))
    ));
}

#[test]
fn unbound_reference_metadata_is_rejected() {
    let descriptor = TypeDescriptor::builder::<ReferenceComponent>(
        TypeKey::new("demo.unbound-reference").unwrap(),
        1,
        "Unbound Reference",
        || ReferenceComponent {
            target: BoundReference(String::from("entity:default")),
        },
    )
    .field(FieldRegistration::new(
        FieldKey::new("target").unwrap(),
        FieldMetadata::new("Target", FieldKind::Reference(ReferenceSemantic::Entity)),
        |value: &ReferenceComponent| Value::Reference(value.target.to_reference()),
        |value: &mut ReferenceComponent, field: &Value| match field {
            Value::Reference(target) => {
                value.target = BoundReference(target.clone());
                Ok(())
            }
            other => Err(ReflectError::value_kind(
                "target",
                "Reference",
                other.kind(),
            )),
        },
    ))
    .build();

    assert!(matches!(
        descriptor,
        Err(DescriptorError::UnboundReferenceField(field)) if field.as_str() == "target"
    ));
}

#[test]
fn scene_saveable_is_opt_in() {
    let default = TypeDescriptor::builder::<Rotator>(
        TypeKey::new("demo.non-saveable").unwrap(),
        1,
        "Non-saveable",
        || Rotator {
            speed: 1.0,
            angle: 0.0,
        },
    )
    .build()
    .unwrap();
    let saveable = TypeDescriptor::builder::<Rotator>(
        TypeKey::new("demo.saveable").unwrap(),
        1,
        "Saveable",
        || Rotator {
            speed: 1.0,
            angle: 0.0,
        },
    )
    .scene_saveable()
    .build()
    .unwrap();

    assert!(!default.scene_saveable());
    assert!(saveable.scene_saveable());
}

#[test]
fn registry_descriptors_are_type_key_sorted() {
    let mut registry = TypeRegistry::new();
    registry
        .register(
            TypeDescriptor::builder::<bool>(TypeKey::new("demo.zeta").unwrap(), 1, "Zeta", || {
                false
            })
            .build()
            .unwrap(),
        )
        .unwrap();
    registry
        .register(
            TypeDescriptor::builder::<String>(
                TypeKey::new("demo.alpha").unwrap(),
                1,
                "Alpha",
                String::new,
            )
            .build()
            .unwrap(),
        )
        .unwrap();
    registry
        .register(
            TypeDescriptor::builder::<u32>(TypeKey::new("demo.middle").unwrap(), 1, "Middle", || 0)
                .build()
                .unwrap(),
        )
        .unwrap();
    registry.freeze().unwrap();

    assert_eq!(
        registry
            .descriptors()
            .map(|descriptor| descriptor.type_key().as_str())
            .collect::<Vec<_>>(),
        ["demo.alpha", "demo.middle", "demo.zeta"]
    );
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

// Copyright The SimpleGameEngine Contributors

use sge_reflect::{
    FieldKey, FieldKind, FieldMetadata, FieldRegistration, ReflectError, TypeDescriptor, TypeKey,
    ValidationErrors, ValidationIssue, Value,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotator {
    radians_per_second: f32,
}

impl Rotator {
    #[must_use]
    pub const fn new(radians_per_second: f32) -> Self {
        Self { radians_per_second }
    }

    #[must_use]
    pub const fn radians_per_second(self) -> f32 {
        self.radians_per_second
    }
}

impl Default for Rotator {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerController {
    movement_speed: f32,
}

impl PlayerController {
    #[must_use]
    pub const fn new(movement_speed: f32) -> Self {
        Self { movement_speed }
    }

    #[must_use]
    pub const fn movement_speed(self) -> f32 {
        self.movement_speed
    }
}

impl Default for PlayerController {
    fn default() -> Self {
        Self::new(3.0)
    }
}

pub(crate) fn rotator_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Rotator>(key("demo.rotator"), 1, "Rotator", Rotator::default)
        .field(FieldRegistration::new(
            field("radians_per_second"),
            FieldMetadata::new("Radians Per Second", FieldKind::F32),
            |value: &Rotator| Value::F32(value.radians_per_second),
            |value: &mut Rotator, field| {
                value.radians_per_second = number(field, "radians_per_second")?;
                Ok(())
            },
        ))
        .validator(|value| finite("Rotator radians_per_second", value.radians_per_second))
        .scene_saveable()
        .build()
        .expect("static Rotator descriptor must be valid")
}

pub(crate) fn player_controller_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<PlayerController>(
        key("demo.player_controller"),
        1,
        "Player Controller",
        PlayerController::default,
    )
    .field(FieldRegistration::new(
        field("movement_speed"),
        FieldMetadata::new("Movement Speed", FieldKind::F32),
        |value: &PlayerController| Value::F32(value.movement_speed),
        |value: &mut PlayerController, field| {
            value.movement_speed = number(field, "movement_speed")?;
            Ok(())
        },
    ))
    .validator(|value| {
        finite("PlayerController movement_speed", value.movement_speed)?;
        if value.movement_speed > 0.0 {
            Ok(())
        } else {
            invalid("PlayerController movement_speed must be positive")
        }
    })
    .scene_saveable()
    .build()
    .expect("static PlayerController descriptor must be valid")
}

fn number(value: &Value, field: &str) -> Result<f32, ReflectError> {
    match value {
        Value::F32(value) => Ok(*value),
        other => Err(ReflectError::value_kind(field, "F32", other.kind())),
    }
}

fn finite(name: &str, value: f32) -> Result<(), ValidationErrors> {
    if value.is_finite() {
        Ok(())
    } else {
        invalid(format!("{name} must be finite"))
    }
}

fn invalid(message: impl Into<String>) -> Result<(), ValidationErrors> {
    Err(ValidationErrors::one(ValidationIssue::component(message)))
}

fn key(value: &'static str) -> TypeKey {
    TypeKey::new(value).expect("demo component type key must be valid")
}

fn field(value: &'static str) -> FieldKey {
    FieldKey::new(value).expect("demo component field key must be valid")
}

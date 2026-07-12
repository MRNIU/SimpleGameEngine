// Copyright The SimpleGameEngine Contributors
//
//! Static game library composition through the Core Kernel only.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use sge_app::{
    EngineApp, EngineBuildError, FixedTime, GameDescriptor, Plugin, RegistrationError,
    ScheduleLabel, System,
};
use sge_input::{Button, InputFrame, KeyCode};
use sge_reflect::{
    FieldKey, FieldKind, FieldMetadata, FieldRegistration, ReflectError, TypeDescriptor, TypeKey,
    ValidationErrors, ValidationIssue, Value,
};

#[derive(Debug, Clone, PartialEq)]
struct Rotator {
    speed: f32,
    angle: f32,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StageCounts {
    startup: u32,
    fixed: u32,
    update: u32,
    post_update: u32,
    jump_pressed: bool,
}

struct HeadlessGamePlugin;

impl Plugin for HeadlessGamePlugin {
    fn build(&self, app: &mut EngineApp) -> Result<(), RegistrationError> {
        app.register_reflected_component::<Rotator>(rotator_descriptor())?;
        app.insert_resource(StageCounts::default())?;
        app.add_system(ScheduleLabel::Startup, startup_system())?;
        app.add_system(ScheduleLabel::FixedUpdate, rotate_system())?;
        app.add_system(ScheduleLabel::Update, update_system())?;
        app.add_system(ScheduleLabel::PostUpdate, post_update_system())?;
        Ok(())
    }
}

fn rotator_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<Rotator>(TypeKey::new("demo.rotator").unwrap(), 1, "Rotator", || {
        Rotator {
            speed: 1.0,
            angle: 0.0,
        }
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
        .validator(|value: &Value| match value {
            Value::F32(speed) if *speed > 0.0 => Ok(()),
            _ => Err(ValidationIssue::field(
                FieldKey::new("speed").unwrap(),
                "speed must be positive",
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

fn startup_system() -> System {
    let mut builder = System::builder();
    let rotators = builder.component::<Rotator>();
    let counts = builder.resource::<StageCounts>();
    builder.build(move |context| {
        let entity = context.spawn();
        let _ = context.insert(
            &rotators,
            entity,
            Rotator {
                speed: 2.0,
                angle: 0.0,
            },
        )?;
        context.resource_mut(&counts)?.startup += 1;
        Ok(())
    })
}

fn rotate_system() -> System {
    let mut builder = System::builder();
    let rotators = builder.component::<Rotator>();
    let fixed_time = builder.resource::<FixedTime>();
    let input = builder.resource::<InputFrame>();
    let counts = builder.resource::<StageCounts>();
    builder.build(move |context| {
        let step = context.resource(&fixed_time)?.step().as_secs_f32();
        let moving = context
            .resource(&input)?
            .is_held(Button::Key(KeyCode::KeyW));
        if moving {
            for (_, rotator) in context.query_mut(&rotators)? {
                rotator.angle += rotator.speed * step;
            }
        }
        context.resource_mut(&counts)?.fixed += 1;
        Ok(())
    })
}

fn update_system() -> System {
    let mut builder = System::builder();
    let input = builder.resource::<InputFrame>();
    let counts = builder.resource::<StageCounts>();
    builder.build(move |context| {
        let jump = context
            .resource(&input)?
            .is_pressed(Button::Key(KeyCode::Space));
        let counts = context.resource_mut(&counts)?;
        counts.update += 1;
        counts.jump_pressed = jump;
        Ok(())
    })
}

fn post_update_system() -> System {
    let mut builder = System::builder();
    let counts = builder.resource::<StageCounts>();
    builder.build(move |context| {
        context.resource_mut(&counts)?.post_update += 1;
        Ok(())
    })
}

fn create_game_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.set_fixed_step(Duration::from_millis(10))?;
    app.add_plugin(HeadlessGamePlugin)?;
    app.finish()?;
    Ok(app)
}

#[test]
fn same_descriptor_builds_and_advances_the_headless_game() {
    let descriptor = GameDescriptor::new("demo-game", create_game_app);
    let mut app = descriptor.create_app().unwrap();
    let mut input = InputFrame::new();
    input.hold(Button::Key(KeyCode::KeyW));
    input.press(Button::Key(KeyCode::Space));

    app.advance(Duration::from_millis(25), input).unwrap();

    let rotator = app.world().query::<Rotator>().next().unwrap().1;
    assert!((rotator.angle - 0.04).abs() < f32::EPSILON);
    assert_eq!(
        app.world().resource::<StageCounts>().unwrap(),
        &StageCounts {
            startup: 1,
            fixed: 2,
            update: 1,
            post_update: 1,
            jump_pressed: true,
        }
    );
    assert!(app.type_registry().descriptor("demo.rotator").is_some());
}

fn create_unfinished_app() -> Result<EngineApp, EngineBuildError> {
    Ok(EngineApp::new())
}

static FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

fn create_counted_app() -> Result<EngineApp, EngineBuildError> {
    FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    create_game_app()
}

#[test]
fn descriptor_rejects_invalid_id_and_unfinished_factory() {
    assert!(matches!(
        GameDescriptor::new("", create_game_app).create_app(),
        Err(EngineBuildError::InvalidGameId)
    ));
    assert!(matches!(
        GameDescriptor::new("unfinished", create_unfinished_app).create_app(),
        Err(EngineBuildError::FactoryReturnedUnfinishedApp)
    ));
}

#[test]
fn descriptor_validates_type_key_grammar_before_calling_factory()
-> Result<(), Box<dyn std::error::Error>> {
    FACTORY_CALLS.store(0, Ordering::SeqCst);
    for invalid in ["", "game/id", "game id", "游戏"] {
        assert!(matches!(
            GameDescriptor::new(invalid, create_counted_app).create_app(),
            Err(EngineBuildError::InvalidGameId)
        ));
        assert_eq!(FACTORY_CALLS.load(Ordering::SeqCst), 0);
    }

    let mut app = GameDescriptor::new("demo.game-1_ok", create_counted_app).create_app()?;
    assert_eq!(FACTORY_CALLS.load(Ordering::SeqCst), 1);
    assert!(app.world_initializer().is_ok());
    Ok(())
}

// Copyright The SimpleGameEngine Contributors
//
//! EngineApp lifecycle and schedule contract tests.

use std::time::Duration;

use sge_app::{
    AdvanceError, EngineApp, EngineBuildError, FixedTime, GameDescriptor, InitializationError,
    RegistrationError, ScheduleLabel, System, SystemError, Time,
};
use sge_input::{Button, InputFrame, KeyCode};
use sge_reflect::{RegistryError, TypeDescriptor, TypeKey};

#[derive(Debug, Default, PartialEq, Eq)]
struct StageLog(Vec<&'static str>);

#[derive(Debug, Default, PartialEq, Eq)]
struct InputSeen(bool);

#[derive(Clone)]
struct FirstReflected;

#[derive(Clone)]
struct SecondReflected;

struct MissingComponent;

struct MissingResource;

#[derive(Debug, PartialEq, Eq)]
struct InitialValue(u32);

fn first_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<FirstReflected>(
        TypeKey::new("test.shared-key").unwrap(),
        1,
        "First",
        || FirstReflected,
    )
    .build()
    .unwrap()
}

fn second_descriptor() -> TypeDescriptor {
    TypeDescriptor::builder::<SecondReflected>(
        TypeKey::new("test.shared-key").unwrap(),
        1,
        "Second",
        || SecondReflected,
    )
    .build()
    .unwrap()
}

fn startup_system() -> System {
    let mut builder = System::builder();
    let log = builder.resource::<StageLog>();
    builder.build(move |context| {
        context.resource_mut(&log)?.0.push("startup");
        Ok(())
    })
}

fn fixed_system() -> System {
    let mut builder = System::builder();
    let fixed_time = builder.resource::<FixedTime>();
    let log = builder.resource::<StageLog>();
    builder.build(move |context| {
        let tick = context.resource(&fixed_time)?.tick_index();
        context.resource_mut(&log)?.0.push(if tick == 1 {
            "fixed-1"
        } else if tick == 2 {
            "fixed-2"
        } else {
            "fixed-3"
        });
        Ok(())
    })
}

fn update_system() -> System {
    let mut builder = System::builder();
    let input = builder.resource::<InputFrame>();
    let input_seen = builder.resource::<InputSeen>();
    let log = builder.resource::<StageLog>();
    builder.build(move |context| {
        let pressed = context
            .resource(&input)?
            .is_pressed(Button::Key(KeyCode::Space));
        context.resource_mut(&input_seen)?.0 = pressed;
        context.resource_mut(&log)?.0.push("update");
        Ok(())
    })
}

fn second_update_system() -> System {
    let mut builder = System::builder();
    let log = builder.resource::<StageLog>();
    builder.build(move |context| {
        context.resource_mut(&log)?.0.push("update-second");
        Ok(())
    })
}

fn post_update_system() -> System {
    let mut builder = System::builder();
    let log = builder.resource::<StageLog>();
    builder.build(move |context| {
        context.resource_mut(&log)?.0.push("post");
        Ok(())
    })
}

fn ready_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.finish()?;
    Ok(app)
}

fn unfinished_app() -> Result<EngineApp, EngineBuildError> {
    Ok(EngineApp::new())
}

fn started_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = ready_app()?;
    app.advance(Duration::ZERO, InputFrame::new()).unwrap();
    Ok(app)
}

fn failed_started_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.add_system(
        ScheduleLabel::Update,
        System::builder().build(|_| Err(SystemError::MissingResource("forced failure"))),
    )?;
    app.finish()?;
    assert!(matches!(
        app.advance(Duration::ZERO, InputFrame::new()),
        Err(AdvanceError::System(_))
    ));
    Ok(app)
}

#[test]
fn advance_requires_finish_and_runs_fixed_stages_in_order() {
    let mut app = EngineApp::new();
    app.set_fixed_step(Duration::from_millis(10)).unwrap();
    app.insert_resource(StageLog::default()).unwrap();
    app.insert_resource(InputSeen::default()).unwrap();
    app.add_system(ScheduleLabel::Startup, startup_system())
        .unwrap();
    app.add_system(ScheduleLabel::FixedUpdate, fixed_system())
        .unwrap();
    app.add_system(ScheduleLabel::Update, update_system())
        .unwrap();
    app.add_system(ScheduleLabel::Update, second_update_system())
        .unwrap();
    app.add_system(ScheduleLabel::PostUpdate, post_update_system())
        .unwrap();

    assert_eq!(
        app.advance(Duration::from_millis(25), InputFrame::new())
            .unwrap_err(),
        AdvanceError::NotFinished
    );
    app.finish().unwrap();

    let mut first_input = InputFrame::new();
    first_input.press(Button::Key(KeyCode::Space));
    app.advance(Duration::from_millis(25), first_input).unwrap();
    assert_eq!(
        app.world().resource::<StageLog>().unwrap().0,
        vec![
            "startup",
            "fixed-1",
            "fixed-2",
            "update",
            "update-second",
            "post",
        ]
    );
    assert!(app.world().resource::<InputSeen>().unwrap().0);
    assert_eq!(app.world().resource::<Time>().unwrap().frame_index(), 1);

    app.advance(Duration::from_millis(5), InputFrame::new())
        .unwrap();
    assert_eq!(
        app.world().resource::<StageLog>().unwrap().0,
        vec![
            "startup",
            "fixed-1",
            "fixed-2",
            "update",
            "update-second",
            "post",
            "fixed-3",
            "update",
            "update-second",
            "post",
        ]
    );
    assert_eq!(app.world().resource::<FixedTime>().unwrap().tick_index(), 3);
    assert_eq!(app.world().resource::<Time>().unwrap().frame_index(), 2);
}

#[test]
fn finish_closes_all_registration_paths() {
    let mut app = EngineApp::new();
    app.finish().unwrap();

    assert!(app.register_component::<u32>().is_err());
    assert!(app.insert_resource(String::from("late")).is_err());
    assert!(
        app.add_system(ScheduleLabel::Update, System::builder().build(|_| Ok(())),)
            .is_err()
    );
    assert!(app.finish().is_err());
}

#[test]
fn zero_fixed_step_is_rejected() {
    let mut app = EngineApp::new();
    assert!(app.set_fixed_step(Duration::ZERO).is_err());
}

#[test]
fn finish_rejects_missing_system_requirements_before_startup() {
    let mut missing_component = EngineApp::new();
    let mut component_builder = System::builder();
    let _missing_component = component_builder.component::<MissingComponent>();
    missing_component
        .add_system(ScheduleLabel::Startup, component_builder.build(|_| Ok(())))
        .unwrap();
    assert!(matches!(
        missing_component.finish(),
        Err(RegistrationError::MissingSystemComponent(_))
    ));
    missing_component
        .register_component::<MissingComponent>()
        .unwrap();
    missing_component.finish().unwrap();

    let mut missing_resource = EngineApp::new();
    let mut resource_builder = System::builder();
    let _missing_resource = resource_builder.resource::<MissingResource>();
    missing_resource
        .add_system(ScheduleLabel::Startup, resource_builder.build(|_| Ok(())))
        .unwrap();
    assert!(matches!(
        missing_resource.finish(),
        Err(RegistrationError::MissingSystemResource(_))
    ));
    missing_resource.insert_resource(MissingResource).unwrap();
    missing_resource.finish().unwrap();
}

#[test]
fn access_tokens_cannot_cross_system_boundaries() {
    let mut app = EngineApp::new();
    app.insert_resource(StageLog::default()).unwrap();

    let mut issuing_builder = System::builder();
    let foreign_log = issuing_builder.resource::<StageLog>();
    let foreign_system = System::builder().build(move |context| {
        let _ = context.resource(&foreign_log)?;
        Ok(())
    });
    app.add_system(ScheduleLabel::Update, foreign_system)
        .unwrap();
    app.finish().unwrap();

    assert!(matches!(
        app.advance(Duration::ZERO, InputFrame::new()),
        Err(AdvanceError::System(SystemError::ForeignAccessToken(_)))
    ));
}

#[test]
fn reflected_component_registration_is_atomic() {
    let mut duplicate_key = EngineApp::new();
    duplicate_key
        .register_reflected_component::<FirstReflected>(first_descriptor())
        .unwrap();
    assert!(matches!(
        duplicate_key.register_reflected_component::<SecondReflected>(second_descriptor()),
        Err(sge_app::RegistrationError::Reflect(
            RegistryError::DuplicateTypeKey(_)
        ))
    ));
    assert!(
        !duplicate_key
            .world()
            .component_is_registered::<SecondReflected>()
    );
    duplicate_key.finish().unwrap();
    assert!(
        duplicate_key
            .type_registry()
            .descriptor_of::<FirstReflected>()
            .is_some()
    );
    assert!(
        duplicate_key
            .type_registry()
            .descriptor_of::<SecondReflected>()
            .is_none()
    );

    let mut duplicate_component = EngineApp::new();
    duplicate_component
        .register_component::<FirstReflected>()
        .unwrap();
    assert!(
        duplicate_component
            .register_reflected_component::<FirstReflected>(first_descriptor())
            .is_err()
    );
    duplicate_component.finish().unwrap();
    assert!(
        duplicate_component
            .type_registry()
            .descriptor_of::<FirstReflected>()
            .is_none()
    );

    let mut mismatched_type = EngineApp::new();
    assert!(matches!(
        mismatched_type.register_reflected_component::<FirstReflected>(second_descriptor()),
        Err(RegistrationError::ReflectedTypeMismatch)
    ));
    assert!(
        !mismatched_type
            .world()
            .component_is_registered::<FirstReflected>()
    );
    assert!(
        mismatched_type
            .type_registry()
            .descriptor_of::<FirstReflected>()
            .is_none()
    );
    assert!(
        mismatched_type
            .type_registry()
            .descriptor_of::<SecondReflected>()
            .is_none()
    );
    mismatched_type.finish().unwrap();
}

#[test]
fn system_failure_stops_the_app_without_replaying_startup() {
    let mut app = EngineApp::new();
    app.insert_resource(StageLog::default()).unwrap();

    let mut first_builder = System::builder();
    let first_log = first_builder.resource::<StageLog>();
    app.add_system(
        ScheduleLabel::Startup,
        first_builder.build(move |context| {
            context
                .resource_mut(&first_log)?
                .0
                .push("startup-before-error");
            Ok(())
        }),
    )
    .unwrap();
    app.add_system(
        ScheduleLabel::Startup,
        System::builder().build(|_| Err(SystemError::MissingResource("forced failure"))),
    )
    .unwrap();
    app.add_system(ScheduleLabel::Startup, startup_system())
        .unwrap();
    app.finish().unwrap();

    assert!(matches!(
        app.advance(Duration::ZERO, InputFrame::new()),
        Err(AdvanceError::System(SystemError::MissingResource(
            "forced failure"
        )))
    ));
    assert_eq!(
        app.world().resource::<StageLog>().unwrap().0,
        vec!["startup-before-error"]
    );
    assert_eq!(app.world().resource::<Time>().unwrap().frame_index(), 1);

    assert_eq!(
        app.advance(Duration::from_millis(10), InputFrame::new())
            .unwrap_err(),
        AdvanceError::Failed
    );
    assert_eq!(
        app.world().resource::<StageLog>().unwrap().0,
        vec!["startup-before-error"]
    );
    assert_eq!(app.world().resource::<Time>().unwrap().frame_index(), 1);
}

#[test]
fn game_descriptor_accepts_only_fresh_ready_apps() {
    assert!(GameDescriptor::new("ready", ready_app).create_app().is_ok());
    assert!(matches!(
        GameDescriptor::new("unfinished", unfinished_app).create_app(),
        Err(EngineBuildError::FactoryReturnedUnfinishedApp)
    ));
    assert!(matches!(
        GameDescriptor::new("started", started_app).create_app(),
        Err(EngineBuildError::FactoryReturnedStartedApp)
    ));
    assert!(matches!(
        GameDescriptor::new("failed-started", failed_started_app).create_app(),
        Err(EngineBuildError::FactoryReturnedStartedApp)
    ));
}

#[test]
fn initializer_rejects_configuring_app() {
    let mut app = EngineApp::new();

    assert!(matches!(
        app.world_initializer(),
        Err(InitializationError::NotFinished)
    ));
}

#[test]
fn initializer_populates_ready_app_then_releases_world_borrow()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.register_component::<InitialValue>()?;
    app.finish()?;

    let entity = {
        let mut initializer = app.world_initializer()?;
        assert!(initializer.component_is_registered(std::any::TypeId::of::<InitialValue>()));
        let entity = initializer.spawn();
        initializer.insert_erased(
            entity,
            std::any::TypeId::of::<InitialValue>(),
            Box::new(InitialValue(7)),
        )?;
        entity
    };

    assert_eq!(
        app.world().get::<InitialValue>(entity),
        Some(&InitialValue(7))
    );
    Ok(())
}

#[test]
fn initializer_rejects_running_app() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ready_app()?;
    app.advance(Duration::ZERO, InputFrame::new())?;

    assert!(matches!(
        app.world_initializer(),
        Err(InitializationError::AlreadyStarted)
    ));
    Ok(())
}

#[test]
fn initializer_reports_failed_before_already_started() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = EngineApp::new();
    app.add_system(
        ScheduleLabel::Update,
        System::builder().build(|_| Err(SystemError::MissingResource("forced failure"))),
    )?;
    app.finish()?;
    assert!(matches!(
        app.advance(Duration::ZERO, InputFrame::new()),
        Err(AdvanceError::System(_))
    ));

    assert!(matches!(
        app.world_initializer(),
        Err(InitializationError::Failed)
    ));
    Ok(())
}

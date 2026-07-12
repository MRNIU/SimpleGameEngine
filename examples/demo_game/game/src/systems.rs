// Copyright The SimpleGameEngine Contributors

use sge_app::{EngineApp, FixedTime, RegistrationError, ScheduleLabel, System, Time};
use sge_input::{Button, InputFrame, KeyCode};
use sge_math::{Quat, Transform, Vec3};

use crate::{PlayerController, Rotator};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GameRuntimeState {
    startup_runs: u64,
    fixed_updates: u64,
    updates: u64,
    post_updates: u64,
}

impl GameRuntimeState {
    #[must_use]
    pub const fn startup_runs(&self) -> u64 {
        self.startup_runs
    }

    #[must_use]
    pub const fn fixed_updates(&self) -> u64 {
        self.fixed_updates
    }

    #[must_use]
    pub const fn updates(&self) -> u64 {
        self.updates
    }

    #[must_use]
    pub const fn post_updates(&self) -> u64 {
        self.post_updates
    }
}

pub(crate) fn install(app: &mut EngineApp) -> Result<(), RegistrationError> {
    app.add_system(ScheduleLabel::Startup, startup_system())?;
    app.add_system(ScheduleLabel::FixedUpdate, controller_system())?;
    app.add_system(ScheduleLabel::Update, rotator_system())?;
    app.add_system(ScheduleLabel::PostUpdate, post_update_system())?;
    Ok(())
}

fn startup_system() -> System {
    let mut builder = System::builder();
    let state = builder.resource::<GameRuntimeState>();
    builder.build(move |context| {
        context.resource_mut(&state)?.startup_runs += 1;
        Ok(())
    })
}

fn controller_system() -> System {
    let mut builder = System::builder();
    let input = builder.resource::<InputFrame>();
    let fixed_time = builder.resource::<FixedTime>();
    let controllers = builder.component::<PlayerController>();
    let transforms = builder.component::<Transform>();
    let state = builder.resource::<GameRuntimeState>();
    builder.build(move |context| {
        let direction = movement_direction(context.resource(&input)?);
        let step = context.resource(&fixed_time)?.step().as_secs_f32();
        let speeds = context
            .query(&controllers)?
            .map(|(entity, controller)| (entity, controller.movement_speed()))
            .collect::<Vec<_>>();
        if direction != Vec3::ZERO {
            for (entity, transform) in context.query_mut(&transforms)? {
                if let Some((_, speed)) = speeds.iter().find(|(candidate, _)| *candidate == entity)
                {
                    let displacement = direction * *speed * step;
                    for (coordinate, delta) in transform
                        .translation
                        .iter_mut()
                        .zip(displacement.to_array())
                    {
                        *coordinate += delta;
                    }
                }
            }
        }
        context.resource_mut(&state)?.fixed_updates += 1;
        Ok(())
    })
}

fn rotator_system() -> System {
    let mut builder = System::builder();
    let time = builder.resource::<Time>();
    let rotators = builder.component::<Rotator>();
    let transforms = builder.component::<Transform>();
    let state = builder.resource::<GameRuntimeState>();
    builder.build(move |context| {
        let delta = context.resource(&time)?.delta().as_secs_f32();
        let speeds = context
            .query(&rotators)?
            .map(|(entity, rotator)| (entity, rotator.radians_per_second()))
            .collect::<Vec<_>>();
        for (entity, transform) in context.query_mut(&transforms)? {
            if let Some((_, speed)) = speeds.iter().find(|(candidate, _)| *candidate == entity) {
                transform.rotation = (Quat::from_array(transform.rotation)
                    * Quat::from_rotation_y(*speed * delta))
                .normalize()
                .to_array();
            }
        }
        context.resource_mut(&state)?.updates += 1;
        Ok(())
    })
}

fn post_update_system() -> System {
    let mut builder = System::builder();
    let transforms = builder.component::<Transform>();
    let state = builder.resource::<GameRuntimeState>();
    builder.build(move |context| {
        for (_, transform) in context.query_mut(&transforms)? {
            transform.rotation = Quat::from_array(transform.rotation).normalize().to_array();
        }
        context.resource_mut(&state)?.post_updates += 1;
        Ok(())
    })
}

fn movement_direction(input: &InputFrame) -> Vec3 {
    let mut direction = Vec3::ZERO;
    if input.is_held(Button::Key(KeyCode::KeyW)) {
        direction.z -= 1.0;
    }
    if input.is_held(Button::Key(KeyCode::KeyS)) {
        direction.z += 1.0;
    }
    if input.is_held(Button::Key(KeyCode::KeyA)) {
        direction.x -= 1.0;
    }
    if input.is_held(Button::Key(KeyCode::KeyD)) {
        direction.x += 1.0;
    }
    direction.normalize_or_zero()
}

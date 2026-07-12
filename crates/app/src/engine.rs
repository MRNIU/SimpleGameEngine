// Copyright The SimpleGameEngine Contributors

use std::{
    any::{TypeId, type_name},
    time::Duration,
};

use sge_ecs::{EcsError, World};
use sge_input::InputFrame;
use sge_reflect::{RegistryError, TypeDescriptor, TypeRegistry};

use crate::{
    Plugin, ScheduleLabel, System, SystemError,
    schedule::{RequirementKind, Schedules},
    time::{DEFAULT_FIXED_STEP, FixedTime, Time},
};

pub struct EngineApp {
    world: World,
    type_registry: TypeRegistry,
    schedules: Schedules,
    fixed_step: Duration,
    accumulator: Duration,
    startup_ran: bool,
    finished: bool,
    started: bool,
    failed: bool,
}

impl EngineApp {
    #[must_use]
    pub fn new() -> Self {
        let mut world = World::new();
        world
            .register_resource::<Time>()
            .expect("new World accepts Time registration");
        world
            .insert_resource(Time::new())
            .expect("Time was just registered");
        world
            .register_resource::<FixedTime>()
            .expect("new World accepts FixedTime registration");
        world
            .insert_resource(FixedTime::new(DEFAULT_FIXED_STEP))
            .expect("FixedTime was just registered");
        world
            .register_resource::<InputFrame>()
            .expect("new World accepts InputFrame registration");
        world
            .insert_resource(InputFrame::new())
            .expect("InputFrame was just registered");

        Self {
            world,
            type_registry: TypeRegistry::new(),
            schedules: Schedules::default(),
            fixed_step: DEFAULT_FIXED_STEP,
            accumulator: Duration::ZERO,
            startup_ran: false,
            finished: false,
            started: false,
            failed: false,
        }
    }

    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    #[must_use]
    pub const fn type_registry(&self) -> &TypeRegistry {
        &self.type_registry
    }

    #[must_use]
    pub(crate) const fn is_finished(&self) -> bool {
        self.finished
    }

    #[must_use]
    pub(crate) const fn is_started(&self) -> bool {
        self.started
    }

    pub fn set_fixed_step(&mut self, step: Duration) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        if step.is_zero() {
            return Err(RegistrationError::ZeroFixedStep);
        }

        let _ = self
            .world
            .insert_resource(FixedTime::new(step))
            .expect("FixedTime is registered by EngineApp");
        self.fixed_step = step;
        Ok(self)
    }

    pub fn register_component<T: 'static>(&mut self) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        self.world.register_component::<T>()?;
        Ok(self)
    }

    pub fn register_reflected_component<T: Clone + 'static>(
        &mut self,
        descriptor: TypeDescriptor,
    ) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        if descriptor.rust_type_id() != TypeId::of::<T>() {
            return Err(RegistrationError::ReflectedTypeMismatch);
        }
        if self.world.registration_is_finished() {
            return Err(EcsError::RegistrationFinished.into());
        }
        if self.world.component_is_registered::<T>() {
            return Err(EcsError::DuplicateComponentType(type_name::<T>()).into());
        }
        if self.type_registry.is_frozen() {
            return Err(RegistryError::Frozen.into());
        }
        if self
            .type_registry
            .descriptor(descriptor.type_key().as_str())
            .is_some()
        {
            return Err(RegistryError::DuplicateTypeKey(descriptor.type_key().clone()).into());
        }
        if self.type_registry.descriptor_of::<T>().is_some() {
            return Err(RegistryError::DuplicateRustType(descriptor.rust_type_name()).into());
        }

        self.world
            .register_component::<T>()
            .expect("component registration was exhaustively preflighted");
        self.type_registry
            .register(descriptor)
            .expect("Reflect registration was exhaustively preflighted");
        Ok(self)
    }

    pub fn insert_resource<R: 'static>(
        &mut self,
        resource: R,
    ) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        self.world.register_resource::<R>()?;
        let _ = self
            .world
            .insert_resource(resource)
            .expect("resource insertion follows successful registration");
        Ok(self)
    }

    pub fn add_system(
        &mut self,
        schedule: ScheduleLabel,
        system: System,
    ) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        self.schedules.add(schedule, system);
        Ok(self)
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) -> Result<&mut Self, RegistrationError> {
        self.ensure_open()?;
        plugin.build(self)?;
        Ok(self)
    }

    pub fn finish(&mut self) -> Result<(), RegistrationError> {
        self.ensure_open()?;
        for system in self.schedules.iter() {
            if let Some((kind, type_name)) = system.first_unsatisfied(&self.world) {
                return Err(match kind {
                    RequirementKind::Component => {
                        RegistrationError::MissingSystemComponent(type_name)
                    }
                    RequirementKind::Resource => {
                        RegistrationError::MissingSystemResource(type_name)
                    }
                });
            }
        }

        self.world.finish_registration();
        self.type_registry
            .freeze()
            .expect("open EngineApp owns an unfrozen Reflect registry");
        self.finished = true;
        Ok(())
    }

    /// Advances one presentation frame after [`Self::finish`].
    ///
    /// The advance is not transactional: changes made before a system error remain visible. Any
    /// system error puts the app into a terminal failed state and is returned unchanged for that
    /// call. Later calls return [`AdvanceError::Failed`] without advancing input, time, or systems.
    pub fn advance(&mut self, delta: Duration, input: InputFrame) -> Result<(), AdvanceError> {
        if !self.is_finished() {
            return Err(AdvanceError::NotFinished);
        }
        if self.failed {
            return Err(AdvanceError::Failed);
        }
        self.started = true;

        let result = (|| {
            let _ = self
                .world
                .insert_resource(input)
                .expect("InputFrame was registered by EngineApp");
            self.world
                .resource_mut::<Time>()
                .expect("Time exists")
                .advance(delta);
            if !self.startup_ran {
                self.schedules
                    .run(ScheduleLabel::Startup, &mut self.world)?;
                self.startup_ran = true;
            }
            self.accumulator = self.accumulator.saturating_add(delta);
            while self.accumulator >= self.fixed_step {
                self.accumulator -= self.fixed_step;
                self.world
                    .resource_mut::<FixedTime>()
                    .expect("FixedTime exists")
                    .advance();
                self.schedules
                    .run(ScheduleLabel::FixedUpdate, &mut self.world)?;
            }
            self.schedules.run(ScheduleLabel::Update, &mut self.world)?;
            self.schedules
                .run(ScheduleLabel::PostUpdate, &mut self.world)?;
            Ok(())
        })();

        if let Err(error) = result {
            self.failed = true;
            return Err(AdvanceError::System(error));
        }
        Ok(())
    }

    fn ensure_open(&self) -> Result<(), RegistrationError> {
        if self.finished {
            Err(RegistrationError::Closed)
        } else {
            Ok(())
        }
    }
}

impl Default for EngineApp {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("EngineApp registration is closed")]
    Closed,
    #[error("fixed step must be greater than zero")]
    ZeroFixedStep,
    #[error(transparent)]
    Ecs(#[from] sge_ecs::EcsError),
    #[error(transparent)]
    Reflect(#[from] sge_reflect::RegistryError),
    #[error("reflected descriptor Rust type does not match registration type")]
    ReflectedTypeMismatch,
    #[error("system requires an unregistered component type: {0}")]
    MissingSystemComponent(&'static str),
    #[error("system requires a missing resource: {0}")]
    MissingSystemResource(&'static str),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdvanceError {
    #[error("EngineApp must be finished before advance")]
    NotFinished,
    #[error("EngineApp stopped after a system failure")]
    Failed,
    #[error(transparent)]
    System(#[from] SystemError),
}

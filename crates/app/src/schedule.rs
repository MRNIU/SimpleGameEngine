// Copyright The SimpleGameEngine Contributors

use std::{marker::PhantomData, rc::Rc};

use sge_ecs::{Entity, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleLabel {
    Startup,
    FixedUpdate,
    Update,
    PostUpdate,
}

type SystemFn =
    Box<dyn for<'world> FnMut(&mut SystemContext<'world>) -> Result<(), SystemError> + 'static>;
type RequirementCheck = fn(&World) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequirementKind {
    Component,
    Resource,
}

pub(crate) struct SystemRequirement {
    kind: RequirementKind,
    type_name: &'static str,
    check: RequirementCheck,
}

pub struct ComponentAccess<T: 'static> {
    owner: Rc<()>,
    marker: PhantomData<fn() -> T>,
}

pub struct ResourceAccess<R: 'static> {
    owner: Rc<()>,
    marker: PhantomData<fn() -> R>,
}

pub struct SystemBuilder {
    owner: Rc<()>,
    requirements: Vec<SystemRequirement>,
}

impl SystemBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            owner: Rc::new(()),
            requirements: Vec::new(),
        }
    }

    pub fn component<T: 'static>(&mut self) -> ComponentAccess<T> {
        self.requirements.push(SystemRequirement {
            kind: RequirementKind::Component,
            type_name: std::any::type_name::<T>(),
            check: World::component_is_registered::<T>,
        });
        ComponentAccess {
            owner: Rc::clone(&self.owner),
            marker: PhantomData,
        }
    }

    pub fn resource<R: 'static>(&mut self) -> ResourceAccess<R> {
        self.requirements.push(SystemRequirement {
            kind: RequirementKind::Resource,
            type_name: std::any::type_name::<R>(),
            check: |world| world.resource::<R>().is_some(),
        });
        ResourceAccess {
            owner: Rc::clone(&self.owner),
            marker: PhantomData,
        }
    }

    pub fn build(
        self,
        run: impl for<'world> FnMut(&mut SystemContext<'world>) -> Result<(), SystemError> + 'static,
    ) -> System {
        System {
            owner: self.owner,
            run: Box::new(run),
            requirements: self.requirements,
        }
    }
}

impl Default for SystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct System {
    owner: Rc<()>,
    run: SystemFn,
    requirements: Vec<SystemRequirement>,
}

impl System {
    #[must_use]
    pub fn builder() -> SystemBuilder {
        SystemBuilder::new()
    }

    pub(crate) fn first_unsatisfied(
        &self,
        world: &World,
    ) -> Option<(RequirementKind, &'static str)> {
        self.requirements
            .iter()
            .find(|requirement| !(requirement.check)(world))
            .map(|requirement| (requirement.kind, requirement.type_name))
    }

    fn run(&mut self, world: &mut World) -> Result<(), SystemError> {
        let mut context = SystemContext {
            world,
            owner: &self.owner,
        };
        (self.run)(&mut context)
    }
}

pub struct SystemContext<'world> {
    world: &'world mut World,
    owner: &'world Rc<()>,
}

impl SystemContext<'_> {
    pub fn spawn(&mut self) -> Entity {
        self.world.spawn()
    }

    pub fn insert<T: 'static>(
        &mut self,
        access: &ComponentAccess<T>,
        entity: Entity,
        value: T,
    ) -> Result<Option<T>, SystemError> {
        self.check_component(access)?;
        self.world.insert(entity, value).map_err(SystemError::from)
    }

    pub fn query<T: 'static>(
        &self,
        access: &ComponentAccess<T>,
    ) -> Result<impl Iterator<Item = (Entity, &T)> + '_, SystemError> {
        self.check_component(access)?;
        Ok(self.world.query::<T>())
    }

    pub fn query_mut<T: 'static>(
        &mut self,
        access: &ComponentAccess<T>,
    ) -> Result<impl Iterator<Item = (Entity, &mut T)> + '_, SystemError> {
        self.check_component(access)?;
        Ok(self.world.query_mut::<T>())
    }

    pub fn resource<R: 'static>(&self, access: &ResourceAccess<R>) -> Result<&R, SystemError> {
        self.check_resource(access)?;
        self.world
            .resource::<R>()
            .ok_or(SystemError::MissingResource(std::any::type_name::<R>()))
    }

    pub fn resource_mut<R: 'static>(
        &mut self,
        access: &ResourceAccess<R>,
    ) -> Result<&mut R, SystemError> {
        self.check_resource(access)?;
        self.world
            .resource_mut::<R>()
            .ok_or(SystemError::MissingResource(std::any::type_name::<R>()))
    }

    fn check_component<T: 'static>(&self, access: &ComponentAccess<T>) -> Result<(), SystemError> {
        self.check_owner(&access.owner, std::any::type_name::<T>())?;
        if self.world.component_is_registered::<T>() {
            Ok(())
        } else {
            Err(SystemError::MissingComponent(std::any::type_name::<T>()))
        }
    }

    fn check_resource<R: 'static>(&self, access: &ResourceAccess<R>) -> Result<(), SystemError> {
        self.check_owner(&access.owner, std::any::type_name::<R>())
    }

    fn check_owner(
        &self,
        token_owner: &Rc<()>,
        type_name: &'static str,
    ) -> Result<(), SystemError> {
        if Rc::ptr_eq(self.owner, token_owner) {
            Ok(())
        } else {
            Err(SystemError::ForeignAccessToken(type_name))
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SystemError {
    #[error("component is not registered for system access: {0}")]
    MissingComponent(&'static str),
    #[error("resource is missing for system access: {0}")]
    MissingResource(&'static str),
    #[error("access token belongs to another system: {0}")]
    ForeignAccessToken(&'static str),
    #[error(transparent)]
    Ecs(#[from] sge_ecs::EcsError),
}

#[derive(Default)]
pub(crate) struct Schedules {
    startup: Vec<System>,
    fixed_update: Vec<System>,
    update: Vec<System>,
    post_update: Vec<System>,
}

impl Schedules {
    pub(crate) fn add(&mut self, label: ScheduleLabel, system: System) {
        self.systems_mut(label).push(system);
    }

    pub(crate) fn run(
        &mut self,
        label: ScheduleLabel,
        world: &mut World,
    ) -> Result<(), SystemError> {
        for system in self.systems_mut(label) {
            system.run(world)?;
        }
        Ok(())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &System> {
        self.startup
            .iter()
            .chain(&self.fixed_update)
            .chain(&self.update)
            .chain(&self.post_update)
    }

    fn systems_mut(&mut self, label: ScheduleLabel) -> &mut Vec<System> {
        match label {
            ScheduleLabel::Startup => &mut self.startup,
            ScheduleLabel::FixedUpdate => &mut self.fixed_update,
            ScheduleLabel::Update => &mut self.update,
            ScheduleLabel::PostUpdate => &mut self.post_update,
        }
    }
}

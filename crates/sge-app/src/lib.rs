// Copyright The SimpleGameEngine Contributors
//
//! 平台与 renderer 无关的 SimpleGameEngine runtime kernel。

mod engine;
mod game;
mod schedule;
mod time;

pub use engine::{AdvanceError, EngineApp, InitializationError, RegistrationError};
pub use game::{CreateAppFn, EngineBuildError, GameDescriptor, Plugin};
pub use schedule::{
    ComponentAccess, ResourceAccess, ScheduleLabel, System, SystemBuilder, SystemContext,
    SystemError,
};
pub use time::{DEFAULT_FIXED_STEP, FixedTime, Time};

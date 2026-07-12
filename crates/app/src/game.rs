// Copyright The SimpleGameEngine Contributors

use sge_reflect::TypeKey;

use crate::{EngineApp, RegistrationError};

pub trait Plugin {
    fn build(&self, app: &mut EngineApp) -> Result<(), RegistrationError>;
}

pub type CreateAppFn = fn() -> Result<EngineApp, EngineBuildError>;

#[derive(Clone, Copy)]
pub struct GameDescriptor {
    game_id: &'static str,
    create_app: CreateAppFn,
}

impl GameDescriptor {
    #[must_use]
    pub const fn new(game_id: &'static str, create_app: CreateAppFn) -> Self {
        Self {
            game_id,
            create_app,
        }
    }

    #[must_use]
    pub const fn game_id(&self) -> &'static str {
        self.game_id
    }

    /// Creates a fresh Ready app whose registration is finished and whose runtime has not started.
    pub fn create_app(&self) -> Result<EngineApp, EngineBuildError> {
        let _ = TypeKey::new(self.game_id).map_err(|_| EngineBuildError::InvalidGameId)?;
        let app = (self.create_app)()?;
        if !app.is_finished() {
            return Err(EngineBuildError::FactoryReturnedUnfinishedApp);
        }
        if app.is_started() {
            return Err(EngineBuildError::FactoryReturnedStartedApp);
        }
        Ok(app)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineBuildError {
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error("game id must use reflected TypeKey syntax")]
    InvalidGameId,
    #[error("game factory returned an unfinished EngineApp")]
    FactoryReturnedUnfinishedApp,
    #[error("game factory returned an EngineApp that has already started")]
    FactoryReturnedStartedApp,
}

// Copyright The SimpleGameEngine Contributors

mod components;
mod systems;

use sge_app::{EngineApp, EngineBuildError, GameDescriptor};
use sge_render::RenderPlugin;
use sge_scene::{
    Parent, SceneEntityId, SceneName, parent_descriptor, scene_entity_id_descriptor,
    scene_name_descriptor,
};

pub use components::{PlayerController, Rotator};
pub use systems::GameRuntimeState;

pub const GAME_ID: &str = "demo.game";
pub const GAME: GameDescriptor = GameDescriptor::new(GAME_ID, create_app);

fn create_app() -> Result<EngineApp, EngineBuildError> {
    let mut app = EngineApp::new();
    app.register_reflected_component::<SceneEntityId>(
        scene_entity_id_descriptor().expect("built-in scene identity descriptor must be valid"),
    )?;
    app.register_reflected_component::<Parent>(
        parent_descriptor().expect("built-in parent descriptor must be valid"),
    )?;
    app.register_reflected_component::<SceneName>(
        scene_name_descriptor().expect("built-in scene name descriptor must be valid"),
    )?;
    app.add_plugin(RenderPlugin)?;
    app.register_reflected_component::<Rotator>(components::rotator_descriptor())?;
    app.register_reflected_component::<PlayerController>(
        components::player_controller_descriptor(),
    )?;
    app.insert_resource(GameRuntimeState::default())?;
    systems::install(&mut app)?;
    app.finish()?;
    Ok(app)
}

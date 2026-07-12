// Copyright The SimpleGameEngine Contributors

use sge_app::{EngineApp, EngineBuildError, GameDescriptor};
use sge_render::RenderPlugin;
use sge_scene::{Parent, SceneEntityId, parent_descriptor, scene_entity_id_descriptor};

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
    app.add_plugin(RenderPlugin)?;
    app.finish()?;
    Ok(app)
}

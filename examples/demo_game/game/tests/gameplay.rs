// Copyright The SimpleGameEngine Contributors

use std::time::Duration;

use demo_game::{GameRuntimeState, PlayerController, Rotator};
use sge_input::{Button, InputFrame, KeyCode};
use sge_math::Transform;
use sge_scene::SceneEntityId;

#[test]
fn shared_game_descriptor_runs_all_stages_and_gameplay_input()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = demo_game::GAME.create_app()?;
    let entity = {
        let mut initializer = app.world_initializer()?;
        let entity = initializer.spawn();
        initializer.insert(
            entity,
            "70000000-0000-4000-8000-000000000001".parse::<SceneEntityId>()?,
        )?;
        initializer.insert(entity, Transform::identity())?;
        initializer.insert(entity, Rotator::new(2.0))?;
        initializer.insert(entity, PlayerController::new(6.0))?;
        entity
    };
    let mut input = InputFrame::new();
    input.hold(Button::Key(KeyCode::KeyW));

    app.advance(Duration::from_millis(20), input)?;

    let transform = app
        .world()
        .get::<Transform>(entity)
        .ok_or("missing Transform")?;
    assert!(transform.translation[2] < 0.0);
    assert_ne!(transform.rotation, Transform::identity().rotation);
    let state = app
        .world()
        .resource::<GameRuntimeState>()
        .ok_or("missing runtime state")?;
    assert_eq!(state.startup_runs(), 1);
    assert_eq!(state.fixed_updates(), 1);
    assert_eq!(state.updates(), 1);
    assert_eq!(state.post_updates(), 1);
    Ok(())
}

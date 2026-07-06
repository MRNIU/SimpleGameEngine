// Copyright The SimpleGameEngine Contributors
//
//! 引擎生命周期与调度胶水。

use ecs::World;
use input::InputState;
use render::{RenderScene, extract_render_scene};
use window::WindowConfig;

#[derive(Debug, Clone)]
pub struct Engine {
    world: World,
    input: InputState,
    window: WindowConfig,
    frame_index: u64,
}

impl Engine {
    #[must_use]
    pub fn new(world: World, window: WindowConfig) -> Self {
        Self {
            world,
            input: InputState::new(),
            window,
            frame_index: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        tracing::trace!(frame_index = self.frame_index, "engine tick");
    }

    #[must_use]
    pub fn render_scene(&self) -> RenderScene {
        extract_render_scene(&self.world)
    }

    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    #[must_use]
    pub fn input(&self) -> &InputState {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    #[must_use]
    pub fn window(&self) -> &WindowConfig {
        &self.window
    }

    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(World::new(), WindowConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    #[test]
    fn tick_advances_frame_index() {
        let mut engine = Engine::default();

        engine.tick();

        assert_eq!(engine.frame_index(), 1);
    }
}

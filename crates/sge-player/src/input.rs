// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeSet;

use sge_input::{Button, InputFrame, KeyCode as GameKeyCode, MouseButton as GameMouseButton};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub(crate) struct InputAccumulator {
    held: BTreeSet<Button>,
    pressed: BTreeSet<Button>,
    released: BTreeSet<Button>,
    cursor_position: Option<[f64; 2]>,
    pointer_delta: [f32; 2],
    wheel_delta: [f32; 2],
    focused: bool,
}

impl InputAccumulator {
    pub(crate) fn handle_window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            self.focused = *focused;
            if !focused {
                self.clear();
            }
            return;
        }
        if !self.focused {
            return;
        }
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(event.physical_key, event.state);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse(*button, *state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor([position.x, position.y]);
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_wheel(delta),
            _ => {}
        }
    }

    pub(crate) fn take_frame(&mut self) -> InputFrame {
        let mut frame = InputFrame::new();
        for button in &self.pressed {
            frame.press(*button);
        }
        for button in &self.released {
            frame.release(*button);
        }
        for button in &self.held {
            frame.hold(*button);
        }
        frame.set_pointer_delta(self.pointer_delta);
        frame.set_wheel_delta(self.wheel_delta);

        self.pressed.clear();
        self.released.clear();
        self.pointer_delta = [0.0; 2];
        self.wheel_delta = [0.0; 2];
        frame
    }

    fn handle_key(&mut self, key: PhysicalKey, state: ElementState) {
        let PhysicalKey::Code(key) = key else {
            return;
        };
        let button = match key {
            KeyCode::KeyW => Button::Key(GameKeyCode::KeyW),
            KeyCode::KeyA => Button::Key(GameKeyCode::KeyA),
            KeyCode::KeyS => Button::Key(GameKeyCode::KeyS),
            KeyCode::KeyD => Button::Key(GameKeyCode::KeyD),
            KeyCode::Space => Button::Key(GameKeyCode::Space),
            _ => return,
        };
        self.handle_button(button, state);
    }

    fn handle_mouse(&mut self, button: MouseButton, state: ElementState) {
        let button = match button {
            MouseButton::Left => Button::Mouse(GameMouseButton::Left),
            MouseButton::Right => Button::Mouse(GameMouseButton::Right),
            MouseButton::Middle => Button::Mouse(GameMouseButton::Middle),
            _ => return,
        };
        self.handle_button(button, state);
    }

    fn handle_button(&mut self, button: Button, state: ElementState) {
        match state {
            ElementState::Pressed if self.held.insert(button) => {
                self.pressed.insert(button);
            }
            ElementState::Released if self.held.remove(&button) => {
                self.released.insert(button);
            }
            ElementState::Pressed | ElementState::Released => {}
        }
    }

    fn handle_cursor(&mut self, position: [f64; 2]) {
        if let Some(previous) = self.cursor_position {
            self.pointer_delta[0] += (position[0] - previous[0]) as f32;
            self.pointer_delta[1] += (position[1] - previous[1]) as f32;
        }
        self.cursor_position = Some(position);
    }

    fn handle_wheel(&mut self, delta: &MouseScrollDelta) {
        let delta = match delta {
            MouseScrollDelta::LineDelta(x, y) => [*x, *y],
            MouseScrollDelta::PixelDelta(position) => [position.x as f32, position.y as f32],
        };
        self.wheel_delta[0] += delta[0];
        self.wheel_delta[1] += delta[1];
    }

    fn clear(&mut self) {
        self.held.clear();
        self.pressed.clear();
        self.released.clear();
        self.cursor_position = None;
        self.pointer_delta = [0.0; 2];
        self.wheel_delta = [0.0; 2];
    }
}

impl Default for InputAccumulator {
    fn default() -> Self {
        Self {
            held: BTreeSet::new(),
            pressed: BTreeSet::new(),
            released: BTreeSet::new(),
            cursor_position: None,
            pointer_delta: [0.0; 2],
            wheel_delta: [0.0; 2],
            focused: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use sge_input::{Button, KeyCode as GameKeyCode, MouseButton as GameMouseButton};
    use winit::{
        dpi::PhysicalPosition,
        event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
        keyboard::{KeyCode, PhysicalKey},
    };

    use super::InputAccumulator;

    #[test]
    fn maps_physical_keys_and_mouse_buttons_with_frame_edges() {
        let mut input = InputAccumulator::default();
        let keys = [
            (KeyCode::KeyW, GameKeyCode::KeyW),
            (KeyCode::KeyA, GameKeyCode::KeyA),
            (KeyCode::KeyS, GameKeyCode::KeyS),
            (KeyCode::KeyD, GameKeyCode::KeyD),
            (KeyCode::Space, GameKeyCode::Space),
        ];
        let mouse_buttons = [
            (MouseButton::Left, GameMouseButton::Left),
            (MouseButton::Right, GameMouseButton::Right),
            (MouseButton::Middle, GameMouseButton::Middle),
        ];

        for (key, _) in keys {
            input.handle_key(PhysicalKey::Code(key), ElementState::Pressed);
        }
        input.handle_key(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed);
        for (button, _) in mouse_buttons {
            input.handle_mouse(button, ElementState::Pressed);
        }
        let first = input.take_frame();
        for (_, key) in keys {
            let button = Button::Key(key);
            assert!(first.is_held(button));
            assert!(first.is_pressed(button));
        }
        for (_, mouse) in mouse_buttons {
            let button = Button::Mouse(mouse);
            assert!(first.is_held(button));
            assert!(first.is_pressed(button));
        }

        let second = input.take_frame();
        let key = Button::Key(GameKeyCode::KeyW);
        let mouse = Button::Mouse(GameMouseButton::Right);
        assert!(second.is_held(key));
        assert!(!second.is_pressed(key));

        input.handle_key(PhysicalKey::Code(KeyCode::KeyW), ElementState::Released);
        input.handle_mouse(MouseButton::Right, ElementState::Released);
        let third = input.take_frame();
        assert!(!third.is_held(key));
        assert!(third.is_released(key));
        assert!(third.is_released(mouse));
    }

    #[test]
    fn accumulates_pointer_and_wheel_deltas_for_one_frame() {
        let mut input = InputAccumulator::default();
        input.handle_cursor([10.0, 20.0]);
        input.handle_cursor([13.0, 18.0]);
        input.handle_cursor([14.5, 22.0]);
        input.handle_wheel(&MouseScrollDelta::LineDelta(1.0, -2.0));
        input.handle_wheel(&MouseScrollDelta::PixelDelta(PhysicalPosition::new(
            3.0, 4.5,
        )));

        let frame = input.take_frame();
        assert_eq!(frame.pointer_delta(), [4.5, 2.0]);
        assert_eq!(frame.wheel_delta(), [4.0, 2.5]);

        let next = input.take_frame();
        assert_eq!(next.pointer_delta(), [0.0, 0.0]);
        assert_eq!(next.wheel_delta(), [0.0, 0.0]);
    }

    #[test]
    fn losing_focus_clears_state_without_synthetic_release() {
        let mut input = InputAccumulator::default();
        let key = Button::Key(GameKeyCode::Space);
        input.handle_key(PhysicalKey::Code(KeyCode::Space), ElementState::Pressed);
        input.handle_cursor([1.0, 1.0]);
        input.handle_cursor([2.0, 3.0]);

        input.handle_window_event(&WindowEvent::Focused(false));
        assert!(!input.focused);
        let frame = input.take_frame();
        assert!(!frame.is_held(key));
        assert!(!frame.is_pressed(key));
        assert!(!frame.is_released(key));
        assert_eq!(frame.pointer_delta(), [0.0, 0.0]);

        input.handle_window_event(&WindowEvent::Focused(true));
        assert!(input.focused);
    }
}

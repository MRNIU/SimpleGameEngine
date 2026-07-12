// Copyright The SimpleGameEngine Contributors
//
//! 平台无关的逐帧 gameplay input snapshot。

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCode {
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    Key(KeyCode),
    Mouse(MouseButton),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputFrame {
    held: BTreeSet<Button>,
    pressed: BTreeSet<Button>,
    released: BTreeSet<Button>,
    pointer_delta: [f32; 2],
    wheel_delta: [f32; 2],
}

impl InputFrame {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hold(&mut self, button: Button) {
        self.held.insert(button);
    }

    pub fn press(&mut self, button: Button) {
        self.held.insert(button);
        self.pressed.insert(button);
    }

    pub fn release(&mut self, button: Button) {
        self.held.remove(&button);
        self.released.insert(button);
    }

    #[must_use]
    pub fn is_held(&self, button: Button) -> bool {
        self.held.contains(&button)
    }

    #[must_use]
    pub fn is_pressed(&self, button: Button) -> bool {
        self.pressed.contains(&button)
    }

    #[must_use]
    pub fn is_released(&self, button: Button) -> bool {
        self.released.contains(&button)
    }

    pub fn set_pointer_delta(&mut self, delta: [f32; 2]) {
        self.pointer_delta = delta;
    }

    #[must_use]
    pub const fn pointer_delta(&self) -> [f32; 2] {
        self.pointer_delta
    }

    pub fn set_wheel_delta(&mut self, delta: [f32; 2]) {
        self.wheel_delta = delta;
    }

    #[must_use]
    pub const fn wheel_delta(&self) -> [f32; 2] {
        self.wheel_delta
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, InputFrame, KeyCode, MouseButton};

    #[test]
    fn press_and_release_preserve_same_frame_edges() {
        let jump = Button::Key(KeyCode::Space);
        let mut input = InputFrame::new();

        input.press(jump);
        input.release(jump);

        assert!(!input.is_held(jump));
        assert!(input.is_pressed(jump));
        assert!(input.is_released(jump));
    }

    #[test]
    fn holds_buttons_and_keeps_pointer_and_wheel_deltas() {
        let primary = Button::Mouse(MouseButton::Left);
        let mut input = InputFrame::new();
        input.hold(primary);
        input.set_pointer_delta([3.0, -2.0]);
        input.set_wheel_delta([0.0, 1.5]);

        assert!(input.is_held(primary));
        assert!(!input.is_pressed(primary));
        assert_eq!(input.pointer_delta(), [3.0, -2.0]);
        assert_eq!(input.wheel_delta(), [0.0, 1.5]);
    }
}

// Copyright The SimpleGameEngine Contributors
//
//! 键盘与鼠标输入状态。

use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    pressed_keys: BTreeSet<String>,
    pub mouse_position: Option<[f32; 2]>,
}

impl InputState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press_key(&mut self, key: impl Into<String>) {
        self.pressed_keys.insert(key.into());
    }

    pub fn release_key(&mut self, key: impl AsRef<str>) {
        self.pressed_keys.remove(key.as_ref());
    }

    #[must_use]
    pub fn is_key_pressed(&self, key: impl AsRef<str>) -> bool {
        self.pressed_keys.contains(key.as_ref())
    }

    pub fn set_mouse_position(&mut self, position: [f32; 2]) {
        self.mouse_position = Some(position);
    }
}

#[cfg(test)]
mod tests {
    use super::InputState;

    #[test]
    fn tracks_pressed_keys() {
        let mut input = InputState::new();

        input.press_key("Space");
        assert!(input.is_key_pressed("Space"));

        input.release_key("Space");
        assert!(!input.is_key_pressed("Space"));
    }
}

// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeSet;

use eframe::egui;
use sge_input::{Button, InputFrame, KeyCode, MouseButton};

#[derive(Default)]
pub(crate) struct EditorInputAccumulator {
    held: BTreeSet<Button>,
    pressed: BTreeSet<Button>,
    released: BTreeSet<Button>,
    pointer_delta: [f32; 2],
    wheel_delta: [f32; 2],
    keyboard_capture: bool,
    pointer_capture: bool,
}

impl EditorInputAccumulator {
    pub(crate) fn handle_events(
        &mut self,
        events: &[egui::Event],
        keyboard_capture: bool,
        pointer_capture: bool,
    ) {
        self.set_keyboard_capture(keyboard_capture);
        self.set_pointer_capture(pointer_capture);
        for event in events {
            match event {
                egui::Event::Key {
                    physical_key,
                    pressed,
                    repeat,
                    ..
                } if keyboard_capture => {
                    if let Some(button) = physical_key.and_then(key_button) {
                        self.button(button, *pressed, *repeat);
                    }
                }
                egui::Event::PointerButton {
                    button, pressed, ..
                } if pointer_capture => {
                    if let Some(button) = pointer_button(*button) {
                        self.button(button, *pressed, false);
                    }
                }
                egui::Event::MouseMoved(delta) if pointer_capture => {
                    self.add_pointer_delta([delta.x, delta.y]);
                }
                egui::Event::MouseWheel { delta, .. } if pointer_capture => {
                    self.add_wheel_delta([delta.x, delta.y]);
                }
                egui::Event::WindowFocused(false) => self.reset(),
                egui::Event::PointerGone => self.set_pointer_capture(false),
                _ => {}
            }
        }
    }

    pub(crate) fn set_keyboard_capture(&mut self, captured: bool) {
        if self.keyboard_capture && !captured {
            self.clear_where(|button| matches!(button, Button::Key(_)));
        }
        self.keyboard_capture = captured;
    }

    pub(crate) fn set_pointer_capture(&mut self, captured: bool) {
        if self.pointer_capture && !captured {
            self.clear_where(|button| matches!(button, Button::Mouse(_)));
            self.pointer_delta = [0.0; 2];
            self.wheel_delta = [0.0; 2];
        }
        self.pointer_capture = captured;
    }

    pub(crate) fn button(&mut self, button: Button, pressed: bool, repeat: bool) {
        let captured = match button {
            Button::Key(_) => self.keyboard_capture,
            Button::Mouse(_) => self.pointer_capture,
        };
        if !captured {
            return;
        }
        if pressed {
            if self.held.insert(button) && !repeat {
                self.pressed.insert(button);
            }
        } else if self.held.remove(&button) {
            self.released.insert(button);
        }
    }

    pub(crate) fn add_pointer_delta(&mut self, delta: [f32; 2]) {
        if self.pointer_capture {
            add(&mut self.pointer_delta, delta);
        }
    }

    pub(crate) fn add_wheel_delta(&mut self, delta: [f32; 2]) {
        if self.pointer_capture {
            add(&mut self.wheel_delta, delta);
        }
    }

    pub(crate) fn reset(&mut self) {
        self.held.clear();
        self.pressed.clear();
        self.released.clear();
        self.pointer_delta = [0.0; 2];
        self.wheel_delta = [0.0; 2];
        self.keyboard_capture = false;
        self.pointer_capture = false;
    }

    pub(crate) fn take_frame(&mut self) -> InputFrame {
        let mut frame = InputFrame::new();
        for button in &self.held {
            frame.hold(*button);
        }
        for button in &self.pressed {
            frame.press(*button);
        }
        for button in &self.released {
            frame.release(*button);
        }
        frame.set_pointer_delta(self.pointer_delta);
        frame.set_wheel_delta(self.wheel_delta);
        self.pressed.clear();
        self.released.clear();
        self.pointer_delta = [0.0; 2];
        self.wheel_delta = [0.0; 2];
        frame
    }

    fn clear_where(&mut self, predicate: impl Fn(&Button) -> bool) {
        self.held.retain(|button| !predicate(button));
        self.pressed.retain(|button| !predicate(button));
        self.released.retain(|button| !predicate(button));
    }
}

fn key_button(key: egui::Key) -> Option<Button> {
    let key = match key {
        egui::Key::W => KeyCode::KeyW,
        egui::Key::A => KeyCode::KeyA,
        egui::Key::S => KeyCode::KeyS,
        egui::Key::D => KeyCode::KeyD,
        egui::Key::Space => KeyCode::Space,
        _ => return None,
    };
    Some(Button::Key(key))
}

fn pointer_button(button: egui::PointerButton) -> Option<Button> {
    let button = match button {
        egui::PointerButton::Primary => MouseButton::Left,
        egui::PointerButton::Secondary => MouseButton::Right,
        egui::PointerButton::Middle => MouseButton::Middle,
        egui::PointerButton::Extra1 | egui::PointerButton::Extra2 => return None,
    };
    Some(Button::Mouse(button))
}

fn add(total: &mut [f32; 2], delta: [f32; 2]) {
    total[0] += delta[0];
    total[1] += delta[1];
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    use sge_input::{Button, KeyCode, MouseButton};

    use super::EditorInputAccumulator;

    #[test]
    fn frame_edges_clear_while_held_and_deltas_accumulate() {
        let key = Button::Key(KeyCode::KeyW);
        let mouse = Button::Mouse(MouseButton::Left);
        let mut input = EditorInputAccumulator::default();
        input.set_keyboard_capture(true);
        input.set_pointer_capture(true);
        input.button(key, true, false);
        input.button(mouse, true, false);
        input.add_pointer_delta([1.0, 2.0]);
        input.add_pointer_delta([3.0, -1.0]);
        input.add_wheel_delta([0.0, 1.0]);
        input.add_wheel_delta([2.0, 3.0]);

        let first = input.take_frame();
        assert!(first.is_held(key));
        assert!(first.is_pressed(key));
        assert!(first.is_pressed(mouse));
        assert_eq!(first.pointer_delta(), [4.0, 1.0]);
        assert_eq!(first.wheel_delta(), [2.0, 4.0]);

        let second = input.take_frame();
        assert!(second.is_held(key));
        assert!(!second.is_pressed(key));
        assert_eq!(second.pointer_delta(), [0.0; 2]);
        assert_eq!(second.wheel_delta(), [0.0; 2]);
    }

    #[test]
    fn capture_loss_and_stop_reset_prevent_stuck_buttons() {
        let key = Button::Key(KeyCode::KeyW);
        let mouse = Button::Mouse(MouseButton::Left);
        let mut input = EditorInputAccumulator::default();
        input.set_keyboard_capture(true);
        input.set_pointer_capture(true);
        input.button(key, true, false);
        input.button(mouse, true, false);

        input.set_keyboard_capture(false);
        let pointer_only = input.take_frame();
        assert!(!pointer_only.is_held(key));
        assert!(pointer_only.is_held(mouse));
        assert!(!pointer_only.is_released(key));

        input.reset();
        input.set_keyboard_capture(true);
        input.set_pointer_capture(true);
        let empty = input.take_frame();
        assert!(!empty.is_held(key));
        assert!(!empty.is_held(mouse));
    }

    #[test]
    fn egui_events_only_route_while_the_play_viewport_has_unconsumed_capture() {
        let key = Button::Key(KeyCode::KeyW);
        let event = egui::Event::Key {
            key: egui::Key::W,
            physical_key: Some(egui::Key::W),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut input = EditorInputAccumulator::default();

        input.handle_events(std::slice::from_ref(&event), false, false);
        assert!(!input.take_frame().is_held(key));

        input.handle_events(std::slice::from_ref(&event), true, false);
        assert!(input.take_frame().is_held(key));

        input.handle_events(&[], false, false);
        assert!(!input.take_frame().is_held(key));
    }
}

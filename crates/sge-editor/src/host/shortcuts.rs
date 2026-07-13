// Copyright The SimpleGameEngine Contributors

use eframe::egui;

use super::{EditorApp, EditorUiAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorShortcut {
    Save,
    SaveAs,
    Undo,
    Redo,
    Duplicate,
    Delete,
    TogglePlay,
    ViewWireframe,
    ViewUnlit,
    ViewLit,
}

impl EditorApp {
    pub(super) fn apply_editor_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F11)) {
            self.immersive_viewport = !self.immersive_viewport;
        }
        if ui.ctx().text_edit_focused() {
            return;
        }
        let shortcut = editor_shortcut(ui);
        let render_mode = match shortcut {
            Some(EditorShortcut::ViewWireframe) => Some(sge_render::RenderMode::Wireframe),
            Some(EditorShortcut::ViewUnlit) => Some(sge_render::RenderMode::Unlit),
            Some(EditorShortcut::ViewLit) => Some(sge_render::RenderMode::Lit),
            _ => None,
        };
        if let Some(render_mode) = render_mode {
            let _ = self.apply_ui_action(EditorUiAction::SetRenderMode(render_mode));
            return;
        }
        if self.build_running() {
            if shortcut == Some(EditorShortcut::TogglePlay) {
                let _ = self.apply_ui_action(EditorUiAction::StartPlay);
            }
            return;
        }
        match shortcut {
            Some(EditorShortcut::TogglePlay) if self.play.is_some() => self.stop_play(),
            Some(EditorShortcut::TogglePlay) => {
                let _ = self.apply_ui_action(EditorUiAction::StartPlay);
            }
            _ if self.play.is_some() => {}
            Some(EditorShortcut::Save) => {
                let _ = self.apply_ui_action(EditorUiAction::Save);
            }
            Some(EditorShortcut::SaveAs) => {
                if let Some(dialogs) = self.file_dialogs {
                    self.save_scene_as(dialogs);
                }
            }
            Some(EditorShortcut::Undo) => {
                let _ = self.apply_ui_action(EditorUiAction::Undo);
            }
            Some(EditorShortcut::Redo) => {
                let _ = self.apply_ui_action(EditorUiAction::Redo);
            }
            Some(EditorShortcut::Duplicate) => {
                let _ = self.apply_ui_action(EditorUiAction::DuplicateSelection);
            }
            Some(EditorShortcut::Delete) => {
                if let Some(selection) = self.session.selection() {
                    let result = self.session.remove_subtree(selection);
                    self.apply_edit(result);
                }
            }
            Some(
                EditorShortcut::ViewWireframe | EditorShortcut::ViewUnlit | EditorShortcut::ViewLit,
            )
            | None => {}
        }
    }
}

fn editor_shortcut(ui: &egui::Ui) -> Option<EditorShortcut> {
    ui.input(|input| {
        let command = input.modifiers.command;
        let alt = input.modifiers.alt;
        let shift = input.modifiers.shift;
        if alt && !command && input.key_pressed(egui::Key::Num2) {
            Some(EditorShortcut::ViewWireframe)
        } else if alt && !command && input.key_pressed(egui::Key::Num3) {
            Some(EditorShortcut::ViewUnlit)
        } else if alt && !command && input.key_pressed(egui::Key::Num4) {
            Some(EditorShortcut::ViewLit)
        } else if alt && !command && input.key_pressed(egui::Key::P) {
            Some(EditorShortcut::TogglePlay)
        } else if command && alt && input.key_pressed(egui::Key::S) {
            Some(EditorShortcut::SaveAs)
        } else if command && input.key_pressed(egui::Key::S) {
            Some(EditorShortcut::Save)
        } else if command && input.key_pressed(egui::Key::D) {
            Some(EditorShortcut::Duplicate)
        } else if command && input.key_pressed(egui::Key::Z) && !shift {
            Some(EditorShortcut::Undo)
        } else if command
            && (input.key_pressed(egui::Key::Y) || (shift && input.key_pressed(egui::Key::Z)))
        {
            Some(EditorShortcut::Redo)
        } else if input.key_pressed(egui::Key::Delete) {
            Some(EditorShortcut::Delete)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_ue_shortcuts_are_recognized_from_software_key_events() {
        assert_eq!(
            shortcut_for(egui::Key::D, command_modifiers()),
            Some(EditorShortcut::Duplicate)
        );
        assert_eq!(
            shortcut_for(
                egui::Key::S,
                egui::Modifiers {
                    alt: true,
                    ..command_modifiers()
                }
            ),
            Some(EditorShortcut::SaveAs)
        );
        assert_eq!(
            shortcut_for(egui::Key::P, egui::Modifiers::ALT),
            Some(EditorShortcut::TogglePlay)
        );
        assert_eq!(
            shortcut_for(egui::Key::Num2, egui::Modifiers::ALT),
            Some(EditorShortcut::ViewWireframe)
        );
        assert_eq!(
            shortcut_for(egui::Key::Num3, egui::Modifiers::ALT),
            Some(EditorShortcut::ViewUnlit)
        );
        assert_eq!(
            shortcut_for(egui::Key::Num4, egui::Modifiers::ALT),
            Some(EditorShortcut::ViewLit)
        );
    }

    fn shortcut_for(key: egui::Key, modifiers: egui::Modifiers) -> Option<EditorShortcut> {
        let context = egui::Context::default();
        let mut shortcut = None;
        let _ = context.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                modifiers,
                ..Default::default()
            },
            |ui| shortcut = editor_shortcut(ui),
        );
        shortcut
    }

    fn command_modifiers() -> egui::Modifiers {
        egui::Modifiers {
            command: true,
            mac_cmd: true,
            ..egui::Modifiers::NONE
        }
    }
}

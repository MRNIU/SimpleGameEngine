// Copyright The SimpleGameEngine Contributors

use std::{sync::atomic::Ordering, time::Instant};

use eframe::egui;
use sge_input::{Button, KeyCode};

use crate::{EditSession, PlaySession, PreviewFrame};

use super::{EditorApp, EditorUiAction};

impl EditorApp {
    pub(super) fn refresh_frame(&mut self) {
        match current_frame(&self.session, self.play.as_ref()) {
            Ok(mut frame) => {
                if self.play.is_none() {
                    self.viewport.prepare(&mut frame);
                }
                self.frame = Some(frame);
                self.diagnostic = None;
            }
            Err(error) => {
                self.frame = None;
                self.diagnostic = Some(error);
            }
        }
    }

    pub(super) fn start_play(&mut self) -> Result<(), String> {
        match self.session.start_play() {
            Ok(play) => {
                self.play = Some(play);
                self.input.reset();
                self.play_viewport_focused = false;
                self.last_tick = Instant::now();
                self.last_error = None;
                self.refresh_frame();
                Ok(())
            }
            Err(error) => {
                let error = error.to_string();
                self.last_error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn stop_play(&mut self) {
        self.play = None;
        self.input.reset();
        self.play_viewport_focused = false;
        self.refresh_frame();
    }

    pub(super) fn apply_edit(&mut self, result: Result<(), crate::EditError>) {
        let _ = self.finish_edit(result);
    }

    fn finish_edit(&mut self, result: Result<(), crate::EditError>) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.last_error = None;
                self.component_draft = None;
                self.inspector_drafts.clear();
                self.refresh_frame();
                Ok(())
            }
            Err(error) => {
                let error = error.to_string();
                self.last_error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn apply_ui_action(&mut self, action: EditorUiAction) -> Result<(), String> {
        if self.play.is_some()
            && matches!(
                action,
                EditorUiAction::CreateEmptyActor
                    | EditorUiAction::CreatePrimitive(_)
                    | EditorUiAction::DuplicateSelection
                    | EditorUiAction::Save
                    | EditorUiAction::Undo
                    | EditorUiAction::Redo
            )
        {
            return Err("authoring action is unavailable during Play".to_owned());
        }
        match action {
            EditorUiAction::CreateEmptyActor => {
                let result = self
                    .session
                    .create_entity("Empty Actor")
                    .and_then(|entity| self.session.select(Some(entity)));
                self.finish_edit(result)
            }
            EditorUiAction::CreatePrimitive(primitive) => {
                let result = self
                    .session
                    .create_primitive(primitive)
                    .and_then(|created| self.session.select(Some(created.entity)));
                self.finish_edit(result)
            }
            EditorUiAction::DuplicateSelection => {
                let selection = self
                    .session
                    .selection()
                    .ok_or_else(|| "no Actor is selected".to_owned())?;
                let result = self
                    .session
                    .duplicate_entity(selection)
                    .and_then(|entity| self.session.select(Some(entity)));
                self.finish_edit(result)
            }
            EditorUiAction::SelectHierarchyIndex(index) => {
                let entity = self
                    .session
                    .snapshot()
                    .map_err(|error| error.to_string())?
                    .entities()
                    .nth(index)
                    .map(sge_scene::AuthoringEntity::id)
                    .ok_or_else(|| format!("Hierarchy index {index} does not exist"))?;
                let result = self.session.select(Some(entity));
                self.finish_edit(result)
            }
            EditorUiAction::SelectEntity(entity) => {
                let result = self.session.select(Some(entity));
                self.finish_edit(result)
            }
            EditorUiAction::Save => {
                let result = self.session.save();
                self.finish_edit(result)
            }
            EditorUiAction::Undo => {
                let result = self.session.undo();
                self.finish_edit(result)
            }
            EditorUiAction::Redo => {
                let result = self.session.redo();
                self.finish_edit(result)
            }
            EditorUiAction::StartPlay => {
                if self.play.is_some() {
                    Err("Play is already running".to_owned())
                } else {
                    self.start_play()
                }
            }
            EditorUiAction::StopPlay => {
                if self.play.is_none() {
                    Err("Play is not running".to_owned())
                } else {
                    self.stop_play();
                    Ok(())
                }
            }
            EditorUiAction::Build => {
                let build = self
                    .build
                    .as_mut()
                    .ok_or_else(|| "Build launcher is unavailable".to_owned())?;
                if build.start(&self.project_root) {
                    Ok(())
                } else {
                    Err(build.status_text().to_owned())
                }
            }
        }
    }

    pub(super) fn build_controls(&mut self, ui: &mut egui::Ui) {
        let Some(build) = self.build.as_ref() else {
            return;
        };
        let start = ui
            .add_enabled(!build.is_running(), egui::Button::new("Build"))
            .clicked();
        if start {
            let _ = self.apply_ui_action(EditorUiAction::Build);
        }
        let Some(build) = self.build.as_ref() else {
            return;
        };
        let color = if build.failed() {
            egui::Color32::LIGHT_RED
        } else {
            ui.visuals().text_color()
        };
        ui.colored_label(color, build.status_text());
    }

    pub(super) fn advance_play(&mut self, context: &egui::Context, viewport_hovered: bool) {
        let keyboard_capture =
            self.play.is_some() && self.play_viewport_focused && !context.text_edit_focused();
        let pointer_capture = self.play.is_some() && self.play_viewport_focused && viewport_hovered;
        context.input(|input| {
            self.input
                .handle_events(&input.events, keyboard_capture, pointer_capture);
        });
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        let Some(play) = self.play.as_mut() else {
            let _ = self.input.take_frame();
            return;
        };
        let input = self.input.take_frame();
        if !input.is_empty() {
            self.gameplay_input_frames.fetch_add(1, Ordering::Relaxed);
        }
        if input.is_held(Button::Key(KeyCode::KeyW)) {
            self.gameplay_key_w_frames.fetch_add(1, Ordering::Relaxed);
        }
        match play.advance(delta, input) {
            Ok(()) => {
                self.play_frames.fetch_add(1, Ordering::Relaxed);
                self.refresh_frame();
            }
            Err(error) => {
                self.input.reset();
                self.last_error = Some(error.to_string());
            }
        }
    }
}

pub(super) fn current_frame(
    session: &EditSession,
    play: Option<&PlaySession>,
) -> Result<PreviewFrame, String> {
    play.map_or_else(
        || session.preview_frame().map_err(|error| error.to_string()),
        |play| play.preview_frame().map_err(|error| error.to_string()),
    )
}

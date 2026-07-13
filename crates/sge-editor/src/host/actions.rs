// Copyright The SimpleGameEngine Contributors

use std::{sync::atomic::Ordering, time::Instant};

use eframe::egui;
use sge_input::{Button, KeyCode};

use crate::{EditSession, PlaySession, PreviewFrame};

use super::{EditorApp, EditorUiAction};

impl EditorApp {
    fn build_request_state(&self) -> BuildRequestState {
        build_request_state(&self.session)
    }

    fn start_build(&mut self) -> Result<(), String> {
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

    fn request_build(&mut self) -> Result<(), String> {
        match self.build_request_state() {
            BuildRequestState::Ready => self.start_build(),
            BuildRequestState::RequiresSave => {
                self.last_error = None;
                self.pending_build_confirmation = true;
                Ok(())
            }
            BuildRequestState::WrongScene => Err(format!(
                "Build uses entry scene {}; the current scene is {}",
                self.session.descriptor().default_authoring_scene(),
                self.session.scene_path()
            )),
        }
    }

    pub(super) fn build_confirmation_dialog(&mut self, context: &egui::Context) {
        let mut save_and_build = false;
        egui::Window::new("Save before Build?")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label("Build reads the saved project entry scene.");
                ui.horizontal(|ui| {
                    save_and_build = ui.button("Save and Build").clicked();
                    if ui.button("Cancel").clicked() {
                        self.pending_build_confirmation = false;
                    }
                });
                if let Some(error) = &self.last_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
        if !save_and_build {
            return;
        }
        if let Err(error) = save_for_build(&mut self.session) {
            self.last_error = Some(error);
            return;
        }
        self.pending_build_confirmation = false;
        match self.start_build() {
            Ok(()) => self.last_error = None,
            Err(error) => self.last_error = Some(error),
        }
    }

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
            EditorUiAction::Build => self.request_build(),
        }
    }

    pub(super) fn build_controls(&mut self, ui: &mut egui::Ui) {
        let Some(build) = self.build.as_ref() else {
            return;
        };
        let start = ui
            .add_enabled(
                !build.is_running() && !self.pending_build_confirmation,
                egui::Button::new("Build"),
            )
            .clicked();
        if start && let Err(error) = self.apply_ui_action(EditorUiAction::Build) {
            self.last_error = Some(error);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildRequestState {
    Ready,
    RequiresSave,
    WrongScene,
}

fn build_request_state(session: &EditSession) -> BuildRequestState {
    if session.scene_path() != session.descriptor().default_authoring_scene() {
        BuildRequestState::WrongScene
    } else if session.is_dirty() {
        BuildRequestState::RequiresSave
    } else {
        BuildRequestState::Ready
    }
}

fn save_for_build(session: &mut EditSession) -> Result<(), String> {
    if build_request_state(session) != BuildRequestState::RequiresSave {
        return Err("Build save confirmation is no longer valid".to_owned());
    }
    session.save().map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use sge_project::ProjectPath;

    use super::{BuildRequestState, build_request_state, save_for_build};
    use crate::EditSession;

    #[test]
    fn build_requires_the_saved_project_entry_scene() -> Result<(), Box<dyn std::error::Error>> {
        let project = TestProject::new("build-policy")?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        assert_eq!(build_request_state(&session), BuildRequestState::Ready);

        session.create_entity("Unsaved")?;
        assert_eq!(
            build_request_state(&session),
            BuildRequestState::RequiresSave
        );
        save_for_build(&mut session)?;
        assert_eq!(build_request_state(&session), BuildRequestState::Ready);

        session.save_as(ProjectPath::new("Scenes/alternate.scene.ron")?)?;
        assert_eq!(build_request_state(&session), BuildRequestState::WrongScene);
        Ok(())
    }

    #[test]
    fn failed_build_save_keeps_the_session_dirty() -> Result<(), Box<dyn std::error::Error>> {
        let project = TestProject::new("build-save-failure")?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        session.create_entity("Unsaved")?;
        fs::remove_dir_all(project.path().join("Scenes"))?;

        let error = save_for_build(&mut session).expect_err("save must fail");
        assert!(error.contains("Scenes/main.scene.ron"), "{error}");
        assert!(session.is_dirty());
        assert_eq!(
            build_request_state(&session),
            BuildRequestState::RequiresSave
        );
        Ok(())
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(name: &str) -> Result<Self, std::io::Error> {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/tmp/sge_editor_host_actions")
                .join(format!("{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("Content/Meshes"))?;
            fs::create_dir_all(root.join("Scenes"))?;
            let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo_game");
            for relative in [
                "project.sge.ron",
                "Content/asset_manifest.ron",
                "Scenes/main.scene.ron",
            ] {
                fs::copy(demo.join(relative), root.join(relative))?;
            }
            for entry in fs::read_dir(demo.join("Content/Meshes"))? {
                let entry = entry?;
                fs::copy(
                    entry.path(),
                    root.join("Content/Meshes").join(entry.file_name()),
                )?;
            }
            Ok(Self { root })
        }

        fn path(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

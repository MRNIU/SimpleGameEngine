// Copyright The SimpleGameEngine Contributors

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::{EditSession, localization::EditorText, viewport::EditorViewport};

use super::{EditorApp, EditorFileDialogs};

#[derive(Debug, Clone, Copy)]
pub(super) enum ReplacementDialog {
    NewProject,
    OpenProject,
    OpenScene,
}

impl EditorApp {
    pub(super) fn intercept_close_request(&mut self, context: &egui::Context) {
        let build_running = self
            .build
            .as_ref()
            .is_some_and(super::BuildProcess::is_running);
        if intercept_close_request(
            context,
            self.session.is_dirty(),
            build_running,
            self.close_authorized,
        ) {
            self.pending_close_confirmation = true;
        }
    }

    pub(super) fn close_now(&mut self, context: &egui::Context) {
        self.close_authorized = true;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub(super) fn close_confirmation_dialog(&mut self, context: &egui::Context) {
        let language = self.language;
        let dirty = self.session.is_dirty();
        let build_running = self
            .build
            .as_ref()
            .is_some_and(super::BuildProcess::is_running);
        let mut decision = None;
        egui::Window::new(language.text(EditorText::CloseEditorTitle))
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                if dirty {
                    ui.label(language.text(EditorText::UnsavedChangesNotice));
                }
                if build_running {
                    ui.label(language.text(EditorText::BuildStopOnCloseNotice));
                }
                ui.horizontal(|ui| {
                    let save_label = if build_running {
                        language.text(EditorText::SaveStopBuildClose)
                    } else {
                        language.text(EditorText::SaveClose)
                    };
                    if dirty && ui.button(save_label).clicked() {
                        decision = Some(CloseDecision::Save);
                    }
                    let discard_label = match (dirty, build_running) {
                        (true, true) => language.text(EditorText::DiscardStopBuildClose),
                        (true, false) => language.text(EditorText::DiscardClose),
                        (false, true) => language.text(EditorText::StopBuildClose),
                        (false, false) => language.text(EditorText::Close),
                    };
                    if ui.button(discard_label).clicked() {
                        decision = Some(CloseDecision::Discard);
                    }
                    if ui.button(language.text(EditorText::Cancel)).clicked() {
                        decision = Some(CloseDecision::Cancel);
                    }
                });
                if let Some(error) = &self.last_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
        let Some(decision) = decision else {
            return;
        };
        match apply_close_decision(&mut self.session, decision) {
            Ok(CloseOutcome::KeepOpen) => {
                self.pending_close_confirmation = false;
                return;
            }
            Ok(CloseOutcome::Close) => {}
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        }
        if let Some(build) = self.build.as_mut()
            && let Err(error) = build.cancel()
        {
            self.last_error = Some(error);
            return;
        }
        self.pending_close_confirmation = false;
        self.close_now(context);
    }

    pub(super) fn file_controls(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        if !self.authoring_enabled() {
            return;
        }
        let Some(dialogs) = self.file_dialogs else {
            return;
        };
        let mut replacement = None;
        ui.menu_button(language.text(EditorText::File), |ui| {
            if ui.button(language.text(EditorText::NewProject)).clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::NewProject);
            }
            if ui.button(language.text(EditorText::OpenProject)).clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::OpenProject);
            }
            if ui.button(language.text(EditorText::OpenScene)).clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::OpenScene);
            }
            if ui.button(language.text(EditorText::SaveSceneAs)).clicked() {
                ui.close();
                self.save_scene_as(dialogs);
            }
            if ui.button(language.text(EditorText::ImportObj)).clicked() {
                ui.close();
                if let Some(path) = (dialogs.import_obj)(language, &self.project_root) {
                    match self.session.import_obj(path) {
                        Ok(created) => {
                            let result = self.session.select(Some(created.entity));
                            self.apply_edit(result);
                        }
                        Err(error) => self.last_error = Some(error.to_string()),
                    }
                }
            }
            if ui.button(language.text(EditorText::ImportPng)).clicked() {
                ui.close();
                if let Some(path) = (dialogs.import_png)(language, &self.project_root) {
                    match self.session.import_png(path) {
                        Ok(_) => self.apply_edit(Ok(())),
                        Err(error) => self.last_error = Some(error.to_string()),
                    }
                }
            }
        });
        if let Some(replacement) = replacement {
            if self.session.is_dirty() {
                self.pending_replacement = Some(replacement);
            } else {
                self.run_replacement_dialog(dialogs, replacement);
            }
        }
    }

    pub(super) fn save_scene_as(&mut self, dialogs: EditorFileDialogs) {
        if !self.authoring_enabled() {
            self.last_error = Some("Save As is unavailable while Play or Build is running".into());
            return;
        }
        if let Some(path) = (dialogs.save_scene)(self.language, &self.project_root) {
            match project_path(&self.project_root, &path).and_then(|path| {
                self.session
                    .save_as(path)
                    .map_err(|error| error.to_string())
            }) {
                Ok(()) => self.apply_edit(Ok(())),
                Err(error) => self.last_error = Some(error),
            }
        }
    }

    pub(super) fn unsaved_changes_dialog(&mut self, context: &egui::Context) {
        let language = self.language;
        let Some(replacement) = self.pending_replacement else {
            return;
        };
        let mut decision = None;
        egui::Window::new(language.text(EditorText::UnsavedSceneChangesTitle))
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(language.text(EditorText::SaveCurrentBeforeContinue));
                ui.horizontal(|ui| {
                    if ui.button(language.text(EditorText::Save)).clicked() {
                        decision = Some(true);
                    }
                    if ui.button(language.text(EditorText::Discard)).clicked() {
                        decision = Some(false);
                    }
                    if ui.button(language.text(EditorText::Cancel)).clicked() {
                        self.pending_replacement = None;
                    }
                });
            });
        let Some(save) = decision else {
            return;
        };
        if save && let Err(error) = self.session.save() {
            self.last_error = Some(error.to_string());
            return;
        }
        self.pending_replacement = None;
        if let Some(dialogs) = self.file_dialogs {
            self.run_replacement_dialog(dialogs, replacement);
        }
    }

    fn run_replacement_dialog(
        &mut self,
        dialogs: EditorFileDialogs,
        replacement: ReplacementDialog,
    ) {
        if !self.authoring_enabled() {
            self.last_error = Some(
                "project and scene replacement is unavailable while Play or Build is running"
                    .into(),
            );
            return;
        }
        match replacement {
            ReplacementDialog::NewProject => {
                match (dialogs.new_project)(self.language).and_then(|path| {
                    path.map_or(Ok(None), |path| {
                        EditSession::open(self.session.game(), &path)
                            .map_err(|error| error.to_string())
                            .map(|session| Some((session, path)))
                    })
                }) {
                    Ok(Some((session, path))) => self.replace_session(session, path),
                    Ok(None) => {}
                    Err(error) => self.last_error = Some(error),
                }
            }
            ReplacementDialog::OpenProject => {
                if let Some(root) = (dialogs.open_project)(self.language) {
                    match EditSession::open(self.session.game(), &root) {
                        Ok(session) => self.replace_session(session, root),
                        Err(error) => self.last_error = Some(error.to_string()),
                    }
                }
            }
            ReplacementDialog::OpenScene => {
                if let Some(path) = (dialogs.open_scene)(self.language, &self.project_root) {
                    match project_path(&self.project_root, &path)
                        .and_then(|path| self.session.open_scene(path).map_err(|e| e.to_string()))
                    {
                        Ok(()) => self.apply_edit(Ok(())),
                        Err(error) => self.last_error = Some(error),
                    }
                }
            }
        }
    }

    fn replace_session(&mut self, session: EditSession, root: PathBuf) {
        self.session = session;
        self.project_root = root;
        self.play = None;
        self.viewport = EditorViewport::default();
        self.last_error = None;
        self.refresh_frame();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseOutcome {
    KeepOpen,
    Close,
}

fn apply_close_decision(
    session: &mut EditSession,
    decision: CloseDecision,
) -> Result<CloseOutcome, crate::EditError> {
    match decision {
        CloseDecision::Save => {
            session.save()?;
            Ok(CloseOutcome::Close)
        }
        CloseDecision::Discard => Ok(CloseOutcome::Close),
        CloseDecision::Cancel => Ok(CloseOutcome::KeepOpen),
    }
}

fn close_requires_confirmation(dirty: bool, build_running: bool) -> bool {
    dirty || build_running
}

fn intercept_close_request(
    context: &egui::Context,
    dirty: bool,
    build_running: bool,
    close_authorized: bool,
) -> bool {
    let close_requested = context.input(|input| input.viewport().close_requested());
    if close_requested && !close_authorized && close_requires_confirmation(dirty, build_running) {
        context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        true
    } else {
        false
    }
}

fn project_path(root: &Path, path: &Path) -> Result<sge_project::ProjectPath, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "selected path must remain inside the project root".to_owned())?;
    let text = relative
        .to_str()
        .ok_or_else(|| "project path must be UTF-8".to_owned())?;
    sge_project::ProjectPath::new(text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use eframe::egui;

    use super::super::test_support::TestProject;
    use super::{
        CloseDecision, CloseOutcome, apply_close_decision, close_requires_confirmation,
        intercept_close_request,
    };
    use crate::EditSession;

    #[test]
    fn close_only_needs_confirmation_for_unsaved_work_or_a_running_build() {
        assert!(!close_requires_confirmation(false, false));
        assert!(close_requires_confirmation(true, false));
        assert!(close_requires_confirmation(false, true));
        assert!(close_requires_confirmation(true, true));
    }

    #[test]
    fn dirty_native_close_event_is_cancelled_before_the_confirmation() {
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root viewport")
            .events
            .push(egui::ViewportEvent::Close);

        let output = context.run_ui(input, |ui| {
            assert!(intercept_close_request(ui.ctx(), true, false, false));
        });
        let commands = &output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport output")
            .commands;
        assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    }

    #[test]
    fn authorized_close_is_not_cancelled_for_a_dirty_session() {
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root viewport")
            .events
            .push(egui::ViewportEvent::Close);

        let output = context.run_ui(input, |ui| {
            assert!(!intercept_close_request(ui.ctx(), true, false, true));
        });
        let commands = &output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport output")
            .commands;
        assert!(!commands.contains(&egui::ViewportCommand::CancelClose));
    }

    #[test]
    fn cancel_close_preserves_scene_selection_and_history() -> Result<(), Box<dyn std::error::Error>>
    {
        let project = TestProject::new("cancel-close")?;
        let scene_path = project.path().join("Scenes/main.scene.ron");
        let disk_before = fs::read(&scene_path)?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        let entity = session.create_entity("Unsaved Before Cancel")?;
        session.select(Some(entity))?;
        let scene_before = session.snapshot()?.to_ron()?;
        let cursor_before = session.history_cursor();

        assert_eq!(
            apply_close_decision(&mut session, CloseDecision::Cancel)?,
            CloseOutcome::KeepOpen
        );
        assert!(session.is_dirty());
        assert_eq!(session.selection(), Some(entity));
        assert_eq!(session.history_cursor(), cursor_before);
        assert_eq!(session.snapshot()?.to_ron()?, scene_before);
        assert_eq!(fs::read(&scene_path)?, disk_before);
        session.undo()?;
        assert_eq!(session.history_cursor(), cursor_before - 1);
        Ok(())
    }

    #[test]
    fn save_close_persists_before_authorizing_close() -> Result<(), Box<dyn std::error::Error>> {
        let project = TestProject::new("save-close")?;
        let scene_path = project.path().join("Scenes/main.scene.ron");
        let disk_before = fs::read(&scene_path)?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        session.create_entity("Saved Before Close")?;

        assert_eq!(
            apply_close_decision(&mut session, CloseDecision::Save)?,
            CloseOutcome::Close
        );
        assert!(!session.is_dirty());
        let disk_after = fs::read(&scene_path)?;
        assert_ne!(disk_after, disk_before);
        assert!(
            std::str::from_utf8(&disk_after)?.contains("Saved Before Close"),
            "saved scene must contain the pending edit"
        );
        Ok(())
    }

    #[test]
    fn failed_save_close_keeps_the_session_recoverable() -> Result<(), Box<dyn std::error::Error>> {
        let project = TestProject::new("failed-save-close")?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        let entity = session.create_entity("Unsaved After Failure")?;
        session.select(Some(entity))?;
        let scene_before = session.snapshot()?.to_ron()?;
        let cursor_before = session.history_cursor();
        fs::remove_dir_all(project.path().join("Scenes"))?;

        assert!(apply_close_decision(&mut session, CloseDecision::Save).is_err());
        assert!(session.is_dirty());
        assert_eq!(session.selection(), Some(entity));
        assert_eq!(session.history_cursor(), cursor_before);
        assert_eq!(session.snapshot()?.to_ron()?, scene_before);
        session.undo()?;
        assert_eq!(session.history_cursor(), cursor_before - 1);
        Ok(())
    }

    #[test]
    fn discard_close_does_not_write_the_pending_edit() -> Result<(), Box<dyn std::error::Error>> {
        let project = TestProject::new("discard-close")?;
        let scene_path = project.path().join("Scenes/main.scene.ron");
        let disk_before = fs::read(&scene_path)?;
        let mut session = EditSession::open(demo_game::GAME, project.path())?;
        session.create_entity("Discarded Before Close")?;

        assert_eq!(
            apply_close_decision(&mut session, CloseDecision::Discard)?,
            CloseOutcome::Close
        );
        assert!(session.is_dirty());
        assert_eq!(fs::read(scene_path)?, disk_before);
        Ok(())
    }
}

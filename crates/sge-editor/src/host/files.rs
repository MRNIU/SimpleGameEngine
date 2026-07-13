// Copyright The SimpleGameEngine Contributors

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::{EditSession, viewport::EditorViewport};

use super::{EditorApp, EditorFileDialogs};

#[derive(Debug, Clone, Copy)]
pub(super) enum ReplacementDialog {
    NewProject,
    OpenProject,
    OpenScene,
}

impl EditorApp {
    pub(super) fn file_controls(&mut self, ui: &mut egui::Ui) {
        if self.play.is_some() {
            return;
        }
        let Some(dialogs) = self.file_dialogs else {
            return;
        };
        let mut replacement = None;
        ui.menu_button("File", |ui| {
            if ui.button("New Project…").clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::NewProject);
            }
            if ui.button("Open Project…").clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::OpenProject);
            }
            if ui.button("Open Scene…").clicked() {
                ui.close();
                replacement = Some(ReplacementDialog::OpenScene);
            }
            if ui.button("Save Scene As…").clicked() {
                ui.close();
                self.save_scene_as(dialogs);
            }
            if ui.button("Import OBJ…").clicked() {
                ui.close();
                if let Some(path) = (dialogs.import_obj)(&self.project_root) {
                    match self.session.import_obj(path) {
                        Ok(created) => {
                            let result = self.session.select(Some(created.entity));
                            self.apply_edit(result);
                        }
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
        if let Some(path) = (dialogs.save_scene)(&self.project_root) {
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
        let Some(replacement) = self.pending_replacement else {
            return;
        };
        let mut decision = None;
        egui::Window::new("Unsaved scene changes")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label("Save the current scene before continuing?");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Discard").clicked() {
                        decision = Some(false);
                    }
                    if ui.button("Cancel").clicked() {
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
        match replacement {
            ReplacementDialog::NewProject => match (dialogs.new_project)().and_then(|path| {
                path.map_or(Ok(None), |path| {
                    EditSession::open(self.session.game(), &path)
                        .map_err(|error| error.to_string())
                        .map(|session| Some((session, path)))
                })
            }) {
                Ok(Some((session, path))) => self.replace_session(session, path),
                Ok(None) => {}
                Err(error) => self.last_error = Some(error),
            },
            ReplacementDialog::OpenProject => {
                if let Some(root) = (dialogs.open_project)() {
                    match EditSession::open(self.session.game(), &root) {
                        Ok(session) => self.replace_session(session, root),
                        Err(error) => self.last_error = Some(error.to_string()),
                    }
                }
            }
            ReplacementDialog::OpenScene => {
                if let Some(path) = (dialogs.open_scene)(&self.project_root) {
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

fn project_path(root: &Path, path: &Path) -> Result<sge_project::ProjectPath, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "selected path must remain inside the project root".to_owned())?;
    let text = relative
        .to_str()
        .ok_or_else(|| "project path must be UTF-8".to_owned())?;
    sge_project::ProjectPath::new(text).map_err(|error| error.to_string())
}

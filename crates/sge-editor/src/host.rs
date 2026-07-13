// Copyright The SimpleGameEngine Contributors

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
    time::{Duration, Instant},
};

use eframe::egui;
use sge_app::GameDescriptor;
use sge_input::{Button, KeyCode};
use sge_reflect::{ReflectedValue, TypeKey};

use crate::{
    EditSession, EditorBuildLauncher, EditorInputAccumulator, EditorOpenError, PlaySession,
    PlayStartError, PreviewFrame, PreviewProbe, build::BuildProcess, inspector_ui, preview,
    viewport::EditorViewport,
};

mod panels;

pub type NewProjectDialog = fn() -> Result<Option<PathBuf>, String>;
pub type OpenProjectDialog = fn() -> Option<PathBuf>;
pub type ProjectFileDialog = fn(&Path) -> Option<PathBuf>;

#[derive(Debug, Clone, Copy)]
pub struct EditorFileDialogs {
    pub new_project: NewProjectDialog,
    pub open_project: OpenProjectDialog,
    pub open_scene: ProjectFileDialog,
    pub save_scene: ProjectFileDialog,
    pub import_obj: ProjectFileDialog,
}

#[derive(Debug, Clone)]
pub struct EditorRunOptions {
    pub max_frames: Option<u64>,
    pub initial_size: [u32; 2],
    pub start_in_play: bool,
    pub screenshot: Option<PathBuf>,
    pub ui_actions: Vec<EditorUiAction>,
    pub build_launcher: Option<EditorBuildLauncher>,
    pub file_dialogs: Option<EditorFileDialogs>,
}

impl Default for EditorRunOptions {
    fn default() -> Self {
        Self {
            max_frames: None,
            initial_size: [1280, 720],
            start_in_play: false,
            screenshot: None,
            ui_actions: Vec::new(),
            build_launcher: None,
            file_dialogs: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorUiAction {
    CreateEntity,
    CreatePrimitive(crate::PrimitiveKind),
    SelectEntity(sge_scene::SceneEntityId),
    SelectHierarchyIndex(usize),
    Save,
    Undo,
    Redo,
    StartPlay,
    StopPlay,
    Build,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRunReport {
    pub preview: crate::PreviewProbeReport,
    pub play_frames: u64,
    pub gameplay_input_frames: u64,
    pub gameplay_key_w_frames: u64,
    pub ui_actions: u64,
}

pub fn run(
    game: GameDescriptor,
    project_root: impl AsRef<Path>,
    options: EditorRunOptions,
) -> Result<EditorRunReport, EditorRunError> {
    if options.initial_size.contains(&0) {
        return Err(EditorRunError::InvalidInitialSize);
    }
    if options.screenshot.is_some() && options.max_frames.is_some() {
        return Err(EditorRunError::ScreenshotWithFrameLimit);
    }
    let project_root = project_root.as_ref().to_path_buf();
    let session = EditSession::open(game, &project_root)?;
    let play = if options.start_in_play {
        Some(session.start_play()?)
    } else {
        None
    };
    let mut viewport = EditorViewport::default();
    let mut initial_frame = current_frame(&session, play.as_ref());
    if let Ok(frame) = &mut initial_frame {
        viewport.prepare(frame);
    }
    let preview_expected = initial_frame.is_ok();
    let probe = PreviewProbe::default();
    let report_probe = probe.clone();
    let play_frames = Arc::new(AtomicU64::new(0));
    let report_play_frames = Arc::clone(&play_frames);
    let gameplay_input_frames = Arc::new(AtomicU64::new(0));
    let report_gameplay_input_frames = Arc::clone(&gameplay_input_frames);
    let gameplay_key_w_frames = Arc::new(AtomicU64::new(0));
    let report_gameplay_key_w_frames = Arc::clone(&gameplay_key_w_frames);
    let build = options.build_launcher.map(BuildProcess::new);
    let expected_ui_actions = options.ui_actions.len() as u64;
    let ui_action_count = Arc::new(AtomicU64::new(0));
    let report_ui_action_count = Arc::clone(&ui_action_count);
    let (screenshot_result, screenshot_receiver) = if options.screenshot.is_some() {
        let (sender, receiver) = mpsc::channel();
        (Some(sender), Some(receiver))
    } else {
        (None, None)
    };
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default().with_inner_size([
            options.initial_size[0] as f32,
            options.initial_size[1] as f32,
        ]),
        ..Default::default()
    };
    eframe::run_native(
        "SimpleGameEngine Demo Editor",
        native_options,
        Box::new(move |creation_context| {
            preview::install_renderer(creation_context).map_err(|error| error.to_owned())?;
            let (frame, diagnostic) = match initial_frame {
                Ok(frame) => (Some(frame), None),
                Err(error) => (None, Some(error)),
            };
            Ok(Box::new(EditorApp {
                session,
                play,
                frame,
                diagnostic,
                last_error: None,
                probe,
                project_root,
                max_frames: options.max_frames,
                frames: 0,
                screenshot: options.screenshot,
                screenshot_requested_at: None,
                screenshot_result,
                ui_actions: options.ui_actions.into(),
                ui_action_count,
                pending_ui_build: false,
                play_frames,
                gameplay_input_frames,
                gameplay_key_w_frames,
                last_tick: Instant::now(),
                input: EditorInputAccumulator::default(),
                play_viewport_focused: false,
                component_to_add: None,
                component_draft: None,
                inspector_drafts: inspector_ui::InspectorDrafts::default(),
                build,
                file_dialogs: options.file_dialogs,
                pending_replacement: None,
                viewport,
                immersive_viewport: false,
            }))
        }),
    )
    .map_err(|error| EditorRunError::Eframe(error.to_string()))?;
    if let Some(receiver) = screenshot_receiver {
        receiver
            .recv()
            .map_err(|_| EditorRunError::ScreenshotIncomplete)?
            .map_err(EditorRunError::Screenshot)?;
    }
    let preview = report_probe.report();
    if let Some(error) = &preview.error {
        return Err(EditorRunError::Preview(error.clone()));
    }
    if preview_expected && (preview.prepare_count == 0 || preview.paint_count == 0) {
        return Err(EditorRunError::PreviewIncomplete);
    }
    let completed_ui_actions = report_ui_action_count.load(Ordering::Relaxed);
    if completed_ui_actions != expected_ui_actions {
        return Err(EditorRunError::UiActionsIncomplete {
            expected: expected_ui_actions,
            completed: completed_ui_actions,
        });
    }
    Ok(EditorRunReport {
        preview,
        play_frames: report_play_frames.load(Ordering::Relaxed),
        gameplay_input_frames: report_gameplay_input_frames.load(Ordering::Relaxed),
        gameplay_key_w_frames: report_gameplay_key_w_frames.load(Ordering::Relaxed),
        ui_actions: completed_ui_actions,
    })
}

struct EditorApp {
    session: EditSession,
    play: Option<PlaySession>,
    frame: Option<PreviewFrame>,
    diagnostic: Option<String>,
    last_error: Option<String>,
    probe: PreviewProbe,
    project_root: PathBuf,
    max_frames: Option<u64>,
    frames: u64,
    screenshot: Option<PathBuf>,
    screenshot_requested_at: Option<Instant>,
    screenshot_result: Option<Sender<Result<(), String>>>,
    ui_actions: VecDeque<EditorUiAction>,
    ui_action_count: Arc<AtomicU64>,
    pending_ui_build: bool,
    play_frames: Arc<AtomicU64>,
    gameplay_input_frames: Arc<AtomicU64>,
    gameplay_key_w_frames: Arc<AtomicU64>,
    last_tick: Instant,
    input: EditorInputAccumulator,
    play_viewport_focused: bool,
    component_to_add: Option<TypeKey>,
    component_draft: Option<ReflectedValue>,
    inspector_drafts: inspector_ui::InspectorDrafts,
    build: Option<BuildProcess>,
    file_dialogs: Option<EditorFileDialogs>,
    pending_replacement: Option<ReplacementDialog>,
    viewport: EditorViewport,
    immersive_viewport: bool,
}

#[derive(Debug, Clone, Copy)]
enum ReplacementDialog {
    NewProject,
    OpenProject,
    OpenScene,
}

impl EditorApp {
    fn refresh_frame(&mut self) {
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

    fn start_play(&mut self) -> Result<(), String> {
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

    fn stop_play(&mut self) {
        self.play = None;
        self.input.reset();
        self.play_viewport_focused = false;
        self.refresh_frame();
    }

    fn apply_edit(&mut self, result: Result<(), crate::EditError>) {
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

    fn apply_ui_action(&mut self, action: EditorUiAction) -> Result<(), String> {
        if self.play.is_some()
            && matches!(
                action,
                EditorUiAction::CreateEntity
                    | EditorUiAction::CreatePrimitive(_)
                    | EditorUiAction::Save
                    | EditorUiAction::Undo
                    | EditorUiAction::Redo
            )
        {
            return Err("authoring action is unavailable during Play".to_owned());
        }
        match action {
            EditorUiAction::CreateEntity => {
                let result = self
                    .session
                    .create_entity("Entity")
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
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if context.current_pass_index() != 0 {
            return;
        }
        if let Some(build) = self.build.as_mut() {
            build.poll();
        }
        if self.screenshot_requested_at.is_some()
            && let Some(image) = context.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
            })
        {
            let result = save_screenshot(self.screenshot.take(), image.as_ref());
            if let Some(sender) = self.screenshot_result.take() {
                let _ = sender.send(result);
            }
            context.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self
            .screenshot_requested_at
            .is_some_and(|requested| requested.elapsed() >= Duration::from_secs(5))
        {
            if let Some(sender) = self.screenshot_result.take() {
                let _ = sender.send(Err("GPU screenshot readback timed out".to_owned()));
            }
            context.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.screenshot_requested_at.is_some() {
            request_screenshot(context);
            return;
        }
        self.frames = self.frames.saturating_add(1);
        if self.pending_ui_build {
            let Some(build) = self.build.as_ref() else {
                self.pending_ui_build = false;
                self.last_error = Some("Build launcher disappeared while running".to_owned());
                self.ui_actions.clear();
                context.request_repaint();
                return;
            };
            if build.is_running() {
                context.request_repaint();
                return;
            }
            self.pending_ui_build = false;
            if build.failed() {
                self.last_error = Some(build.status_text().to_owned());
                self.ui_actions.clear();
            } else {
                self.ui_action_count.fetch_add(1, Ordering::Relaxed);
            }
            context.request_repaint();
            return;
        }
        if self.frames >= 3
            && let Some(action) = self.ui_actions.pop_front()
        {
            let build = action == EditorUiAction::Build;
            match self.apply_ui_action(action) {
                Ok(()) => {
                    if build {
                        self.pending_ui_build = true;
                    } else {
                        self.ui_action_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(error) => {
                    self.last_error = Some(error);
                    self.ui_actions.clear();
                }
            }
            context.request_repaint();
            return;
        }
        if self.screenshot.is_some()
            && self.screenshot_requested_at.is_none()
            && self.frames >= 3
            && self.ui_actions.is_empty()
        {
            self.screenshot_requested_at = Some(Instant::now());
            request_screenshot(context);
            return;
        }
        if self.probe.report().error.is_some()
            || self.max_frames.is_some_and(|max| self.frames >= max)
        {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            context.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.pending_replacement.is_some() {
            self.unsaved_changes_dialog(ui.ctx());
            return;
        }
        self.apply_editor_shortcuts(ui);
        if !self.immersive_viewport {
            egui::Panel::top("project_identity").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for label in project_identity_labels(
                        self.session.descriptor().game_id().as_str(),
                        &self.project_root,
                    ) {
                        ui.label(label);
                        ui.separator();
                    }
                    self.file_controls(ui);
                    if self.play.is_some() {
                        if ui.button("Stop").clicked() {
                            let _ = self.apply_ui_action(EditorUiAction::StopPlay);
                        }
                    } else if ui.button("Play").clicked() {
                        let _ = self.apply_ui_action(EditorUiAction::StartPlay);
                    }
                    self.build_controls(ui);
                    if ui
                        .add_enabled(self.play.is_none(), egui::Button::new("Save"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Save);
                    }
                    if ui
                        .add_enabled(self.play.is_none(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Undo);
                    }
                    if ui
                        .add_enabled(self.play.is_none(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Redo);
                    }
                    ui.label(if self.session.is_dirty() {
                        "modified"
                    } else {
                        "saved"
                    });
                });
            });
        }
        if self.pending_replacement.is_some() {
            self.unsaved_changes_dialog(ui.ctx());
            return;
        }

        if !self.immersive_viewport {
            self.hierarchy(ui);
            self.inspector(ui);
        }

        let response = if let Some(frame) = &self.frame {
            preview::paint(ui, frame, &self.probe, |ui, rect| {
                self.viewport.paint_background(ui, rect, frame);
            })
        } else {
            let available = ui.available_size_before_wrap();
            let (rect, response) =
                ui.allocate_exact_size(available, egui::Sense::focusable_noninteractive());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_gray(24));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.diagnostic.as_deref().unwrap_or("Preview unavailable"),
                egui::FontId::proportional(16.0),
                egui::Color32::LIGHT_GRAY,
            );
            response
        };
        if self.play.is_none()
            && let Some(frame) = self.frame.clone()
        {
            let result = self
                .viewport
                .interact(ui, &response, &frame, &mut self.session);
            if let Err(error) = result {
                self.last_error = Some(error.to_string());
            } else {
                self.refresh_frame();
            }
        }
        if response.hovered() && ui.input(|input| input.pointer.any_pressed()) {
            response.request_focus();
        }
        self.play_viewport_focused = self.play.is_some() && response.has_focus();

        if let Some(error) = &self.last_error {
            egui::Panel::bottom("editor_error").show(ui, |ui| {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            });
        }
        if ui.ctx().current_pass_index() == 0 {
            self.advance_play(ui.ctx(), response.hovered());
        }
    }
}

impl EditorApp {
    fn apply_editor_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F11)) {
            self.immersive_viewport = !self.immersive_viewport;
        }
        if ui.ctx().text_edit_focused() || self.play.is_some() {
            return;
        }
        let (command, shift, save, undo, redo, delete) = ui.input(|input| {
            (
                input.modifiers.command,
                input.modifiers.shift,
                input.key_pressed(egui::Key::S),
                input.key_pressed(egui::Key::Z),
                input.key_pressed(egui::Key::Y),
                input.key_pressed(egui::Key::Delete),
            )
        });
        if command && save {
            let _ = self.apply_ui_action(EditorUiAction::Save);
        } else if command && undo && !shift {
            let _ = self.apply_ui_action(EditorUiAction::Undo);
        } else if command && (redo || (shift && undo)) {
            let _ = self.apply_ui_action(EditorUiAction::Redo);
        } else if delete && let Some(selection) = self.session.selection() {
            let result = self.session.remove_subtree(selection);
            self.apply_edit(result);
        }
    }
}

fn project_identity_labels(game_id: &str, project_root: &Path) -> [String; 2] {
    [
        format!("game_id: {game_id}"),
        project_root.display().to_string(),
    ]
}

fn request_screenshot(context: &egui::Context) {
    context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    context.request_repaint();
}

fn save_screenshot(path: Option<PathBuf>, screenshot: &egui::ColorImage) -> Result<(), String> {
    let path = path.ok_or_else(|| "missing screenshot output path".to_owned())?;
    let rgba = screenshot
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect::<Vec<_>>();
    image::save_buffer(
        &path,
        &rgba,
        screenshot.size[0] as u32,
        screenshot.size[1] as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|error| format!("cannot save Editor screenshot {}: {error}", path.display()))
}

impl EditorApp {
    fn file_controls(&mut self, ui: &mut egui::Ui) {
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
                if let Some(path) = (dialogs.save_scene)(&self.project_root) {
                    match project_path(&self.project_root, &path)
                        .and_then(|path| self.session.save_as(path).map_err(|e| e.to_string()))
                    {
                        Ok(()) => self.apply_edit(Ok(())),
                        Err(error) => self.last_error = Some(error),
                    }
                }
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

    fn unsaved_changes_dialog(&mut self, context: &egui::Context) {
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

    fn build_controls(&mut self, ui: &mut egui::Ui) {
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

    fn advance_play(&mut self, context: &egui::Context, viewport_hovered: bool) {
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

fn project_path(root: &Path, path: &Path) -> Result<sge_project::ProjectPath, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "selected path must remain inside the project root".to_owned())?;
    let text = relative
        .to_str()
        .ok_or_else(|| "project path must be UTF-8".to_owned())?;
    sge_project::ProjectPath::new(text).map_err(|error| error.to_string())
}

fn current_frame(
    session: &EditSession,
    play: Option<&PlaySession>,
) -> Result<PreviewFrame, String> {
    play.map_or_else(
        || session.preview_frame().map_err(|error| error.to_string()),
        |play| play.preview_frame().map_err(|error| error.to_string()),
    )
}

#[derive(Debug, thiserror::Error)]
pub enum EditorRunError {
    #[error(transparent)]
    Open(#[from] EditorOpenError),
    #[error(transparent)]
    Play(#[from] PlayStartError),
    #[error("initial Editor window size must be non-zero")]
    InvalidInitialSize,
    #[error("Editor screenshot cannot be combined with a frame limit")]
    ScreenshotWithFrameLimit,
    #[error("eframe Editor failed: {0}")]
    Eframe(String),
    #[error("Editor preview WGPU callback failed: {0}")]
    Preview(String),
    #[error("Editor preview WGPU callback did not prepare and paint")]
    PreviewIncomplete,
    #[error("cannot save Editor screenshot: {0}")]
    Screenshot(String),
    #[error("Editor screenshot was not captured before the window closed")]
    ScreenshotIncomplete,
    #[error("Editor UI action tape stopped after {completed} of {expected} actions")]
    UiActionsIncomplete { expected: u64, completed: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_screenshot_is_requested_on_every_poll() {
        let context = egui::Context::default();
        for _ in 0..2 {
            let output = context.run_ui(Default::default(), |ui| request_screenshot(ui.ctx()));
            let commands = &output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .expect("root viewport output")
                .commands;
            assert!(
                commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::Screenshot(_)))
            );
        }
    }

    #[test]
    fn toolbar_identity_contains_only_game_and_project_context() {
        assert_eq!(
            project_identity_labels("demo.game", Path::new("examples/demo_game")),
            [
                "game_id: demo.game".to_owned(),
                "examples/demo_game".to_owned(),
            ]
        );
    }
}

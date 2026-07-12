// Copyright The SimpleGameEngine Contributors

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use eframe::egui;
use sge_app::GameDescriptor;
use sge_reflect::TypeKey;
use sge_scene::{AuthoringEntity, SceneEntityId};

use crate::{
    EditSession, EditorInputAccumulator, EditorOpenError, PlaySession, PlayStartError,
    PreviewFrame, PreviewProbe, inspector_ui, preview,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorRunOptions {
    pub max_frames: Option<u64>,
    pub initial_size: [u32; 2],
    pub start_in_play: bool,
}

impl Default for EditorRunOptions {
    fn default() -> Self {
        Self {
            max_frames: None,
            initial_size: [1280, 720],
            start_in_play: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRunReport {
    pub preview: crate::PreviewProbeReport,
    pub play_frames: u64,
}

pub fn run(
    game: GameDescriptor,
    project_root: impl AsRef<Path>,
    options: EditorRunOptions,
) -> Result<EditorRunReport, EditorRunError> {
    if options.initial_size.contains(&0) {
        return Err(EditorRunError::InvalidInitialSize);
    }
    let project_root = project_root.as_ref().to_path_buf();
    let session = EditSession::open(game, &project_root)?;
    let play = if options.start_in_play {
        Some(session.start_play()?)
    } else {
        None
    };
    let initial_frame = current_frame(&session, play.as_ref());
    let preview_expected = initial_frame.is_ok();
    let probe = PreviewProbe::default();
    let report_probe = probe.clone();
    let play_frames = Arc::new(AtomicU64::new(0));
    let report_play_frames = Arc::clone(&play_frames);
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
                play_frames,
                last_tick: Instant::now(),
                input: EditorInputAccumulator::default(),
                play_viewport_focused: false,
                component_to_add: None,
            }))
        }),
    )
    .map_err(|error| EditorRunError::Eframe(error.to_string()))?;
    let preview = report_probe.report();
    if let Some(error) = &preview.error {
        return Err(EditorRunError::Preview(error.clone()));
    }
    if preview_expected && (preview.prepare_count == 0 || preview.paint_count == 0) {
        return Err(EditorRunError::PreviewIncomplete);
    }
    Ok(EditorRunReport {
        preview,
        play_frames: report_play_frames.load(Ordering::Relaxed),
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
    play_frames: Arc<AtomicU64>,
    last_tick: Instant,
    input: EditorInputAccumulator,
    play_viewport_focused: bool,
    component_to_add: Option<TypeKey>,
}

impl EditorApp {
    fn refresh_frame(&mut self) {
        match current_frame(&self.session, self.play.as_ref()) {
            Ok(frame) => {
                self.frame = Some(frame);
                self.diagnostic = None;
            }
            Err(error) => {
                self.frame = None;
                self.diagnostic = Some(error);
            }
        }
    }

    fn start_play(&mut self) {
        match self.session.start_play() {
            Ok(play) => {
                self.play = Some(play);
                self.input.reset();
                self.play_viewport_focused = false;
                self.last_tick = Instant::now();
                self.last_error = None;
                self.refresh_frame();
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }

    fn stop_play(&mut self) {
        self.play = None;
        self.input.reset();
        self.play_viewport_focused = false;
        self.refresh_frame();
    }

    fn apply_edit(&mut self, result: Result<(), crate::EditError>) {
        match result {
            Ok(()) => {
                self.last_error = None;
                self.refresh_frame();
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames = self.frames.saturating_add(1);
        let keyboard_capture = self.play.is_some()
            && self.play_viewport_focused
            && !context.egui_wants_keyboard_input();
        let pointer_capture = self.play.is_some()
            && self.play_viewport_focused
            && !context.egui_wants_pointer_input();
        context.input(|input| {
            self.input
                .handle_events(&input.events, keyboard_capture, pointer_capture);
        });

        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        if let Some(play) = self.play.as_mut() {
            match play.advance(delta, self.input.take_frame()) {
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

        if self.probe.report().error.is_some()
            || self.max_frames.is_some_and(|max| self.frames >= max)
        {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            context.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("project_identity").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SimpleGameEngine Editor");
                ui.separator();
                ui.label(format!(
                    "game_id: {}",
                    self.session.descriptor().game_id().as_str()
                ));
                ui.separator();
                ui.label(self.project_root.display().to_string());
                ui.separator();
                if self.play.is_some() {
                    if ui.button("Stop").clicked() {
                        self.stop_play();
                    }
                } else if ui.button("Play").clicked() {
                    self.start_play();
                }
                if ui
                    .add_enabled(self.play.is_none(), egui::Button::new("Save"))
                    .clicked()
                {
                    let result = self.session.save();
                    self.apply_edit(result);
                }
                if ui
                    .add_enabled(self.play.is_none(), egui::Button::new("Undo"))
                    .clicked()
                {
                    let result = self.session.undo();
                    self.apply_edit(result);
                }
                if ui
                    .add_enabled(self.play.is_none(), egui::Button::new("Redo"))
                    .clicked()
                {
                    let result = self.session.redo();
                    self.apply_edit(result);
                }
                ui.label(if self.session.is_dirty() {
                    "modified"
                } else {
                    "saved"
                });
            });
        });

        self.hierarchy(ui);
        self.inspector(ui);

        let response = if let Some(frame) = &self.frame {
            preview::paint(ui, frame, &self.probe)
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
        if response.hovered() && ui.input(|input| input.pointer.any_pressed()) {
            response.request_focus();
        }
        self.play_viewport_focused = self.play.is_some() && response.has_focus();

        if let Some(error) = &self.last_error {
            egui::Panel::bottom("editor_error").show(ui, |ui| {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            });
        }
    }
}

impl EditorApp {
    fn hierarchy(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("hierarchy")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| {
                ui.heading("Hierarchy");
                if self.play.is_none() && ui.button("New Entity").clicked() {
                    let id = SceneEntityId::new_v4();
                    let result = AuthoringEntity::new(id, None, Vec::new())
                        .map_err(crate::EditError::from)
                        .and_then(|entity| self.session.add_entity(entity))
                        .and_then(|()| self.session.select(Some(id)));
                    self.apply_edit(result);
                }
                let selection = self.session.selection();
                match self.session.snapshot() {
                    Ok(scene) => {
                        for entity in scene.entities() {
                            if ui
                                .selectable_label(
                                    selection == Some(entity.id()),
                                    entity.id().to_string(),
                                )
                                .clicked()
                            {
                                let result = self.session.select(Some(entity.id()));
                                self.apply_edit(result);
                            }
                        }
                    }
                    Err(error) => self.last_error = Some(error.to_string()),
                }
                if self.play.is_none()
                    && let Some(selection) = self.session.selection()
                    && ui.button("Delete Selected").clicked()
                {
                    let result = self.session.remove_entity(selection);
                    self.apply_edit(result);
                }
            });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                ui.heading("Inspector");
                let components = match self.session.inspector() {
                    Ok(components) => components,
                    Err(error) => {
                        self.last_error = Some(error.to_string());
                        return;
                    }
                };
                let Some(entity) = self.session.selection() else {
                    return;
                };
                let available = self
                    .session
                    .component_types()
                    .into_iter()
                    .filter(|candidate| {
                        !components
                            .iter()
                            .any(|component| component.type_key() == candidate.type_key())
                    })
                    .collect::<Vec<_>>();
                if !available
                    .iter()
                    .any(|candidate| self.component_to_add.as_ref() == Some(candidate.type_key()))
                {
                    self.component_to_add = available
                        .first()
                        .map(|candidate| candidate.type_key().clone());
                }
                let mut add = None;
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("add_component_type")
                        .selected_text(
                            self.component_to_add
                                .as_ref()
                                .and_then(|selected| {
                                    available
                                        .iter()
                                        .find(|candidate| candidate.type_key() == selected)
                                })
                                .map_or("No component", |candidate| candidate.display_name()),
                        )
                        .show_ui(ui, |ui| {
                            for candidate in &available {
                                ui.selectable_value(
                                    &mut self.component_to_add,
                                    Some(candidate.type_key().clone()),
                                    candidate.display_name(),
                                );
                            }
                        });
                    if self.play.is_none() && ui.button("Add Component").clicked() {
                        add = self.component_to_add.clone();
                    }
                });
                if let Some(component) = add {
                    let result = self.session.add_component(entity, component.as_str());
                    self.apply_edit(result);
                    return;
                }
                let action = ui
                    .add_enabled_ui(self.play.is_none(), |ui| {
                        inspector_ui::draw(ui, &components)
                    })
                    .inner;
                let Some(action) = action else {
                    return;
                };
                let result = match action {
                    inspector_ui::InspectorAction::SetField {
                        component,
                        field,
                        value,
                    } => self
                        .session
                        .set_field(entity, component.as_str(), field.as_str(), value),
                    inspector_ui::InspectorAction::RemoveComponent(component) => {
                        self.session.remove_component(entity, component.as_str())
                    }
                };
                self.apply_edit(result);
            });
    }
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
    #[error("eframe Editor failed: {0}")]
    Eframe(String),
    #[error("Editor preview WGPU callback failed: {0}")]
    Preview(String),
    #[error("Editor preview WGPU callback did not prepare and paint")]
    PreviewIncomplete,
}

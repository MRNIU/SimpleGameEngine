// Copyright The SimpleGameEngine Contributors

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
    time::Instant,
};

use eframe::egui;
use sge_app::GameDescriptor;
use sge_reflect::{ReflectedValue, TypeKey};

use crate::{
    EditSession, EditorBuildLauncher, EditorInputAccumulator, EditorOpenError, PlaySession,
    PlayStartError, PreviewFrame, PreviewProbe, build::BuildProcess, inspector_ui, preview,
    viewport::EditorViewport,
};

mod actions;
mod app;
mod files;
mod panels;
mod shortcuts;

use actions::current_frame;
use files::ReplacementDialog;

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
    CreateEmptyActor,
    CreatePrimitive(crate::PrimitiveKind),
    DuplicateSelection,
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
                pending_close_confirmation: false,
                close_authorized: false,
                pending_build_confirmation: false,
                viewport,
                immersive_viewport: false,
                panel_layout: panels::PanelLayout::default(),
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
    pending_close_confirmation: bool,
    close_authorized: bool,
    pending_build_confirmation: bool,
    viewport: EditorViewport,
    immersive_viewport: bool,
    panel_layout: panels::PanelLayout,
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

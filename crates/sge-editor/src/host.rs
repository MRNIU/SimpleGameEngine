// Copyright The SimpleGameEngine Contributors

use std::path::{Path, PathBuf};

use eframe::egui;
use sge_app::GameDescriptor;

use crate::{EditorOpenError, EditorSession, PreviewFrame, PreviewProbe, preview};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorRunOptions {
    pub max_frames: Option<u64>,
    pub initial_size: [u32; 2],
}

impl Default for EditorRunOptions {
    fn default() -> Self {
        Self {
            max_frames: None,
            initial_size: [1280, 720],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRunReport {
    pub preview: crate::PreviewProbeReport,
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
    let session = EditorSession::open(game, &project_root)?;
    let frame = session.preview_frame()?;
    let probe = PreviewProbe::default();
    let report_probe = probe.clone();
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
            Ok(Box::new(PreviewApp {
                session,
                frame,
                probe,
                project_root,
                max_frames: options.max_frames,
                frames: 0,
            }))
        }),
    )
    .map_err(|error| EditorRunError::Eframe(error.to_string()))?;
    let preview = report_probe.report();
    if let Some(error) = &preview.error {
        return Err(EditorRunError::Preview(error.clone()));
    }
    if preview.prepare_count == 0 || preview.paint_count == 0 {
        return Err(EditorRunError::PreviewIncomplete);
    }
    Ok(EditorRunReport { preview })
}

struct PreviewApp {
    session: EditorSession,
    frame: PreviewFrame,
    probe: PreviewProbe,
    project_root: PathBuf,
    max_frames: Option<u64>,
    frames: u64,
}

impl eframe::App for PreviewApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames = self.frames.saturating_add(1);
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
                ui.heading("SimpleGameEngine Preview");
                ui.separator();
                ui.label(format!(
                    "game_id: {}",
                    self.session.descriptor().game_id().as_str()
                ));
                ui.separator();
                ui.label(self.project_root.display().to_string());
            });
        });
        preview::paint(ui, &self.frame, &self.probe);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EditorRunError {
    #[error(transparent)]
    Open(#[from] EditorOpenError),
    #[error("initial Editor window size must be non-zero")]
    InvalidInitialSize,
    #[error("eframe Editor failed: {0}")]
    Eframe(String),
    #[error("Editor preview WGPU callback failed: {0}")]
    Preview(String),
    #[error("Editor preview WGPU callback did not prepare and paint")]
    PreviewIncomplete,
}

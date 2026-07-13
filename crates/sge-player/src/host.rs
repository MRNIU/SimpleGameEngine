// Copyright The SimpleGameEngine Contributors

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use sge_app::{AdvanceError, GameDescriptor};
use sge_render::{
    FramePerformanceMonitor, FramePerformanceSummary, FramePhaseDurations, RenderBackend,
    SkippedSurfaceFrame, SurfaceRenderError, SurfaceRenderOutcome, SurfaceRenderer,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::{PlayerFrameError, PlayerLoadError, PlayerSession, input::InputAccumulator};

const TITLE_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub max_frames: Option<u64>,
    pub initial_size: [u32; 2],
    pub screenshot: Option<PathBuf>,
    pub backend: RenderBackend,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            max_frames: None,
            initial_size: [1280, 720],
            screenshot: None,
            backend: RenderBackend::Wgpu,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    presented_frames: u64,
    input_frames: u64,
    performance: FramePerformanceSummary,
}

impl RunReport {
    #[must_use]
    pub const fn presented_frames(self) -> u64 {
        self.presented_frames
    }

    #[must_use]
    pub const fn input_frames(self) -> u64 {
        self.input_frames
    }

    #[must_use]
    pub const fn performance(self) -> FramePerformanceSummary {
        self.performance
    }
}

pub fn run(
    game: GameDescriptor,
    cooked_root: impl AsRef<Path>,
    options: RunOptions,
) -> Result<RunReport, PlayerRunError> {
    let session = PlayerSession::load(game, cooked_root)?;
    run_session(session, options)
}

pub fn run_session(
    session: PlayerSession,
    options: RunOptions,
) -> Result<RunReport, PlayerRunError> {
    if options.initial_size.contains(&0) {
        return Err(PlayerRunError::InvalidInitialSize);
    }
    if options.screenshot.is_some() && options.max_frames.is_some() {
        return Err(PlayerRunError::ScreenshotWithFrameLimit);
    }
    let event_loop = create_event_loop()?;
    let mut host = PlayerHost::new(session, options);
    event_loop.run_app(&mut host)?;
    if let Some(error) = host.error {
        return Err(error);
    }
    Ok(RunReport {
        presented_frames: host.presented_frames,
        input_frames: host.input_frames,
        performance: host.performance.summary(),
    })
}

fn create_event_loop() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_any_thread(true);
    }
    builder.build()
}

struct PlayerHost {
    session: PlayerSession,
    options: RunOptions,
    window: Option<Arc<Window>>,
    surface: Option<SurfaceRenderer<Window>>,
    last_redraw: Instant,
    last_title_update: Instant,
    presented_frames: u64,
    input_frames: u64,
    performance: FramePerformanceMonitor,
    occluded: bool,
    error: Option<PlayerRunError>,
    input: InputAccumulator,
}

impl PlayerHost {
    fn new(session: PlayerSession, options: RunOptions) -> Self {
        Self {
            session,
            options,
            window: None,
            surface: None,
            last_redraw: Instant::now(),
            last_title_update: Instant::now(),
            presented_frames: 0,
            input_frames: 0,
            performance: FramePerformanceMonitor::new(),
            occluded: false,
            error: None,
            input: InputAccumulator::default(),
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<PlayerRunError>) {
        self.error = Some(error.into());
        event_loop.exit();
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        if self.occluded {
            return;
        }
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_redraw);
        self.last_redraw = now;
        let input = self.input.take_frame();
        if !input.is_empty() {
            self.input_frames = self.input_frames.saturating_add(1);
        }
        let advance_started = Instant::now();
        if let Err(error) = self.session.advance(delta, input) {
            self.fail(event_loop, error);
            return;
        }
        let advance = advance_started.elapsed();
        let extract_started = Instant::now();
        let (snapshot, view) = match self.session.render_frame() {
            Ok(frame) => frame,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        let extract = extract_started.elapsed();
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let render_started = Instant::now();
        let render = if self.options.screenshot.is_some() {
            surface.render_with_readback(&snapshot, view, self.session.assets())
        } else {
            surface
                .render(&snapshot, view, self.session.assets())
                .map(|outcome| (outcome, None))
        };
        let render_duration = render_started.elapsed();
        let phases = FramePhaseDurations::new(advance, extract, render_duration);
        match render {
            Ok((SurfaceRenderOutcome::Presented, readback)) => {
                record_surface_outcome(
                    &mut self.performance,
                    SurfaceRenderOutcome::Presented,
                    phases,
                );
                self.presented_frames = self.presented_frames.saturating_add(1);
                if self.last_title_update.elapsed() >= TITLE_UPDATE_INTERVAL
                    && let Some(window) = &self.window
                {
                    self.last_title_update = Instant::now();
                    window.set_title(&player_window_title(
                        self.session.game_id(),
                        self.options.backend,
                        self.performance.summary().frames_per_second(),
                    ));
                }
                if let Some(path) = self.options.screenshot.take() {
                    let Some(readback) = readback else {
                        self.fail(event_loop, PlayerRunError::ScreenshotIncomplete);
                        return;
                    };
                    if let Err(error) = image::save_buffer_with_format(
                        &path,
                        readback.rgba(),
                        readback.size()[0],
                        readback.size()[1],
                        image::ColorType::Rgba8,
                        image::ImageFormat::Png,
                    ) {
                        self.fail(event_loop, PlayerRunError::Screenshot(path, error));
                        return;
                    }
                    event_loop.exit();
                    return;
                }
                if self
                    .options
                    .max_frames
                    .is_some_and(|max| self.presented_frames >= max)
                {
                    event_loop.exit();
                } else if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Ok((SurfaceRenderOutcome::Skipped(reason), _)) => {
                record_surface_outcome(
                    &mut self.performance,
                    SurfaceRenderOutcome::Skipped(reason),
                    phases,
                );
                if matches!(
                    reason,
                    SkippedSurfaceFrame::Timeout | SkippedSurfaceFrame::Outdated
                ) && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
            Err(error) => self.fail(event_loop, error),
        }
    }
}

impl ApplicationHandler for PlayerHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.options.max_frames == Some(0) {
            event_loop.exit();
            return;
        }
        if self.window.is_none() {
            let attributes = Window::default_attributes()
                .with_title(player_window_title(
                    self.session.game_id(),
                    self.options.backend,
                    None,
                ))
                .with_inner_size(LogicalSize::new(
                    f64::from(self.options.initial_size[0]),
                    f64::from(self.options.initial_size[1]),
                ));
            match event_loop.create_window(attributes) {
                Ok(window) => self.window = Some(Arc::new(window)),
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        }
        if self.surface.is_none() {
            let Some(window) = self.window.as_ref() else {
                return;
            };
            let size = window.inner_size();
            let surface = if self.options.screenshot.is_some() {
                SurfaceRenderer::new_with_readback_and_backend(
                    Arc::clone(window),
                    [size.width, size.height],
                    self.options.backend,
                )
            } else {
                SurfaceRenderer::new_with_backend(
                    Arc::clone(window),
                    [size.width, size.height],
                    self.options.backend,
                )
            };
            match surface {
                Ok(surface) => {
                    self.surface = Some(surface);
                }
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        }
        self.last_redraw = Instant::now();
        self.last_title_update = Instant::now();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.surface = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }
        self.input.handle_window_event(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if let Some(surface) = self.surface.as_mut()
                    && let Err(error) = surface.resize([width, height])
                {
                    self.fail(event_loop, error);
                } else if width > 0
                    && height > 0
                    && !self.occluded
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(window), Some(surface)) = (&self.window, self.surface.as_mut()) {
                    let size = window.inner_size();
                    if let Err(error) = surface.resize([size.width, size.height]) {
                        self.fail(event_loop, error);
                    } else if !self.occluded && size.width > 0 && size.height > 0 {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded && let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}

fn record_surface_outcome(
    performance: &mut FramePerformanceMonitor,
    outcome: SurfaceRenderOutcome,
    phases: FramePhaseDurations,
) {
    match outcome {
        SurfaceRenderOutcome::Presented => performance.record_completed(phases),
        SurfaceRenderOutcome::Skipped(reason) => performance.record_surface_skip(reason),
    }
}

fn player_window_title(
    game_id: &str,
    backend: RenderBackend,
    frames_per_second: Option<u32>,
) -> String {
    let fps = frames_per_second.map_or_else(|| "--".to_owned(), |value| value.to_string());
    format!("{game_id} | {} | FPS: {fps}", backend.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_title_exposes_backend_and_live_frame_rate() {
        assert_eq!(
            player_window_title("demo", RenderBackend::Cpu, Some(58)),
            "demo | CPU | FPS: 58"
        );
        assert_eq!(
            player_window_title("demo", RenderBackend::Wgpu, None),
            "demo | WGPU | FPS: --"
        );
    }

    #[test]
    fn skipped_surface_outcomes_do_not_record_completed_frames() {
        let mut performance = FramePerformanceMonitor::new();
        record_surface_outcome(
            &mut performance,
            SurfaceRenderOutcome::Skipped(SkippedSurfaceFrame::Timeout),
            FramePhaseDurations::default(),
        );

        let summary = performance.summary();
        assert_eq!(summary.sample_count(), 0);
        assert_eq!(summary.surface_skips().timeout(), 1);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerRunError {
    #[error(transparent)]
    Load(#[from] PlayerLoadError),
    #[error("initial player window size must be non-zero")]
    InvalidInitialSize,
    #[error("Player screenshot cannot be combined with a frame limit")]
    ScreenshotWithFrameLimit,
    #[error("cannot create player window: {0}")]
    Window(#[from] winit::error::OsError),
    #[error("player event loop failed: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error(transparent)]
    Advance(#[from] AdvanceError),
    #[error(transparent)]
    Frame(#[from] PlayerFrameError),
    #[error(transparent)]
    Surface(#[from] SurfaceRenderError),
    #[error("cannot save Player screenshot {0}: {1}")]
    Screenshot(PathBuf, image::ImageError),
    #[error("Player surface did not return a screenshot")]
    ScreenshotIncomplete,
}

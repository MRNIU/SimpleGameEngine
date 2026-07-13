// Copyright The SimpleGameEngine Contributors

use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use eframe::egui;

use crate::preview;

use super::{
    EditorApp, EditorUiAction, project_identity_labels, request_screenshot, save_screenshot,
};

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if context.current_pass_index() != 0 {
            return;
        }
        if let Some(build) = self.build.as_mut() {
            build.poll();
        }
        self.intercept_close_request(context);
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
            self.close_now(context);
            return;
        }
        if self
            .screenshot_requested_at
            .is_some_and(|requested| requested.elapsed() >= Duration::from_secs(5))
        {
            if let Some(sender) = self.screenshot_result.take() {
                let _ = sender.send(Err("GPU screenshot readback timed out".to_owned()));
            }
            self.close_now(context);
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
                        if self.pending_build_confirmation {
                            self.last_error = Some(
                                "Build action tape requires interactive save confirmation"
                                    .to_owned(),
                            );
                            self.ui_actions.clear();
                        } else {
                            self.pending_ui_build = true;
                        }
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
        if self.probe.error().is_some() || self.max_frames.is_some_and(|max| self.frames >= max) {
            self.close_now(context);
        } else {
            context.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let modal_open = self.pending_close_confirmation
            || self.pending_replacement.is_some()
            || self.pending_build_confirmation;
        if modal_open {
            ui.disable();
        } else {
            self.apply_editor_shortcuts(ui);
        }
        if !self.immersive_viewport {
            egui::Panel::top("project_identity").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let [game, project] = project_identity_labels(
                        self.session.descriptor().game_id().as_str(),
                        &self.project_root,
                    );
                    ui.label(game);
                    ui.separator();
                    ui.label(project)
                        .on_hover_text(self.project_root.display().to_string());
                    ui.separator();
                    self.file_controls(ui);
                    egui::ComboBox::from_id_salt("render_backend")
                        .selected_text(self.backend.label())
                        .show_ui(ui, |ui| {
                            for backend in sge_render::RenderBackend::ALL {
                                if ui
                                    .selectable_value(&mut self.backend, backend, backend.label())
                                    .changed()
                                {
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                    ui.monospace(frame_rate_label(self.probe.frames_per_second()));
                    ui.toggle_value(&mut self.performance_open, "Perf")
                        .on_hover_text("Performance");
                    if self.play.is_some() {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "PLAY");
                        if ui.button("Stop").clicked() {
                            let _ = self.apply_ui_action(EditorUiAction::StopPlay);
                        }
                    } else if ui
                        .add_enabled(
                            !self
                                .build
                                .as_ref()
                                .is_some_and(super::BuildProcess::is_running),
                            egui::Button::new("Play"),
                        )
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::StartPlay);
                    }
                    self.build_controls(ui);
                    if ui
                        .add_enabled(self.authoring_enabled(), egui::Button::new("Save"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Save);
                    }
                    if ui
                        .add_enabled(self.authoring_enabled(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Undo);
                    }
                    if ui
                        .add_enabled(self.authoring_enabled(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        let _ = self.apply_ui_action(EditorUiAction::Redo);
                    }
                    let (status, color) = if self.session.is_dirty() {
                        ("Modified", egui::Color32::GOLD)
                    } else {
                        ("Saved", egui::Color32::from_rgb(80, 170, 100))
                    };
                    ui.colored_label(color, status);
                });
            });
        }
        if !self.immersive_viewport {
            self.panel_layout
                .begin_frame(ui.ctx(), ui.available_width());
            self.inspector(ui);
            self.hierarchy(ui);
        }
        self.performance_panel(ui.ctx());

        if let Some(error) = self.last_error.clone() {
            let mut dismiss = false;
            egui::Panel::bottom("editor_error").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Error:");
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                    dismiss = ui.button("Dismiss").clicked();
                });
            });
            if dismiss {
                self.last_error = None;
            }
        }

        let response = if let Some(frame) = &self.frame {
            preview::paint(ui, frame, &self.probe, self.backend, |ui, rect| {
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
        if self.authoring_enabled()
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
        if self.pending_close_confirmation {
            self.close_confirmation_dialog(ui.ctx());
            return;
        }
        if self.pending_replacement.is_some() {
            self.unsaved_changes_dialog(ui.ctx());
            return;
        }
        if self.pending_build_confirmation {
            self.build_confirmation_dialog(ui.ctx());
            return;
        }
        if ui.ctx().current_pass_index() == 0 {
            self.advance_play(ui.ctx(), response.hovered());
        }
    }
}

fn frame_rate_label(frames_per_second: Option<u32>) -> String {
    frames_per_second.map_or_else(|| "FPS: --".to_owned(), |fps| format!("FPS: {fps}"))
}

#[cfg(test)]
mod tests {
    use super::frame_rate_label;

    #[test]
    fn frame_rate_label_has_pending_and_live_states() {
        assert_eq!(frame_rate_label(None), "FPS: --");
        assert_eq!(frame_rate_label(Some(60)), "FPS: 60");
    }
}

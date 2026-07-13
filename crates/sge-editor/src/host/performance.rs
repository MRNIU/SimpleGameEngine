// Copyright The SimpleGameEngine Contributors

use std::time::Duration;

use eframe::egui;
use sge_render::{FramePerformanceSummary, FrameTimeSummary};

use crate::{EditorLanguage, localization::EditorText};

use super::EditorApp;

impl EditorApp {
    pub(super) fn performance_panel(&mut self, context: &egui::Context) {
        if !self.performance_open {
            return;
        }
        let preview = self.probe.report().performance;
        let play = self.play_performance.summary();
        let language = self.language;
        egui::Window::new(language.text(EditorText::Performance))
            .open(&mut self.performance_open)
            .default_width(300.0)
            .show(context, |ui| {
                summary_ui(ui, language.text(EditorText::Preview), preview, language);
                ui.separator();
                summary_ui(ui, language.text(EditorText::Play), play, language);
            });
    }
}

fn summary_ui(
    ui: &mut egui::Ui,
    label: &str,
    summary: FramePerformanceSummary,
    language: EditorLanguage,
) {
    ui.strong(label);
    ui.monospace(format!(
        "FPS: {}  {}: {}",
        summary
            .frames_per_second()
            .map_or_else(|| "--".to_owned(), |fps| fps.to_string()),
        language.text(EditorText::Samples),
        summary.sample_count(),
    ));
    ui.label(frame_time_label(summary.frame_time()));
    ui.label(format!(
        "{}: {}/{}/{}",
        language.text(EditorText::AveragePhases),
        optional_duration_label(summary.average_advance()),
        optional_duration_label(summary.average_extract()),
        optional_duration_label(summary.average_render()),
    ));
    ui.label(format!(
        "{}: {}/{}",
        language.text(EditorText::OverBudgets),
        summary.frames_over_60_fps_budget(),
        summary.frames_over_30_fps_budget(),
    ));
}

fn frame_time_label(summary: Option<FrameTimeSummary>) -> String {
    summary.map_or_else(
        || "p50/p95/max: --".to_owned(),
        |summary| {
            format!(
                "p50/p95/max: {}/{}/{}",
                duration_label(summary.p50()),
                duration_label(summary.p95()),
                duration_label(summary.max()),
            )
        },
    )
}

fn optional_duration_label(duration: Option<Duration>) -> String {
    duration.map_or_else(|| "--".to_owned(), duration_label)
}

fn duration_label(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1_000.0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn duration_labels_use_milliseconds_with_two_decimals() {
        assert_eq!(duration_label(Duration::from_micros(1_250)), "1.25 ms");
    }

    #[test]
    fn missing_frame_time_has_a_pending_label() {
        assert_eq!(frame_time_label(None), "p50/p95/max: --");
    }
}

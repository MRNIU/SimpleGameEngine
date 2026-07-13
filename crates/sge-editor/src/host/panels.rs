// Copyright The SimpleGameEngine Contributors
//
//! Responsive panel sizing is shared here; panel contents stay in private
//! Hierarchy and Inspector modules.

use eframe::egui;

mod hierarchy;
mod inspector;

const HIERARCHY_PANEL_ID: &str = "hierarchy";
const INSPECTOR_PANEL_ID: &str = "inspector";
const DEFAULT_HIERARCHY_FRACTION: f32 = 0.18;
const DEFAULT_INSPECTOR_FRACTION: f32 = 0.24;
const MIN_HIERARCHY_FRACTION: f32 = 0.12;
const MIN_INSPECTOR_FRACTION: f32 = 0.18;
const MIN_VIEWPORT_FRACTION: f32 = 0.40;
const MAX_SIDE_PANEL_FRACTION: f32 = 0.35;

pub(super) struct PanelLayout {
    total_width: Option<f32>,
    hierarchy_fraction: f32,
    inspector_fraction: f32,
}

#[derive(Clone, Copy)]
struct PanelSizes {
    default: f32,
    minimum: f32,
    maximum: f32,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            total_width: None,
            hierarchy_fraction: DEFAULT_HIERARCHY_FRACTION,
            inspector_fraction: DEFAULT_INSPECTOR_FRACTION,
        }
    }
}

impl PanelLayout {
    pub(super) fn begin_frame(&mut self, context: &egui::Context, total_width: f32) {
        if self
            .total_width
            .is_some_and(|previous| (previous - total_width).abs() > 0.5)
        {
            context.data_mut(|data| {
                data.remove::<egui::containers::PanelState>(egui::Id::new(HIERARCHY_PANEL_ID));
                data.remove::<egui::containers::PanelState>(egui::Id::new(INSPECTOR_PANEL_ID));
            });
        }
        self.total_width = Some(total_width.max(f32::EPSILON));
    }

    fn inspector_sizes(&self) -> PanelSizes {
        self.sizes(
            self.inspector_fraction,
            MIN_INSPECTOR_FRACTION,
            MAX_SIDE_PANEL_FRACTION.min(1.0 - MIN_HIERARCHY_FRACTION - MIN_VIEWPORT_FRACTION),
        )
    }

    fn hierarchy_sizes(&self) -> PanelSizes {
        self.sizes(
            self.hierarchy_fraction,
            MIN_HIERARCHY_FRACTION,
            MAX_SIDE_PANEL_FRACTION
                .min(1.0 - self.inspector_fraction - MIN_VIEWPORT_FRACTION)
                .max(MIN_HIERARCHY_FRACTION),
        )
    }

    fn sizes(&self, default: f32, minimum: f32, maximum: f32) -> PanelSizes {
        let total_width = self.total_width.unwrap_or(1.0);
        PanelSizes {
            default: total_width * default.clamp(minimum, maximum),
            minimum: total_width * minimum,
            maximum: total_width * maximum,
        }
    }

    fn record_inspector(&mut self, width: f32) {
        self.inspector_fraction = self
            .width_fraction(width)
            .clamp(MIN_INSPECTOR_FRACTION, MAX_SIDE_PANEL_FRACTION);
    }

    fn record_hierarchy(&mut self, width: f32) {
        let maximum = MAX_SIDE_PANEL_FRACTION
            .min(1.0 - self.inspector_fraction - MIN_VIEWPORT_FRACTION)
            .max(MIN_HIERARCHY_FRACTION);
        self.hierarchy_fraction = self
            .width_fraction(width)
            .clamp(MIN_HIERARCHY_FRACTION, maximum);
    }

    fn width_fraction(&self, width: f32) -> f32 {
        width / self.total_width.unwrap_or(1.0)
    }
}

fn fill_resizable_panel(ui: &mut egui::Ui) {
    ui.take_available_space();
}

#[cfg(test)]
mod tests {
    use super::{HIERARCHY_PANEL_ID, INSPECTOR_PANEL_ID, PanelLayout, fill_resizable_panel};
    use eframe::egui;

    #[test]
    fn resizable_panel_keeps_its_configured_width_with_short_content() {
        let context = egui::Context::default();
        let mut width = 0.0;
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                let response = egui::Panel::left("resizable_panel_test")
                    .resizable(true)
                    .default_size(230.0)
                    .show(ui, |ui| {
                        ui.label("Short");
                        fill_resizable_panel(ui);
                    });
                width = response.response.rect.width();
            },
        );

        assert!((width - 230.0).abs() <= 1.0, "panel width was {width}");
    }

    #[test]
    fn percent_layout_scales_all_columns_with_the_window() {
        let context = egui::Context::default();
        let mut layout = PanelLayout::default();
        let small = panel_widths(&context, &mut layout, 1_000.0);
        let large = panel_widths(&context, &mut layout, 2_000.0);

        assert!((large.0 - small.0 * 2.0).abs() <= 2.0);
        assert!((large.1 - small.1 * 2.0).abs() <= 2.0);
        assert!((large.2 - small.2 * 2.0).abs() <= 2.0);
        assert!(small.2 >= 400.0);
    }

    fn panel_widths(
        context: &egui::Context,
        layout: &mut PanelLayout,
        width: f32,
    ) -> (f32, f32, f32) {
        let mut widths = (0.0, 0.0, 0.0);
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                layout.begin_frame(ui.ctx(), ui.available_width());
                let inspector = layout.inspector_sizes();
                let response = egui::Panel::right(INSPECTOR_PANEL_ID)
                    .resizable(true)
                    .default_size(inspector.default)
                    .min_size(inspector.minimum)
                    .max_size(inspector.maximum)
                    .show(ui, fill_resizable_panel);
                widths.1 = response.response.rect.width();
                layout.record_inspector(widths.1);

                let hierarchy = layout.hierarchy_sizes();
                let response = egui::Panel::left(HIERARCHY_PANEL_ID)
                    .resizable(true)
                    .default_size(hierarchy.default)
                    .min_size(hierarchy.minimum)
                    .max_size(hierarchy.maximum)
                    .show(ui, fill_resizable_panel);
                widths.0 = response.response.rect.width();
                layout.record_hierarchy(widths.0);
                widths.2 = ui.available_width();
            },
        );
        widths
    }
}

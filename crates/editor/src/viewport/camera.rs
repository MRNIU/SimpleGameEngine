// Copyright The SimpleGameEngine Contributors

use ecs::{EntityId, Projection};
use eframe::egui;
use math::{Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportProjection, ViewportView};

const EDITOR_VIEW_ENTITY: &str = "editor_view";
const LOOK_SENSITIVITY: f32 = 0.01;
const MOVE_SCALE: f32 = 4.0;
const SPEED_SCROLL_SCALE: f32 = 0.05;
const FIT_SCREEN_TO_WORLD_SCALE: f32 = 1.0 / 0.12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    speed: f32,
    mode: ViewMode,
    ortho_center: [f32; 3],
    ortho_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Perspective,
    Orthographic(ViewPreset),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ViewMoveInput {
    pub(crate) forward: bool,
    pub(crate) backward: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
}

impl ViewCamera {
    pub(crate) const MIN_SPEED: f32 = 0.05;
    pub(crate) const MAX_SPEED: f32 = 20.0;
    pub(crate) const MIN_PITCH: f32 = -1.45;
    pub(crate) const MAX_PITCH: f32 = 1.45;

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn pitch(self) -> f32 {
        self.pitch
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn speed(self) -> f32 {
        self.speed
    }

    pub(crate) fn look(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.yaw -= delta.x * LOOK_SENSITIVITY;
        self.pitch =
            (self.pitch - delta.y * LOOK_SENSITIVITY).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
    }

    pub(crate) fn adjust_speed(&mut self, scroll_y: f32) {
        if !scroll_y.is_finite() {
            return;
        }
        self.speed =
            (self.speed + scroll_y * SPEED_SCROLL_SCALE).clamp(Self::MIN_SPEED, Self::MAX_SPEED);
    }

    pub(crate) fn move_local(&mut self, input: ViewMoveInput, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let forward = self.forward();
        let right = self.right();
        let mut direction = Vec3::ZERO;
        if input.forward {
            direction += forward;
        }
        if input.backward {
            direction -= forward;
        }
        if input.right {
            direction += right;
        }
        if input.left {
            direction -= right;
        }
        if direction.length_squared() <= f32::EPSILON {
            return;
        }
        let moved =
            Vec3::from_array(self.position) + direction.normalize() * self.speed * dt * MOVE_SCALE;
        self.position = moved.to_array();
    }

    #[must_use]
    pub(crate) fn to_viewport_view(self) -> ViewportView {
        match self.mode {
            ViewMode::Perspective => ViewportView::new(
                EntityId::new(EDITOR_VIEW_ENTITY),
                Transform {
                    translation: self.position,
                    rotation: self.rotation().to_array(),
                    scale: [1.0, 1.0, 1.0],
                },
                Projection::Perspective {
                    fov_y_degrees: 60.0,
                },
            ),
            ViewMode::Orthographic(preset) => ViewportView::new(
                EntityId::new(EDITOR_VIEW_ENTITY),
                Transform {
                    translation: self.ortho_center,
                    rotation: preset_rotation(preset).to_array(),
                    scale: [1.0, 1.0, 1.0],
                },
                Projection::Orthographic {
                    vertical_size: self.ortho_scale,
                },
            ),
        }
    }

    pub(crate) fn fit_draw(
        &mut self,
        draw: &ViewportDrawCall,
        selected: Option<&EntityId>,
    ) -> bool {
        let Some((min, max, center)) = visible_bounds(draw, selected) else {
            return false;
        };
        match self.mode {
            ViewMode::Perspective => {
                let Some(projected) = ViewportProjection::from_view(&self.to_viewport_view())
                    .and_then(|projection| projection.project_world_point(center.to_array()))
                else {
                    return false;
                };
                let offset = self.rotation()
                    * Vec3::new(
                        projected[0] * FIT_SCREEN_TO_WORLD_SCALE,
                        projected[1] * FIT_SCREEN_TO_WORLD_SCALE,
                        0.0,
                    );
                if !offset.is_finite() {
                    return false;
                }
                self.position = (Vec3::from_array(self.position) + offset).to_array();
            }
            ViewMode::Orthographic(_) => {
                self.ortho_center = center.to_array();
                self.ortho_scale = (max - min).max_element().max(0.5);
            }
        }
        true
    }

    pub(crate) fn set_preset(&mut self, preset: ViewPreset) {
        self.mode = ViewMode::Orthographic(preset);
    }

    pub(crate) fn return_to_perspective(&mut self) {
        self.mode = ViewMode::Perspective;
    }

    #[must_use]
    pub(crate) fn view_mode_label(&self) -> &'static str {
        match self.mode {
            ViewMode::Perspective => "Perspective",
            ViewMode::Orthographic(ViewPreset::Top) => "Top Orthographic",
            ViewMode::Orthographic(ViewPreset::Bottom) => "Bottom Orthographic",
            ViewMode::Orthographic(ViewPreset::Front) => "Front Orthographic",
            ViewMode::Orthographic(ViewPreset::Back) => "Back Orthographic",
            ViewMode::Orthographic(ViewPreset::Right) => "Right Orthographic",
            ViewMode::Orthographic(ViewPreset::Left) => "Left Orthographic",
        }
    }

    #[must_use]
    pub(crate) fn hint_text(
        &self,
        draw: Option<&ViewportDrawCall>,
        selected: Option<&EntityId>,
    ) -> String {
        match self.mode {
            ViewMode::Perspective => {
                let distance = draw
                    .and_then(|draw| visible_center(draw, selected))
                    .unwrap_or(Vec3::ZERO)
                    .distance(Vec3::from_array(self.position));
                format!(
                    "Perspective  Speed {:.2}  Distance {:.2}",
                    self.speed, distance
                )
            }
            ViewMode::Orthographic(_) => {
                format!(
                    "{}  Ortho Scale {:.2}",
                    self.view_mode_label(),
                    self.ortho_scale
                )
            }
        }
    }

    fn rotation(self) -> Quat {
        let yaw = Quat::from_rotation_z(self.yaw);
        let right = yaw * Vec3::Y;
        Quat::from_axis_angle(right, self.pitch) * yaw
    }

    fn forward(self) -> Vec3 {
        self.rotation() * Vec3::X
    }

    fn right(self) -> Vec3 {
        self.rotation() * Vec3::Y
    }
}

impl Default for ViewCamera {
    fn default() -> Self {
        Self {
            position: [-5.0, -5.0, 4.0],
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: -0.45,
            speed: 1.0,
            mode: ViewMode::Perspective,
            ortho_center: [0.0, 0.0, 0.0],
            ortho_scale: 5.0,
        }
    }
}

fn preset_rotation(preset: ViewPreset) -> Quat {
    let (forward, up) = match preset {
        ViewPreset::Top => (Vec3::NEG_Z, Vec3::X),
        ViewPreset::Bottom => (Vec3::Z, Vec3::X),
        ViewPreset::Front => (Vec3::X, Vec3::Z),
        ViewPreset::Back => (Vec3::NEG_X, Vec3::Z),
        ViewPreset::Right => (Vec3::NEG_Y, Vec3::Z),
        ViewPreset::Left => (Vec3::Y, Vec3::Z),
    };
    let right = forward.cross(up).normalize_or_zero();
    Quat::from_mat3(&math::Mat3::from_cols(forward, right, up))
}

fn visible_center(draw: &ViewportDrawCall, selected: Option<&EntityId>) -> Option<Vec3> {
    visible_bounds(draw, selected).map(|(_, _, center)| center)
}

fn visible_bounds(
    draw: &ViewportDrawCall,
    selected: Option<&EntityId>,
) -> Option<(Vec3, Vec3, Vec3)> {
    if let Some(span) =
        selected.and_then(|id| draw.mesh_spans.iter().find(|span| &span.entity == id))
    {
        let min = Vec3::from_array(span.world_bounds_min);
        let max = Vec3::from_array(span.world_bounds_max);
        let center = Vec3::from_array(span.world_center);
        return (min.is_finite() && max.is_finite() && center.is_finite())
            .then_some((min, max, center));
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for span in &draw.mesh_spans {
        let span_min = Vec3::from_array(span.world_bounds_min);
        let span_max = Vec3::from_array(span.world_bounds_max);
        if !span_min.is_finite() || !span_max.is_finite() {
            continue;
        }
        min = min.min(span_min);
        max = max.max(span_max);
        found = true;
    }
    let center = (min + max) * 0.5;
    (found && center.is_finite()).then_some((min, max, center))
}

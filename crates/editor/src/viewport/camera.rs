// Copyright The SimpleGameEngine Contributors

use ecs::{EntityId, Projection};
use eframe::egui;
use math::{Mat3, Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportView};

const EDITOR_VIEW_ENTITY: &str = "editor_view";
const LOOK_SENSITIVITY: f32 = 0.01;
const ORBIT_SENSITIVITY: f32 = 0.01;
const PAN_SENSITIVITY: f32 = 0.0025;
const DOLLY_SENSITIVITY: f32 = 0.02;
const MOVE_SCALE: f32 = 4.0;
const SPEED_SCROLL_SCALE: f32 = 0.05;
const DEFAULT_ORBIT_DISTANCE: f32 = 8.0;
const DEFAULT_FOV_Y_DEGREES: f32 = 60.0;
const FRAME_MARGIN: f32 = 1.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    speed: f32,
    orbit_pivot: [f32; 3],
    orbit_distance: f32,
    fov_y_degrees: f32,
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
    pub(crate) const MIN_ORBIT_DISTANCE: f32 = 0.25;

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn pitch(self) -> f32 {
        self.pitch
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn yaw(self) -> f32 {
        self.yaw
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn speed(self) -> f32 {
        self.speed
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn orbit_pivot(self) -> [f32; 3] {
        self.orbit_pivot
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn orbit_distance(self) -> f32 {
        self.orbit_distance
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn fov_y_degrees(self) -> f32 {
        self.fov_y_degrees
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn basis(self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        (
            self.forward().to_array(),
            self.right().to_array(),
            self.up().to_array(),
        )
    }

    pub(crate) fn look(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.return_to_perspective();
        self.yaw += delta.x * LOOK_SENSITIVITY;
        self.pitch =
            (self.pitch - delta.y * LOOK_SENSITIVITY).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
        self.sync_pivot_from_position();
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
        self.return_to_perspective();
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
        let movement = direction.normalize() * self.speed * dt * MOVE_SCALE;
        let moved = Vec3::from_array(self.position) + movement;
        if !moved.is_finite() {
            return;
        }
        self.position = moved.to_array();
        let pivot = Vec3::from_array(self.orbit_pivot) + movement;
        if pivot.is_finite() {
            self.orbit_pivot = pivot.to_array();
        }
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
                    fov_y_degrees: self.fov_y_degrees,
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

    #[cfg(test)]
    pub(crate) fn fit_draw(
        &mut self,
        draw: &ViewportDrawCall,
        selected: Option<&EntityId>,
    ) -> bool {
        self.frame_visible(Some(draw), selected)
    }

    pub(crate) fn frame_visible(
        &mut self,
        draw: Option<&ViewportDrawCall>,
        selected: Option<&EntityId>,
    ) -> bool {
        let (min, max, center) = draw
            .and_then(|draw| visible_bounds(draw, selected))
            .unwrap_or((Vec3::ZERO, Vec3::ZERO, Vec3::ZERO));
        let distance = frame_distance(min, max);
        self.orbit_pivot = center.to_array();
        self.orbit_distance = distance;
        self.fov_y_degrees = DEFAULT_FOV_Y_DEGREES;
        self.update_position_from_pivot();
        match self.mode {
            ViewMode::Perspective => {}
            ViewMode::Orthographic(_) => {
                self.ortho_center = center.to_array();
                self.ortho_scale = (max - min).max_element().max(2.0);
            }
        }
        true
    }

    pub(crate) fn begin_navigation(
        &mut self,
        draw: Option<&ViewportDrawCall>,
        selected: Option<&EntityId>,
    ) {
        if matches!(self.mode, ViewMode::Orthographic(_)) {
            self.mode = ViewMode::Perspective;
            self.frame_visible(draw, selected);
        } else {
            self.return_to_perspective();
        }
    }

    pub(crate) fn orbit(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.return_to_perspective();
        self.yaw += delta.x * ORBIT_SENSITIVITY;
        self.pitch =
            (self.pitch - delta.y * ORBIT_SENSITIVITY).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
        self.update_position_from_pivot();
    }

    pub(crate) fn pan(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.return_to_perspective();
        let scale = (self.orbit_distance * PAN_SENSITIVITY).clamp(0.0025, 0.2);
        let movement = (-self.right() * delta.x + self.up() * delta.y) * scale;
        if !movement.is_finite() {
            return;
        }
        self.position = (Vec3::from_array(self.position) + movement).to_array();
        self.orbit_pivot = (Vec3::from_array(self.orbit_pivot) + movement).to_array();
    }

    pub(crate) fn dolly(&mut self, delta_y: f32) {
        if !delta_y.is_finite() {
            return;
        }
        self.return_to_perspective();
        let next = self.orbit_distance * (1.0 + delta_y * DOLLY_SENSITIVITY);
        if !next.is_finite() {
            return;
        }
        self.orbit_distance = next.clamp(Self::MIN_ORBIT_DISTANCE, 10_000.0);
        self.update_position_from_pivot();
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
                    "Perspective  Camera Speed {:.2}  Distance {:.2}",
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
        let forward = self.forward();
        let right = self.right();
        let up = self.up();
        Quat::from_mat3(&Mat3::from_cols(right, up, forward))
    }

    fn forward(self) -> Vec3 {
        let yaw = Quat::from_rotation_z(self.yaw);
        let flat_forward = yaw * Vec3::X;
        let flat_right = yaw * Vec3::Y;
        (Quat::from_axis_angle(flat_right, -self.pitch) * flat_forward).normalize_or_zero()
    }

    fn right(self) -> Vec3 {
        (Quat::from_rotation_z(self.yaw) * Vec3::Y).normalize_or_zero()
    }

    fn up(self) -> Vec3 {
        self.forward().cross(self.right()).normalize_or_zero()
    }

    fn update_position_from_pivot(&mut self) {
        let position = Vec3::from_array(self.orbit_pivot) - self.centered_pivot_offset();
        if position.is_finite() {
            self.position = position.to_array();
        }
    }

    fn sync_pivot_from_position(&mut self) {
        let pivot = Vec3::from_array(self.position) + self.centered_pivot_offset();
        if pivot.is_finite() {
            self.orbit_pivot = pivot.to_array();
        }
    }

    fn centered_pivot_offset(self) -> Vec3 {
        self.forward() * self.orbit_distance
    }
}

impl Default for ViewCamera {
    fn default() -> Self {
        Self {
            position: [-5.0, -5.0, 4.0],
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: -0.45,
            speed: 1.0,
            orbit_pivot: [0.0, 0.0, 0.0],
            orbit_distance: DEFAULT_ORBIT_DISTANCE,
            fov_y_degrees: DEFAULT_FOV_Y_DEGREES,
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
        if min.is_finite() && max.is_finite() && center.is_finite() {
            return Some((min, max, center));
        }
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

fn frame_distance(min: Vec3, max: Vec3) -> f32 {
    let extent = max - min;
    let radius = (extent.length() * 0.5).max(extent.max_element() * 0.5);
    if !radius.is_finite() || radius <= f32::EPSILON {
        return DEFAULT_ORBIT_DISTANCE;
    }
    (radius / (30.0_f32.to_radians()).tan() * FRAME_MARGIN)
        .clamp(ViewCamera::MIN_ORBIT_DISTANCE, 10_000.0)
}

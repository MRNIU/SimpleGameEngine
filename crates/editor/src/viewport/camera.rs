// Copyright The SimpleGameEngine Contributors

use ecs::{EntityId, Projection};
use eframe::egui;
use math::{Mat3, Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportView};

use super::{
    ViewportNavigationGesture,
    grid::{GridPlane, GridState, grid_plane_for_preset},
};

const EDITOR_VIEW_ENTITY: &str = "editor_view";
const LOOK_SENSITIVITY: f32 = 0.01;
const ORBIT_SENSITIVITY: f32 = 0.01;
const PAN_SENSITIVITY: f32 = 0.0025;
const DOLLY_SENSITIVITY: f32 = 0.02;
const DEFAULT_ORBIT_DISTANCE: f32 = 8.0;
const ORTHOGRAPHIC_CAMERA_DISTANCE: f32 = 1_000.0;
const DEFAULT_HORIZONTAL_FOV_DEGREES: f32 = 90.0;
const DEFAULT_SPEED_LEVEL: u8 = 4;
const DEFAULT_SPEED_SCALAR: f32 = 1.0;
const SPEED_MULTIPLIERS: [f32; 8] = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
const BASE_MOVE_SPEED: f32 = 4.0;
const FRAME_MARGIN: f32 = 1.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    speed_level: u8,
    speed_scalar: f32,
    orbit_pivot: [f32; 3],
    orbit_distance: f32,
    horizontal_fov_degrees: f32,
    mode: ViewMode,
    ortho_center: [f32; 3],
    ortho_scale: f32,
    grid: GridState,
    navigation_gesture: Option<ViewportNavigationGesture>,
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
    pub(crate) up: bool,
    pub(crate) down: bool,
}

impl ViewCamera {
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

    #[must_use]
    pub(crate) const fn horizontal_fov_degrees(self) -> f32 {
        self.horizontal_fov_degrees
    }

    #[must_use]
    pub(crate) const fn speed_level(self) -> u8 {
        self.speed_level
    }

    pub(crate) fn set_speed_level(&mut self, level: u8) {
        self.speed_level = level.clamp(1, 8);
    }

    pub(crate) fn adjust_speed_level(&mut self, delta: i8) {
        let level = i16::from(self.speed_level) + i16::from(delta);
        self.speed_level = level.clamp(1, 8) as u8;
    }

    #[must_use]
    pub(crate) const fn speed_scalar(self) -> f32 {
        self.speed_scalar
    }

    pub(crate) fn set_speed_scalar(&mut self, scalar: f32) {
        if scalar.is_finite() {
            self.speed_scalar = scalar.clamp(0.1, 10.0);
        }
    }

    #[must_use]
    pub(crate) fn effective_speed(self) -> f32 {
        BASE_MOVE_SPEED
            * SPEED_MULTIPLIERS[usize::from(self.speed_level.saturating_sub(1))]
            * self.speed_scalar
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
        if input.up {
            direction += Vec3::Z;
        }
        if input.down {
            direction -= Vec3::Z;
        }
        if direction.length_squared() <= f32::EPSILON {
            return;
        }
        let movement = direction.normalize() * self.effective_speed() * dt;
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
    pub(crate) fn to_viewport_view(self, viewport_size: render::ViewportSize) -> ViewportView {
        match self.mode {
            ViewMode::Perspective => ViewportView::new(
                EntityId::new(EDITOR_VIEW_ENTITY),
                Transform {
                    translation: self.position,
                    rotation: self.rotation().to_array(),
                    scale: [1.0, 1.0, 1.0],
                },
                Projection::Perspective {
                    fov_y_degrees: vertical_fov_degrees(
                        self.horizontal_fov_degrees,
                        viewport_size.aspect_ratio(),
                    ),
                },
            ),
            ViewMode::Orthographic(preset) => {
                let rotation = preset_rotation(preset);
                let forward = rotation * Vec3::Z;
                ViewportView::new(
                    EntityId::new(EDITOR_VIEW_ENTITY),
                    Transform {
                        translation: (Vec3::from_array(self.ortho_center)
                            - forward * ORTHOGRAPHIC_CAMERA_DISTANCE)
                            .to_array(),
                        rotation: rotation.to_array(),
                        scale: [1.0, 1.0, 1.0],
                    },
                    Projection::Orthographic {
                        vertical_size: self.ortho_scale,
                    },
                )
            }
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
        self.horizontal_fov_degrees = DEFAULT_HORIZONTAL_FOV_DEGREES;
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

    pub(crate) fn begin_navigation(&mut self) {
        if matches!(self.mode, ViewMode::Perspective) {
            self.return_to_perspective();
        }
    }

    #[must_use]
    pub(crate) const fn navigation_gesture(self) -> Option<ViewportNavigationGesture> {
        self.navigation_gesture
    }

    pub(crate) const fn set_navigation_gesture(
        &mut self,
        gesture: Option<ViewportNavigationGesture>,
    ) {
        self.navigation_gesture = gesture;
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

    pub(crate) fn lmb_navigate(&mut self, delta: egui::Vec2) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        self.look(egui::vec2(delta.x, 0.0));
        self.wheel_move(-delta.y.signum());
    }

    pub(crate) fn wheel_move(&mut self, wheel_y: f32) {
        if !wheel_y.is_finite() || wheel_y == 0.0 {
            return;
        }
        let movement = self.forward() * wheel_y.signum() * self.effective_speed() * 0.25;
        self.position = (Vec3::from_array(self.position) + movement).to_array();
        self.orbit_pivot = (Vec3::from_array(self.orbit_pivot) + movement).to_array();
    }

    pub(crate) fn ortho_pan(&mut self, delta: egui::Vec2) {
        let ViewMode::Orthographic(preset) = self.mode else {
            return;
        };
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        let rotation = preset_rotation(preset);
        let right = rotation * Vec3::X;
        let up = rotation * Vec3::Y;
        let scale = self.ortho_scale * 0.0015;
        let movement = (-right * delta.x + up * delta.y) * scale;
        if movement.is_finite() {
            self.ortho_center = (Vec3::from_array(self.ortho_center) + movement).to_array();
        }
    }

    pub(crate) fn ortho_zoom(&mut self, delta: f32) {
        if !matches!(self.mode, ViewMode::Orthographic(_)) || !delta.is_finite() {
            return;
        }
        self.ortho_scale = (self.ortho_scale * (-delta * 0.1).exp()).clamp(0.01, 100_000.0);
    }

    #[must_use]
    pub(crate) const fn is_orthographic(self) -> bool {
        matches!(self.mode, ViewMode::Orthographic(_))
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

    #[must_use]
    pub(crate) const fn grid_plane(self) -> GridPlane {
        match self.mode {
            ViewMode::Perspective => GridPlane::XY,
            ViewMode::Orthographic(preset) => grid_plane_for_preset(preset),
        }
    }

    #[must_use]
    pub(crate) const fn grid_minor_step(self) -> f32 {
        self.grid.minor_step
    }

    pub(crate) fn set_grid_minor_step(&mut self, minor_step: f32) {
        self.grid.minor_step = minor_step;
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
                    "Perspective\nFOV X {:.0}  Camera Speed {:.2}  Distance {:.2}",
                    self.horizontal_fov_degrees(),
                    self.effective_speed(),
                    distance
                )
            }
            ViewMode::Orthographic(_) => {
                format!(
                    "{}\nOrtho Scale {:.2}",
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
            speed_level: DEFAULT_SPEED_LEVEL,
            speed_scalar: DEFAULT_SPEED_SCALAR,
            orbit_pivot: [0.0, 0.0, 0.0],
            orbit_distance: DEFAULT_ORBIT_DISTANCE,
            horizontal_fov_degrees: DEFAULT_HORIZONTAL_FOV_DEGREES,
            mode: ViewMode::Perspective,
            ortho_center: [0.0, 0.0, 0.0],
            ortho_scale: 5.0,
            grid: GridState::DEFAULT,
            navigation_gesture: None,
        }
    }
}

fn vertical_fov_degrees(horizontal_fov_degrees: f32, aspect: f32) -> f32 {
    let half_x = 0.5 * horizontal_fov_degrees.clamp(1.0, 179.0).to_radians();
    (2.0 * (half_x.tan() / aspect.max(f32::EPSILON)).atan()).to_degrees()
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
    let right = up.cross(forward).normalize_or_zero();
    Quat::from_mat3(&math::Mat3::from_cols(right, up, forward))
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

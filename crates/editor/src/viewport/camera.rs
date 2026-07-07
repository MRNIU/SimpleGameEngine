// Copyright The SimpleGameEngine Contributors

use ecs::EntityId;
use eframe::egui;
use math::{Quat, Transform, Vec3};
use render::{ViewportDrawCall, ViewportView};

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
        let rotation = self.rotation();
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
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
        ViewportView::new(
            EntityId::new(EDITOR_VIEW_ENTITY),
            Transform {
                translation: self.position,
                rotation: self.rotation().to_array(),
                scale: [1.0, 1.0, 1.0],
            },
        )
    }

    pub(crate) fn fit_draw(
        &mut self,
        draw: &ViewportDrawCall,
        selected: Option<&EntityId>,
    ) -> bool {
        let selected_span = selected
            .and_then(|id| draw.cube_spans.iter().find(|span| &span.entity == id))
            .or_else(|| draw.cube_spans.first().filter(|_| selected.is_some()));
        let mut center = Vec3::ZERO;
        let mut count = 0usize;
        let mut accumulate_span = |span: &render::ViewportCubeSpan| {
            for index in span.vertex_range.clone() {
                let Some(vertex) = draw.vertices.get(index) else {
                    continue;
                };
                center += Vec3::from_array(vertex.position);
                count += 1;
            }
        };

        if let Some(span) = selected_span {
            accumulate_span(span);
        } else {
            for span in &draw.cube_spans {
                accumulate_span(span);
            }
        }
        if count == 0 {
            return false;
        }
        let center = center / count as f32;
        if !center.is_finite() {
            return false;
        }
        let moved = Vec3::from_array(self.position)
            + self.rotation()
                * Vec3::new(
                    center.x * FIT_SCREEN_TO_WORLD_SCALE,
                    center.y * FIT_SCREEN_TO_WORLD_SCALE,
                    0.0,
                );
        self.position = moved.to_array();
        true
    }

    fn rotation(self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }
}

impl Default for ViewCamera {
    fn default() -> Self {
        Self {
            position: [0.0, 2.0, 5.0],
            yaw: 0.0,
            pitch: 0.0,
            speed: 1.0,
        }
    }
}

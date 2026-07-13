// Copyright The SimpleGameEngine Contributors
//
//! Authoring camera and gesture-priority state stay here. Private gizmo,
//! projection, overlay and ViewCube geometry live in sibling modules without
//! widening this host-only contract.

use eframe::egui;
use sge_math::{Mat3, Quat, Transform, Vec3};
use sge_reflect::Value;
use sge_render::{Camera, RenderView};
use sge_scene::SceneEntityId;

use crate::{EditSession, EditorLanguage, PreviewFrame, localization::EditorText};

mod actor_visuals;
mod gizmo;
mod overlays;
mod view_cube;

use gizmo::{
    GizmoDrag, GizmoMode, gizmo_handles, gizmo_transform, paint_gizmo, pick_mesh,
    update_drag_preview,
};
use overlays::{draw_grid, draw_world_axes};
use view_cube::{point_in_polygon, preset_axes, view_cube_faces};

#[cfg(test)]
use gizmo::{Axis, transform_for_drag, triangle_depth};
#[cfg(test)]
use overlays::{ScreenPoint, line_count, project_segment, visible_grid_layout};

const CAMERA_MOVE_SPEED: f32 = 4.0;
const CAMERA_SPEED_MULTIPLIERS: [f32; 8] = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
const CAMERA_LOOK_SENSITIVITY: f32 = 0.01;
const CAMERA_MIN_ELEVATION: f32 = -1.45;
const CAMERA_MAX_ELEVATION: f32 = 1.45;

pub(crate) struct EditorViewport {
    transform: Transform,
    camera: Camera,
    pivot: Vec3,
    distance: f32,
    initialized: bool,
    gizmo: GizmoMode,
    drag: Option<GizmoDrag>,
    orbiting: bool,
    cube_pointer_active: bool,
    camera_speed_level: u8,
    game_view: bool,
}

impl Default for EditorViewport {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            camera: Camera::default(),
            pivot: Vec3::ZERO,
            distance: 8.0,
            initialized: false,
            gizmo: GizmoMode::Move,
            drag: None,
            orbiting: false,
            cube_pointer_active: false,
            camera_speed_level: 4,
            game_view: false,
        }
    }
}

impl EditorViewport {
    pub(crate) fn drag_preview(&self) -> Option<(SceneEntityId, Transform)> {
        self.drag.map(|drag| (drag.entity, drag.preview))
    }

    pub(crate) fn prepare(&mut self, frame: &mut PreviewFrame) {
        if !self.initialized {
            self.camera = frame.view.camera();
            if let Some((minimum, maximum)) = scene_bounds(frame) {
                self.pivot = (minimum + maximum) * 0.5;
                self.distance =
                    frame_distance(minimum, maximum, self.camera.vertical_fov_radians());
            }
            self.transform.rotation = initial_camera_rotation().to_array();
            self.sync_position_from_pivot();
            self.initialized = true;
        }
        frame.view = RenderView::editor(self.transform, self.camera);
        if let Some(drag) = &self.drag
            && let Some((_, runtime)) = frame
                .scene_entities
                .iter()
                .find(|(scene, _)| *scene == drag.entity)
        {
            let _ = frame.snapshot.set_mesh_transform(*runtime, drag.preview);
        }
    }

    pub(crate) fn interact(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        frame: &PreviewFrame,
        session: &mut EditSession,
        language: EditorLanguage,
    ) -> Result<(), crate::EditError> {
        self.update_mode(ui, response);
        let overlay_consumed = if self.game_view {
            false
        } else {
            draw_world_axes(ui, response.rect, frame);
            actor_visuals::paint(
                ui,
                response.rect,
                frame,
                session.selection(),
                self.drag_preview(),
            );
            self.draw_view_cube(ui, response.rect)
        };
        let camera_consumed = self.navigate(ui, response, session);
        let gizmo_consumed = if camera_consumed {
            false
        } else {
            self.gizmo(ui, response, frame, session)?
        };
        if !overlay_consumed && !camera_consumed && !gizmo_consumed {
            self.select(response, frame, session)?;
        }
        self.paint_status(ui, response.rect, language);
        Ok(())
    }

    pub(crate) fn paint_background(&self, ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
        if !self.game_view {
            draw_grid(ui, rect, frame);
        }
    }

    fn navigate(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        session: &EditSession,
    ) -> bool {
        let keyboard_capture =
            viewport_keyboard_capture(response.has_focus(), ui.ctx().text_edit_focused());
        let (
            delta,
            primary_down,
            middle_down,
            secondary_down,
            alt,
            scroll,
            escape,
            frame_selected,
            stable_dt,
        ) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.pointer.primary_down(),
                input.pointer.middle_down(),
                input.pointer.secondary_down(),
                input.modifiers.alt,
                input.smooth_scroll_delta.y,
                keyboard_capture && input.key_pressed(egui::Key::Escape),
                frame_selected_requested(
                    keyboard_capture,
                    input.pointer.secondary_down(),
                    input.key_pressed(egui::Key::F),
                ),
                input.stable_dt,
            )
        });
        if escape {
            self.drag = None;
        }
        if alt && primary_down && response.hovered() {
            self.orbiting = true;
            self.drag = None;
        }
        if response.hovered()
            && frame_selected
            && let Some(entity) = session.selection()
            && let Some(transform) = session.component::<Transform>(entity)
        {
            self.pivot = Vec3::from_array(transform.translation);
            self.sync_position_from_pivot();
        }
        let finishing_orbit = self.orbiting && !primary_down;
        let orbit = self.orbiting && primary_down;
        let pan = alt && middle_down && response.dragged_by(egui::PointerButton::Middle);
        let dolly = alt && secondary_down && response.dragged_by(egui::PointerButton::Secondary);
        let vertical = vertical_navigation_requested(
            alt,
            primary_down,
            secondary_down,
            response.hovered(),
            response.dragged(),
        );
        let look = !alt && !primary_down && response.dragged_by(egui::PointerButton::Secondary);
        let lmb_navigate = !alt
            && !secondary_down
            && self.drag.is_none()
            && response.dragged_by(egui::PointerButton::Primary);
        if orbit || look {
            self.transform.rotation =
                camera_look_rotation(Quat::from_array(self.transform.rotation), delta).to_array();
            if orbit {
                self.sync_position_from_pivot();
            } else {
                self.sync_pivot_from_position();
            }
        }
        if pan {
            let rotation = Quat::from_array(self.transform.rotation);
            let movement = camera_pan_motion(rotation, self.distance, delta);
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + movement).to_array();
            self.pivot += movement;
        }
        if dolly {
            self.distance = dolly_distance(self.distance, delta.y);
            self.sync_position_from_pivot();
        }
        if vertical {
            let movement = Vec3::Z * -delta.y * self.effective_camera_speed() * 0.01;
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + movement).to_array();
            self.pivot += movement;
        }
        if lmb_navigate {
            let rotation = camera_yaw_rotation(Quat::from_array(self.transform.rotation), delta.x);
            self.transform.rotation = rotation.to_array();
            let movement =
                camera_lmb_forward_motion(rotation, self.effective_camera_speed(), delta.y);
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + movement).to_array();
            self.sync_pivot_from_position();
        }
        if response.hovered() && scroll != 0.0 {
            if secondary_down {
                self.adjust_camera_speed(scroll.signum() as i8);
            } else {
                let forward = Quat::from_array(self.transform.rotation) * Vec3::Z;
                self.transform.translation = (Vec3::from_array(self.transform.translation)
                    + forward * scroll.signum() * self.effective_camera_speed() * 0.25)
                    .to_array();
                self.sync_pivot_from_position();
            }
        }
        if keyboard_capture && response.hovered() && secondary_down {
            let (local_movement, global_vertical) =
                ui.input(|input| camera_fly_axes(|key| input.key_down(key)));
            let rotation = Quat::from_array(self.transform.rotation);
            let motion = rotation * local_movement + Vec3::Z * global_vertical;
            let motion = camera_fly_motion(motion, self.effective_camera_speed(), stable_dt);
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + motion).to_array();
            self.sync_pivot_from_position();
        }
        if finishing_orbit {
            self.orbiting = false;
        }
        orbit
            || finishing_orbit
            || pan
            || dolly
            || vertical
            || look
            || lmb_navigate
            || (response.hovered() && secondary_down)
    }

    fn update_mode(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if !viewport_keyboard_capture(response.has_focus(), ui.ctx().text_edit_focused())
            || ui.input(|input| input.pointer.secondary_down())
        {
            return;
        }
        ui.input(|input| {
            if input.key_pressed(egui::Key::Q) {
                self.gizmo = GizmoMode::Select;
            } else if input.key_pressed(egui::Key::W) {
                self.gizmo = GizmoMode::Move;
            } else if input.key_pressed(egui::Key::E) {
                self.gizmo = GizmoMode::Rotate;
            } else if input.key_pressed(egui::Key::R) {
                self.gizmo = GizmoMode::Scale;
            } else if input.key_pressed(egui::Key::Space) {
                self.gizmo = self.gizmo.next();
            } else if input.key_pressed(egui::Key::G) {
                self.game_view = !self.game_view;
            }
        });
    }

    fn effective_camera_speed(&self) -> f32 {
        CAMERA_MOVE_SPEED
            * CAMERA_SPEED_MULTIPLIERS[usize::from(self.camera_speed_level.saturating_sub(1))]
    }

    fn adjust_camera_speed(&mut self, delta: i8) {
        self.camera_speed_level =
            (i16::from(self.camera_speed_level) + i16::from(delta)).clamp(1, 8) as u8;
    }

    fn paint_status(&self, ui: &egui::Ui, viewport: egui::Rect, language: EditorLanguage) {
        let text = if self.game_view {
            format!("{} (G)", language.text(EditorText::GameView))
        } else {
            self.gizmo.status_text(language)
        };
        let rect = egui::Rect::from_min_size(
            viewport.left_top() + egui::vec2(10.0, 10.0),
            egui::vec2(116.0, 26.0),
        );
        let painter = ui.painter_at(viewport);
        painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(190));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &text,
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );
    }

    fn draw_view_cube(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let faces = view_cube_faces(rect, Quat::from_array(self.transform.rotation));
        let painter = ui.painter_at(rect);
        for face in &faces {
            painter.add(egui::Shape::convex_polygon(
                face.polygon.to_vec(),
                face.color,
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            ));
            let center = face
                .polygon
                .into_iter()
                .fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2())
                / 4.0;
            painter.text(
                center.to_pos2(),
                egui::Align2::CENTER_CENTER,
                face.label,
                egui::FontId::proportional(10.0),
                egui::Color32::WHITE,
            );
        }
        let (primary_down, pointer) = ui.input(|input| {
            (
                input.pointer.primary_down(),
                input
                    .pointer
                    .primary_pressed()
                    .then(|| input.pointer.interact_pos())
                    .flatten(),
            )
        });
        if self.cube_pointer_active {
            if !primary_down {
                self.cube_pointer_active = false;
            }
            return true;
        }
        let Some(preset) = pointer.and_then(|pointer| {
            faces
                .iter()
                .rev()
                .find(|face| point_in_polygon(pointer, face.polygon))
                .map(|face| face.preset)
        }) else {
            return false;
        };
        self.cube_pointer_active = true;
        let (forward, up) = preset_axes(preset);
        self.transform.rotation = Quat::from_rotation_arc(Vec3::Z, forward).to_array();
        let actual_up = Quat::from_array(self.transform.rotation) * Vec3::Y;
        if actual_up.dot(up) < 0.0 {
            self.transform.rotation = (Quat::from_axis_angle(forward, std::f32::consts::PI)
                * Quat::from_array(self.transform.rotation))
            .to_array();
        }
        self.sync_position_from_pivot();
        true
    }

    fn sync_position_from_pivot(&mut self) {
        let forward = Quat::from_array(self.transform.rotation) * Vec3::Z;
        self.transform.translation = (self.pivot - forward * self.distance).to_array();
    }

    fn sync_pivot_from_position(&mut self) {
        let forward = Quat::from_array(self.transform.rotation) * Vec3::Z;
        self.pivot = Vec3::from_array(self.transform.translation) + forward * self.distance;
    }

    fn select(
        &self,
        response: &egui::Response,
        frame: &PreviewFrame,
        session: &mut EditSession,
    ) -> Result<(), crate::EditError> {
        if !response.clicked_by(egui::PointerButton::Primary) {
            return Ok(());
        }
        let Some(pointer) = response.interact_pointer_pos() else {
            return Ok(());
        };
        session.select(
            actor_visuals::pick(frame, response.rect, pointer, self.drag_preview())
                .or_else(|| pick_mesh(frame, response.rect, pointer)),
        )
    }

    fn gizmo(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        frame: &PreviewFrame,
        session: &mut EditSession,
    ) -> Result<bool, crate::EditError> {
        if self.game_view || self.gizmo == GizmoMode::Select {
            self.drag = None;
            return Ok(false);
        }
        if self
            .drag
            .is_some_and(|drag| session.selection() != Some(drag.entity))
        {
            self.drag = None;
        }
        let Some(entity) = session.selection() else {
            return Ok(false);
        };
        let Some(committed) = session.component::<Transform>(entity).copied() else {
            return Ok(false);
        };
        let pointer = ui.input(|input| input.pointer.interact_pos());
        update_drag_preview(&mut self.drag, pointer);
        let transform = gizmo_transform(committed, self.drag, entity);
        let handles = gizmo_handles(frame, response.rect, transform);
        paint_gizmo(ui, transform, frame, response.rect, &handles, self.gizmo);
        let pressed = ui.input(|input| input.pointer.primary_pressed());
        if self.drag.is_none()
            && pressed
            && let Some(pointer) = pointer
            && let Some(handle) = handles.iter().find(|handle| handle.hit.contains(pointer))
        {
            self.drag = Some(GizmoDrag {
                entity,
                mode: self.gizmo,
                axis: handle.axis,
                screen_axis: handle.screen_axis,
                start: transform,
                preview: transform,
                pointer,
            });
        }
        if self.drag.is_none() {
            return Ok(false);
        }
        if ui.input(|input| input.pointer.primary_released()) {
            let drag = self.drag.take().expect("checked above");
            let (field, value) = match drag.mode {
                GizmoMode::Select => return Ok(false),
                GizmoMode::Move => (
                    "translation",
                    Value::Vec3(Vec3::from_array(drag.preview.translation)),
                ),
                GizmoMode::Rotate => (
                    "rotation",
                    Value::Quat(Quat::from_array(drag.preview.rotation)),
                ),
                GizmoMode::Scale => ("scale", Value::Vec3(Vec3::from_array(drag.preview.scale))),
            };
            session.set_field(drag.entity, "sge.transform", field, value)?;
        }
        Ok(true)
    }
}

fn viewport_keyboard_capture(has_focus: bool, text_edit_focused: bool) -> bool {
    has_focus && !text_edit_focused
}

fn frame_selected_requested(keyboard_capture: bool, secondary_down: bool, f_pressed: bool) -> bool {
    keyboard_capture && !secondary_down && f_pressed
}

fn vertical_navigation_requested(
    alt: bool,
    primary_down: bool,
    secondary_down: bool,
    hovered: bool,
    dragged: bool,
) -> bool {
    !alt && primary_down && secondary_down && hovered && dragged
}

fn camera_look_rotation(rotation: Quat, delta: egui::Vec2) -> Quat {
    let yawed = camera_yaw_rotation(rotation, delta.x);
    let elevation = (yawed * Vec3::Z).z.clamp(-1.0, 1.0).asin();
    let next_elevation = (elevation - delta.y * CAMERA_LOOK_SENSITIVITY)
        .clamp(CAMERA_MIN_ELEVATION, CAMERA_MAX_ELEVATION);
    let pitch_delta = elevation - next_elevation;
    (yawed * Quat::from_rotation_x(pitch_delta)).normalize()
}

fn camera_yaw_rotation(rotation: Quat, delta_x: f32) -> Quat {
    (Quat::from_rotation_z(delta_x * CAMERA_LOOK_SENSITIVITY) * rotation).normalize()
}

fn camera_fly_axes(key_down: impl Fn(egui::Key) -> bool) -> (Vec3, f32) {
    let axis = |positive, negative| f32::from(key_down(positive)) - f32::from(key_down(negative));
    (
        Vec3::new(
            axis(egui::Key::D, egui::Key::A),
            axis(egui::Key::R, egui::Key::F),
            axis(egui::Key::W, egui::Key::S),
        ),
        axis(egui::Key::E, egui::Key::Q),
    )
}

fn camera_pan_motion(rotation: Quat, distance: f32, delta: egui::Vec2) -> Vec3 {
    let scale = (distance * 0.0025).clamp(0.0025, 0.2);
    (-(rotation * Vec3::X) * delta.x + (rotation * Vec3::Y) * delta.y) * scale
}

fn dolly_distance(distance: f32, delta_y: f32) -> f32 {
    (distance * (1.0 + delta_y * 0.01)).clamp(0.05, 10_000.0)
}

fn camera_fly_motion(direction: Vec3, speed: f32, stable_dt: f32) -> Vec3 {
    if !stable_dt.is_finite() || stable_dt <= 0.0 {
        return Vec3::ZERO;
    }
    direction.normalize_or_zero() * speed * stable_dt
}

fn camera_lmb_forward_motion(rotation: Quat, speed: f32, delta_y: f32) -> Vec3 {
    rotation * Vec3::Z * -delta_y * speed * 0.01
}

fn scene_bounds(frame: &PreviewFrame) -> Option<(Vec3, Vec3)> {
    let mut bounds = None::<(Vec3, Vec3)>;
    for instance in frame.snapshot.meshes() {
        let mesh = frame.assets.mesh(instance.mesh()).ok()?;
        let model = instance.transform().matrix();
        for vertex in mesh.vertices() {
            let point = model.transform_point3(Vec3::from_array(*vertex.position()));
            extend_bounds(&mut bounds, point);
        }
    }
    for camera in frame.snapshot.cameras() {
        extend_bounds(
            &mut bounds,
            Vec3::from_array(camera.transform().translation),
        );
    }
    for light in frame.snapshot.lights() {
        extend_bounds(&mut bounds, Vec3::from_array(light.transform().translation));
    }
    bounds
}

fn extend_bounds(bounds: &mut Option<(Vec3, Vec3)>, point: Vec3) {
    *bounds = Some(bounds.map_or((point, point), |(minimum, maximum)| {
        (minimum.min(point), maximum.max(point))
    }));
}

fn frame_distance(minimum: Vec3, maximum: Vec3, vertical_fov_radians: f32) -> f32 {
    let radius = ((maximum - minimum).length() * 0.5).max(0.5);
    (radius / (vertical_fov_radians * 0.5).tan() * 2.7).max(2.5)
}

fn initial_camera_rotation() -> Quat {
    camera_rotation(3.0 * std::f32::consts::FRAC_PI_4, -0.45)
}

fn camera_rotation(yaw: f32, pitch: f32) -> Quat {
    let yaw_rotation = Quat::from_rotation_z(yaw);
    let flat_forward = yaw_rotation * Vec3::X;
    let right = yaw_rotation * Vec3::Y;
    let forward = (Quat::from_axis_angle(right, -pitch) * flat_forward).normalize();
    let up = forward.cross(right).normalize();
    Quat::from_mat3(&Mat3::from_cols(right, up, forward))
}

#[cfg(test)]
mod tests;

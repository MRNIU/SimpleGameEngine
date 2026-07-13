// Copyright The SimpleGameEngine Contributors
//
//! Authoring camera, ViewCube, selection and gizmo stay together because their
//! pointer gestures share one latched priority state; splitting that state
//! would create wider cross-module contracts than this private host feature.

use eframe::egui;
use sge_math::{Mat3, Mat4, Quat, Transform, Vec3, Vec4};
use sge_reflect::Value;
use sge_render::{Camera, RenderView, view_projection_matrix};
use sge_scene::SceneEntityId;

use crate::{EditSession, PreviewFrame};

const HANDLE_LENGTH: f32 = 46.0;
const HANDLE_SIZE: f32 = 14.0;
const UNITS_PER_PIXEL: f32 = 0.01;
const WORLD_AXIS_LENGTH: f32 = 1.0;
const CAMERA_MOVE_SPEED: f32 = 4.0;
const CAMERA_SPEED_MULTIPLIERS: [f32; 8] = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GizmoMode {
    Select,
    #[default]
    Move,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    const fn color(self) -> egui::Color32 {
        match self {
            Self::X => egui::Color32::from_rgb(230, 80, 80),
            Self::Y => egui::Color32::from_rgb(80, 210, 110),
            Self::Z => egui::Color32::from_rgb(90, 150, 240),
        }
    }
}

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

#[derive(Clone, Copy)]
struct GizmoDrag {
    entity: SceneEntityId,
    mode: GizmoMode,
    axis: Axis,
    screen_axis: egui::Vec2,
    start: Transform,
    preview: Transform,
    pointer: egui::Pos2,
}

#[derive(Clone, Copy)]
struct GizmoHandle {
    axis: Axis,
    screen_axis: egui::Vec2,
    end: egui::Pos2,
    hit: egui::Rect,
}

#[derive(Clone, Copy)]
struct ScreenPoint {
    position: egui::Pos2,
    depth: f32,
}

#[derive(Clone, Copy)]
enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
}

struct CubeFace {
    polygon: [egui::Pos2; 4],
    preset: ViewPreset,
    depth: f32,
    color: egui::Color32,
    label: &'static str,
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
    ) -> Result<(), crate::EditError> {
        self.update_mode(ui, response);
        let overlay_consumed = if self.game_view {
            false
        } else {
            draw_world_axes(ui, response.rect, frame);
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
        self.paint_status(ui, response.rect);
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
                keyboard_capture && input.key_pressed(egui::Key::F),
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
        let vertical = !alt && primary_down && secondary_down;
        let look = !alt && !primary_down && response.dragged_by(egui::PointerButton::Secondary);
        let lmb_navigate = !alt
            && !secondary_down
            && self.drag.is_none()
            && response.dragged_by(egui::PointerButton::Primary);
        if orbit || look {
            let rotation = Quat::from_rotation_z(-delta.x * 0.01)
                * Quat::from_array(self.transform.rotation)
                * Quat::from_rotation_x(-delta.y * 0.01);
            self.transform.rotation = rotation.normalize().to_array();
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
            let rotation =
                Quat::from_rotation_z(-delta.x * 0.01) * Quat::from_array(self.transform.rotation);
            self.transform.rotation = rotation.normalize().to_array();
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
            let movement = ui.input(|input| {
                let axis = |positive, negative| {
                    f32::from(input.key_down(positive)) - f32::from(input.key_down(negative))
                };
                Vec3::new(
                    axis(egui::Key::D, egui::Key::A),
                    axis(egui::Key::E, egui::Key::Q),
                    axis(egui::Key::W, egui::Key::S),
                )
            });
            let rotation = Quat::from_array(self.transform.rotation);
            let motion = rotation * Vec3::X * movement.x
                + Vec3::Z * movement.y
                + rotation * Vec3::Z * movement.z;
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

    fn paint_status(&self, ui: &egui::Ui, viewport: egui::Rect) {
        let text = if self.game_view {
            "Game View (G)"
        } else {
            self.gizmo.status_text()
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
            text,
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
        session.select(pick_mesh(frame, response.rect, pointer))
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

fn transform_for_drag(mut drag: GizmoDrag, pointer: egui::Pos2) -> Transform {
    let amount = (pointer - drag.pointer).dot(drag.screen_axis) * UNITS_PER_PIXEL;
    drag.preview = drag.start;
    match drag.mode {
        GizmoMode::Select => {}
        GizmoMode::Move => {
            drag.preview.translation =
                (Vec3::from_array(drag.start.translation) + drag.axis.vector() * amount).to_array();
        }
        GizmoMode::Rotate => {
            drag.preview.rotation = (Quat::from_axis_angle(drag.axis.vector(), amount)
                * Quat::from_array(drag.start.rotation))
            .normalize()
            .to_array();
        }
        GizmoMode::Scale => {
            let factor = (1.0 + amount).max(0.01);
            let mut scale = drag.start.scale;
            match drag.axis {
                Axis::X => scale[0] = (scale[0] * factor).max(0.01),
                Axis::Y => scale[1] = (scale[1] * factor).max(0.01),
                Axis::Z => scale[2] = (scale[2] * factor).max(0.01),
            }
            drag.preview.scale = scale;
        }
    }
    drag.preview
}

impl GizmoMode {
    const fn next(self) -> Self {
        match self {
            Self::Select | Self::Scale => Self::Move,
            Self::Move => Self::Rotate,
            Self::Rotate => Self::Scale,
        }
    }

    const fn status_text(self) -> &'static str {
        match self {
            Self::Select => "Select (Q)",
            Self::Move => "Move (W)",
            Self::Rotate => "Rotate (E)",
            Self::Scale => "Scale (R)",
        }
    }
}

fn gizmo_transform(
    committed: Transform,
    drag: Option<GizmoDrag>,
    entity: SceneEntityId,
) -> Transform {
    drag.filter(|drag| drag.entity == entity)
        .map_or(committed, |drag| drag.preview)
}

fn update_drag_preview(drag: &mut Option<GizmoDrag>, pointer: Option<egui::Pos2>) {
    if let (Some(drag), Some(pointer)) = (drag.as_mut(), pointer) {
        drag.preview = transform_for_drag(*drag, pointer);
    }
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

fn gizmo_handles(frame: &PreviewFrame, rect: egui::Rect, transform: Transform) -> Vec<GizmoHandle> {
    let Some(matrix) = projection(frame, rect) else {
        return Vec::new();
    };
    let origin = Vec3::from_array(transform.translation);
    let Some(center) = project(matrix, origin, rect) else {
        return Vec::new();
    };
    Axis::ALL
        .into_iter()
        .filter_map(|axis| {
            let end = project(matrix, origin + axis.vector(), rect)?;
            let delta = end.position - center.position;
            let screen_axis = if delta.length_sq() > 0.0001 {
                delta.normalized()
            } else {
                return None;
            };
            let end = center.position + screen_axis * HANDLE_LENGTH;
            Some(GizmoHandle {
                axis,
                screen_axis,
                end,
                hit: egui::Rect::from_center_size(end, egui::vec2(HANDLE_SIZE, HANDLE_SIZE)),
            })
        })
        .collect()
}

fn paint_gizmo(
    ui: &egui::Ui,
    transform: Transform,
    frame: &PreviewFrame,
    rect: egui::Rect,
    handles: &[GizmoHandle],
    mode: GizmoMode,
) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let Some(center) = project(matrix, Vec3::from_array(transform.translation), rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for handle in handles {
        painter.line_segment(
            [center.position, handle.end],
            egui::Stroke::new(3.0, handle.axis.color()),
        );
        match mode {
            GizmoMode::Select => {}
            GizmoMode::Move => {
                painter.rect_filled(handle.hit, 1.0, handle.axis.color());
            }
            GizmoMode::Rotate => {
                painter.circle_stroke(handle.end, 6.0, egui::Stroke::new(3.0, handle.axis.color()));
            }
            GizmoMode::Scale => {
                painter.rect_stroke(
                    handle.hit,
                    1.0,
                    egui::Stroke::new(3.0, handle.axis.color()),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

fn pick_mesh(frame: &PreviewFrame, rect: egui::Rect, pointer: egui::Pos2) -> Option<SceneEntityId> {
    let matrix = projection(frame, rect)?;
    frame
        .snapshot
        .meshes()
        .iter()
        .filter_map(|instance| {
            let mesh = frame.assets.mesh(instance.mesh()).ok()?;
            let model = instance.transform().matrix();
            let mut closest = None::<f32>;
            for triangle in mesh.indices().chunks_exact(3) {
                let [a, b, c] = triangle else { continue };
                let points = [*a, *b, *c].map(|index| {
                    let vertex = mesh.vertices().get(index as usize)?;
                    project(
                        matrix,
                        model.transform_point3(Vec3::from_array(*vertex.position())),
                        rect,
                    )
                });
                let [Some(a), Some(b), Some(c)] = points else {
                    continue;
                };
                if let Some(depth) = triangle_depth(pointer, a, b, c) {
                    closest = Some(closest.map_or(depth, |current| current.min(depth)));
                }
            }
            let scene = frame
                .scene_entities
                .iter()
                .find(|(_, runtime)| *runtime == instance.entity())?
                .0;
            closest.map(|depth| (depth, scene))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, scene)| scene)
}

fn triangle_depth(
    point: egui::Pos2,
    a: ScreenPoint,
    b: ScreenPoint,
    c: ScreenPoint,
) -> Option<f32> {
    let edge = |from: egui::Pos2, to: egui::Pos2, point: egui::Pos2| {
        (point.x - from.x) * (to.y - from.y) - (point.y - from.y) * (to.x - from.x)
    };
    let area = edge(a.position, b.position, c.position);
    if area.abs() <= f32::EPSILON {
        return None;
    }
    let wa = edge(b.position, c.position, point) / area;
    let wb = edge(c.position, a.position, point) / area;
    let wc = 1.0 - wa - wb;
    (wa >= 0.0 && wb >= 0.0 && wc >= 0.0).then_some(wa * a.depth + wb * b.depth + wc * c.depth)
}

fn projection(frame: &PreviewFrame, rect: egui::Rect) -> Option<Mat4> {
    view_projection_matrix(
        frame.view,
        [rect.width().max(1.0) as u32, rect.height().max(1.0) as u32],
    )
    .ok()
    .map(|matrix| Mat4::from_cols_array(&matrix))
}

fn project(matrix: Mat4, point: Vec3, rect: egui::Rect) -> Option<ScreenPoint> {
    let clip = matrix * Vec4::new(point.x, point.y, point.z, 1.0);
    project_clip_point(clip, rect)
}

fn draw_grid(ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let Some((minimum, maximum, step)) = visible_grid_layout(matrix, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for index in 0..=line_count(minimum.x, maximum.x, step) {
        let value = minimum.x + index as f32 * step;
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(value, minimum.y, 0.0),
            Vec3::new(value, maximum.y, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
    }
    for index in 0..=line_count(minimum.y, maximum.y, step) {
        let value = minimum.y + index as f32 * step;
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(minimum.x, value, 0.0),
            Vec3::new(maximum.x, value, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
    }
}

fn visible_grid_layout(matrix: Mat4, rect: egui::Rect) -> Option<(Vec3, Vec3, f32)> {
    let inverse = matrix.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let corners = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ]
    .map(|point| unproject(inverse, point))
    .into_iter()
    .collect::<Option<Vec<_>>>()?;
    let edges = [
        (0, 1),
        (1, 3),
        (3, 2),
        (2, 0),
        (4, 5),
        (5, 7),
        (7, 6),
        (6, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut intersections = Vec::new();
    for (start, end) in edges {
        let start = corners[start];
        let end = corners[end];
        if start.z.abs() <= 0.0001 {
            intersections.push(start);
        }
        if end.z.abs() <= 0.0001 {
            intersections.push(end);
        }
        if (start.z < 0.0) != (end.z < 0.0) {
            let fraction = start.z / (start.z - end.z);
            intersections.push(start.lerp(end, fraction));
        }
    }
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for point in intersections {
        minimum = minimum.min(point);
        maximum = maximum.max(point);
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        return None;
    }
    let anchor = ground_intersection(inverse, [0.0, 0.0]).unwrap_or((minimum + maximum) * 0.5);
    let mut step = projected_grid_step(matrix, rect, anchor).unwrap_or(1.0);
    while line_count(minimum.x, maximum.x, step) > 256
        || line_count(minimum.y, maximum.y, step) > 256
    {
        step *= 2.0;
    }
    minimum.x = (minimum.x / step).floor() * step;
    minimum.y = (minimum.y / step).floor() * step;
    maximum.x = (maximum.x / step).ceil() * step;
    maximum.y = (maximum.y / step).ceil() * step;
    Some((minimum, maximum, step))
}

fn unproject(inverse: Mat4, point: Vec3) -> Option<Vec3> {
    let world = inverse * point.extend(1.0);
    (world.is_finite() && world.w.abs() > f32::EPSILON).then(|| world.truncate() / world.w)
}

fn ground_intersection(inverse: Mat4, ndc: [f32; 2]) -> Option<Vec3> {
    let near = unproject(inverse, Vec3::new(ndc[0], ndc[1], 0.0))?;
    let far = unproject(inverse, Vec3::new(ndc[0], ndc[1], 1.0))?;
    if near.z.abs() <= 0.0001 {
        return Some(near);
    }
    if (near.z < 0.0) == (far.z < 0.0) {
        return None;
    }
    let fraction = near.z / (near.z - far.z);
    Some(near.lerp(far, fraction))
}

fn projected_grid_step(matrix: Mat4, rect: egui::Rect, anchor: Vec3) -> Option<f32> {
    let center = project(matrix, anchor, rect)?.position;
    let x = project(matrix, anchor + Vec3::X, rect)?.position;
    let y = project(matrix, anchor + Vec3::Y, rect)?.position;
    let spacing = center.distance(x).max(center.distance(y));
    (spacing.is_finite() && spacing > f32::EPSILON).then(|| nice_grid_step(32.0 / spacing))
}

fn nice_grid_step(target: f32) -> f32 {
    if !target.is_finite() || target <= 0.0 {
        return 1.0;
    }
    let magnitude = 10.0_f32.powf(target.log10().floor());
    let fraction = target / magnitude;
    let factor = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    factor * magnitude
}

fn line_count(minimum: f32, maximum: f32, step: f32) -> usize {
    ((maximum - minimum) / step).round().max(0.0) as usize
}

fn draw_world_axes(ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for (axis, end) in [(Axis::X, Vec3::X), (Axis::Y, Vec3::Y), (Axis::Z, Vec3::Z)] {
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::ZERO,
            end * WORLD_AXIS_LENGTH,
            axis.color(),
            2.5,
        );
    }
}

fn scene_bounds(frame: &PreviewFrame) -> Option<(Vec3, Vec3)> {
    let mut bounds = None::<(Vec3, Vec3)>;
    for instance in frame.snapshot.meshes() {
        let mesh = frame.assets.mesh(instance.mesh()).ok()?;
        let model = instance.transform().matrix();
        for vertex in mesh.vertices() {
            let point = model.transform_point3(Vec3::from_array(*vertex.position()));
            bounds = Some(bounds.map_or((point, point), |(minimum, maximum)| {
                (minimum.min(point), maximum.max(point))
            }));
        }
    }
    bounds
}

fn frame_distance(minimum: Vec3, maximum: Vec3, vertical_fov_radians: f32) -> f32 {
    let radius = ((maximum - minimum).length() * 0.5).max(0.5);
    (radius / (vertical_fov_radians * 0.5).tan() * 2.7).max(2.5)
}

fn draw_world_line(
    painter: &egui::Painter,
    rect: egui::Rect,
    matrix: Mat4,
    start: Vec3,
    end: Vec3,
    color: egui::Color32,
    width: f32,
) {
    if let Some([start, end]) = project_segment(matrix, start, end, rect) {
        painter.line_segment(
            [start.position, end.position],
            egui::Stroke::new(width, color),
        );
    }
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

fn project_segment(
    matrix: Mat4,
    start: Vec3,
    end: Vec3,
    rect: egui::Rect,
) -> Option<[ScreenPoint; 2]> {
    let start = matrix * start.extend(1.0);
    let end = matrix * end.extend(1.0);
    let (start, end) = clip_segment(start, end)?;
    Some([
        project_clip_point(start, rect)?,
        project_clip_point(end, rect)?,
    ])
}

fn clip_segment(start: Vec4, end: Vec4) -> Option<(Vec4, Vec4)> {
    if !start.is_finite() || !end.is_finite() {
        return None;
    }
    let planes: [fn(Vec4) -> f32; 6] = [
        |point| point.x + point.w,
        |point| point.w - point.x,
        |point| point.y + point.w,
        |point| point.w - point.y,
        |point| point.z,
        |point| point.w - point.z,
    ];
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for plane in planes {
        let start_distance = plane(start);
        let end_distance = plane(end);
        if start_distance < 0.0 && end_distance < 0.0 {
            return None;
        }
        if (start_distance < 0.0) != (end_distance < 0.0) {
            let crossing = start_distance / (start_distance - end_distance);
            if start_distance < 0.0 {
                lower = lower.max(crossing);
            } else {
                upper = upper.min(crossing);
            }
        }
    }
    (lower <= upper).then(|| {
        let delta = end - start;
        (start + delta * lower, start + delta * upper)
    })
}

fn project_clip_point(clip: Vec4, rect: egui::Rect) -> Option<ScreenPoint> {
    if !clip.is_finite() || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    Some(ScreenPoint {
        position: egui::pos2(
            rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
            rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
        ),
        depth: ndc.z,
    })
}

fn preset_axes(preset: ViewPreset) -> (Vec3, Vec3) {
    match preset {
        ViewPreset::Top => (-Vec3::Z, Vec3::Y),
        ViewPreset::Bottom => (Vec3::Z, Vec3::Y),
        ViewPreset::Front => (Vec3::X, Vec3::Z),
        ViewPreset::Back => (-Vec3::X, Vec3::Z),
        ViewPreset::Right => (-Vec3::Y, Vec3::Z),
        ViewPreset::Left => (Vec3::Y, Vec3::Z),
    }
}

fn view_cube_faces(rect: egui::Rect, rotation: Quat) -> Vec<CubeFace> {
    let center = rect.right_top() + egui::vec2(-48.0, 48.0);
    let inverse = rotation.normalize().inverse();
    let definitions = [
        (
            ViewPreset::Top,
            Vec3::Z,
            [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
            [70, 115, 205],
            "T",
        ),
        (
            ViewPreset::Bottom,
            -Vec3::Z,
            [
                [-1., 1., -1.],
                [1., 1., -1.],
                [1., -1., -1.],
                [-1., -1., -1.],
            ],
            [70, 115, 205],
            "B",
        ),
        (
            ViewPreset::Front,
            -Vec3::X,
            [
                [-1., -1., -1.],
                [-1., 1., -1.],
                [-1., 1., 1.],
                [-1., -1., 1.],
            ],
            [165, 65, 65],
            "F",
        ),
        (
            ViewPreset::Back,
            Vec3::X,
            [[1., 1., -1.], [1., -1., -1.], [1., -1., 1.], [1., 1., 1.]],
            [165, 65, 65],
            "Bk",
        ),
        (
            ViewPreset::Right,
            Vec3::Y,
            [[-1., 1., -1.], [1., 1., -1.], [1., 1., 1.], [-1., 1., 1.]],
            [55, 145, 75],
            "R",
        ),
        (
            ViewPreset::Left,
            -Vec3::Y,
            [
                [1., -1., -1.],
                [-1., -1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
            ],
            [55, 145, 75],
            "L",
        ),
    ];
    let mut faces = definitions
        .into_iter()
        .filter_map(|(preset, normal, corners, color, label)| {
            let normal = inverse * normal;
            (normal.z < -0.001).then(|| {
                let corners = corners.map(|corner| inverse * Vec3::from_array(corner));
                CubeFace {
                    polygon: corners.map(|corner| center + egui::vec2(corner.x, -corner.y) * 24.0),
                    preset,
                    depth: corners.iter().map(|corner| corner.z).sum::<f32>() / 4.0,
                    color: egui::Color32::from_rgb(color[0], color[1], color[2]),
                    label,
                }
            })
        })
        .collect::<Vec<_>>();
    faces.sort_by(|left, right| right.depth.total_cmp(&left.depth));
    faces
}

fn point_in_polygon(pointer: egui::Pos2, polygon: [egui::Pos2; 4]) -> bool {
    let mut sign = 0.0_f32;
    for (start, end) in polygon
        .into_iter()
        .zip(polygon.into_iter().cycle().skip(1))
        .take(4)
    {
        let edge = end - start;
        let offset = pointer - start;
        let cross = edge.x * offset.y - edge.y * offset.x;
        if cross.abs() <= f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign != cross.signum() {
            return false;
        }
    }
    sign != 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(mode: GizmoMode, axis: Axis) -> GizmoDrag {
        GizmoDrag {
            entity: "50000000-0000-4000-8000-000000000001".parse().unwrap(),
            mode,
            axis,
            screen_axis: egui::Vec2::X,
            start: Transform::identity(),
            preview: Transform::identity(),
            pointer: egui::Pos2::ZERO,
        }
    }

    #[test]
    fn each_gizmo_mode_changes_only_its_latched_axis() {
        let moved = transform_for_drag(drag(GizmoMode::Move, Axis::Y), egui::pos2(100.0, 0.0));
        assert_eq!(moved.translation, [0.0, 1.0, 0.0]);
        let scaled = transform_for_drag(drag(GizmoMode::Scale, Axis::Z), egui::pos2(100.0, 0.0));
        assert_eq!(scaled.scale, [1.0, 1.0, 2.0]);
        let rotated = transform_for_drag(drag(GizmoMode::Rotate, Axis::X), egui::pos2(100.0, 0.0));
        assert_ne!(rotated.rotation, Transform::identity().rotation);
    }

    #[test]
    fn active_drag_preview_is_the_gizmo_transform() {
        let committed = Transform::identity();
        let mut active = Some(drag(GizmoMode::Move, Axis::X));
        update_drag_preview(&mut active, Some(egui::pos2(100.0, 0.0)));
        let active = active.unwrap();

        assert_eq!(
            gizmo_transform(committed, Some(active), active.entity),
            active.preview
        );
        assert_eq!(active.preview.translation, [1.0, 0.0, 0.0]);
        assert_eq!(
            gizmo_transform(
                committed,
                Some(active),
                "50000000-0000-4000-8000-000000000002".parse().unwrap()
            ),
            committed
        );
    }

    #[test]
    fn ue_transform_tool_cycle_matches_qwer_and_space() {
        assert_eq!(GizmoMode::Select.next(), GizmoMode::Move);
        assert_eq!(GizmoMode::Move.next(), GizmoMode::Rotate);
        assert_eq!(GizmoMode::Rotate.next(), GizmoMode::Scale);
        assert_eq!(GizmoMode::Scale.next(), GizmoMode::Move);
        assert_eq!(GizmoMode::Select.status_text(), "Select (Q)");
        assert_eq!(GizmoMode::Move.status_text(), "Move (W)");
        assert_eq!(GizmoMode::Rotate.status_text(), "Rotate (E)");
        assert_eq!(GizmoMode::Scale.status_text(), "Scale (R)");
    }

    #[test]
    fn viewport_keyboard_requires_its_own_focus_and_no_text_editor() {
        assert!(viewport_keyboard_capture(true, false));
        assert!(!viewport_keyboard_capture(false, false));
        assert!(!viewport_keyboard_capture(true, true));
    }

    #[test]
    fn ue_camera_navigation_is_distance_scaled_and_frame_rate_independent() {
        let pan = camera_pan_motion(Quat::IDENTITY, 8.0, egui::vec2(20.0, -10.0));
        assert!((pan - Vec3::new(-0.4, -0.2, 0.0)).length() < 0.0001);
        assert!(dolly_distance(8.0, 10.0) > 8.0);
        let straight = camera_fly_motion(Vec3::X, 4.0, 0.25);
        let diagonal = camera_fly_motion(Vec3::new(1.0, 1.0, 0.0), 4.0, 0.25);
        assert!((straight.length() - 1.0).abs() < 0.0001);
        assert!((diagonal.length() - straight.length()).abs() < 0.0001);
    }

    #[test]
    fn lmb_forward_motion_tracks_pointer_distance_without_frame_acceleration() {
        let rotation = Quat::IDENTITY;
        let whole = camera_lmb_forward_motion(rotation, 4.0, 20.0);
        let split = camera_lmb_forward_motion(rotation, 4.0, 8.0)
            + camera_lmb_forward_motion(rotation, 4.0, 12.0);

        assert_eq!(whole, split);
        assert_eq!(camera_lmb_forward_motion(rotation, 4.0, 0.0), Vec3::ZERO);
        assert!((whole.length() - 0.8).abs() < 0.0001);
    }

    #[test]
    fn triangle_hit_test_accepts_interior_and_rejects_exterior() {
        let a = egui::pos2(0.0, 0.0);
        let b = egui::pos2(10.0, 0.0);
        let c = egui::pos2(0.0, 10.0);
        let point = |position, depth| ScreenPoint { position, depth };
        let depth = triangle_depth(
            egui::pos2(2.0, 2.0),
            point(a, 0.1),
            point(b, 0.4),
            point(c, 0.7),
        )
        .unwrap();
        assert!((depth - 0.28).abs() < 0.0001);
        assert!(
            triangle_depth(
                egui::pos2(9.0, 9.0),
                point(a, 0.1),
                point(b, 0.4),
                point(c, 0.7)
            )
            .is_none()
        );
    }

    #[test]
    fn view_cube_layout_tracks_camera_rotation_and_exposes_clickable_faces() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let identity = view_cube_faces(rect, Quat::IDENTITY);
        let rotated = view_cube_faces(rect, Quat::from_rotation_y(0.7));
        assert!(!identity.is_empty());
        assert_ne!(identity[0].polygon, rotated[0].polygon);
        let face = &rotated[0];
        let center = face
            .polygon
            .into_iter()
            .fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2())
            / 4.0;
        assert!(point_in_polygon(center.to_pos2(), face.polygon));
    }

    #[test]
    fn initial_framing_scales_with_visible_geometry() {
        let small = frame_distance(
            Vec3::splat(-0.5),
            Vec3::splat(0.5),
            std::f32::consts::FRAC_PI_3,
        );
        let large = frame_distance(
            Vec3::splat(-5.0),
            Vec3::splat(5.0),
            std::f32::consts::FRAC_PI_3,
        );
        assert!(small >= 2.5);
        assert!(large > small * 5.0);
    }

    #[test]
    fn initial_camera_is_z_up_without_roll() {
        let rotation = initial_camera_rotation();
        let right = rotation * Vec3::X;
        let up = rotation * Vec3::Y;
        assert!(right.z.abs() < 0.0001);
        assert!(up.dot(Vec3::Z) > 0.5);
    }

    #[test]
    fn world_segments_are_clipped_to_the_wgpu_frustum() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let [start, end] = project_segment(
            Mat4::IDENTITY,
            Vec3::new(-2.0, 0.0, 0.5),
            Vec3::new(0.0, 0.0, 0.5),
            rect,
        )
        .expect("crossing segment must remain visible");
        assert_eq!(start.position, egui::pos2(0.0, 50.0));
        assert_eq!(end.position, egui::pos2(50.0, 50.0));
    }

    #[test]
    fn grid_layout_covers_the_visible_ground_plane() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 500.0));
        let matrix = Mat4::from_scale(Vec3::new(0.01, 0.02, 1.0));

        let (minimum, maximum, step) = visible_grid_layout(matrix, rect).unwrap();

        assert!(minimum.x <= -100.0 && maximum.x >= 100.0);
        assert!(minimum.y <= -50.0 && maximum.y >= 50.0);
        assert!(step > 0.0);
        assert!(line_count(minimum.x, maximum.x, step) <= 256);
        assert!(line_count(minimum.y, maximum.y, step) <= 256);
    }
}

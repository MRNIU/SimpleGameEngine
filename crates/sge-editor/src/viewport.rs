// Copyright The SimpleGameEngine Contributors
//
//! Authoring camera, ViewCube, selection and gizmo stay together because their
//! pointer gestures share one latched priority state; splitting that state
//! would create wider cross-module contracts than this private host feature.

use eframe::egui;
use sge_math::{Mat4, Quat, Transform, Vec3, Vec4};
use sge_reflect::Value;
use sge_render::{Camera, RenderView, view_projection_matrix};
use sge_scene::SceneEntityId;

use crate::{EditSession, PreviewFrame};

const HANDLE_LENGTH: f32 = 46.0;
const HANDLE_SIZE: f32 = 14.0;
const UNITS_PER_PIXEL: f32 = 0.01;
const GRID_HALF_EXTENT: i32 = 10;
const WORLD_AXIS_LENGTH: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GizmoMode {
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
        }
    }
}

impl EditorViewport {
    pub(crate) fn prepare(&mut self, frame: &mut PreviewFrame) {
        if !self.initialized {
            self.camera = frame.view.camera();
            if let Some((minimum, maximum)) = scene_bounds(frame) {
                self.pivot = (minimum + maximum) * 0.5;
                self.distance =
                    frame_distance(minimum, maximum, self.camera.vertical_fov_radians());
            }
            let forward = Vec3::new(-1.0, 1.0, -0.65).normalize();
            self.transform.rotation = Quat::from_rotation_arc(Vec3::Z, forward).to_array();
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
        draw_world_axes(ui, response.rect, frame);
        let overlay_consumed = self.draw_view_cube(ui, response.rect);
        self.update_mode(ui, response);
        let camera_consumed = self.navigate(ui, response, session);
        let gizmo_consumed = if camera_consumed {
            false
        } else {
            self.gizmo(ui, response, frame, session)?
        };
        if !overlay_consumed && !camera_consumed && !gizmo_consumed {
            self.select(response, frame, session)?;
        }
        Ok(())
    }

    pub(crate) fn paint_background(&self, ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
        draw_grid(ui, rect, frame);
    }

    fn navigate(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        session: &EditSession,
    ) -> bool {
        let (delta, primary_down, secondary_down, alt, scroll, escape, frame_selected) =
            ui.input(|input| {
                (
                    input.pointer.delta(),
                    input.pointer.primary_down(),
                    input.pointer.secondary_down(),
                    input.modifiers.alt,
                    input.smooth_scroll_delta.y,
                    input.key_pressed(egui::Key::Escape),
                    input.key_pressed(egui::Key::F),
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
        let look = response.dragged_by(egui::PointerButton::Secondary);
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
        if response.hovered() && scroll != 0.0 {
            let forward = Quat::from_array(self.transform.rotation) * Vec3::Z;
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + forward * scroll * 0.02).to_array();
            self.sync_pivot_from_position();
        }
        if response.hovered() && secondary_down {
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
            self.transform.translation =
                (Vec3::from_array(self.transform.translation) + motion * 0.08).to_array();
            self.sync_pivot_from_position();
        }
        if finishing_orbit {
            self.orbiting = false;
        }
        orbit || finishing_orbit || look || (response.hovered() && secondary_down)
    }

    fn update_mode(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if !response.has_focus()
            || ui.ctx().text_edit_focused()
            || ui.input(|input| input.pointer.secondary_down())
        {
            return;
        }
        ui.input(|input| {
            if input.key_pressed(egui::Key::W) {
                self.gizmo = GizmoMode::Move;
            } else if input.key_pressed(egui::Key::E) {
                self.gizmo = GizmoMode::Rotate;
            } else if input.key_pressed(egui::Key::R) {
                self.gizmo = GizmoMode::Scale;
            }
        });
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
        if self
            .drag
            .is_some_and(|drag| session.selection() != Some(drag.entity))
        {
            self.drag = None;
        }
        let Some(entity) = session.selection() else {
            return Ok(false);
        };
        let Some(transform) = session.component::<Transform>(entity).copied() else {
            return Ok(false);
        };
        let handles = gizmo_handles(frame, response.rect, transform);
        paint_gizmo(ui, transform, frame, response.rect, &handles, self.gizmo);
        let pointer = ui.input(|input| input.pointer.interact_pos());
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
        let Some(drag) = self.drag.as_mut() else {
            return Ok(false);
        };
        if let Some(pointer) = pointer {
            drag.preview = transform_for_drag(*drag, pointer);
        }
        if ui.input(|input| input.pointer.primary_released()) {
            let drag = self.drag.take().expect("checked above");
            let (field, value) = match drag.mode {
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

fn transform_for_drag(mut drag: GizmoDrag, pointer: egui::Pos2) -> Transform {
    let amount = (pointer - drag.pointer).dot(drag.screen_axis) * UNITS_PER_PIXEL;
    drag.preview = drag.start;
    match drag.mode {
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
    if !clip.is_finite() || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(ScreenPoint {
        position: egui::pos2(
            rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
            rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
        ),
        depth: ndc.z,
    })
}

fn draw_grid(ui: &egui::Ui, rect: egui::Rect, frame: &PreviewFrame) {
    let Some(matrix) = projection(frame, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    for index in -GRID_HALF_EXTENT..=GRID_HALF_EXTENT {
        let value = index as f32;
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(-GRID_HALF_EXTENT as f32, value, 0.0),
            Vec3::new(GRID_HALF_EXTENT as f32, value, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
        draw_world_line(
            &painter,
            rect,
            matrix,
            Vec3::new(value, -GRID_HALF_EXTENT as f32, 0.0),
            Vec3::new(value, GRID_HALF_EXTENT as f32, 0.0),
            egui::Color32::from_gray(52),
            1.0,
        );
    }
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
    if let (Some(start), Some(end)) = (project(matrix, start, rect), project(matrix, end, rect)) {
        painter.line_segment(
            [start.position, end.position],
            egui::Stroke::new(width, color),
        );
    }
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
}

// Copyright The SimpleGameEngine Contributors
//
//! 编辑器状态、面板与 smoke 行为。

use std::{
    fs,
    path::{Path, PathBuf},
};

use ecs::{Camera, EntityId, MeshRef, Projection, World};
use eframe::egui;
use math::Transform;
use render::{RenderScene, ViewportDrawCall, extract_render_scene, viewport_draw_call};

const ROOT_ID: &str = "root";
const CAMERA_ID: &str = "camera";
const DEFAULT_SCENE_PATH: &str = "assets/examples/editor_smoke.scene.ron";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorLaunchOptions {
    pub smoke_path: Option<PathBuf>,
}

impl EditorLaunchOptions {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut smoke_path = None;
        let mut args = args.into_iter();
        let _program = args.next();
        while let Some(arg) = args.next() {
            if arg == "--smoke" {
                smoke_path = Some(
                    args.next()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("target/tmp/editor_smoke.scene.ron")),
                );
            } else {
                anyhow::bail!("unknown editor argument: {arg}");
            }
        }
        Ok(Self { smoke_path })
    }
}

#[derive(Debug, Clone)]
pub struct EditorModel {
    world: World,
    selected: Option<EntityId>,
    dirty: bool,
    next_cube_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSmokeReport {
    pub mesh_count: usize,
    pub has_camera: bool,
    pub viewport_index_count: usize,
}

impl EditorModel {
    #[must_use]
    pub fn new() -> Self {
        let mut world = World::new();
        world.spawn(EntityId::new(ROOT_ID), "Root", Transform::identity());
        world.spawn(
            EntityId::new(CAMERA_ID),
            "Camera",
            Transform::from_translation([0.0, 2.0, 5.0]),
        );
        debug_assert!(world.set_parent(CAMERA_ID, ROOT_ID).is_ok());
        debug_assert!(
            world
                .insert_camera(
                    CAMERA_ID,
                    Camera::new(Projection::Perspective {
                        fov_y_degrees: 60.0
                    })
                )
                .is_ok()
        );

        Self {
            world,
            selected: None,
            dirty: false,
            next_cube_index: 0,
        }
    }

    pub fn from_scene_str(input: &str) -> Result<Self, scene::SceneError> {
        let world = scene::load_scene(input)?;
        Ok(Self {
            world,
            selected: None,
            dirty: false,
            next_cube_index: 0,
        })
    }

    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    #[must_use]
    pub fn selected(&self) -> Option<&EntityId> {
        self.selected.as_ref()
    }

    pub fn select(&mut self, entity: EntityId) {
        self.selected = Some(entity);
    }

    pub fn create_cube(&mut self) -> EntityId {
        let id = self.next_cube_id();
        self.world.spawn(id.clone(), "Cube", Transform::identity());
        debug_assert!(self.world.set_parent(id.as_str(), ROOT_ID).is_ok());
        debug_assert!(
            self.world
                .insert_mesh(
                    id.as_str(),
                    MeshRef::new("primitive:cube", "primitive:default_material"),
                )
                .is_ok()
        );
        self.selected = Some(id.clone());
        self.dirty = true;
        id
    }

    pub fn set_translation(
        &mut self,
        id: &EntityId,
        translation: [f32; 3],
    ) -> Result<(), ecs::EcsError> {
        self.world.set_translation(id.as_str(), translation)?;
        self.dirty = true;
        Ok(())
    }

    pub fn save_scene_to_string(&self) -> Result<String, scene::SceneError> {
        scene::save_scene(&self.world)
    }

    #[must_use]
    pub fn render_scene(&self) -> RenderScene {
        extract_render_scene(&self.world)
    }

    #[must_use]
    pub fn viewport_draw_call(&self) -> Option<ViewportDrawCall> {
        viewport_draw_call(&self.render_scene())
    }

    pub fn run_smoke_actions(mut self, path: &Path) -> anyhow::Result<EditorSmokeReport> {
        let cube = self.create_cube();
        self.set_translation(&cube, [1.0, 2.0, 3.0])?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.save_scene_to_string()?)?;

        let reopened = Self::from_scene_str(&fs::read_to_string(path)?)?;
        let render_scene = reopened.render_scene();
        let viewport_draw = reopened
            .viewport_draw_call()
            .ok_or_else(|| anyhow::anyhow!("viewport draw call missing after reopen"))?;

        Ok(EditorSmokeReport {
            mesh_count: render_scene.meshes.len(),
            has_camera: render_scene.active_camera.is_some(),
            viewport_index_count: viewport_draw.index_count,
        })
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn next_cube_id(&mut self) -> EntityId {
        loop {
            let id = if self.next_cube_index == 0 {
                EntityId::new("cube")
            } else {
                EntityId::new(format!("cube_{}", self.next_cube_index))
            };
            self.next_cube_index = self.next_cube_index.saturating_add(1);
            if self.world.entity(id.as_str()).is_none() {
                return id;
            }
        }
    }
}

impl Default for EditorModel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct EditorApp {
    model: EditorModel,
    status: String,
    options: EditorLaunchOptions,
    smoke_ran: bool,
}

impl EditorApp {
    #[must_use]
    pub fn new(_creation_context: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_options(_creation_context, EditorLaunchOptions::default())
    }

    #[must_use]
    pub fn new_with_options(
        _creation_context: &eframe::CreationContext<'_>,
        options: EditorLaunchOptions,
    ) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    fn save_default_scene(&mut self) {
        match self.save_scene_to_path(Path::new(DEFAULT_SCENE_PATH)) {
            Ok(()) => self.status = format!("Saved {DEFAULT_SCENE_PATH}"),
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn reopen_default_scene(&mut self) {
        match fs::read_to_string(DEFAULT_SCENE_PATH)
            .map_err(anyhow::Error::from)
            .and_then(|input| Self::model_from_scene(&input))
        {
            Ok(model) => {
                self.model = model;
                self.status = format!("Opened {DEFAULT_SCENE_PATH}");
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn save_scene_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.model.save_scene_to_string()?)?;
        Ok(())
    }

    fn model_from_scene(input: &str) -> anyhow::Result<EditorModel> {
        Ok(EditorModel::from_scene_str(input)?)
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.smoke_ran
            && let Some(path) = self.options.smoke_path.clone()
        {
            self.smoke_ran = true;
            let model = std::mem::take(&mut self.model);
            match model.run_smoke_actions(&path) {
                Ok(report) => {
                    println!(
                        "editor smoke ok: meshes={}, camera={}, viewport_indices={}",
                        report.mesh_count, report.has_camera, report.viewport_index_count
                    );
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(error) => {
                    eprintln!("editor smoke failed: {error:#}");
                    std::process::exit(1);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            if ui.button("New Cube").clicked() {
                self.model.create_cube();
            }
            if ui.button("Save").clicked() {
                self.save_default_scene();
            }
            if ui.button("Reopen").clicked() {
                self.reopen_default_scene();
            }
            ui.label(&self.status);
        });

        ui.separator();
        ui.columns(3, |columns| {
            draw_hierarchy(&mut columns[0], &mut self.model);
            draw_inspector(&mut columns[1], &mut self.model, &mut self.status);
            draw_viewport(&mut columns[2], self.model.viewport_draw_call().as_ref());
        });
    }
}

fn draw_hierarchy(ui: &mut egui::Ui, model: &mut EditorModel) {
    ui.heading("Hierarchy");
    let rows: Vec<(EntityId, String)> = model
        .world()
        .entities()
        .map(|entity| (entity.id.clone(), entity.name.clone()))
        .collect();
    for (id, name) in rows {
        let selected = model.selected().is_some_and(|selected| selected == &id);
        if ui.selectable_label(selected, name).clicked() {
            model.select(id);
        }
    }
}

fn draw_inspector(ui: &mut egui::Ui, model: &mut EditorModel, status: &mut String) {
    ui.heading("Inspector");
    if let Some(selected) = model.selected().cloned()
        && let Some(entity) = model.world().entity(selected.as_str())
    {
        ui.label(entity.name.clone());
        let mut translation = entity.transform.translation;
        let changed = ui
            .horizontal(|ui| {
                ui.label("T");
                ui.add(egui::DragValue::new(&mut translation[0]).speed(0.1))
                    .changed()
                    | ui.add(egui::DragValue::new(&mut translation[1]).speed(0.1))
                        .changed()
                    | ui.add(egui::DragValue::new(&mut translation[2]).speed(0.1))
                        .changed()
            })
            .inner;
        if changed {
            match model.set_translation(&selected, translation) {
                Ok(()) => *status = "Transform updated".to_owned(),
                Err(error) => *status = format!("Edit failed: {error}"),
            }
        }
    }
}

fn draw_viewport(ui: &mut egui::Ui, draw: Option<&ViewportDrawCall>) {
    ui.heading("Viewport");
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    let painter = ui.painter_at(response.rect);
    painter.rect_filled(response.rect, 0.0, egui::Color32::from_rgb(18, 24, 29));
    if let Some(draw) = draw {
        paint_fallback_viewport(response.rect, &painter, draw);
    }
}

fn paint_fallback_viewport(rect: egui::Rect, painter: &egui::Painter, draw: &ViewportDrawCall) {
    let min_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::INFINITY, f32::min);
    let max_x = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = draw
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let to_screen = |x: f32, y: f32| rect.center() + egui::vec2(x * 86.0, -y * 86.0);
    let cube = egui::Rect::from_two_pos(to_screen(min_x, min_y), to_screen(max_x, max_y));
    painter.rect_filled(cube, 2.0, egui::Color32::from_rgb(77, 163, 255));
    painter.rect_stroke(
        cube,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::{EditorLaunchOptions, EditorModel};
    use std::path::PathBuf;

    #[test]
    fn new_editor_starts_with_camera() {
        let editor = EditorModel::new();

        assert!(
            editor
                .world()
                .entity("camera")
                .and_then(|entity| entity.camera.as_ref())
                .is_some()
        );
    }

    #[test]
    fn parses_smoke_argument() {
        let options = EditorLaunchOptions::from_args([
            "editor".to_owned(),
            "--smoke".to_owned(),
            "target/tmp/smoke.scene.ron".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            options.smoke_path,
            Some(PathBuf::from("target/tmp/smoke.scene.ron"))
        );
    }
}

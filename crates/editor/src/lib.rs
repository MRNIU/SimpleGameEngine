// Copyright The SimpleGameEngine Contributors
//
//! 编辑器状态、面板与 smoke 行为。

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use ecs::{Camera, EntityId, MeshRef, Projection, World};
use eframe::{egui, egui_wgpu, wgpu};
use math::Transform;
use render::{
    RenderScene, ViewportDrawCall, ViewportRenderer, extract_render_scene,
    fit_viewport_draw_to_size, viewport_draw_call,
};

const ROOT_ID: &str = "root";
const CAMERA_ID: &str = "camera";
const DEFAULT_SCENE_PATH: &str = "target/tmp/editor_manual.scene.ron";
const SMOKE_MAX_VIEWPORT_FRAMES: u32 = 120;
const VIEWPORT_MIN_SIZE: egui::Vec2 = egui::vec2(240.0, 180.0);

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ViewportWgpuReport {
    prepare_count: usize,
    paint_count: usize,
    completed: bool,
}

#[derive(Debug, Clone, Default)]
struct ViewportWgpuProbe {
    inner: Arc<ViewportWgpuProbeInner>,
}

#[derive(Debug, Default)]
struct ViewportWgpuProbeInner {
    prepare_count: AtomicUsize,
    paint_count: AtomicUsize,
}

impl ViewportWgpuProbe {
    fn mark_prepared(&self) {
        self.inner.prepare_count.fetch_add(1, Ordering::Relaxed);
    }

    fn mark_painted(&self) {
        self.inner.paint_count.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    fn report(&self) -> ViewportWgpuReport {
        let prepare_count = self.inner.prepare_count.load(Ordering::Relaxed);
        let paint_count = self.inner.paint_count.load(Ordering::Relaxed);
        ViewportWgpuReport {
            prepare_count,
            paint_count,
            completed: prepare_count > 0 && paint_count > 0,
        }
    }
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
        self.run_smoke_actions_in_place(path)
    }

    pub fn run_smoke_actions_in_place(&mut self, path: &Path) -> anyhow::Result<EditorSmokeReport> {
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

        let report = EditorSmokeReport {
            mesh_count: render_scene.meshes.len(),
            has_camera: render_scene.active_camera.is_some(),
            viewport_index_count: viewport_draw.index_count,
        };
        *self = reopened;
        Ok(report)
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
    smoke_report: Option<EditorSmokeReport>,
    smoke_frame_count: u32,
    viewport_probe: ViewportWgpuProbe,
    wgpu_viewport_available: bool,
}

impl EditorApp {
    #[must_use]
    pub fn new(_creation_context: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_options(_creation_context, EditorLaunchOptions::default())
    }

    #[must_use]
    pub fn new_with_options(
        creation_context: &eframe::CreationContext<'_>,
        options: EditorLaunchOptions,
    ) -> Self {
        let wgpu_viewport_available = install_viewport_renderer(creation_context);
        Self {
            options,
            wgpu_viewport_available,
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
        if let Some(path) = self.options.smoke_path.clone() {
            if self.smoke_report.is_none() {
                match self.model.run_smoke_actions_in_place(&path) {
                    Ok(report) => self.smoke_report = Some(report),
                    Err(error) => {
                        eprintln!("editor smoke failed: {error:#}");
                        std::process::exit(1);
                    }
                }
            }

            self.smoke_frame_count = self.smoke_frame_count.saturating_add(1);
            let viewport_report = self.viewport_probe.report();
            if viewport_report.completed {
                let report = self
                    .smoke_report
                    .as_ref()
                    .expect("smoke report is set before viewport completion");
                println!(
                    "editor smoke ok: meshes={}, camera={}, viewport_indices={}, viewport_prepare={}, viewport_paint={}",
                    report.mesh_count,
                    report.has_camera,
                    report.viewport_index_count,
                    viewport_report.prepare_count,
                    viewport_report.paint_count
                );
                self.options.smoke_path = None;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.smoke_frame_count > SMOKE_MAX_VIEWPORT_FRAMES {
                match self.smoke_report.as_ref() {
                    Some(report) => eprintln!(
                        "editor smoke failed: wgpu viewport path not reached after {} frames: meshes={}, camera={}, viewport_indices={}, viewport_prepare={}, viewport_paint={}",
                        self.smoke_frame_count,
                        report.mesh_count,
                        report.has_camera,
                        report.viewport_index_count,
                        viewport_report.prepare_count,
                        viewport_report.paint_count
                    ),
                    None => eprintln!("editor smoke failed: model smoke did not produce a report"),
                }
                std::process::exit(1);
            } else if !self.wgpu_viewport_available {
                eprintln!("editor smoke failed: eframe wgpu render state is unavailable");
                std::process::exit(1);
            } else {
                context.request_repaint();
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
            let draw = self.model.viewport_draw_call();
            let wgpu_probe = self.wgpu_viewport_available.then_some(&self.viewport_probe);
            draw_viewport(&mut columns[2], draw.as_ref(), wgpu_probe);
        });
    }
}

struct ViewportGpuResources {
    renderer: ViewportRenderer,
}

impl ViewportGpuResources {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: ViewportRenderer::new(device, color_format),
        }
    }
}

#[derive(Clone)]
struct ViewportWgpuCallback {
    draw: ViewportDrawCall,
    probe: ViewportWgpuProbe,
}

impl egui_wgpu::CallbackTrait for ViewportWgpuCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = callback_resources.get_mut::<ViewportGpuResources>() {
            resources.renderer.prepare(device, Some(&self.draw));
            self.probe.mark_prepared();
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(resources) = callback_resources.get::<ViewportGpuResources>() {
            resources.renderer.paint(render_pass);
            self.probe.mark_painted();
        }
    }
}

fn install_viewport_renderer(creation_context: &eframe::CreationContext<'_>) -> bool {
    let Some(render_state) = creation_context.wgpu_render_state.as_ref() else {
        return false;
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(ViewportGpuResources::new(
            &render_state.device,
            render_state.target_format,
        ));
    true
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

fn draw_viewport(
    ui: &mut egui::Ui,
    draw: Option<&ViewportDrawCall>,
    wgpu_probe: Option<&ViewportWgpuProbe>,
) {
    ui.heading("Viewport");
    let (rect, _response) = ui.allocate_exact_size(
        viewport_canvas_size(ui.available_size_before_wrap()),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 24, 29));
    if let Some((draw, probe)) = draw.zip(wgpu_probe) {
        let draw = fit_viewport_draw_to_size(draw, [rect.width(), rect.height()]);
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportWgpuCallback {
                draw,
                probe: probe.clone(),
            },
        ));
    } else if let Some(draw) = draw {
        paint_fallback_viewport(rect, &painter, draw);
    }
}

fn viewport_canvas_size(available: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        available.x.max(VIEWPORT_MIN_SIZE.x),
        available.y.max(VIEWPORT_MIN_SIZE.y),
    )
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
    use super::{DEFAULT_SCENE_PATH, EditorLaunchOptions, EditorModel, ViewportWgpuProbe};
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

    #[test]
    fn manual_save_path_stays_out_of_tracked_assets() {
        assert_eq!(DEFAULT_SCENE_PATH, "target/tmp/editor_manual.scene.ron");
    }

    #[test]
    fn viewport_wgpu_probe_requires_prepare_and_paint() {
        let probe = ViewportWgpuProbe::default();

        assert!(!probe.report().completed);

        probe.mark_prepared();
        assert!(!probe.report().completed);

        probe.mark_painted();
        let report = probe.report();

        assert!(report.completed);
        assert_eq!(report.prepare_count, 1);
        assert_eq!(report.paint_count, 1);
    }

    #[test]
    fn viewport_canvas_keeps_nonzero_paint_area() {
        assert_eq!(
            super::viewport_canvas_size(egui::vec2(0.0, 0.0)),
            egui::vec2(240.0, 180.0)
        );
        assert_eq!(
            super::viewport_canvas_size(egui::vec2(320.0, 240.0)),
            egui::vec2(320.0, 240.0)
        );
    }
}

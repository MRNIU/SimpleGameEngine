// Copyright The SimpleGameEngine Contributors

use std::{fs, path::Path};

use ecs::{Camera, EntityId, MeshRef, Projection, World};
use math::Transform;
use render::{RenderScene, ViewportDrawCall, extract_render_scene, viewport_draw_call};

const ROOT_ID: &str = "root";
const CAMERA_ID: &str = "camera";

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

#[cfg(test)]
mod tests {
    use super::EditorModel;

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
}

// Copyright The SimpleGameEngine Contributors

use ecs::{Camera, EntityId, MeshRef, Projection, World};
use math::Transform;
use render::{
    RenderScene, ViewportDrawCall, ViewportView, extract_render_scene,
    viewport_draw_call_with_selection, viewport_draw_call_with_view,
};
use thiserror::Error;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditorError {
    #[error("protected entity cannot be changed: {0}")]
    ProtectedEntity(EntityId),
    #[error("no entity is selected")]
    MissingSelection,
    #[error("entity name cannot be blank")]
    InvalidEntityName,
    #[error("transform must be finite, have non-zero scale, and have non-zero rotation")]
    InvalidTransformValue,
    #[error("entity id generation exhausted")]
    IdGenerationExhausted,
    #[error(transparent)]
    Ecs(#[from] ecs::EcsError),
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

    pub fn clear_selection(&mut self) {
        self.selected = None;
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
    ) -> Result<(), EditorError> {
        let mut transform = self
            .world
            .entity(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?
            .transform;
        transform.translation = translation;
        self.set_transform(id, transform)
    }

    pub fn set_transform(
        &mut self,
        id: &EntityId,
        transform: Transform,
    ) -> Result<(), EditorError> {
        let transform = validated_transform(transform)?;
        let record = self
            .world
            .entity_mut(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?;
        record.transform = transform;
        self.dirty = true;
        Ok(())
    }

    pub fn rename_entity(&mut self, id: &EntityId, name: &str) -> Result<(), EditorError> {
        if name.trim().is_empty() {
            return Err(EditorError::InvalidEntityName);
        }
        self.world.rename_entity(id.as_str(), name)?;
        self.dirty = true;
        Ok(())
    }

    pub fn duplicate_selected(&mut self) -> Result<EntityId, EditorError> {
        let id = self.selected.clone().ok_or(EditorError::MissingSelection)?;
        self.duplicate_entity(&id)
    }

    pub fn duplicate_entity(&mut self, id: &EntityId) -> Result<EntityId, EditorError> {
        ensure_unprotected(id)?;
        let source = self
            .world
            .entity(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?
            .clone();
        let new_id = self.next_duplicate_id(id)?;
        let new_name = self.next_copy_name(&source.name)?;

        self.world.spawn(new_id.clone(), new_name, source.transform);
        if let Some(parent) = source.parent {
            self.world.set_parent(new_id.as_str(), parent.as_str())?;
        }
        if let Some(mesh) = source.mesh {
            self.world.insert_mesh(new_id.as_str(), mesh)?;
        }
        if let Some(light) = source.light {
            self.world.insert_light(new_id.as_str(), light)?;
        }
        self.selected = Some(new_id.clone());
        self.dirty = true;
        Ok(new_id)
    }

    pub fn delete_selected(&mut self) -> Result<(), EditorError> {
        let id = self.selected.clone().ok_or(EditorError::MissingSelection)?;
        self.delete_entity(&id)
    }

    pub fn delete_entity(&mut self, id: &EntityId) -> Result<(), EditorError> {
        ensure_unprotected(id)?;
        let fallback = self.world.delete_subtree(id.as_str())?;
        self.selected = fallback.filter(|parent| self.world.entity(parent.as_str()).is_some());
        self.dirty = true;
        Ok(())
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn reopen_scene_from_str(&mut self, input: &str) -> Result<(), scene::SceneError> {
        let selected = self.selected.clone();
        let mut reopened = Self::from_scene_str(input)?;
        reopened.selected =
            selected.filter(|entity| reopened.world.entity(entity.as_str()).is_some());
        *self = reopened;
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
        viewport_draw_call_with_selection(&self.render_scene(), self.selected.as_ref())
    }

    #[must_use]
    pub fn viewport_draw_call_for_view(&self, view: &ViewportView) -> Option<ViewportDrawCall> {
        viewport_draw_call_with_view(&self.render_scene(), self.selected.as_ref(), view)
    }

    pub fn run_smoke_actions(mut self) -> anyhow::Result<EditorSmokeReport> {
        self.run_smoke_actions_in_place()
    }

    pub fn run_smoke_actions_in_place(&mut self) -> anyhow::Result<EditorSmokeReport> {
        let _first = self.create_cube();
        let second = self.create_cube();
        self.rename_entity(&second, "Smoke Cube")?;
        self.set_transform(
            &second,
            Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 2.0, 0.0],
                scale: [2.0, 1.5, 1.0],
            },
        )?;
        let _duplicate = self.duplicate_selected()?;
        self.smoke_report()
    }

    pub fn smoke_report(&self) -> anyhow::Result<EditorSmokeReport> {
        let render_scene = self.render_scene();
        let viewport_draw = self
            .viewport_draw_call()
            .ok_or_else(|| anyhow::anyhow!("viewport draw call missing after reopen"))?;

        let report = EditorSmokeReport {
            mesh_count: render_scene.meshes.len(),
            has_camera: render_scene.active_camera.is_some(),
            viewport_index_count: viewport_draw.index_count,
        };
        Ok(report)
    }

    pub fn smoke_report_for_view(&self, view: &ViewportView) -> anyhow::Result<EditorSmokeReport> {
        let render_scene = self.render_scene();
        let viewport_draw = self
            .viewport_draw_call_for_view(view)
            .ok_or_else(|| anyhow::anyhow!("viewport draw call missing after reopen"))?;

        let report = EditorSmokeReport {
            mesh_count: render_scene.meshes.len(),
            has_camera: render_scene.active_camera.is_some(),
            viewport_index_count: viewport_draw.index_count,
        };
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

    fn next_duplicate_id(&self, source: &EntityId) -> Result<EntityId, EditorError> {
        let base = duplicate_id_base(source.as_str());
        for index in 1..=u32::MAX {
            let id = EntityId::new(format!("{base}_{index}"));
            if self.world.entity(id.as_str()).is_none() {
                return Ok(id);
            }
        }
        Err(EditorError::IdGenerationExhausted)
    }

    fn next_copy_name(&self, source_name: &str) -> Result<String, EditorError> {
        let base = format!("{source_name} Copy");
        if !self.entity_name_exists(&base) {
            return Ok(base);
        }
        for index in 2..=u32::MAX {
            let name = format!("{base} {index}");
            if !self.entity_name_exists(&name) {
                return Ok(name);
            }
        }
        Err(EditorError::IdGenerationExhausted)
    }

    fn entity_name_exists(&self, name: &str) -> bool {
        self.world.entities().any(|entity| entity.name == name)
    }
}

impl Default for EditorModel {
    fn default() -> Self {
        Self::new()
    }
}

fn ensure_unprotected(id: &EntityId) -> Result<(), EditorError> {
    if matches!(id.as_str(), ROOT_ID | CAMERA_ID) {
        return Err(EditorError::ProtectedEntity(id.clone()));
    }
    Ok(())
}

fn duplicate_id_base(source: &str) -> &str {
    if let Some((base, suffix)) = source.rsplit_once('_')
        && !base.is_empty()
        && suffix.parse::<u32>().is_ok()
    {
        return base;
    }
    source
}

fn validated_transform(mut transform: Transform) -> Result<Transform, EditorError> {
    if !transform
        .translation
        .into_iter()
        .chain(transform.rotation)
        .chain(transform.scale)
        .all(f32::is_finite)
        || transform.scale.contains(&0.0)
    {
        return Err(EditorError::InvalidTransformValue);
    }

    let rotation_len = transform
        .rotation
        .into_iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if rotation_len == 0.0 {
        return Err(EditorError::InvalidTransformValue);
    }
    for value in &mut transform.rotation {
        *value /= rotation_len;
    }
    Ok(transform)
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

// Copyright The SimpleGameEngine Contributors

use ecs::{Camera, EntityId, Light, LightKind, MaterialOverride, MeshRef, Projection, World};
use math::Transform;
use render::{
    RenderScene, ViewportDrawCall, ViewportView, extract_render_scene,
    viewport_draw_call_with_selection, viewport_draw_call_with_view,
};
use thiserror::Error;

mod validation;

use validation::{
    canonical_transform, validated_camera, validated_light, validated_material_override,
    validated_transform,
};

const ROOT_ID: &str = "root";
const CAMERA_ID: &str = "camera";
const LIGHT_ID: &str = "directional_light";
const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone)]
pub struct EditorModel {
    world: World,
    selected: Option<EntityId>,
    dirty: bool,
    next_cube_index: u32,
    undo_stack: Vec<EditorCommand>,
    redo_stack: Vec<EditorCommand>,
}

#[derive(Debug, Clone, PartialEq)]
enum EditorCommand {
    CreateEntity {
        record: ecs::EntityRecord,
        previous_selection: Option<EntityId>,
    },
    DeleteEntity {
        deleted_root: EntityId,
        records: Vec<ecs::EntityRecord>,
        previous_selection: Option<EntityId>,
    },
    DuplicateEntity {
        source: EntityId,
        created: ecs::EntityRecord,
        previous_selection: Option<EntityId>,
    },
    RenameEntity {
        id: EntityId,
        before: String,
        after: String,
    },
    SetTransform {
        id: EntityId,
        before: Transform,
        after: Transform,
    },
    SetMaterialOverride {
        id: EntityId,
        before: Option<MaterialOverride>,
        after: Option<MaterialOverride>,
    },
    SetLight {
        id: EntityId,
        before: Light,
        after: Light,
    },
    SetCamera {
        id: EntityId,
        before: Camera,
        after: Camera,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSmokeReport {
    pub mesh_count: usize,
    pub has_camera: bool,
    pub has_light: bool,
    pub viewport_index_count: usize,
    pub transform_undo_redo_ok: bool,
    pub content_reopen_ok: bool,
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
    #[error("scene content value is invalid")]
    InvalidSceneContentValue,
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
        world.spawn(
            EntityId::new(LIGHT_ID),
            "Directional Light",
            Transform::from_translation([0.0, 4.0, 2.0]),
        );
        debug_assert!(world.set_parent(LIGHT_ID, ROOT_ID).is_ok());
        debug_assert!(
            world
                .insert_light(
                    LIGHT_ID,
                    Light {
                        kind: LightKind::Directional,
                        color: [1.0, 1.0, 1.0],
                        intensity: 1.0,
                    },
                )
                .is_ok()
        );

        Self {
            world,
            selected: None,
            dirty: false,
            next_cube_index: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_scene_str(input: &str) -> Result<Self, scene::SceneError> {
        let world = scene::load_scene(input)?;
        Ok(Self {
            world,
            selected: None,
            dirty: false,
            next_cube_index: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
        let previous_selection = self.selected.clone();
        let mut record = ecs::EntityRecord::new(id.clone(), "Cube", Transform::identity());
        record.parent = Some(EntityId::new(ROOT_ID));
        record.mesh = Some(MeshRef::new("primitive:cube", "primitive:default_material"));
        let command = EditorCommand::CreateEntity {
            record,
            previous_selection,
        };
        self.apply_command(&command)
            .expect("create cube command is internally valid");
        self.push_undo(command);
        id
    }

    pub fn create_imported_mesh(
        &mut self,
        asset_uuid: &asset::AssetUuid,
        asset_name: &str,
    ) -> Result<EntityId, EditorError> {
        let id = self.next_imported_entity_id(asset_name)?;
        let name = self.next_imported_entity_name(asset_name)?;
        let previous_selection = self.selected.clone();
        let mut record = ecs::EntityRecord::new(id.clone(), name, Transform::identity());
        record.parent = Some(EntityId::new(ROOT_ID));
        record.mesh = Some(MeshRef::new(
            asset_uuid.to_asset_ref(),
            "primitive:default_material",
        ));
        let command = EditorCommand::CreateEntity {
            record,
            previous_selection,
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(id)
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
        let before = self
            .world
            .entity(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?
            .transform;
        let _ = self.commit_transform_edit(id, before, transform)?;
        Ok(())
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Result<bool, EditorError> {
        let Some(command) = self.undo_stack.pop() else {
            return Ok(false);
        };
        self.revert_command(&command)?;
        self.redo_stack.push(command);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, EditorError> {
        let Some(command) = self.redo_stack.pop() else {
            return Ok(false);
        };
        self.apply_command(&command)?;
        self.undo_stack.push(command);
        Ok(true)
    }

    pub fn preview_transform(
        &mut self,
        id: &EntityId,
        transform: Transform,
    ) -> Result<(), EditorError> {
        let transform = canonical_transform(transform)?;
        self.write_transform(id, transform)
    }

    pub fn restore_transform_preview(
        &mut self,
        id: &EntityId,
        transform: Transform,
        dirty: bool,
    ) -> Result<(), EditorError> {
        let transform = canonical_transform(transform)?;
        self.write_transform(id, transform)?;
        self.dirty = dirty;
        Ok(())
    }

    pub fn commit_transform_edit(
        &mut self,
        id: &EntityId,
        before: Transform,
        after: Transform,
    ) -> Result<bool, EditorError> {
        let before = canonical_transform(before)?;
        let after = canonical_transform(after)?;
        if before == after {
            self.write_transform(id, after)?;
            return Ok(false);
        }
        let command = EditorCommand::SetTransform {
            id: id.clone(),
            before,
            after,
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(true)
    }

    pub fn set_material_override(
        &mut self,
        id: &EntityId,
        material: Option<MaterialOverride>,
    ) -> Result<(), EditorError> {
        let after = validated_material_override(material)?;
        let before = self
            .world
            .entity(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?
            .material_override;
        let _ = self.commit_material_override_edit(id, before, after)?;
        Ok(())
    }

    pub fn preview_material_override(
        &mut self,
        id: &EntityId,
        material: Option<MaterialOverride>,
    ) -> Result<(), EditorError> {
        self.write_material_override(id, validated_material_override(material)?)
    }

    pub fn restore_material_override_preview(
        &mut self,
        id: &EntityId,
        material: Option<MaterialOverride>,
        dirty: bool,
    ) -> Result<(), EditorError> {
        self.write_material_override(id, validated_material_override(material)?)?;
        self.dirty = dirty;
        Ok(())
    }

    pub fn commit_material_override_edit(
        &mut self,
        id: &EntityId,
        before: Option<MaterialOverride>,
        after: Option<MaterialOverride>,
    ) -> Result<bool, EditorError> {
        let before = validated_material_override(before)?;
        let after = validated_material_override(after)?;
        if before == after {
            self.write_material_override(id, after)?;
            return Ok(false);
        }
        let command = EditorCommand::SetMaterialOverride {
            id: id.clone(),
            before,
            after,
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(true)
    }

    pub fn set_light(&mut self, id: &EntityId, light: Light) -> Result<(), EditorError> {
        let after = validated_light(light)?;
        let before = self
            .world
            .entity(id.as_str())
            .and_then(|record| record.light.clone())
            .ok_or(EditorError::InvalidSceneContentValue)?;
        let _ = self.commit_light_edit(id, before, after)?;
        Ok(())
    }

    pub fn preview_light(&mut self, id: &EntityId, light: Light) -> Result<(), EditorError> {
        self.write_light(id, validated_light(light)?)
    }

    pub fn restore_light_preview(
        &mut self,
        id: &EntityId,
        light: Light,
        dirty: bool,
    ) -> Result<(), EditorError> {
        self.write_light(id, validated_light(light)?)?;
        self.dirty = dirty;
        Ok(())
    }

    pub fn commit_light_edit(
        &mut self,
        id: &EntityId,
        before: Light,
        after: Light,
    ) -> Result<bool, EditorError> {
        let before = validated_light(before)?;
        let after = validated_light(after)?;
        if before == after {
            self.write_light(id, after)?;
            return Ok(false);
        }
        let command = EditorCommand::SetLight {
            id: id.clone(),
            before,
            after,
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(true)
    }

    pub fn set_camera(&mut self, id: &EntityId, camera: Camera) -> Result<(), EditorError> {
        let after = validated_camera(camera)?;
        let before = self
            .world
            .entity(id.as_str())
            .and_then(|record| record.camera.clone())
            .ok_or(EditorError::InvalidSceneContentValue)?;
        let _ = self.commit_camera_edit(id, before, after)?;
        Ok(())
    }

    pub fn preview_camera(&mut self, id: &EntityId, camera: Camera) -> Result<(), EditorError> {
        self.write_camera(id, validated_camera(camera)?)
    }

    pub fn restore_camera_preview(
        &mut self,
        id: &EntityId,
        camera: Camera,
        dirty: bool,
    ) -> Result<(), EditorError> {
        self.write_camera(id, validated_camera(camera)?)?;
        self.dirty = dirty;
        Ok(())
    }

    pub fn commit_camera_edit(
        &mut self,
        id: &EntityId,
        before: Camera,
        after: Camera,
    ) -> Result<bool, EditorError> {
        let before = validated_camera(before)?;
        let after = validated_camera(after)?;
        if before == after {
            self.write_camera(id, after)?;
            return Ok(false);
        }
        let command = EditorCommand::SetCamera {
            id: id.clone(),
            before,
            after,
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(true)
    }

    fn push_undo(&mut self, command: EditorCommand) {
        self.undo_stack.push(command);
        if self.undo_stack.len() > HISTORY_LIMIT {
            let overflow = self.undo_stack.len() - HISTORY_LIMIT;
            self.undo_stack.drain(0..overflow);
        }
        self.redo_stack.clear();
    }

    fn records_with_replacements(
        &self,
        replacements: &[ecs::EntityRecord],
    ) -> Vec<ecs::EntityRecord> {
        let replacement_ids: std::collections::BTreeSet<_> = replacements
            .iter()
            .map(|record| record.id.clone())
            .collect();
        self.world
            .entities()
            .filter(|record| !replacement_ids.contains(&record.id))
            .cloned()
            .chain(replacements.iter().cloned())
            .collect()
    }

    fn replace_world_records(
        &mut self,
        records: Vec<ecs::EntityRecord>,
    ) -> Result<(), EditorError> {
        self.world = World::from_records(records)?;
        Ok(())
    }

    fn subtree_records(&self, root: &EntityId) -> Result<Vec<ecs::EntityRecord>, EditorError> {
        let mut stack = vec![root.clone()];
        let mut records = Vec::new();
        while let Some(current) = stack.pop() {
            let record = self
                .world
                .entity(current.as_str())
                .ok_or_else(|| ecs::EcsError::MissingEntity(current.to_string()))?
                .clone();
            stack.extend(self.world.children_of(current.as_str()));
            records.push(record);
        }
        Ok(records)
    }

    fn apply_command(&mut self, command: &EditorCommand) -> Result<(), EditorError> {
        match command {
            EditorCommand::SetTransform { id, after, .. } => {
                self.write_transform(id, *after)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetMaterialOverride { id, after, .. } => {
                self.write_material_override(id, *after)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetLight { id, after, .. } => {
                self.write_light(id, after.clone())?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetCamera { id, after, .. } => {
                self.write_camera(id, after.clone())?;
                self.selected = Some(id.clone());
            }
            EditorCommand::RenameEntity { id, after, .. } => {
                self.world.rename_entity(id.as_str(), after)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::CreateEntity { record, .. }
            | EditorCommand::DuplicateEntity {
                created: record, ..
            } => {
                let records = self.records_with_replacements(std::slice::from_ref(record));
                self.replace_world_records(records)?;
                self.selected = Some(record.id.clone());
            }
            EditorCommand::DeleteEntity { deleted_root, .. } => {
                let fallback = self.world.delete_subtree(deleted_root.as_str())?;
                self.selected =
                    fallback.filter(|parent| self.world.entity(parent.as_str()).is_some());
            }
        }
        self.dirty = true;
        Ok(())
    }

    fn revert_command(&mut self, command: &EditorCommand) -> Result<(), EditorError> {
        match command {
            EditorCommand::SetTransform { id, before, .. } => {
                self.write_transform(id, *before)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetMaterialOverride { id, before, .. } => {
                self.write_material_override(id, *before)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetLight { id, before, .. } => {
                self.write_light(id, before.clone())?;
                self.selected = Some(id.clone());
            }
            EditorCommand::SetCamera { id, before, .. } => {
                self.write_camera(id, before.clone())?;
                self.selected = Some(id.clone());
            }
            EditorCommand::RenameEntity { id, before, .. } => {
                self.world.rename_entity(id.as_str(), before)?;
                self.selected = Some(id.clone());
            }
            EditorCommand::CreateEntity {
                record,
                previous_selection,
            }
            | EditorCommand::DuplicateEntity {
                created: record,
                previous_selection,
                ..
            } => {
                let _ = self.world.delete_subtree(record.id.as_str())?;
                self.selected = previous_selection
                    .clone()
                    .filter(|id| self.world.entity(id.as_str()).is_some());
            }
            EditorCommand::DeleteEntity {
                deleted_root,
                records,
                previous_selection,
            } => {
                let world_records = self.records_with_replacements(records);
                self.replace_world_records(world_records)?;
                self.selected = Some(deleted_root.clone())
                    .filter(|id| self.world.entity(id.as_str()).is_some())
                    .or_else(|| {
                        previous_selection
                            .clone()
                            .filter(|id| self.world.entity(id.as_str()).is_some())
                    });
            }
        }
        self.dirty = true;
        Ok(())
    }

    fn write_transform(&mut self, id: &EntityId, transform: Transform) -> Result<(), EditorError> {
        let record = self
            .world
            .entity_mut(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?;
        record.transform = transform;
        Ok(())
    }

    fn write_material_override(
        &mut self,
        id: &EntityId,
        material: Option<MaterialOverride>,
    ) -> Result<(), EditorError> {
        let record = self
            .world
            .entity_mut(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?;
        if record.mesh.is_none() {
            return Err(EditorError::InvalidSceneContentValue);
        }
        record.material_override = material;
        Ok(())
    }

    fn write_light(&mut self, id: &EntityId, light: Light) -> Result<(), EditorError> {
        let record = self
            .world
            .entity_mut(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?;
        if record.light.is_none() {
            return Err(EditorError::InvalidSceneContentValue);
        }
        record.light = Some(light);
        Ok(())
    }

    fn write_camera(&mut self, id: &EntityId, camera: Camera) -> Result<(), EditorError> {
        let record = self
            .world
            .entity_mut(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?;
        if record.camera.is_none() {
            return Err(EditorError::InvalidSceneContentValue);
        }
        record.camera = Some(camera);
        Ok(())
    }

    pub fn rename_entity(&mut self, id: &EntityId, name: &str) -> Result<(), EditorError> {
        if name.trim().is_empty() {
            return Err(EditorError::InvalidEntityName);
        }
        let before = self
            .world
            .entity(id.as_str())
            .ok_or_else(|| ecs::EcsError::MissingEntity(id.to_string()))?
            .name
            .clone();
        if before == name {
            return Ok(());
        }
        let command = EditorCommand::RenameEntity {
            id: id.clone(),
            before,
            after: name.to_owned(),
        };
        self.apply_command(&command)?;
        self.push_undo(command);
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

        let mut created = ecs::EntityRecord::new(new_id.clone(), new_name, source.transform);
        created.parent = source.parent;
        created.mesh = source.mesh;
        created.material_override = source.material_override;
        created.light = source.light;
        let command = EditorCommand::DuplicateEntity {
            source: id.clone(),
            created,
            previous_selection: self.selected.clone(),
        };
        self.apply_command(&command)?;
        self.push_undo(command);
        Ok(new_id)
    }

    pub fn delete_selected(&mut self) -> Result<(), EditorError> {
        let id = self.selected.clone().ok_or(EditorError::MissingSelection)?;
        self.delete_entity(&id)
    }

    pub fn delete_entity(&mut self, id: &EntityId) -> Result<(), EditorError> {
        ensure_unprotected(id)?;
        let command = EditorCommand::DeleteEntity {
            deleted_root: id.clone(),
            records: self.subtree_records(id)?,
            previous_selection: self.selected.clone(),
        };
        self.apply_command(&command)?;
        self.push_undo(command);
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

    #[must_use]
    pub fn selected_camera_view(&self) -> Option<ViewportView> {
        let selected = self.selected.as_ref()?;
        let record = self.world.entity(selected.as_str())?;
        let camera = record.camera.as_ref()?;
        Some(ViewportView::new(
            selected.clone(),
            record.transform,
            camera.projection.clone(),
        ))
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
        self.set_material_override(
            &second,
            Some(MaterialOverride {
                base_color: [0.4, 0.9, 0.5, 1.0],
            }),
        )?;
        let _duplicate = self.duplicate_selected()?;
        self.set_light(
            &EntityId::new(LIGHT_ID),
            Light {
                kind: LightKind::Directional,
                color: [0.8, 0.9, 1.0],
                intensity: 1.25,
            },
        )?;
        self.set_camera(
            &EntityId::new(CAMERA_ID),
            Camera::new(Projection::Perspective {
                fov_y_degrees: 55.0,
            }),
        )?;
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
            has_light: self.world.entities().any(|entity| entity.light.is_some()),
            viewport_index_count: viewport_draw.index_count,
            transform_undo_redo_ok: false,
            content_reopen_ok: false,
        };
        Ok(report)
    }

    pub fn smoke_report_for_view(&self, view: &ViewportView) -> anyhow::Result<EditorSmokeReport> {
        self.smoke_report_for_view_with_checks(view, false, false)
    }

    pub fn smoke_report_for_view_with_checks(
        &self,
        view: &ViewportView,
        transform_undo_redo_ok: bool,
        content_reopen_ok: bool,
    ) -> anyhow::Result<EditorSmokeReport> {
        let render_scene = self.render_scene();
        let viewport_draw = self
            .viewport_draw_call_for_view(view)
            .ok_or_else(|| anyhow::anyhow!("viewport draw call missing after reopen"))?;

        let report = EditorSmokeReport {
            mesh_count: render_scene.meshes.len(),
            has_camera: render_scene.active_camera.is_some(),
            has_light: self.world.entities().any(|entity| entity.light.is_some()),
            viewport_index_count: viewport_draw.index_count,
            transform_undo_redo_ok,
            content_reopen_ok,
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

    fn next_imported_entity_id(&self, asset_name: &str) -> Result<EntityId, EditorError> {
        let base = format!("asset_{}", safe_entity_slug(asset_name));
        for index in 0..=u32::MAX {
            let id = if index == 0 {
                EntityId::new(&base)
            } else {
                EntityId::new(format!("{base}_{index}"))
            };
            if self.world.entity(id.as_str()).is_none() {
                return Ok(id);
            }
        }
        Err(EditorError::IdGenerationExhausted)
    }

    fn next_imported_entity_name(&self, asset_name: &str) -> Result<String, EditorError> {
        let base = clean_entity_name(asset_name);
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

fn clean_entity_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Imported Asset".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn safe_entity_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        "imported".to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests;

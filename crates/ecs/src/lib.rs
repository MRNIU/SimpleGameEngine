// Copyright The SimpleGameEngine Contributors
//
//! 最小 ECS 真源。

use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

use serde::{Deserialize, Serialize};
use sge_math::Transform;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(String);

impl EntityId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for EntityId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for EntityId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: EntityId,
    pub name: String,
    pub transform: Transform,
    pub parent: Option<EntityId>,
    #[serde(skip)]
    children: Vec<EntityId>,
    pub camera: Option<Camera>,
    pub mesh: Option<MeshRef>,
    pub material_override: Option<MaterialOverride>,
    pub light: Option<Light>,
}

impl EntityRecord {
    #[must_use]
    pub fn new(id: EntityId, name: impl Into<String>, transform: Transform) -> Self {
        Self {
            id,
            name: name.into(),
            transform,
            parent: None,
            children: Vec::new(),
            camera: None,
            mesh: None,
            material_override: None,
            light: None,
        }
    }

    #[must_use]
    pub fn children(&self) -> &[EntityId] {
        &self.children
    }

    fn clear_children(&mut self) {
        self.children.clear();
    }

    fn push_child(&mut self, child: EntityId) {
        self.children.push(child);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshRef {
    pub asset: String,
    pub material: String,
}

impl MeshRef {
    #[must_use]
    pub fn new(asset: impl Into<String>, material: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            material: material.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaterialOverride {
    pub base_color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub projection: Projection,
}

impl Camera {
    #[must_use]
    pub const fn new(projection: Projection) -> Self {
        Self { projection }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Projection {
    Perspective { fov_y_degrees: f32 },
    Orthographic { vertical_size: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Light {
    pub kind: LightKind,
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightKind {
    Directional,
    Point,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct World {
    entities: BTreeMap<EntityId, EntityRecord>,
}

impl World {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_records(records: Vec<EntityRecord>) -> Result<Self, EcsError> {
        let mut world = Self::new();
        for record in records {
            if world.entities.insert(record.id.clone(), record).is_some() {
                return Err(EcsError::DuplicateEntity);
            }
        }
        world.validate_parent_refs()?;
        world.rebuild_children_cache();
        Ok(world)
    }

    pub fn spawn(
        &mut self,
        id: EntityId,
        name: impl Into<String>,
        transform: Transform,
    ) -> EntityId {
        let record = EntityRecord::new(id.clone(), name, transform);
        self.entities.insert(id.clone(), record);
        id
    }

    #[must_use]
    pub fn entity(&self, id: impl AsRef<str>) -> Option<&EntityRecord> {
        self.entities.get(id.as_ref())
    }

    pub fn entity_mut(&mut self, id: impl AsRef<str>) -> Option<&mut EntityRecord> {
        self.entities.get_mut(id.as_ref())
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntityRecord> {
        self.entities.values()
    }

    pub fn rename_entity(
        &mut self,
        id: impl AsRef<str>,
        name: impl Into<String>,
    ) -> Result<(), EcsError> {
        let id = id.as_ref();
        let record = self
            .entities
            .get_mut(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?;
        record.name = name.into();
        Ok(())
    }

    pub fn delete_subtree(&mut self, id: impl AsRef<str>) -> Result<Option<EntityId>, EcsError> {
        let id = id.as_ref();
        let parent = self
            .entities
            .get(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?
            .parent
            .clone();
        let mut stack = vec![EntityId::new(id)];
        let mut subtree = BTreeSet::new();

        while let Some(current) = stack.pop() {
            if !subtree.insert(current.clone()) {
                continue;
            }
            stack.extend(
                self.entities
                    .values()
                    .filter(|record| record.parent.as_ref() == Some(&current))
                    .map(|record| record.id.clone()),
            );
        }

        for entity in subtree {
            self.entities.remove(&entity);
        }
        self.rebuild_children_cache();
        Ok(parent)
    }

    pub fn set_parent(
        &mut self,
        child: impl AsRef<str>,
        parent: impl AsRef<str>,
    ) -> Result<(), EcsError> {
        let child = child.as_ref();
        let parent = parent.as_ref();
        if child == parent {
            return Err(EcsError::SelfParent);
        }
        if !self.entities.contains_key(parent) {
            return Err(EcsError::MissingEntity(parent.to_owned()));
        }
        let record = self
            .entities
            .get_mut(child)
            .ok_or_else(|| EcsError::MissingEntity(child.to_owned()))?;
        record.parent = Some(EntityId::new(parent));
        self.rebuild_children_cache();
        Ok(())
    }

    pub fn set_translation(
        &mut self,
        id: impl AsRef<str>,
        translation: [f32; 3],
    ) -> Result<(), EcsError> {
        let id = id.as_ref();
        let record = self
            .entities
            .get_mut(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?;
        record.transform.translation = translation;
        Ok(())
    }

    pub fn insert_camera(&mut self, id: impl AsRef<str>, camera: Camera) -> Result<(), EcsError> {
        let id = id.as_ref();
        let record = self
            .entities
            .get_mut(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?;
        record.camera = Some(camera);
        Ok(())
    }

    pub fn insert_mesh(&mut self, id: impl AsRef<str>, mesh: MeshRef) -> Result<(), EcsError> {
        let id = id.as_ref();
        let record = self
            .entities
            .get_mut(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?;
        record.mesh = Some(mesh);
        Ok(())
    }

    pub fn insert_light(&mut self, id: impl AsRef<str>, light: Light) -> Result<(), EcsError> {
        let id = id.as_ref();
        let record = self
            .entities
            .get_mut(id)
            .ok_or_else(|| EcsError::MissingEntity(id.to_owned()))?;
        record.light = Some(light);
        Ok(())
    }

    #[must_use]
    pub fn children_of(&self, id: impl AsRef<str>) -> Vec<EntityId> {
        self.entities
            .get(id.as_ref())
            .map_or_else(Vec::new, |record| record.children.clone())
    }

    pub fn rebuild_children_cache(&mut self) {
        for record in self.entities.values_mut() {
            record.clear_children();
        }

        let edges: Vec<(EntityId, EntityId)> = self
            .entities
            .values()
            .filter_map(|record| {
                record
                    .parent
                    .as_ref()
                    .map(|parent| (parent.clone(), record.id.clone()))
            })
            .collect();

        for (parent, child) in edges {
            if let Some(parent_record) = self.entities.get_mut(&parent) {
                parent_record.push_child(child);
            }
        }
    }

    fn validate_parent_refs(&self) -> Result<(), EcsError> {
        for record in self.entities.values() {
            if let Some(parent) = &record.parent {
                if parent == &record.id {
                    return Err(EcsError::SelfParent);
                }
                if !self.entities.contains_key(parent) {
                    return Err(EcsError::MissingEntity(parent.to_string()));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EcsError {
    #[error("entity is missing: {0}")]
    MissingEntity(String),
    #[error("duplicate entity id")]
    DuplicateEntity,
    #[error("entity cannot be its own parent")]
    SelfParent,
}

#[cfg(test)]
mod tests {
    use super::{EcsError, EntityId, EntityRecord, World};
    use sge_math::Transform;

    #[test]
    fn rebuilds_children_from_parent_components() {
        let mut world = World::new();
        world.spawn(EntityId::new("root"), "Root", Transform::identity());
        world.spawn(EntityId::new("child"), "Child", Transform::identity());

        world.set_parent("child", "root").unwrap();

        assert_eq!(world.children_of("root"), vec![EntityId::new("child")]);
    }

    #[test]
    fn renames_entity_without_changing_its_id() {
        let mut world = World::new();
        world.spawn(EntityId::new("cube"), "Cube", Transform::identity());

        world.rename_entity("cube", "Player Cube").unwrap();

        let entity = world.entity("cube").unwrap();
        assert_eq!(entity.id, EntityId::new("cube"));
        assert_eq!(entity.name, "Player Cube");
    }

    #[test]
    fn deletes_subtree_and_rebuilds_children_cache() {
        let mut world = World::new();
        world.spawn(EntityId::new("root"), "Root", Transform::identity());
        world.spawn(EntityId::new("parent"), "Parent", Transform::identity());
        world.spawn(EntityId::new("child"), "Child", Transform::identity());
        world.spawn(EntityId::new("sibling"), "Sibling", Transform::identity());
        world.set_parent("parent", "root").unwrap();
        world.set_parent("child", "parent").unwrap();
        world.set_parent("sibling", "root").unwrap();

        let fallback = world.delete_subtree("parent").unwrap();

        assert_eq!(fallback, Some(EntityId::new("root")));
        assert!(world.entity("parent").is_none());
        assert!(world.entity("child").is_none());
        assert_eq!(world.children_of("root"), vec![EntityId::new("sibling")]);
    }

    #[test]
    fn from_records_rejects_duplicate_entity_ids() {
        let first = EntityRecord::new(EntityId::new("cube"), "Cube", Transform::identity());
        let second = EntityRecord::new(EntityId::new("cube"), "Cube Copy", Transform::identity());

        assert_eq!(
            World::from_records(vec![first, second]).unwrap_err(),
            EcsError::DuplicateEntity
        );
    }
}

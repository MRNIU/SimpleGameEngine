// Copyright The SimpleGameEngine Contributors

use sge_reflect::{FieldKey, ReflectError, ReflectedValue, Value};
use sge_scene::{AuthoringEntity, SceneEntityId};

use super::{EditSession, HistoryCommand, find_component, find_entity};
use crate::{EditError, InspectorComponent};

impl EditSession {
    pub fn add_component(
        &mut self,
        entity: SceneEntityId,
        component: &str,
    ) -> Result<(), EditError> {
        let value = self.component_draft(component)?;
        self.add_component_value(entity, value)
    }

    pub fn component_draft(&self, component: &str) -> Result<ReflectedValue, EditError> {
        Ok(self.app.type_registry().scene_value_draft(component)?)
    }

    pub fn set_component_draft_field(
        &self,
        draft: &ReflectedValue,
        field: &str,
        value: Value,
    ) -> Result<ReflectedValue, EditError> {
        Ok(self.app.type_registry().with_draft_field_value(
            draft,
            &FieldKey::new(field)?,
            &value,
        )?)
    }

    pub fn inspect_component_value(
        &self,
        value: &ReflectedValue,
    ) -> Result<InspectorComponent, EditError> {
        let descriptor = self
            .app
            .type_registry()
            .descriptor(value.type_key().as_str())
            .ok_or_else(|| {
                EditError::Reflect(ReflectError::UnknownTypeKey(value.type_key().to_string()))
            })?;
        InspectorComponent::from_reflected(descriptor, value)
    }

    pub fn add_component_value(
        &mut self,
        entity: SceneEntityId,
        value: ReflectedValue,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let authoring_entity = find_entity(&scene, entity)?;
        if authoring_entity
            .components()
            .any(|component| component.type_key() == value.type_key())
        {
            return Err(EditError::DuplicateComponent {
                entity,
                component: value.type_key().to_string(),
            });
        }
        self.execute(HistoryCommand::Component {
            entity,
            component: value.type_key().clone(),
            before: None,
            after: Some(value),
        })
    }

    pub fn remove_component(
        &mut self,
        entity: SceneEntityId,
        component: &str,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let value = find_component(find_entity(&scene, entity)?, component)?.clone();
        self.execute(HistoryCommand::Component {
            entity,
            component: value.type_key().clone(),
            before: Some(value),
            after: None,
        })
    }

    pub fn add_entity(&mut self, entity: AuthoringEntity) -> Result<(), EditError> {
        let id = entity.id();
        let scene = self.snapshot()?;
        if scene.entities().any(|candidate| candidate.id() == id) {
            return Err(EditError::DuplicateEntity { entity: id });
        }
        self.execute(HistoryCommand::Entity {
            entity: id,
            before: None,
            after: Some(entity),
        })
    }

    pub fn remove_entity(&mut self, entity: SceneEntityId) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let value = find_entity(&scene, entity)?.clone();
        if scene
            .entities()
            .any(|candidate| candidate.parent() == Some(entity))
        {
            return Err(EditError::EntityHasChildren { entity });
        }
        self.execute(HistoryCommand::Entity {
            entity,
            before: Some(value),
            after: None,
        })
    }
}

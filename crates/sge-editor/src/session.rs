// Copyright The SimpleGameEngine Contributors

use std::{path::Path, sync::Arc};

use sge_app::{EngineApp, GameDescriptor};
use sge_asset::RuntimeAssetStore;
use sge_asset_pipeline::import_project_assets;
use sge_project::{AuthoringAssetManifest, ProjectDescriptor, ProjectRoot};
use sge_reflect::{FieldKey, ReflectError, ReflectedValue, TypeKey, Value};
use sge_render::{RenderSnapshot, RenderView, extract};
use sge_scene::{
    AuthoringEntity, AuthoringScene, SceneEntityId, SceneInstance, instantiate, prepare, snapshot,
};

use crate::{
    EditError, EditorOpenError, EditorPreviewError, InspectorComponent, PlaySession, PlayStartError,
};

pub struct EditSession {
    game: GameDescriptor,
    project: ProjectRoot,
    descriptor: ProjectDescriptor,
    manifest: AuthoringAssetManifest,
    app: EngineApp,
    instance: SceneInstance,
    assets: Arc<RuntimeAssetStore>,
    selection: Option<SceneEntityId>,
    history: Vec<HistoryCommand>,
    cursor: usize,
    saved_cursor: Option<usize>,
}

#[derive(Clone)]
pub struct PreviewFrame {
    pub snapshot: RenderSnapshot,
    pub view: RenderView,
    pub assets: Arc<RuntimeAssetStore>,
}

#[derive(Default)]
pub struct EditorWorkspace {
    live: Option<EditSession>,
}

#[derive(Clone)]
enum HistoryCommand {
    Field {
        entity: SceneEntityId,
        component: TypeKey,
        field: FieldKey,
        before: Value,
        after: Value,
    },
    Component {
        entity: SceneEntityId,
        component: TypeKey,
        before: Option<ReflectedValue>,
        after: Option<ReflectedValue>,
    },
    Entity {
        entity: SceneEntityId,
        before: Option<AuthoringEntity>,
        after: Option<AuthoringEntity>,
    },
}

impl EditSession {
    pub fn open(
        game: GameDescriptor,
        project_root: impl AsRef<Path>,
    ) -> Result<Self, EditorOpenError> {
        let project = ProjectRoot::open(project_root)?;
        let descriptor = ProjectDescriptor::load(&project)?;
        descriptor.validate_for_game(game.game_id())?;
        let manifest = AuthoringAssetManifest::load(&project)?;
        let imported = import_project_assets(&project, &manifest)?;
        let assets = Arc::new(imported.into_parts().0);
        let scene_bytes = project.read(descriptor.default_authoring_scene())?;
        let scene_text = std::str::from_utf8(&scene_bytes)?;
        let scene = AuthoringScene::from_ron(scene_text)?;
        let mut app = game.create_app()?;
        let prepared = prepare(&scene, app.type_registry(), assets.as_ref())?;
        let instance = instantiate(prepared, app.world_initializer()?)?;
        Ok(Self {
            game,
            project,
            descriptor,
            manifest,
            app,
            instance,
            assets,
            selection: None,
            history: Vec::new(),
            cursor: 0,
            saved_cursor: Some(0),
        })
    }

    pub fn snapshot(&self) -> Result<AuthoringScene, EditError> {
        Ok(snapshot(
            self.app.world(),
            self.app.type_registry(),
            self.assets.as_ref(),
        )?)
    }

    pub fn preview_frame(&self) -> Result<PreviewFrame, EditorPreviewError> {
        let snapshot = extract(self.app.world(), self.assets.as_ref())?;
        let view = RenderView::from_active_camera(&snapshot)?;
        Ok(PreviewFrame {
            snapshot,
            view,
            assets: Arc::clone(&self.assets),
        })
    }

    pub fn start_play(&self) -> Result<PlaySession, PlayStartError> {
        PlaySession::start(self)
    }

    #[must_use]
    pub fn component<T: 'static>(&self, entity: SceneEntityId) -> Option<&T> {
        self.app.world().get(self.instance.entity(&entity)?)
    }

    pub fn select(&mut self, selection: Option<SceneEntityId>) -> Result<(), EditError> {
        if let Some(entity) = selection
            && self.instance.entity(&entity).is_none()
        {
            return Err(EditError::MissingEntity { entity });
        }
        self.selection = selection;
        Ok(())
    }

    #[must_use]
    pub const fn selection(&self) -> Option<SceneEntityId> {
        self.selection
    }

    pub fn inspector(&self) -> Result<Vec<InspectorComponent>, EditError> {
        let Some(selection) = self.selection else {
            return Ok(Vec::new());
        };
        let scene = self.snapshot()?;
        let entity = find_entity(&scene, selection)?;
        entity
            .components()
            .map(|component| {
                let descriptor = self
                    .app
                    .type_registry()
                    .descriptor(component.type_key().as_str())
                    .ok_or_else(|| {
                        EditError::Reflect(ReflectError::UnknownTypeKey(
                            component.type_key().to_string(),
                        ))
                    })?;
                InspectorComponent::from_reflected(descriptor, component)
            })
            .collect()
    }

    pub fn set_field(
        &mut self,
        entity: SceneEntityId,
        component: &str,
        field: &str,
        value: Value,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let reflected = find_component(find_entity(&scene, entity)?, component)?;
        let field = FieldKey::new(field)?;
        let before = reflected
            .fields()
            .get(field.as_str())
            .cloned()
            .ok_or_else(|| EditError::Reflect(ReflectError::UnknownField(field.clone())))?;
        if before == value {
            return Ok(());
        }
        self.execute(HistoryCommand::Field {
            entity,
            component: reflected.type_key().clone(),
            field,
            before,
            after: value,
        })
    }

    pub fn add_component(
        &mut self,
        entity: SceneEntityId,
        component: &str,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let authoring_entity = find_entity(&scene, entity)?;
        if authoring_entity
            .components()
            .any(|value| value.type_key().as_str() == component)
        {
            return Err(EditError::DuplicateComponent {
                entity,
                component: component.to_owned(),
            });
        }
        let value = self.app.type_registry().default_scene_value(component)?;
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

    pub fn undo(&mut self) -> Result<(), EditError> {
        if self.cursor == 0 {
            return Err(EditError::NothingToUndo);
        }
        let command = self.history[self.cursor - 1].clone();
        self.apply_and_commit(&command, false)?;
        self.cursor -= 1;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), EditError> {
        let Some(command) = self.history.get(self.cursor).cloned() else {
            return Err(EditError::NothingToRedo);
        };
        self.apply_and_commit(&command, true)?;
        self.cursor += 1;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), EditError> {
        let encoded = self.snapshot()?.to_ron()?;
        self.project.write_atomic(
            self.descriptor.default_authoring_scene(),
            encoded.as_bytes(),
        )?;
        self.saved_cursor = Some(self.cursor);
        Ok(())
    }

    #[must_use]
    pub const fn history_cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub const fn saved_cursor(&self) -> Option<usize> {
        self.saved_cursor
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.saved_cursor != Some(self.cursor)
    }

    #[must_use]
    pub const fn descriptor(&self) -> &ProjectDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn manifest(&self) -> &AuthoringAssetManifest {
        &self.manifest
    }

    #[must_use]
    pub const fn project(&self) -> &ProjectRoot {
        &self.project
    }

    pub(crate) const fn game(&self) -> GameDescriptor {
        self.game
    }

    pub(crate) fn assets(&self) -> &RuntimeAssetStore {
        self.assets.as_ref()
    }

    pub(crate) const fn assets_arc(&self) -> &Arc<RuntimeAssetStore> {
        &self.assets
    }

    fn execute(&mut self, command: HistoryCommand) -> Result<(), EditError> {
        self.apply_and_commit(&command, true)?;
        if self.saved_cursor.is_some_and(|saved| saved > self.cursor) {
            self.saved_cursor = None;
        }
        self.history.truncate(self.cursor);
        self.history.push(command);
        self.cursor += 1;
        Ok(())
    }

    fn apply_and_commit(
        &mut self,
        command: &HistoryCommand,
        forward: bool,
    ) -> Result<(), EditError> {
        let scene = apply_command(self.snapshot()?, self.app.type_registry(), command, forward)?;
        let mut app = self.game.create_app()?;
        let prepared = prepare(&scene, app.type_registry(), self.assets.as_ref())?;
        let instance = instantiate(prepared, app.world_initializer()?)?;
        self.app = app;
        self.instance = instance;
        if self
            .selection
            .is_some_and(|selection| self.instance.entity(&selection).is_none())
        {
            self.selection = None;
        }
        Ok(())
    }
}

impl EditorWorkspace {
    pub fn replace(
        &mut self,
        game: GameDescriptor,
        project_root: impl AsRef<Path>,
    ) -> Result<(), EditorOpenError> {
        let candidate = EditSession::open(game, project_root)?;
        self.live = Some(candidate);
        Ok(())
    }

    #[must_use]
    pub const fn live(&self) -> Option<&EditSession> {
        self.live.as_ref()
    }

    #[must_use]
    pub const fn live_mut(&mut self) -> Option<&mut EditSession> {
        self.live.as_mut()
    }
}

fn apply_command(
    scene: AuthoringScene,
    registry: &sge_reflect::TypeRegistry,
    command: &HistoryCommand,
    forward: bool,
) -> Result<AuthoringScene, EditError> {
    let mut entities = scene.entities().cloned().collect::<Vec<_>>();
    match command {
        HistoryCommand::Field {
            entity,
            component,
            field,
            before,
            after,
        } => {
            let target = if forward { after } else { before };
            let entity = entity_mut(&mut entities, *entity)?;
            let mut components = entity.components().cloned().collect::<Vec<_>>();
            let component_value = component_mut(&mut components, entity.id(), component.as_str())?;
            *component_value = registry.with_field_value(component_value, field, target)?;
            *entity = AuthoringEntity::new(entity.id(), entity.parent(), components)?;
        }
        HistoryCommand::Component {
            entity,
            component,
            before,
            after,
        } => {
            let target = if forward { after } else { before };
            let entity = entity_mut(&mut entities, *entity)?;
            let mut components = entity
                .components()
                .filter(|value| value.type_key() != component)
                .cloned()
                .collect::<Vec<_>>();
            if let Some(value) = target {
                components.push(value.clone());
            }
            *entity = AuthoringEntity::new(entity.id(), entity.parent(), components)?;
        }
        HistoryCommand::Entity {
            entity,
            before,
            after,
        } => {
            let target = if forward { after } else { before };
            entities.retain(|candidate| candidate.id() != *entity);
            if let Some(value) = target {
                entities.push(value.clone());
            }
        }
    }
    Ok(AuthoringScene::new(entities)?)
}

fn find_entity(
    scene: &AuthoringScene,
    entity: SceneEntityId,
) -> Result<&AuthoringEntity, EditError> {
    scene
        .entities()
        .find(|candidate| candidate.id() == entity)
        .ok_or(EditError::MissingEntity { entity })
}

fn find_component<'entity>(
    entity: &'entity AuthoringEntity,
    component: &str,
) -> Result<&'entity ReflectedValue, EditError> {
    entity
        .components()
        .find(|candidate| candidate.type_key().as_str() == component)
        .ok_or_else(|| EditError::MissingComponent {
            entity: entity.id(),
            component: component.to_owned(),
        })
}

fn entity_mut(
    entities: &mut [AuthoringEntity],
    entity: SceneEntityId,
) -> Result<&mut AuthoringEntity, EditError> {
    entities
        .iter_mut()
        .find(|candidate| candidate.id() == entity)
        .ok_or(EditError::MissingEntity { entity })
}

fn component_mut<'components>(
    components: &'components mut [ReflectedValue],
    entity: SceneEntityId,
    component: &str,
) -> Result<&'components mut ReflectedValue, EditError> {
    components
        .iter_mut()
        .find(|candidate| candidate.type_key().as_str() == component)
        .ok_or_else(|| EditError::MissingComponent {
            entity,
            component: component.to_owned(),
        })
}

// Copyright The SimpleGameEngine Contributors

use serde::{Deserialize, Serialize};
use sge_asset::{AssetId, AssetLookup};
use sge_reflect::{ReflectedValue, TypeRegistry};

use crate::{
    AuthoringScene, PreparedScene, SceneEntityId, SceneValidationError,
    validation::{SceneEntityView, validate_scene},
};

pub const RUNTIME_SCENE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeEntity {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: Vec<ReflectedValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeScene {
    entities: Vec<RuntimeEntity>,
}

pub struct RuntimeSceneBuild {
    scene: RuntimeScene,
    root_assets: Vec<AssetId>,
}

#[derive(Serialize, Deserialize)]
enum RuntimeSceneRole {
    Runtime,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSceneWire {
    format_version: u32,
    scene_role: RuntimeSceneRole,
    entities: Vec<RuntimeEntityWire>,
}

#[derive(Deserialize)]
struct RuntimeSceneVersionWire {
    format_version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeEntityWire {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: Vec<ReflectedValue>,
}

impl RuntimeEntity {
    fn new(
        id: SceneEntityId,
        parent: Option<SceneEntityId>,
        mut components: Vec<ReflectedValue>,
    ) -> Result<Self, SceneValidationError> {
        components.sort_unstable_by(|left, right| left.type_key().cmp(right.type_key()));
        if let Some(pair) = components
            .windows(2)
            .find(|pair| pair[0].type_key() == pair[1].type_key())
        {
            return Err(SceneValidationError::DuplicateComponent {
                entity: id,
                component: pair[0].type_key().clone(),
            });
        }
        Ok(Self {
            id,
            parent,
            components,
        })
    }

    #[must_use]
    pub const fn id(&self) -> SceneEntityId {
        self.id
    }

    #[must_use]
    pub const fn parent(&self) -> Option<SceneEntityId> {
        self.parent
    }

    pub fn components(&self) -> impl Iterator<Item = &ReflectedValue> {
        self.components.iter()
    }
}

impl RuntimeScene {
    fn new(mut entities: Vec<RuntimeEntity>) -> Result<Self, SceneValidationError> {
        entities.sort_unstable_by_key(RuntimeEntity::id);
        if let Some(pair) = entities.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(SceneValidationError::DuplicateEntity { entity: pair[0].id });
        }
        Ok(Self { entities })
    }

    pub fn entities(&self) -> impl Iterator<Item = &RuntimeEntity> {
        self.entities.iter()
    }

    pub fn from_ron(input: &str) -> Result<Self, RuntimeSceneFormatError> {
        let version: RuntimeSceneVersionWire =
            ron::from_str(input).map_err(|source| RuntimeSceneFormatError::Parse {
                source: Box::new(source),
            })?;
        if version.format_version != RUNTIME_SCENE_FORMAT_VERSION {
            return Err(RuntimeSceneFormatError::VersionMismatch {
                expected: RUNTIME_SCENE_FORMAT_VERSION,
                found: version.format_version,
            });
        }
        let wire: RuntimeSceneWire =
            ron::from_str(input).map_err(|source| RuntimeSceneFormatError::Parse {
                source: Box::new(source),
            })?;
        let entities = wire
            .entities
            .into_iter()
            .map(|entity| RuntimeEntity::new(entity.id, entity.parent, entity.components))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(entities).map_err(RuntimeSceneFormatError::from)
    }

    pub fn to_ron(&self) -> Result<String, RuntimeSceneFormatError> {
        let wire = RuntimeSceneWire {
            format_version: RUNTIME_SCENE_FORMAT_VERSION,
            scene_role: RuntimeSceneRole::Runtime,
            entities: self
                .entities
                .iter()
                .map(|entity| RuntimeEntityWire {
                    id: entity.id,
                    parent: entity.parent,
                    components: entity.components.clone(),
                })
                .collect(),
        };
        ron::ser::to_string_pretty(&wire, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|source| RuntimeSceneFormatError::Serialize { source })
    }
}

impl RuntimeSceneBuild {
    #[must_use]
    pub const fn scene(&self) -> &RuntimeScene {
        &self.scene
    }

    #[must_use]
    pub fn root_assets(&self) -> &[AssetId] {
        &self.root_assets
    }
}

pub fn build_runtime_scene(
    authoring: &AuthoringScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<RuntimeSceneBuild, RuntimeSceneBuildError> {
    let views = authoring
        .entities()
        .map(|entity| SceneEntityView::new(entity.id(), entity.parent(), entity.components_slice()))
        .collect::<Vec<_>>();
    let (_, root_assets) = validate_scene(&views, registry, assets)?.into_parts();
    let entities = authoring
        .entities()
        .map(|entity| {
            RuntimeEntity::new(
                entity.id(),
                entity.parent(),
                entity.components().cloned().collect(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeSceneBuild {
        scene: RuntimeScene::new(entities)?,
        root_assets,
    })
}

pub fn prepare_runtime(
    scene: &RuntimeScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<PreparedScene, SceneValidationError> {
    let views = scene
        .entities
        .iter()
        .map(|entity| SceneEntityView::new(entity.id, entity.parent, &entity.components))
        .collect::<Vec<_>>();
    validate_scene(&views, registry, assets).map(|output| output.into_prepared())
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeSceneBuildError {
    #[error("cannot build runtime scene: {0}")]
    Validation(Box<SceneValidationError>),
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeSceneFormatError {
    #[error("cannot parse RuntimeScene: {source}")]
    Parse {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize RuntimeScene: {source}")]
    Serialize {
        #[source]
        source: ron::Error,
    },
    #[error("unsupported RuntimeScene version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("invalid RuntimeScene: {0}")]
    Validation(Box<SceneValidationError>),
}

impl From<SceneValidationError> for RuntimeSceneBuildError {
    fn from(source: SceneValidationError) -> Self {
        Self::Validation(Box::new(source))
    }
}

impl From<SceneValidationError> for RuntimeSceneFormatError {
    fn from(source: SceneValidationError) -> Self {
        Self::Validation(Box::new(source))
    }
}

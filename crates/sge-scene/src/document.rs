// Copyright The SimpleGameEngine Contributors

use serde::{Deserialize, Serialize};
use sge_reflect::ReflectedValue;

use crate::{SceneEntityId, SceneValidationError};

pub const AUTHORING_SCENE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoringEntity {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: Vec<ReflectedValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoringScene {
    entities: Vec<AuthoringEntity>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoringSceneWire {
    format_version: u32,
    entities: Vec<AuthoringEntityWire>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoringEntityWire {
    id: SceneEntityId,
    parent: Option<SceneEntityId>,
    components: Vec<ReflectedValue>,
}

impl AuthoringEntity {
    pub fn new(
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

    pub(crate) fn components_slice(&self) -> &[ReflectedValue] {
        &self.components
    }
}

impl AuthoringScene {
    pub fn new(mut entities: Vec<AuthoringEntity>) -> Result<Self, SceneValidationError> {
        entities.sort_unstable_by_key(AuthoringEntity::id);
        if let Some(pair) = entities.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(SceneValidationError::DuplicateEntity { entity: pair[0].id });
        }
        Ok(Self { entities })
    }

    pub fn entities(&self) -> impl Iterator<Item = &AuthoringEntity> {
        self.entities.iter()
    }

    pub fn from_ron(input: &str) -> Result<Self, SceneFormatError> {
        let mut wire: AuthoringSceneWire =
            ron::from_str(input).map_err(|source| SceneFormatError::Parse {
                source: Box::new(source),
            })?;
        if wire.format_version != AUTHORING_SCENE_FORMAT_VERSION {
            return Err(SceneFormatError::VersionMismatch {
                expected: AUTHORING_SCENE_FORMAT_VERSION,
                found: wire.format_version,
            });
        }

        wire.entities.sort_unstable_by_key(|entity| entity.id);
        if let Some(pair) = wire
            .entities
            .windows(2)
            .find(|pair| pair[0].id == pair[1].id)
        {
            return Err(SceneFormatError::validation(
                SceneValidationError::DuplicateEntity { entity: pair[0].id },
            ));
        }
        let entities = wire
            .entities
            .into_iter()
            .map(|entity| AuthoringEntity::new(entity.id, entity.parent, entity.components))
            .collect::<Result<Vec<_>, _>>()
            .map_err(SceneFormatError::validation)?;
        Ok(Self { entities })
    }

    pub fn to_ron(&self) -> Result<String, SceneFormatError> {
        let wire = AuthoringSceneWire {
            format_version: AUTHORING_SCENE_FORMAT_VERSION,
            entities: self
                .entities
                .iter()
                .map(|entity| AuthoringEntityWire {
                    id: entity.id,
                    parent: entity.parent,
                    components: entity.components.clone(),
                })
                .collect(),
        };
        ron::ser::to_string_pretty(&wire, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|source| SceneFormatError::Serialize { source })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SceneFormatError {
    #[error("cannot parse authoring scene: {source}")]
    Parse {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize authoring scene: {source}")]
    Serialize {
        #[source]
        source: ron::Error,
    },
    #[error("unsupported authoring scene version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("invalid authoring scene: {source}")]
    Validation {
        #[source]
        source: Box<SceneValidationError>,
    },
}

impl SceneFormatError {
    fn validation(source: SceneValidationError) -> Self {
        Self::Validation {
            source: Box::new(source),
        }
    }
}

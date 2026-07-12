// Copyright The SimpleGameEngine Contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::SceneEntityId;

use super::SceneValidationError;

pub(super) fn validate_parent_graph(
    entities: impl Iterator<Item = (SceneEntityId, Option<SceneEntityId>)>,
) -> Result<(), SceneValidationError> {
    let parents = entities.collect::<BTreeMap<_, _>>();

    for (entity, parent) in &parents {
        if parent == &Some(*entity) {
            return Err(SceneValidationError::SelfParent { entity: *entity });
        }
    }
    for (entity, parent) in &parents {
        if let Some(parent) = parent
            && !parents.contains_key(parent)
        {
            return Err(SceneValidationError::MissingParent {
                entity: *entity,
                parent: *parent,
            });
        }
    }

    let mut complete = BTreeSet::new();
    for start in parents.keys().copied() {
        if complete.contains(&start) {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = BTreeMap::new();
        let mut current = start;
        loop {
            if complete.contains(&current) {
                break;
            }
            if let Some(position) = positions.get(&current).copied() {
                let cycle_start = path[position..]
                    .iter()
                    .copied()
                    .fold(current, SceneEntityId::min);
                return Err(SceneValidationError::ParentCycle {
                    entity: cycle_start,
                });
            }
            positions.insert(current, path.len());
            path.push(current);
            let Some(parent) = parents.get(&current).copied().flatten() else {
                break;
            };
            current = parent;
        }
        complete.extend(path);
    }
    Ok(())
}

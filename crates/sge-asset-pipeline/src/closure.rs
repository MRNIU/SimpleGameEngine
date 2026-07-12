// Copyright The SimpleGameEngine Contributors

use std::collections::{BTreeMap, BTreeSet};

use sge_asset::AssetId;

pub(crate) fn dependency_closure(
    roots: &[AssetId],
    dependencies: &BTreeMap<AssetId, Vec<AssetId>>,
) -> Result<Vec<AssetId>, ClosureError> {
    let mut pending = roots.iter().copied().collect::<BTreeSet<_>>();
    if let Some(root) = pending.iter().find(|root| !dependencies.contains_key(root)) {
        return Err(ClosureError::MissingRoot { root: *root });
    }

    let mut visited = BTreeSet::new();
    while let Some(asset) = pending.pop_first() {
        if !visited.insert(asset) {
            continue;
        }
        let asset_dependencies = &dependencies[&asset];
        if let Some(dependency) = asset_dependencies
            .iter()
            .filter(|dependency| !dependencies.contains_key(dependency))
            .min()
        {
            return Err(ClosureError::MissingDependency {
                asset,
                dependency: *dependency,
            });
        }
        pending.extend(
            asset_dependencies
                .iter()
                .copied()
                .filter(|dependency| !visited.contains(dependency)),
        );
    }
    Ok(visited.into_iter().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ClosureError {
    #[error("runtime closure root asset is missing: {root}")]
    MissingRoot { root: AssetId },
    #[error("runtime asset {asset} depends on missing asset {dependency}")]
    MissingDependency { asset: AssetId, dependency: AssetId },
}

#[cfg(test)]
mod tests;

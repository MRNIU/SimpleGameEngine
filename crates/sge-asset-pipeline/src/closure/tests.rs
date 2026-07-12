// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::AssetId;

use super::{ClosureError, dependency_closure};

fn id(value: u8) -> AssetId {
    format!("{value:08x}-0000-4000-8000-000000000001")
        .parse()
        .expect("fixture asset ID must be valid")
}

#[test]
fn closure_is_sorted_unique_and_excludes_unreachable_assets() {
    let low = id(1);
    let middle = id(2);
    let high = id(3);
    let unused = id(4);
    let graph = BTreeMap::from([
        (low, vec![high]),
        (middle, Vec::new()),
        (high, vec![middle]),
        (unused, Vec::new()),
    ]);

    let closure = dependency_closure(&[high, low, high], &graph).expect("closure must resolve");

    assert_eq!(closure, vec![low, middle, high]);
}

#[test]
fn closure_reports_the_lowest_missing_root_deterministically() {
    let low_missing = id(1);
    let high_missing = id(3);
    let graph = BTreeMap::from([(id(2), Vec::new())]);

    let error = dependency_closure(&[high_missing, low_missing], &graph)
        .expect_err("missing root must fail");

    assert_eq!(error, ClosureError::MissingRoot { root: low_missing });
}

#[test]
fn closure_reports_missing_dependency_with_owning_asset() {
    let root = id(1);
    let low_missing = id(2);
    let high_missing = id(3);
    let graph = BTreeMap::from([(root, vec![high_missing, low_missing])]);

    let error = dependency_closure(&[root], &graph).expect_err("missing dependency must fail");

    assert_eq!(
        error,
        ClosureError::MissingDependency {
            asset: root,
            dependency: low_missing,
        }
    );
}

#[test]
fn closure_terminates_stably_across_cycles() {
    let first = id(1);
    let second = id(2);
    let third = id(3);
    let graph = BTreeMap::from([
        (first, vec![second]),
        (second, vec![third]),
        (third, vec![first]),
    ]);

    assert_eq!(
        dependency_closure(&[second], &graph).expect("cycle must terminate"),
        vec![first, second, third]
    );
}

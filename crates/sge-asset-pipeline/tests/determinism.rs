// Copyright The SimpleGameEngine Contributors

mod support;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use sge_asset_pipeline::{CacheStatus, CookOutputRoot, full_cook};

use support::{FullCookFixture, GAME_ID, registry, world};

#[test]
fn full_cook_is_identical_across_reuse_clean_root_and_cache_rebuilds()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = FullCookFixture::new("determinism")?;
    let project = fixture.project()?;
    let registry = registry(true)?;
    let world = world(true, true)?;
    let output = CookOutputRoot::open(fixture.output_path())?;

    let first = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert!(
        first
            .import_statuses()
            .iter()
            .all(|(_, status)| *status == CacheStatus::Rebuilt)
    );
    let expected_generation = first.generation().clone();
    let expected_tree = snapshot_tree(fixture.output_path())?;

    let reused = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert_eq!(reused.generation(), &expected_generation);
    assert!(
        reused
            .import_statuses()
            .iter()
            .all(|(_, status)| *status == CacheStatus::Hit)
    );
    assert_eq!(snapshot_tree(fixture.output_path())?, expected_tree);

    let second_root = fixture.create_output("second-output")?;
    let clean = full_cook(
        &project,
        GAME_ID,
        &registry,
        &world,
        &CookOutputRoot::open(&second_root)?,
    )?;
    assert_eq!(clean.generation(), &expected_generation);
    assert_eq!(snapshot_tree(&second_root)?, expected_tree);

    fixture.delete_cache()?;
    let cache_deleted = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert_eq!(cache_deleted.generation(), &expected_generation);
    assert!(
        cache_deleted
            .import_statuses()
            .iter()
            .all(|(_, status)| *status == CacheStatus::Rebuilt)
    );
    assert_eq!(snapshot_tree(fixture.output_path())?, expected_tree);

    fixture.corrupt_all_import_cache_entries()?;
    let cache_corrupt = full_cook(&project, GAME_ID, &registry, &world, &output)?;
    assert_eq!(cache_corrupt.generation(), &expected_generation);
    assert!(
        cache_corrupt
            .import_statuses()
            .iter()
            .all(|(_, status)| *status == CacheStatus::Rebuilt)
    );
    assert_eq!(snapshot_tree(fixture.output_path())?, expected_tree);
    Ok(())
}

fn snapshot_tree(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, std::io::Error> {
    let mut files = BTreeMap::new();
    collect_files(root, Path::new(""), &mut files)?;
    Ok(files)
}

fn collect_files(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let child = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect_files(root, &child, files)?;
        } else {
            files.insert(child.clone(), fs::read(root.join(child))?);
        }
    }
    Ok(())
}

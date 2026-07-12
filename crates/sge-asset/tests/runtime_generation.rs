// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{
    AssetId, RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeCatalogError, RuntimeProductPath,
};
use sge_reflect::TypeKey;

type ProductBytes = BTreeMap<AssetId, Vec<u8>>;
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn id(value: u128) -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok(format!("{value:08x}-0000-4000-8000-000000000001").parse()?)
}

fn record(
    id: AssetId,
    asset_type: &str,
    path: &str,
    dependencies: Vec<AssetId>,
) -> Result<RuntimeAssetRecord, Box<dyn std::error::Error>> {
    Ok(RuntimeAssetRecord::new(
        id,
        TypeKey::new(asset_type)?,
        RuntimeProductPath::new(path)?,
        dependencies,
    )?)
}

fn build(
    game_id: &str,
    entry_path: &str,
    entry_bytes: &[u8],
    records: Vec<RuntimeAssetRecord>,
    products: ProductBytes,
) -> TestResult<RuntimeAssetCatalog> {
    Ok(RuntimeAssetCatalog::build(
        TypeKey::new(game_id)?,
        RuntimeProductPath::new(entry_path)?,
        records,
        entry_bytes,
        &products,
    )?)
}

fn single_input() -> TestResult<(Vec<RuntimeAssetRecord>, ProductBytes)> {
    let asset = id(0x1000_0000)?;
    Ok((
        vec![record(
            asset,
            "demo.product",
            "Content/a.product.ron",
            Vec::new(),
        )?],
        BTreeMap::from([(asset, b"product-a".to_vec())]),
    ))
}

#[test]
fn generation_digest_has_a_stable_golden_value() -> Result<(), Box<dyn std::error::Error>> {
    let (records, products) = single_input()?;
    let catalog = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"scene-a",
        records,
        products,
    )?;

    assert_eq!(
        catalog.generation().as_str(),
        "5ddc9634ad035fedb6e637158aaea4ac517327f03c675b0b487d559e7dcc0c4f"
    );
    Ok(())
}

#[test]
fn generation_changes_for_every_catalog_or_product_input() -> Result<(), Box<dyn std::error::Error>>
{
    let (records, products) = single_input()?;
    let base = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"scene-a",
        records.clone(),
        products.clone(),
    )?;
    let generation = |catalog: RuntimeAssetCatalog| catalog.generation().clone();

    assert_ne!(
        generation(build(
            "other.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            records.clone(),
            products.clone()
        )?),
        *base.generation()
    );
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-b",
            records.clone(),
            products.clone()
        )?),
        *base.generation()
    );
    assert!(
        build(
            "demo.game",
            "Scenes/other.runtime-scene.ron",
            b"scene-a",
            records.clone(),
            products.clone(),
        )
        .is_err()
    );

    let asset = id(0x1000_0000)?;
    let changed_id = id(0x3000_0000)?;
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            vec![record(
                changed_id,
                "demo.product",
                "Content/a.product.ron",
                Vec::new(),
            )?],
            BTreeMap::from([(changed_id, b"product-a".to_vec())]),
        )?),
        *base.generation()
    );
    let changed_type = vec![record(
        asset,
        "demo.other",
        "Content/a.product.ron",
        Vec::new(),
    )?];
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            changed_type,
            products.clone()
        )?),
        *base.generation()
    );
    let changed_path = vec![record(
        asset,
        "demo.product",
        "Content/b.product.ron",
        Vec::new(),
    )?];
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            changed_path,
            products.clone()
        )?),
        *base.generation()
    );

    let dependency = id(0x2000_0000)?;
    let without_dependency = vec![
        record(asset, "demo.product", "Content/a.product.ron", Vec::new())?,
        record(
            dependency,
            "demo.product",
            "Content/d.product.ron",
            Vec::new(),
        )?,
    ];
    let with_dependency = vec![
        record(
            asset,
            "demo.product",
            "Content/a.product.ron",
            vec![dependency],
        )?,
        record(
            dependency,
            "demo.product",
            "Content/d.product.ron",
            Vec::new(),
        )?,
    ];
    let dependency_products = BTreeMap::from([
        (asset, b"product-a".to_vec()),
        (dependency, b"product-d".to_vec()),
    ]);
    let no_dependency = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"scene-a",
        without_dependency,
        dependency_products.clone(),
    )?;
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            with_dependency,
            dependency_products
        )?),
        *no_dependency.generation()
    );

    let changed_product = BTreeMap::from([(asset, b"product-b".to_vec())]);
    assert_ne!(
        generation(build(
            "demo.game",
            "Scenes/entry.runtime-scene.ron",
            b"scene-a",
            records,
            changed_product
        )?),
        *base.generation()
    );
    Ok(())
}

#[test]
fn generation_length_frames_prevent_split_collisions() -> Result<(), Box<dyn std::error::Error>> {
    let asset = id(0x1000_0000)?;
    let records = vec![record(
        asset,
        "demo.product",
        "Content/a.product.ron",
        Vec::new(),
    )?];
    let left = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"ab",
        records.clone(),
        BTreeMap::from([(asset, b"c".to_vec())]),
    )?;
    let right = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"a",
        records,
        BTreeMap::from([(asset, b"bc".to_vec())]),
    )?;

    assert_ne!(left.generation(), right.generation());
    Ok(())
}

#[test]
fn build_and_verify_require_the_exact_product_set() -> Result<(), Box<dyn std::error::Error>> {
    let (records, products) = single_input()?;
    let catalog = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"scene-a",
        records.clone(),
        products.clone(),
    )?;
    catalog.verify_generation(b"scene-a", &products)?;

    let missing = BTreeMap::new();
    assert!(matches!(
        catalog.verify_generation(b"scene-a", &missing),
        Err(RuntimeCatalogError::MissingProductBytes { .. })
    ));
    let mut extra = products.clone();
    extra.insert(id(0x2000_0000)?, b"extra".to_vec());
    assert!(matches!(
        catalog.verify_generation(b"scene-a", &extra),
        Err(RuntimeCatalogError::UnexpectedProductBytes { .. })
    ));
    assert!(
        RuntimeAssetCatalog::build(
            TypeKey::new("demo.game")?,
            RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
            records,
            b"scene-a",
            &missing,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn catalog_only_game_id_tamper_fails_generation_verification()
-> Result<(), Box<dyn std::error::Error>> {
    let (records, products) = single_input()?;
    let catalog = build(
        "demo.game",
        "Scenes/entry.runtime-scene.ron",
        b"scene-a",
        records,
        products.clone(),
    )?;
    let tampered =
        RuntimeAssetCatalog::from_ron(&catalog.to_ron()?.replacen("demo.game", "other.game", 1))?;

    assert!(matches!(
        tampered.verify_generation(b"scene-a", &products),
        Err(RuntimeCatalogError::GenerationMismatch { .. })
    ));
    Ok(())
}

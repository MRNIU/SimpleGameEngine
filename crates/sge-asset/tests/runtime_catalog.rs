// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{
    AssetId, MESH_ASSET_TYPE_KEY, RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeCatalogError,
    RuntimeGenerationId, RuntimeProductPath,
};
use sge_reflect::TypeKey;

fn asset_id(value: u128) -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok(format!("{value:08x}-0000-4000-8000-000000000001").parse()?)
}

fn mesh_record(id: AssetId) -> Result<RuntimeAssetRecord, Box<dyn std::error::Error>> {
    Ok(RuntimeAssetRecord::new(
        id,
        TypeKey::new(MESH_ASSET_TYPE_KEY)?,
        RuntimeProductPath::new(format!("Content/{id}.mesh.ron"))?,
        Vec::new(),
    )?)
}

fn catalog(
    records: Vec<RuntimeAssetRecord>,
) -> Result<RuntimeAssetCatalog, Box<dyn std::error::Error>> {
    let products = records
        .iter()
        .map(|record| (*record.id(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    Ok(RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        records,
        b"scene",
        &products,
    )?)
}

#[test]
fn runtime_product_path_is_canonical_runtime_relative() {
    assert_eq!(
        RuntimeProductPath::new("Content/mesh.mesh.ron")
            .expect("valid path")
            .as_str(),
        "Content/mesh.mesh.ron"
    );
    for invalid in [
        "",
        "/Content/a",
        "Content\\a",
        "C:/a",
        "Content//a",
        "Content/./a",
        "Content/../a",
        "Content/a\0b",
    ] {
        assert!(RuntimeProductPath::new(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn generation_id_accepts_only_lowercase_sha256_text() {
    let valid = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    assert_eq!(
        valid
            .parse::<RuntimeGenerationId>()
            .expect("valid")
            .as_str(),
        valid
    );
    for invalid in [
        "",
        "0123456789abcdef",
        "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef",
        "g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        " 123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    ] {
        assert!(
            invalid.parse::<RuntimeGenerationId>().is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn catalog_sorts_records_and_dependencies_and_exposes_borrowed_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let first = asset_id(0x1000_0000)?;
    let second = asset_id(0x2000_0000)?;
    let unknown = RuntimeAssetRecord::new(
        second,
        TypeKey::new("demo.bundle")?,
        RuntimeProductPath::new("Content/bundle.product.ron")?,
        vec![first],
    )?;
    let catalog = catalog(vec![unknown, mesh_record(first)?])?;

    assert_eq!(catalog.game_id().as_str(), "demo.game");
    assert_eq!(catalog.generation().as_str().len(), 64);
    assert_eq!(
        catalog.entry_scene().as_str(),
        "Scenes/entry.runtime-scene.ron"
    );
    assert_eq!(catalog.assets()[0].id(), &first);
    assert_eq!(catalog.assets()[1].dependencies(), &[first]);
    assert_eq!(
        catalog
            .asset(&second)
            .expect("record")
            .asset_type()
            .as_str(),
        "demo.bundle"
    );
    Ok(())
}

#[test]
fn catalog_rejects_reserved_unassigned_asset_ids() -> Result<(), Box<dyn std::error::Error>> {
    let assigned = asset_id(0x1000_0000)?;
    assert!(matches!(
        RuntimeAssetRecord::new(
            AssetId::nil(),
            TypeKey::new("demo.bundle")?,
            RuntimeProductPath::new("Content/unassigned.product.ron")?,
            Vec::new(),
        ),
        Err(RuntimeCatalogError::InvalidAssetId { .. })
    ));
    assert!(matches!(
        RuntimeAssetRecord::new(
            assigned,
            TypeKey::new("demo.bundle")?,
            RuntimeProductPath::new("Content/bundle.product.ron")?,
            vec![AssetId::nil()],
        ),
        Err(RuntimeCatalogError::InvalidAssetId { .. })
    ));
    Ok(())
}

#[test]
fn catalog_rejects_duplicate_ids_paths_dependencies_and_missing_dependencies()
-> Result<(), Box<dyn std::error::Error>> {
    let first = asset_id(0x1000_0000)?;
    let second = asset_id(0x2000_0000)?;
    assert!(catalog(vec![mesh_record(first)?, mesh_record(first)?]).is_err());

    let shared_path = RuntimeProductPath::new("Content/shared.product.ron")?;
    let bundle_type = TypeKey::new("demo.bundle")?;
    let record = |id, dependencies| {
        RuntimeAssetRecord::new(id, bundle_type.clone(), shared_path.clone(), dependencies)
    };
    assert!(
        catalog(vec![
            record(first, Vec::new())?,
            record(second, Vec::new())?
        ])
        .is_err()
    );
    assert!(
        RuntimeAssetRecord::new(
            second,
            TypeKey::new("demo.bundle")?,
            RuntimeProductPath::new("Content/b.product.ron")?,
            vec![first, first],
        )
        .is_err()
    );
    let missing = RuntimeAssetRecord::new(
        second,
        TypeKey::new("demo.bundle")?,
        RuntimeProductPath::new("Content/b.product.ron")?,
        vec![first],
    )?;
    assert!(catalog(vec![missing]).is_err());
    Ok(())
}

#[test]
fn catalog_enforces_entry_and_product_roles_but_accepts_unknown_types_and_cycles()
-> Result<(), Box<dyn std::error::Error>> {
    let first = asset_id(0x1000_0000)?;
    let second = asset_id(0x2000_0000)?;
    assert!(
        RuntimeAssetCatalog::build(
            TypeKey::new("demo.game")?,
            RuntimeProductPath::new("Scenes/other.runtime-scene.ron")?,
            vec![],
            b"scene",
            &BTreeMap::new(),
        )
        .is_err()
    );
    assert!(
        RuntimeAssetRecord::new(
            first,
            TypeKey::new("demo.bundle")?,
            RuntimeProductPath::new("Scenes/not-a-product.ron")?,
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        RuntimeAssetRecord::new(
            first,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            RuntimeProductPath::new("Content/wrong.mesh.ron")?,
            Vec::new(),
        )
        .is_err()
    );

    let bundle_type = TypeKey::new("demo.bundle")?;
    let cyclic = |id, dependency, path: RuntimeProductPath| {
        RuntimeAssetRecord::new(id, bundle_type.clone(), path, vec![dependency])
    };
    catalog(vec![
        cyclic(
            first,
            second,
            RuntimeProductPath::new("Content/a.product.ron")?,
        )?,
        cyclic(
            second,
            first,
            RuntimeProductPath::new("Content/b.product.ron")?,
        )?,
    ])?;
    Ok(())
}

#[test]
fn catalog_codec_is_strict_independent_canonical_and_idempotent()
-> Result<(), Box<dyn std::error::Error>> {
    let id = asset_id(0x1000_0000)?;
    let encoded = catalog(vec![mesh_record(id)?])?.to_ron()?;
    assert!(!encoded.contains('\r'));
    assert_eq!(RuntimeAssetCatalog::from_ron(&encoded)?.to_ron()?, encoded);

    assert!(matches!(
        RuntimeAssetCatalog::from_ron("(format_version: 2)"),
        Err(RuntimeCatalogError::VersionMismatch {
            expected: 1,
            found: 2,
        })
    ));
    assert!(
        RuntimeAssetCatalog::from_ron(&encoded.replacen(
            "game_id:",
            "unknown: 1,\n    game_id:",
            1
        ))
        .is_err()
    );
    assert!(
        RuntimeAssetCatalog::from_ron(&encoded.replacen("game_id:", "removed_game_id:", 1))
            .is_err()
    );
    assert!(RuntimeAssetCatalog::from_ron(&format!("{encoded}\ntrue")).is_err());
    assert!(RuntimeAssetCatalog::from_ron(&encoded.replacen("demo.game", "Demo Game", 1)).is_err());
    Ok(())
}

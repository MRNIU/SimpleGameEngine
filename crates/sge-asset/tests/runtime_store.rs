// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{
    AssetId, AssetLookup, AssetRef, MESH_ASSET_TYPE_KEY, MeshAsset, MeshVertex,
    RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeAssetStore, RuntimeAssetStoreError,
    RuntimeGeneration, RuntimeProductPath,
};
use sge_reflect::TypeKey;

fn id(value: u128) -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok(format!("{value:08x}-0000-4000-8000-000000000001").parse()?)
}

fn mesh_bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            MeshVertex::new([0.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([1.0, 0.0, 0.0], None, None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, None)?,
        ],
        vec![0, 1, 2],
    )?
    .to_ron()?
    .into_bytes())
}

fn generation(
    asset: AssetId,
    asset_type: &str,
    product: &str,
    bytes: Vec<u8>,
) -> Result<RuntimeGeneration, Box<dyn std::error::Error>> {
    let products = BTreeMap::from([(asset, bytes)]);
    let catalog = RuntimeAssetCatalog::build(
        TypeKey::new("demo.game")?,
        RuntimeProductPath::new("Scenes/entry.runtime-scene.ron")?,
        vec![RuntimeAssetRecord::new(
            asset,
            TypeKey::new(asset_type)?,
            RuntimeProductPath::new(product)?,
            Vec::new(),
        )?],
        b"scene",
        &products,
    )?;
    Ok(RuntimeGeneration::verify_owned(
        catalog,
        b"scene".to_vec(),
        products,
    )?)
}

#[test]
fn runtime_store_decodes_mesh_and_reports_decoded_asset_type()
-> Result<(), Box<dyn std::error::Error>> {
    let asset = id(0x1000_0000)?;
    let generation = generation(
        asset,
        MESH_ASSET_TYPE_KEY,
        &format!("Content/{asset}.mesh.ron"),
        mesh_bytes()?,
    )?;
    let store = RuntimeAssetStore::load(&generation)?;

    assert_eq!(store.mesh(AssetRef::new(asset))?.indices(), &[0, 1, 2]);
    assert_eq!(
        store.asset_type(&asset).map(TypeKey::as_str),
        Some(MESH_ASSET_TYPE_KEY)
    );
    assert!(store.asset_type(&id(0x2000_0000)?).is_none());
    Ok(())
}

#[test]
fn runtime_store_rejects_corrupt_mesh_and_unknown_product_type()
-> Result<(), Box<dyn std::error::Error>> {
    let asset = id(0x1000_0000)?;
    let corrupt = generation(
        asset,
        MESH_ASSET_TYPE_KEY,
        &format!("Content/{asset}.mesh.ron"),
        b"not ron".to_vec(),
    )?;
    assert!(matches!(
        RuntimeAssetStore::load(&corrupt),
        Err(RuntimeAssetStoreError::MeshDecode { .. })
    ));

    let unknown = generation(
        asset,
        "demo.unknown",
        "Content/unknown.product.ron",
        b"opaque".to_vec(),
    )?;
    assert!(matches!(
        RuntimeAssetStore::load(&unknown),
        Err(RuntimeAssetStoreError::UnsupportedProductType { .. })
    ));
    Ok(())
}

#[test]
fn runtime_store_typed_lookup_rejects_missing_asset() -> Result<(), Box<dyn std::error::Error>> {
    let asset = id(0x1000_0000)?;
    let generation = generation(
        asset,
        MESH_ASSET_TYPE_KEY,
        &format!("Content/{asset}.mesh.ron"),
        mesh_bytes()?,
    )?;
    let store = RuntimeAssetStore::load(&generation)?;

    assert!(matches!(
        store.mesh(AssetRef::new(id(0x2000_0000)?)),
        Err(RuntimeAssetStoreError::MissingMesh { .. })
    ));
    Ok(())
}

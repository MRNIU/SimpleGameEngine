// Copyright The SimpleGameEngine Contributors

use serde::Deserialize;
use sge_asset::{AssetId, AssetLookup, AssetRef, AssetType};
use sge_reflect::{ReferenceSemantic, ReferenceValue, TypeKey};

struct TestAsset;

impl AssetType for TestAsset {
    const TYPE_KEY: &'static str = "asset.mesh";
}

struct MarkerWithoutValueTraits;

impl AssetType for MarkerWithoutValueTraits {
    const TYPE_KEY: &'static str = "asset.marker";
}

struct InvalidAssetType;

impl AssetType for InvalidAssetType {
    const TYPE_KEY: &'static str = "asset/mesh";
}

#[test]
fn generated_asset_id_is_uuid_v4() -> Result<(), Box<dyn std::error::Error>> {
    let generated = AssetId::new_v4();

    assert_eq!(
        uuid::Uuid::parse_str(&generated.to_string())?.get_version_num(),
        4
    );
    Ok(())
}

#[test]
fn nil_asset_id_is_stable_and_canonical() {
    assert_eq!(
        AssetId::nil().to_string(),
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(AssetId::nil(), AssetId::nil());
}

#[test]
fn asset_id_accepts_canonical_lowercase_uuid() -> Result<(), Box<dyn std::error::Error>> {
    let id: AssetId = "550e8400-e29b-41d4-a716-446655440000".parse()?;

    assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    Ok(())
}

#[test]
fn asset_id_rejects_uppercase_uuid() {
    assert!(
        "550E8400-E29B-41D4-A716-446655440000"
            .parse::<AssetId>()
            .is_err()
    );
}

#[test]
fn asset_id_rejects_asset_prefix() {
    assert!(
        "asset:550e8400-e29b-41d4-a716-446655440000"
            .parse::<AssetId>()
            .is_err()
    );
}

#[test]
fn asset_id_rejects_empty_input() {
    assert!("".parse::<AssetId>().is_err());
}

#[test]
fn asset_id_rejects_non_uuid_input() {
    assert!("not-a-uuid".parse::<AssetId>().is_err());
}

#[test]
fn asset_id_rejects_other_uuid_spellings_and_whitespace() {
    for invalid in [
        "550e8400e29b41d4a716446655440000",
        "{550e8400-e29b-41d4-a716-446655440000}",
        "urn:uuid:550e8400-e29b-41d4-a716-446655440000",
        " 550e8400-e29b-41d4-a716-446655440000",
        "550e8400-e29b-41d4-a716-446655440000 ",
    ] {
        assert!(invalid.parse::<AssetId>().is_err(), "{invalid:?}");
    }
}

#[test]
fn asset_id_deserializes_only_through_strict_string_codec() -> Result<(), Box<dyn std::error::Error>>
{
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(
        "550e8400-e29b-41d4-a716-446655440000",
    );
    let id = AssetId::deserialize(deserializer)?;

    assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    Ok(())
}

#[test]
fn asset_id_serializes_as_canonical_string() -> Result<(), Box<dyn std::error::Error>> {
    let id: AssetId = "550e8400-e29b-41d4-a716-446655440000".parse()?;

    assert_eq!(
        ron::to_string(&id)?,
        r#""550e8400-e29b-41d4-a716-446655440000""#
    );
    Ok(())
}

#[test]
fn asset_id_deserialize_rejects_noncanonical_uuid_text() {
    for invalid in [
        "550E8400-E29B-41D4-A716-446655440000",
        "550e8400e29b41d4a716446655440000",
    ] {
        let deserializer =
            serde::de::value::StrDeserializer::<serde::de::value::Error>::new(invalid);
        assert!(AssetId::deserialize(deserializer).is_err(), "{invalid:?}");
    }
}

#[test]
fn asset_id_deserialize_rejects_non_string_value() {
    assert!(ron::from_str::<AssetId>("42").is_err());
}

#[test]
fn asset_ref_binds_identity_to_asset_type() -> Result<(), Box<dyn std::error::Error>> {
    let id: AssetId = "550e8400-e29b-41d4-a716-446655440000".parse()?;
    let mesh = AssetRef::<TestAsset>::new(id);

    assert_eq!(mesh.id(), &id);
    assert_eq!(mesh.to_reference(), id.to_string());
    assert_eq!(
        AssetRef::<TestAsset>::semantic()?,
        ReferenceSemantic::Asset {
            asset_type: TypeKey::new("asset.mesh")?
        }
    );
    Ok(())
}

#[test]
fn asset_ref_decodes_only_canonical_asset_id() -> Result<(), Box<dyn std::error::Error>> {
    let id: AssetId = "550e8400-e29b-41d4-a716-446655440000".parse()?;
    let decoded =
        AssetRef::<TestAsset>::from_reference(&id.to_string()).map_err(std::io::Error::other)?;

    assert_eq!(decoded.id(), &id);
    for invalid in [
        "550E8400-E29B-41D4-A716-446655440000",
        "550e8400e29b41d4a716446655440000",
    ] {
        assert!(
            AssetRef::<TestAsset>::from_reference(invalid).is_err(),
            "{invalid:?}"
        );
    }
    Ok(())
}

#[test]
fn asset_ref_value_traits_do_not_require_marker_traits() {
    fn assert_value_traits<T: Clone + Copy + Eq + Ord + std::hash::Hash>() {}

    assert_value_traits::<AssetRef<MarkerWithoutValueTraits>>();
}

#[test]
fn asset_lookup_exposes_only_registered_type() -> Result<(), Box<dyn std::error::Error>> {
    struct Lookup {
        id: AssetId,
        asset_type: TypeKey,
    }

    impl AssetLookup for Lookup {
        fn asset_type(&self, id: &AssetId) -> Option<&TypeKey> {
            (id == &self.id).then_some(&self.asset_type)
        }
    }

    let id: AssetId = "550e8400-e29b-41d4-a716-446655440000".parse()?;
    let missing: AssetId = "550e8400-e29b-41d4-a716-446655440001".parse()?;
    let lookup = Lookup {
        id,
        asset_type: TypeKey::new("asset.mesh")?,
    };

    let lookup: &dyn AssetLookup = &lookup;
    assert_eq!(lookup.asset_type(&id), Some(&TypeKey::new("asset.mesh")?));
    assert_eq!(lookup.asset_type(&missing), None);
    Ok(())
}

#[test]
fn asset_ref_rejects_invalid_asset_type_key() {
    assert!(AssetRef::<InvalidAssetType>::semantic().is_err());
}

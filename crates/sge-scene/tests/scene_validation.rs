// Copyright The SimpleGameEngine Contributors

use std::str::FromStr;

use std::any::TypeId;

use sge_reflect::{
    FieldKind, FieldValues, ReferenceSemantic, ReferenceValue, ReflectedValue, TypeKey,
    TypeRegistry,
};
use sge_scene::{
    AuthoringEntity, AuthoringScene, Parent, SceneEntityId, SceneValidationError,
    parent_descriptor, scene_entity_id_descriptor,
};

#[test]
fn generated_scene_entity_id_is_v4_and_canonical() -> Result<(), Box<dyn std::error::Error>> {
    let id = SceneEntityId::new_v4();
    let encoded = id.to_string();
    let uuid = uuid::Uuid::parse_str(&encoded)?;

    assert_eq!(uuid.get_version_num(), 4);
    assert_eq!(SceneEntityId::from_str(&encoded)?, id);
    assert_eq!(encoded, uuid.hyphenated().to_string());
    Ok(())
}

#[test]
fn scene_entity_id_serde_is_string_only_and_strict() -> Result<(), Box<dyn std::error::Error>> {
    let canonical = "550e8400-e29b-41d4-a716-446655440000";
    let id = SceneEntityId::from_str(canonical)?;

    assert_eq!(ron::to_string(&id)?, format!("\"{canonical}\""));
    assert_eq!(
        ron::from_str::<SceneEntityId>(&format!("\"{canonical}\""))?,
        id
    );
    for rejected in [
        "550E8400-E29B-41D4-A716-446655440000",
        "550e8400e29b41d4a716446655440000",
        "{550e8400-e29b-41d4-a716-446655440000}",
        "urn:uuid:550e8400-e29b-41d4-a716-446655440000",
        "scene:550e8400-e29b-41d4-a716-446655440000",
        " 550e8400-e29b-41d4-a716-446655440000",
        "550e8400-e29b-41d4-a716-446655440000 ",
        "",
    ] {
        assert!(ron::from_str::<SceneEntityId>(&format!("\"{rejected}\"")).is_err());
    }
    assert!(ron::from_str::<SceneEntityId>("42").is_err());
    Ok(())
}

#[test]
fn scene_entity_id_is_an_entity_reference() -> Result<(), Box<dyn std::error::Error>> {
    let id = SceneEntityId::from_str("550e8400-e29b-41d4-a716-446655440000")?;

    assert_eq!(SceneEntityId::semantic()?, ReferenceSemantic::Entity);
    assert_eq!(id.to_reference(), id.to_string());
    assert!(matches!(
        SceneEntityId::from_reference(&id.to_string()),
        Ok(decoded) if decoded == id
    ));
    assert!(SceneEntityId::from_reference("not-an-id").is_err());
    Ok(())
}

#[test]
fn structural_descriptors_roundtrip_and_are_not_scene_saveable()
-> Result<(), Box<dyn std::error::Error>> {
    let identity = scene_entity_id_descriptor()?;
    let parent = parent_descriptor()?;

    assert_eq!(identity.type_key().as_str(), "sge.scene_entity_id");
    assert_eq!(identity.rust_type_id(), TypeId::of::<SceneEntityId>());
    assert!(!identity.scene_saveable());
    assert!(matches!(
        identity.field("id").map(|metadata| metadata.kind()),
        Some(FieldKind::Reference(ReferenceSemantic::Entity))
    ));
    assert_eq!(parent.type_key().as_str(), "sge.parent");
    assert_eq!(parent.rust_type_id(), TypeId::of::<Parent>());
    assert!(!parent.scene_saveable());
    assert!(matches!(
        parent.field("parent").map(|metadata| metadata.kind()),
        Some(FieldKind::Reference(ReferenceSemantic::Entity))
    ));

    let mut registry = TypeRegistry::new();
    registry.register(identity)?;
    registry.register(parent)?;
    registry.freeze()?;
    let id = SceneEntityId::from_str("550e8400-e29b-41d4-a716-446655440000")?;
    let encoded_id = registry.encode(&id)?;
    let encoded_parent = registry.encode(&Parent(id))?;
    let decoded_id = registry.decode(&encoded_id)?;
    let decoded_parent = registry.decode(&encoded_parent)?;

    assert_eq!(decoded_id.downcast_ref::<SceneEntityId>(), Some(&id));
    assert_eq!(decoded_parent.downcast_ref::<Parent>(), Some(&Parent(id)));
    Ok(())
}

#[test]
fn constructors_canonicalize_and_reject_duplicate_identities()
-> Result<(), Box<dyn std::error::Error>> {
    let first = SceneEntityId::from_str("00000000-0000-0000-0000-000000000001")?;
    let second = SceneEntityId::from_str("00000000-0000-0000-0000-000000000002")?;
    let zeta = ReflectedValue::new(TypeKey::new("demo.zeta")?, 1, FieldValues::default());
    let alpha = ReflectedValue::new(TypeKey::new("demo.alpha")?, 1, FieldValues::default());
    let entity = AuthoringEntity::new(first, None, vec![zeta.clone(), alpha.clone()])?;
    assert_eq!(
        entity
            .components()
            .map(|component| component.type_key().as_str())
            .collect::<Vec<_>>(),
        ["demo.alpha", "demo.zeta"]
    );
    assert!(matches!(
        AuthoringEntity::new(first, None, vec![alpha.clone(), alpha]),
        Err(SceneValidationError::DuplicateComponent { entity, component })
            if entity == first && component.as_str() == "demo.alpha"
    ));

    let other = AuthoringEntity::new(second, Some(first), Vec::new())?;
    let scene = AuthoringScene::new(vec![other.clone(), entity.clone()])?;
    assert_eq!(
        scene
            .entities()
            .map(AuthoringEntity::id)
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert!(matches!(
        AuthoringScene::new(vec![entity.clone(), entity]),
        Err(SceneValidationError::DuplicateEntity { entity }) if entity == first
    ));
    Ok(())
}

// Copyright The SimpleGameEngine Contributors

use std::str::FromStr;

use sge_asset::{AssetId, AssetLookup};
use sge_reflect::{FieldValues, ReflectedValue, TypeDescriptor, TypeKey, TypeRegistry};
use sge_scene::{AuthoringEntity, AuthoringScene, SceneEntityId, SceneValidationError, prepare};

struct EmptyAssets;

impl AssetLookup for EmptyAssets {
    fn asset_type(&self, _id: &AssetId) -> Option<&TypeKey> {
        None
    }
}

#[test]
fn prepare_requires_a_frozen_registry_even_for_an_empty_scene()
-> Result<(), Box<dyn std::error::Error>> {
    let scene = AuthoringScene::new(Vec::new())?;
    let registry = TypeRegistry::new();
    assert!(matches!(
        prepare(&scene, &registry, &EmptyAssets),
        Err(SceneValidationError::RegistryNotFrozen)
    ));

    let mut registry = TypeRegistry::new();
    registry.freeze()?;
    assert!(prepare(&scene, &registry, &EmptyAssets).is_ok());
    Ok(())
}

fn id(index: u64) -> Result<SceneEntityId, Box<dyn std::error::Error>> {
    Ok(SceneEntityId::from_str(&format!(
        "00000000-0000-0000-0000-{index:012x}"
    ))?)
}

fn frozen_registry() -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.freeze()?;
    Ok(registry)
}

#[test]
fn prepare_rejects_missing_self_and_cyclic_parents_deterministically()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = frozen_registry()?;
    let first = id(1)?;
    let second = id(2)?;
    let missing = id(99)?;

    let missing_scene = AuthoringScene::new(vec![AuthoringEntity::new(
        first,
        Some(missing),
        Vec::new(),
    )?])?;
    assert!(matches!(
        prepare(&missing_scene, &registry, &EmptyAssets),
        Err(SceneValidationError::MissingParent { entity, parent })
            if entity == first && parent == missing
    ));

    let self_scene =
        AuthoringScene::new(vec![AuthoringEntity::new(first, Some(first), Vec::new())?])?;
    assert!(matches!(
        prepare(&self_scene, &registry, &EmptyAssets),
        Err(SceneValidationError::SelfParent { entity }) if entity == first
    ));

    let cycle_entities = vec![
        AuthoringEntity::new(second, Some(first), Vec::new())?,
        AuthoringEntity::new(first, Some(second), Vec::new())?,
    ];
    let cycle_scene = AuthoringScene::new(cycle_entities.into_iter().rev().collect())?;
    assert!(matches!(
        prepare(&cycle_scene, &registry, &EmptyAssets),
        Err(SceneValidationError::ParentCycle { entity }) if entity == first
    ));

    let depth = 1_024_u64;
    let mut deep_cycle = Vec::new();
    for index in 1..=depth {
        let parent = if index == 1 {
            id(depth)?
        } else {
            id(index - 1)?
        };
        deep_cycle.push(AuthoringEntity::new(id(index)?, Some(parent), Vec::new())?);
    }
    let deep_scene = AuthoringScene::new(deep_cycle)?;
    assert!(matches!(
        prepare(&deep_scene, &registry, &EmptyAssets),
        Err(SceneValidationError::ParentCycle { entity }) if entity == first
    ));
    Ok(())
}

#[derive(Clone)]
struct NonSaveable;

#[derive(Clone)]
struct Saveable;

fn component_registry() -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDescriptor::builder::<NonSaveable>(
            TypeKey::new("demo.non_saveable")?,
            1,
            "Non-saveable",
            || NonSaveable,
        )
        .build()?,
    )?;
    registry.register(
        TypeDescriptor::builder::<Saveable>(TypeKey::new("demo.saveable")?, 1, "Saveable", || {
            Saveable
        })
        .scene_saveable()
        .build()?,
    )?;
    registry.freeze()?;
    Ok(registry)
}

fn one_component_scene(
    type_key: &str,
    schema_version: u32,
) -> Result<AuthoringScene, Box<dyn std::error::Error>> {
    let component = ReflectedValue::new(
        TypeKey::new(type_key)?,
        schema_version,
        FieldValues::default(),
    );
    let entity = AuthoringEntity::new(id(1)?, None, vec![component])?;
    Ok(AuthoringScene::new(vec![entity])?)
}

#[test]
fn prepare_rejects_unknown_non_saveable_and_schema_mismatched_components()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = component_registry()?;
    let entity = id(1)?;

    assert!(matches!(
        prepare(
            &one_component_scene("demo.unknown", 1)?,
            &registry,
            &EmptyAssets
        ),
        Err(SceneValidationError::UnknownComponent {
            entity: found,
            component
        }) if found == entity && component.as_str() == "demo.unknown"
    ));
    assert!(matches!(
        prepare(
            &one_component_scene("demo.non_saveable", 1)?,
            &registry,
            &EmptyAssets
        ),
        Err(SceneValidationError::NonSaveableComponent {
            entity: found,
            component
        }) if found == entity && component.as_str() == "demo.non_saveable"
    ));
    assert!(matches!(
        prepare(
            &one_component_scene("demo.saveable", 2)?,
            &registry,
            &EmptyAssets
        ),
        Err(SceneValidationError::ComponentSchemaMismatch {
            entity: found,
            component,
            expected: 1,
            actual: 2
        }) if found == entity && component.as_str() == "demo.saveable"
    ));
    Ok(())
}

#[test]
fn prepare_first_error_is_stable_across_input_permutations()
-> Result<(), Box<dyn std::error::Error>> {
    let registry = component_registry()?;
    let first = id(1)?;
    let second = id(2)?;
    let alpha = ReflectedValue::new(
        TypeKey::new("demo.alpha_unknown")?,
        1,
        FieldValues::default(),
    );
    let zeta = ReflectedValue::new(
        TypeKey::new("demo.zeta_unknown")?,
        1,
        FieldValues::default(),
    );
    let left = AuthoringScene::new(vec![
        AuthoringEntity::new(second, None, vec![zeta.clone()])?,
        AuthoringEntity::new(first, None, vec![zeta.clone(), alpha.clone()])?,
    ])?;
    let right = AuthoringScene::new(vec![
        AuthoringEntity::new(first, None, vec![alpha, zeta.clone()])?,
        AuthoringEntity::new(second, None, vec![zeta])?,
    ])?;

    for scene in [&left, &right] {
        assert!(matches!(
            prepare(scene, &registry, &EmptyAssets),
            Err(SceneValidationError::UnknownComponent { entity, component })
                if entity == first && component.as_str() == "demo.alpha_unknown"
        ));
    }

    let missing_parent = id(99)?;
    let structural_first = AuthoringScene::new(vec![AuthoringEntity::new(
        first,
        Some(missing_parent),
        vec![ReflectedValue::new(
            TypeKey::new("demo.unknown")?,
            1,
            FieldValues::default(),
        )],
    )?])?;
    assert!(matches!(
        prepare(&structural_first, &registry, &EmptyAssets),
        Err(SceneValidationError::MissingParent { entity, parent })
            if entity == first && parent == missing_parent
    ));
    Ok(())
}

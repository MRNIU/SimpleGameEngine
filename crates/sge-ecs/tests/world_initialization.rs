// Copyright The SimpleGameEngine Contributors
//
//! Ready-only scene transfer support at the raw `World` boundary.

use std::any::TypeId;

use sge_ecs::{EcsError, World};

#[derive(Debug, PartialEq)]
struct Position(f32);

#[derive(Debug, PartialEq)]
struct Velocity(f32);

#[test]
fn component_type_lookup_is_read_only_across_registration_states() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;

    assert!(world.component_type_is_registered(TypeId::of::<Position>()));
    assert!(!world.component_type_is_registered(TypeId::of::<Velocity>()));
    assert!(!world.registration_is_finished());

    world.finish_registration();

    assert!(world.component_type_is_registered(TypeId::of::<Position>()));
    assert!(!world.component_type_is_registered(TypeId::of::<Velocity>()));
    assert!(world.registration_is_finished());
    Ok(())
}

#[test]
fn component_erased_returns_attached_component() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let entity = world.spawn();
    assert!(world.insert(entity, Position(3.0))?.is_none());

    let value = world.component_erased(entity, TypeId::of::<Position>())?;
    assert_eq!(
        value.and_then(|value| value.downcast_ref::<Position>()),
        Some(&Position(3.0))
    );
    Ok(())
}

#[test]
fn component_erased_returns_none_for_registered_missing_component() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let entity = world.spawn();

    assert!(
        world
            .component_erased(entity, TypeId::of::<Position>())?
            .is_none()
    );
    Ok(())
}

#[test]
fn component_erased_rejects_unregistered_type_id() {
    let mut world = World::new();
    world.finish_registration();
    let entity = world.spawn();
    let type_id = TypeId::of::<Position>();

    assert!(matches!(
        world.component_erased(entity, type_id),
        Err(EcsError::UnregisteredComponentTypeId(error_type_id))
            if error_type_id == type_id
    ));
}

#[test]
fn component_erased_rejects_stale_entity_before_type_lookup() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let stale = world.spawn();
    world.despawn(stale)?;
    let replacement = world.spawn();
    assert!(world.insert(replacement, Position(8.0))?.is_none());

    assert!(matches!(
        world.component_erased(stale, TypeId::of::<Position>()),
        Err(EcsError::EntityNotAlive(entity)) if entity == stale
    ));
    assert!(matches!(
        world.component_erased(stale, TypeId::of::<Velocity>()),
        Err(EcsError::EntityNotAlive(entity)) if entity == stale
    ));
    Ok(())
}

#[test]
fn world_initializer_reports_component_registration() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();

    let initializer = world.initializer();
    assert!(initializer.component_is_registered(TypeId::of::<Position>()));
    assert!(!initializer.component_is_registered(TypeId::of::<Velocity>()));
    Ok(())
}

#[test]
fn world_initializer_spawns_and_inserts_typed_component() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert(entity, Position(5.0))?;
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(5.0)));
    Ok(())
}

#[test]
fn world_initializer_inserts_erased_component() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert_erased(entity, TypeId::of::<Position>(), Box::new(Position(6.0)))?;
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(6.0)));
    Ok(())
}

#[test]
fn world_initializer_rejects_preexisting_entity() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let preexisting = world.spawn();

    assert!(matches!(
        world.initializer().insert(preexisting, Position(1.0)),
        Err(EcsError::InitializerEntityNotOwned(entity)) if entity == preexisting
    ));
    Ok(())
}

#[test]
fn world_initializer_rejects_entity_from_previous_initializer() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let previous = world.initializer().spawn();

    assert!(matches!(
        world.initializer().insert(previous, Position(2.0)),
        Err(EcsError::InitializerEntityNotOwned(entity)) if entity == previous
    ));
    Ok(())
}

#[test]
fn world_initializer_rejects_stale_prior_entity() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let stale = world.spawn();
    world.despawn(stale)?;

    assert!(matches!(
        world.initializer().insert(stale, Position(3.0)),
        Err(EcsError::InitializerEntityNotOwned(entity)) if entity == stale
    ));
    Ok(())
}

#[test]
fn world_initializer_rejects_erased_duplicate_without_replacing_typed_value() -> Result<(), EcsError>
{
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let type_id = TypeId::of::<Position>();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert(entity, Position(1.0))?;
        assert!(matches!(
            initializer.insert_erased(entity, type_id, Box::new(Position(2.0))),
            Err(EcsError::DuplicateComponentOnEntity {
                entity: duplicate_entity,
                type_id: duplicate_type_id,
            }) if duplicate_entity == entity && duplicate_type_id == type_id
        ));
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(1.0)));
    Ok(())
}

#[test]
fn world_initializer_rejects_typed_duplicate_without_replacing_erased_value() -> Result<(), EcsError>
{
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let type_id = TypeId::of::<Position>();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert_erased(entity, type_id, Box::new(Position(3.0)))?;
        assert!(matches!(
            initializer.insert(entity, Position(4.0)),
            Err(EcsError::DuplicateComponentOnEntity {
                entity: duplicate_entity,
                type_id: duplicate_type_id,
            }) if duplicate_entity == entity && duplicate_type_id == type_id
        ));
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(3.0)));
    Ok(())
}

#[test]
fn world_initializer_distinguishes_typed_and_erased_unregistered_errors() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.finish_registration();
    let velocity_type_id = TypeId::of::<Velocity>();
    let velocity_type_name = std::any::type_name::<Velocity>();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        assert_eq!(
            initializer.insert(entity, Velocity(1.0)),
            Err(EcsError::UnregisteredComponentType(velocity_type_name))
        );
        assert!(matches!(
            initializer.insert_erased(entity, velocity_type_id, Box::new(Velocity(2.0))),
            Err(EcsError::UnregisteredComponentTypeId(type_id))
                if type_id == velocity_type_id
        ));
        initializer.insert(entity, Position(9.0))?;
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(9.0)));
    assert_eq!(world.get::<Velocity>(entity), None);
    Ok(())
}

#[test]
fn world_initializer_rejects_erased_payload_type_before_duplicate_check() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.register_component::<Velocity>()?;
    world.finish_registration();
    let declared = TypeId::of::<Position>();
    let payload: Box<dyn std::any::Any> = Box::new(Velocity(2.0));
    let actual = payload.as_ref().type_id();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert(entity, Position(1.0))?;
        assert!(matches!(
            initializer.insert_erased(entity, declared, payload),
            Err(EcsError::ComponentTypeMismatch {
                declared: error_declared,
                actual: error_actual,
            }) if error_declared == declared && error_actual == actual
        ));
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(1.0)));
    assert_eq!(world.get::<Velocity>(entity), None);
    Ok(())
}

#[test]
fn world_initializer_allows_different_registered_components() -> Result<(), EcsError> {
    let mut world = World::new();
    world.register_component::<Position>()?;
    world.register_component::<Velocity>()?;
    world.finish_registration();

    let entity = {
        let mut initializer = world.initializer();
        let entity = initializer.spawn();
        initializer.insert(entity, Position(4.0))?;
        initializer.insert_erased(entity, TypeId::of::<Velocity>(), Box::new(Velocity(5.0)))?;
        entity
    };

    assert_eq!(world.get::<Position>(entity), Some(&Position(4.0)));
    assert_eq!(world.get::<Velocity>(entity), Some(&Velocity(5.0)));
    Ok(())
}

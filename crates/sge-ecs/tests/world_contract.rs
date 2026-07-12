// Copyright The SimpleGameEngine Contributors
//
//! `sge-ecs` public contract tests.

use std::panic::{AssertUnwindSafe, catch_unwind};

use sge_ecs::{EcsError, World};

#[derive(Debug, PartialEq)]
struct Position(f32);

#[derive(Debug, PartialEq)]
struct Velocity(f32);

#[derive(Debug, PartialEq)]
struct Score(u32);

struct PanicsOnDrop;

impl Drop for PanicsOnDrop {
    fn drop(&mut self) {
        panic!("component drop panic");
    }
}

struct Marker;

#[test]
fn stale_entity_cannot_access_reused_slot() {
    let mut world = World::new();
    world.register_component::<Position>().unwrap();
    world.finish_registration();
    let stale = world.spawn();
    assert_eq!(world.insert(stale, Position(1.0)).unwrap(), None);

    world.despawn(stale).unwrap();
    let replacement = world.spawn();
    assert_eq!(world.insert(replacement, Position(2.0)).unwrap(), None);

    assert_ne!(stale, replacement);
    assert!(!world.is_alive(stale));
    assert_eq!(world.get::<Position>(stale), None);
    assert_eq!(world.get::<Position>(replacement), Some(&Position(2.0)));
    assert_eq!(
        world.insert(stale, Position(3.0)).unwrap_err(),
        EcsError::EntityNotAlive(stale)
    );
}

#[test]
fn despawn_drop_panic_keeps_entity_alive_and_slot_unrecycled() {
    let mut world = World::new();
    world.register_component::<PanicsOnDrop>().unwrap();
    world.register_component::<Marker>().unwrap();
    world.finish_registration();
    let entity = world.spawn();
    assert!(world.insert(entity, PanicsOnDrop).unwrap().is_none());
    assert!(world.insert(entity, Marker).unwrap().is_none());

    let result = catch_unwind(AssertUnwindSafe(|| world.despawn(entity)));
    assert!(result.is_err());

    let next = world.spawn();
    assert!(world.is_alive(entity));
    assert!(world.is_alive(next));
    assert_eq!(world.entities().count(), 2);
}

#[test]
fn registered_components_support_insert_query_mut_remove_and_despawn_cleanup() {
    let mut world = World::new();
    world.register_component::<Position>().unwrap();
    world.register_component::<Velocity>().unwrap();
    world.finish_registration();
    let first = world.spawn();
    let second = world.spawn();
    let empty = world.spawn();
    assert_eq!(world.insert(first, Position(1.0)).unwrap(), None);
    assert_eq!(world.insert(first, Velocity(5.0)).unwrap(), None);
    assert_eq!(world.insert(second, Position(2.0)).unwrap(), None);

    for (_, position) in world.query_mut::<Position>() {
        position.0 += 10.0;
    }

    assert_eq!(
        world
            .query::<Position>()
            .map(|(_, value)| value.0)
            .collect::<Vec<_>>(),
        vec![11.0, 12.0]
    );
    assert_eq!(world.get::<Velocity>(first), Some(&Velocity(5.0)));
    assert!(!world.contains::<Position>(empty));
    assert_eq!(
        world.remove::<Position>(second).unwrap(),
        Some(Position(12.0))
    );
    world.despawn(first).unwrap();
    assert_eq!(world.get::<Position>(first), None);
    assert_eq!(world.get::<Velocity>(first), None);
}

#[test]
fn component_and_resource_registration_freezes_without_blocking_registered_values() {
    let mut world = World::new();
    world.register_component::<Position>().unwrap();
    world.register_resource::<Score>().unwrap();
    assert_eq!(world.insert_resource(Score(1)).unwrap(), None);
    world.finish_registration();

    assert_eq!(
        world.register_component::<Velocity>().unwrap_err(),
        EcsError::RegistrationFinished
    );
    assert_eq!(
        world.register_resource::<String>().unwrap_err(),
        EcsError::RegistrationFinished
    );
    assert_eq!(
        world.insert_resource(String::from("late")).unwrap_err(),
        EcsError::UnregisteredResourceType(std::any::type_name::<String>())
    );

    let entity = world.spawn();
    assert_eq!(world.insert(entity, Position(4.0)).unwrap(), None);
    world.resource_mut::<Score>().unwrap().0 += 1;
    assert_eq!(world.resource::<Score>(), Some(&Score(2)));
}

#[test]
fn duplicate_and_unregistered_types_fail_closed() {
    let mut world = World::new();
    assert!(!world.component_is_registered::<Position>());
    world.register_component::<Position>().unwrap();
    assert!(world.component_is_registered::<Position>());
    assert_eq!(
        world.register_component::<Position>().unwrap_err(),
        EcsError::DuplicateComponentType(std::any::type_name::<Position>())
    );
    world.register_resource::<Score>().unwrap();
    assert_eq!(
        world.register_resource::<Score>().unwrap_err(),
        EcsError::DuplicateResourceType(std::any::type_name::<Score>())
    );
    let entity = world.spawn();
    assert_eq!(
        world.insert(entity, Velocity(1.0)).unwrap_err(),
        EcsError::UnregisteredComponentType(std::any::type_name::<Velocity>())
    );
}

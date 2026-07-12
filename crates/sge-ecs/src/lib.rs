// Copyright The SimpleGameEngine Contributors
//
//! SimpleGameEngine 的串行 typed ECS runtime world。

mod entity;
mod storage;
mod world;

pub use entity::Entity;
pub use world::{EcsError, World, WorldInitializer};

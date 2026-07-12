// Copyright The SimpleGameEngine Contributors

//! Typed render components and runtime rendering products.

mod components;
mod plugin;

pub use components::{Camera, Light, Material, MeshRenderer, Projection};
pub use plugin::RenderPlugin;

// Copyright The SimpleGameEngine Contributors
//
//! Project identity、portable path 与 authoring data 的 durable 边界。

mod io;
mod path;

pub use io::{ProjectIoError, ProjectRoot};
pub use path::{ProjectPath, ProjectPathError};

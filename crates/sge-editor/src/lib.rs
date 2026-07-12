// Copyright The SimpleGameEngine Contributors

//! Candidate-first EditSession、Reflect Inspector 与 target Editor host。

mod error;
mod host;
mod input;
mod inspector;
mod inspector_ui;
mod play;
mod preview;
mod session;

use input::EditorInputAccumulator;

pub use error::{EditError, EditorOpenError, EditorPreviewError};
pub use host::{EditorRunError, EditorRunOptions, EditorRunReport, run};
pub use inspector::{InspectorComponent, InspectorField, SceneComponentType};
pub use play::{PlaySession, PlayStartError};
pub use preview::{PreviewProbe, PreviewProbeReport};
pub use session::{EditSession, EditorWorkspace, PreviewFrame};

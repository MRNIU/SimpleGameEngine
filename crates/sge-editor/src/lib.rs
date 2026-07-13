// Copyright The SimpleGameEngine Contributors

//! Candidate-first EditSession、Reflect Inspector 与 target Editor host。

mod build;
mod error;
mod host;
mod input;
mod inspector;
mod inspector_ui;
mod play;
mod preview;
mod session;
mod viewport;

use input::EditorInputAccumulator;

pub use build::EditorBuildLauncher;
pub use error::{EditError, EditorOpenError, EditorPreviewError};
pub use host::{
    EditorFileDialogs, EditorRunError, EditorRunOptions, EditorRunReport, NewProjectDialog,
    OpenProjectDialog, ProjectFileDialog, run,
};
pub use inspector::{InspectorComponent, InspectorField, SceneComponentType};
pub use play::{PlaySession, PlayStartError};
pub use preview::{PreviewProbe, PreviewProbeReport};
pub use session::{CreatedMeshEntity, EditSession, EditorWorkspace, PreviewFrame, PrimitiveKind};
